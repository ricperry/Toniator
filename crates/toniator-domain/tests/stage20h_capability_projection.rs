use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure,
    AuthoredStructureId, AuthoredStructureKind, CanvasSpec, ChannelId, CoveragePolicy,
    DensityMetric2D, Document, DocumentCommand, DocumentId, GeneralizedSiteProduct,
    GuideCapabilities, GuideDimension, GuideDimensionId, GuidePrototype, GuidePrototypeKind,
    GuideRepetition, GuideSiteProductCapability, MarkGeometryResponse, MarkOrientation,
    MarkOrientationKind, MarkOutputCapabilityProjection, MarkPrototype, MarkPrototypeKind,
    PatternCapabilityScope, PatternDefinition, PatternDefinitionBundle, PatternDefinitionEdit,
    PatternDefinitionId, PatternFamily, PatternFamilyCapabilityProjection, PatternGeometryResponse,
    PatternMechanismId, PatternOutputCapabilityProjection, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputSettings, PropertyFieldId, PropertyTarget,
    RandomCharacterKind, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference,
    StructuralSupportConstraint, TranslationEditedAxis,
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

/// Returns one public bundle definition by stable ID for stale-aware command construction.
fn definition_by_id(document: &Document, id: PatternDefinitionId) -> PatternDefinition {
    document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == id)
        .expect("fixture definition resolves")
        .definition
        .clone()
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

/// Returns the validated source-weighted placement selected by structural transition tests.
fn artwork_weighted_density_modulation() -> SiteDensityModulation {
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
    }
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
                editable_count_bounds: None,
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
            DensityMetric2D {
                density: 20.0,
                aspect: 1.2,
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

/// Proves every accepted random discriminant projects without name dispatch, while source-weighted
/// placement omits only the incompatible pattern-rotation command from each active scope.
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
        (
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::VisibleMarkMargin { margin: 0.75 },
            RandomCharacterKind::RawUniform,
            toniator_domain::DensityModulationKind::Uniform,
            toniator_domain::ExclusionKind::VisibleMarkMargin,
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
        assert!(first.active_controls.iter().all(|descriptor| {
            !matches!(
                descriptor.field,
                PropertyFieldId::RandomMaximumAttempts
                    | PropertyFieldId::RandomMaximumNeighborChecks
            )
        }));
        assert_eq!(
            first
                .active_controls
                .iter()
                .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees),
            expected_modulation != toniator_domain::DensityModulationKind::ArtworkWeighted,
            "base rotation availability follows source-weighted placement"
        );
        let channel = document
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .expect("random channel projection resolves");
        assert_eq!(
            channel
                .active_controls
                .iter()
                .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees),
            expected_modulation != toniator_domain::DensityModulationKind::ArtworkWeighted,
            "channel rotation availability follows source-weighted placement"
        );
        if expected_exclusion == toniator_domain::ExclusionKind::VisibleMarkMargin {
            let descriptors = document.property_descriptors();
            let selector = descriptors
                .iter()
                .find(|descriptor| descriptor.field == PropertyFieldId::RandomExclusion)
                .expect("visible exclusion selector projects");
            assert_eq!(
                selector.structural_support,
                StructuralSupportConstraint::VisibleMarkMarginUsesMaximumRealizedSupport
            );
            assert!(
                descriptors
                    .iter()
                    .any(|descriptor| descriptor.field == PropertyFieldId::VisibleMarkMargin)
            );
        }
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

/// Proves source-weighted placement resolves dormant document-base rotation to zero while
/// retaining its independent shape-rotation authority.
#[test]
fn source_weighted_sites_keep_pattern_rotation_dormant() {
    let weighted = random_definition(
        RandomSiteCharacter::RawUniform,
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
        SiteExclusionPolicy::None,
    );
    let document = document_for(weighted, Vec::new());
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 11.0;
    settings.shape_rotation_degrees = 7.0;
    let (document, _) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("stored base rotation validates");
    let effective = document
        .effective_channel_pattern(ChannelId(1))
        .expect("weighted effective pattern resolves");
    assert_eq!(effective.pattern_rotation_degrees, 0.0);
    assert_eq!(effective.shape_rotation_degrees, 7.0);
    assert!(
        !document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("weighted base projection resolves")
            .active_controls
            .iter()
            .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees)
    );
    assert!(
        !document
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .expect("weighted channel projection resolves")
            .active_controls
            .iter()
            .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees)
    );
}

/// Proves artwork-weighted placement rejects both public and direct channel rotation commands.
#[test]
fn source_weighted_sites_reject_channel_pattern_rotation_commands() {
    let document = document_for(
        random_definition(
            RandomSiteCharacter::RawUniform,
            artwork_weighted_density_modulation(),
            SiteExclusionPolicy::None,
        ),
        Vec::new(),
    );
    assert_eq!(
        document
            .set_channel_pattern_rotation_for_effective(ChannelId(1), 33.0)
            .expect_err("weighted channel builder rejects rotation")
            .path(),
        "channel.pattern.rotation"
    );
    assert_eq!(
        document
            .apply_command(&DocumentCommand::SetChannelPatternRotationDelta {
                base: document.pattern_settings().clone(),
                channel_id: ChannelId(1),
                rotation_degrees: 22.0,
            })
            .expect_err("weighted direct command rejects rotation")
            .path(),
        "channel.pattern.rotation"
    );
}

/// Proves a copy-on-edit transition to artwork-weighted placement prunes only the selected
/// channel's incompatible rotation while retaining base and unrelated channel intent.
#[test]
fn selected_shared_weighted_edit_prunes_only_the_selected_channel_rotation() {
    let document = document_for(
        random_definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
        ),
        Vec::new(),
    );
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 11.0;
    let (document, _) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("base rotation applies");
    let channel_one_rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 37.0)
        .expect("selected rotation command builds");
    let (document, _) = document
        .apply_command(&channel_one_rotation)
        .expect("selected rotation applies");
    let channel_two_rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(2), 19.0)
        .expect("unrelated rotation command builds");
    let (document, _) = document
        .apply_command(&channel_two_rotation)
        .expect("unrelated rotation applies");
    let base_definition = definition_by_id(&document, PatternDefinitionId(44));
    let (document, _) = document
        .apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition,
            edit: PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: PatternMechanismId(42),
                modulation: artwork_weighted_density_modulation(),
            },
        })
        .expect("selected weighted transition applies");
    assert_eq!(document.pattern_settings().pattern_rotation_degrees, 11.0);
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(1))
            .expect("selected channel persists")
            .layout_delta
            .rotation_degrees,
        None
    );
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(2))
            .expect("unrelated channel persists")
            .layout_delta
            .rotation_degrees,
        Some(8.0)
    );
    assert_eq!(
        document
            .effective_channel_pattern(ChannelId(1))
            .expect("weighted selected channel resolves")
            .pattern_rotation_degrees,
        0.0
    );
    assert_eq!(
        document
            .effective_channel_pattern(ChannelId(2))
            .expect("unrelated uniform channel resolves")
            .pattern_rotation_degrees,
        19.0
    );
}

/// Proves an unshared selected definition transition prunes its now-incompatible rotation
/// without changing the shared document-base rotation or another channel's retained delta.
#[test]
fn selected_unshared_weighted_edit_prunes_only_the_selected_channel_rotation() {
    let document = document_for(
        random_definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
        ),
        Vec::new(),
    );
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 11.0;
    let (document, _) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("base rotation applies");
    let channel_one_rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 37.0)
        .expect("selected rotation command builds");
    let (document, _) = document
        .apply_command(&channel_one_rotation)
        .expect("selected rotation applies");
    let channel_two_rotation = document
        .set_channel_pattern_rotation_for_effective(ChannelId(2), 19.0)
        .expect("unrelated rotation command builds");
    let (document, _) = document
        .apply_command(&channel_two_rotation)
        .expect("unrelated rotation applies");
    let base_definition = definition_by_id(&document, PatternDefinitionId(44));
    let (document, _) = document
        .apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition,
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(41),
                seed: 5678,
            },
        })
        .expect("selected clone transition applies");
    let selected_definition = document
        .effective_channel_pattern(ChannelId(1))
        .expect("selected clone resolves")
        .definition_id;
    let base_definition = definition_by_id(&document, selected_definition);
    let density_modulation_id = match &base_definition.family {
        PatternFamily::RandomSites {
            density_modulation_id,
            ..
        } => *density_modulation_id,
        _ => panic!("selected clone remains a random-site definition"),
    };
    let (document, _) = document
        .apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition,
            edit: PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: density_modulation_id,
                modulation: artwork_weighted_density_modulation(),
            },
        })
        .expect("unshared weighted transition applies");
    assert_eq!(document.pattern_settings().pattern_rotation_degrees, 11.0);
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(1))
            .expect("selected channel persists")
            .layout_delta
            .rotation_degrees,
        None
    );
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(2))
            .expect("unrelated channel persists")
            .layout_delta
            .rotation_degrees,
        Some(8.0)
    );
}

/// Proves a shared structural transition clears every linked rotation delta while preserving
/// the dormant base rotation and unrelated compatible translation intent.
#[test]
fn shared_weighted_edit_prunes_linked_rotation_deltas_only() {
    let document = document_for(
        random_definition(
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
        ),
        Vec::new(),
    );
    let mut settings = document.pattern_settings().clone();
    settings.pattern_rotation_degrees = 11.0;
    let (document, _) = document
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: document.pattern_settings().clone(),
            settings,
        })
        .expect("base rotation applies");
    let mut document = document;
    for (channel_id, rotation_degrees) in [
        (ChannelId(1), 37.0),
        (ChannelId(2), 19.0),
        (ChannelId(3), 13.0),
    ] {
        let rotation = document
            .set_channel_pattern_rotation_for_effective(channel_id, rotation_degrees)
            .expect("linked rotation command builds");
        (document, _) = document
            .apply_command(&rotation)
            .expect("linked rotation applies");
    }
    let (document, _) = document
        .apply_command(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(3),
            edited_axis: TranslationEditedAxis::X,
            value: 16.0,
        })
        .expect("compatible translation applies");
    let base_definition = definition_by_id(&document, PatternDefinitionId(44));
    let (document, _) = document
        .apply_command(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(44),
            base_definition,
            edit: PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: PatternMechanismId(42),
                modulation: artwork_weighted_density_modulation(),
            },
        })
        .expect("shared weighted transition applies");
    assert_eq!(document.pattern_settings().pattern_rotation_degrees, 11.0);
    for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        assert_eq!(
            document
                .channel_pattern_instance(channel_id)
                .expect("linked channel persists")
                .layout_delta
                .rotation_degrees,
            None
        );
        assert_eq!(
            document
                .effective_channel_pattern(channel_id)
                .expect("weighted linked channel resolves")
                .pattern_rotation_degrees,
            0.0
        );
    }
    assert_eq!(
        document
            .channel_pattern_instance(ChannelId(3))
            .expect("translated channel persists")
            .layout_delta
            .translation_x,
        16.0
    );
}
