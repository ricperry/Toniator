//! Current exhaustive descriptor-contract witnesses retained from the Stage 17 authority boundary.

use std::collections::BTreeSet;

use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, CoveragePolicy, DensityModulationKind, Document,
    DocumentCommand, DocumentHistory, DocumentSession, ExclusionKind, GeneralizedSiteProduct,
    InvalidationLevel, MarkOrientation, MarkOrientationKind, MarkPrototypeKind, PatternDefinition,
    PatternDefinitionEdit, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId,
    PropertyAuthority, PropertyCommandKind, PropertyEnumChoice, PropertyFieldId, PropertyTarget,
    RandomCharacterKind, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SourceMapping, SourceMappingComponent, SourceReference, StraightGuideDimension,
    StraightGuideRepetition, StructuralSupportConstraint, VariantTransitionDraft,
    VariantTransitionFieldUpdate, VariantTransitionValue, property_field_contract,
    property_field_contracts,
};

/// Builds one modeled document carrying all current compound-selector authorities.
fn transition_fixture() -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default transition document validates");
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    history
        .apply(&DocumentCommand::AddTypedPatternDefinition {
            definition: PatternDefinition::random_sites(
                PatternDefinitionId(50),
                "transition random",
                PatternMechanismId(60),
                PatternMechanismId(61),
                PatternMechanismId(62),
                PatternMechanismId(63),
                PatternOutputLayerId(70),
                RandomSiteCharacter::RawUniform,
                17,
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::None,
                1_000,
                2_000,
                CoveragePolicy {
                    guard_steps: 3,
                    additional_margin: 8.0,
                },
            ),
        })
        .expect("transition definition publishes");
    history.document().clone()
}

/// Builds one typed straight-guide mark output with valid orientation references.
fn orientation_fixture() -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default orientation document validates");
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "orientation transitions",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: toniator_domain::GuideDimensionId(80),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: toniator_domain::GuideDimensionId(81),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![
                toniator_domain::GuideDimensionId(80),
                toniator_domain::GuideDimensionId(81),
            ],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![{
            let mut bundle = base.pattern_definition_bundles()[0].clone();
            bundle.definition = definition;
            bundle
        }],
        base.pattern_settings().clone(),
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
        Vec::new(),
    )
    .expect("orientation transition document validates")
}

/// Applies one shared structural edit against the exact current definition base.
fn edited_document(
    document: Document,
    definition_id: PatternDefinitionId,
    edit: PatternDefinitionEdit,
) -> Document {
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .map(|bundle| bundle.definition.clone())
        .unwrap();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition,
            edit,
        })
        .expect("shared transition fixture edit publishes");
    history.document().clone()
}

/// Resolves one exact active compound-selector descriptor by stable field and target.
fn selector(
    document: &Document,
    field: PropertyFieldId,
    target: PropertyTarget,
) -> toniator_domain::PropertyDescriptor {
    document
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == field && descriptor.target == target)
        .expect("compound selector is active")
}

/// Checks transition leaves retain their exact ordered current contracts.
fn assert_transition_fields(draft: &VariantTransitionDraft, expected: &[PropertyFieldId]) {
    assert_eq!(
        draft
            .fields()
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        expected
    );
    for field in draft.fields() {
        assert_eq!(field.contract, property_field_contract(field.field));
    }
}

/// Proves the exhaustive current field list has one matching contract per stable discriminator.
#[test]
fn current_property_field_contracts_are_unique_and_exhaustive() {
    let contracts = property_field_contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), toniator_domain::PROPERTY_FIELD_IDS.len());
    let fields = contracts
        .iter()
        .map(|contract| contract.field)
        .collect::<BTreeSet<_>>();
    assert_eq!(fields.len(), contracts.len());
    for field in toniator_domain::PROPERTY_FIELD_IDS {
        assert_eq!(property_field_contract(*field).field, *field);
    }
}

/// Proves the current descriptor/value projection is deterministic and never revives retired controls.
#[test]
fn current_document_descriptors_are_complete_and_authoritative() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default descriptor document validates");
    document
        .validate_property_descriptors()
        .expect("descriptor projection is bidirectionally complete");
    let first = document.property_descriptors();
    let second = document.property_descriptors();
    assert_eq!(first, second);
    assert_eq!(document.property_values().len(), first.len());
    assert!(first.iter().any(|descriptor| {
        descriptor.field == PropertyFieldId::OutputSiteUseFilterKind
            && matches!(descriptor.target, PropertyTarget::OutputLayer(_, _))
            && descriptor.authority == PropertyAuthority::StructuralDefinition
            && descriptor.invalidation == InvalidationLevel::Realization
    }));
    assert!(first.iter().any(|descriptor| {
        descriptor.field == PropertyFieldId::ShapeRotationDegrees
            && descriptor.authority == PropertyAuthority::DocumentBase
    }));
}

/// Proves current exclusion and response fields replace superseded visible-mark sizing controls.
#[test]
fn current_property_vocabulary_uses_minimum_distance_and_output_keyed_responses() {
    assert_eq!(
        property_field_contract(PropertyFieldId::ExclusionMinimumCenterDistance).command_kind,
        PropertyCommandKind::SetExclusionMinimumCenterDistance
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::MarkMinimumFill).command_kind,
        PropertyCommandKind::SetChannelGeometryResponseDelta
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::ConnectedMinimumThickness).command_kind,
        PropertyCommandKind::SetChannelGeometryResponseDelta
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::RegionMinimumFill).command_kind,
        PropertyCommandKind::SetChannelGeometryResponseDelta
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::ExclusionMinimumCenterDistance).structural_support,
        StructuralSupportConstraint::None
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::VisibleMarkMargin).command_kind,
        PropertyCommandKind::SetVisibleMarkMargin
    );
    assert_eq!(
        property_field_contract(PropertyFieldId::VisibleMarkMargin).structural_support,
        StructuralSupportConstraint::VisibleMarkMarginUsesMaximumRealizedSupport
    );
}

/// Proves every current compound selector exposes every accepted alternative and exact leaf route.
#[test]
fn current_compound_transition_alternatives_are_exhaustive() {
    let document = transition_fixture();
    let random_target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60));
    let random = selector(&document, PropertyFieldId::RandomCharacter, random_target);
    assert_eq!(
        random.choices,
        vec![
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        ]
    );
    let even = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    assert_transition_fields(&even, &[PropertyFieldId::RandomEvenMinimumCenterDistance]);
    let clustered = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        )
        .unwrap();
    assert_transition_fields(
        &clustered,
        &[
            PropertyFieldId::RandomClusterDensity,
            PropertyFieldId::RandomClusterSpread,
            PropertyFieldId::RandomClusterStrength,
        ],
    );

    let modulation = selector(
        &document,
        PropertyFieldId::RandomDensityModulation,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
    );
    assert_eq!(
        modulation.choices,
        vec![
            PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
            PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
        ]
    );
    let artwork = document
        .variant_transition_draft(
            &modulation,
            PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
        )
        .unwrap();
    assert_transition_fields(
        &artwork,
        &[
            PropertyFieldId::ArtworkWeightMappingComponent,
            PropertyFieldId::ArtworkWeightMappingPlacement,
            PropertyFieldId::ArtworkWeightMappingInverted,
            PropertyFieldId::ArtworkWeightMappingGain,
            PropertyFieldId::ArtworkWeightMappingBias,
            PropertyFieldId::ArtworkWeightStrength,
            PropertyFieldId::ArtworkWeightResponse,
        ],
    );

    let exclusion = selector(
        &document,
        PropertyFieldId::RandomExclusion,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
    );
    assert_eq!(
        exclusion.choices,
        vec![
            PropertyEnumChoice::Exclusion(ExclusionKind::None),
            PropertyEnumChoice::Exclusion(ExclusionKind::MinimumCenterDistance),
            PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
        ]
    );
    for (choice, fields) in [
        (
            ExclusionKind::MinimumCenterDistance,
            vec![PropertyFieldId::ExclusionMinimumCenterDistance],
        ),
        (
            ExclusionKind::VisibleMarkMargin,
            vec![PropertyFieldId::VisibleMarkMargin],
        ),
    ] {
        let draft = document
            .variant_transition_draft(&exclusion, PropertyEnumChoice::Exclusion(choice))
            .unwrap();
        assert_transition_fields(&draft, &fields);
    }

    let output_target =
        PropertyTarget::OutputLayer(PatternDefinitionId(50), PatternOutputLayerId(70));
    let orientation = selector(&document, PropertyFieldId::OutputOrientation, output_target);
    assert_eq!(
        orientation.choices,
        vec![
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed),
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent),
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal),
        ]
    );
    for choice in [
        MarkOrientationKind::GuideTangent,
        MarkOrientationKind::GuideNormal,
    ] {
        let draft = document
            .variant_transition_draft(&orientation, PropertyEnumChoice::MarkOrientation(choice))
            .unwrap();
        assert_transition_fields(&draft, &[PropertyFieldId::OutputOrientationDimension]);
    }
    let prototype = selector(&document, PropertyFieldId::OutputPrototype, output_target);
    assert_eq!(
        prototype.choices,
        vec![
            PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::Circle),
            PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::AuthoredClosedShape),
        ]
    );
    let authored = document
        .variant_transition_draft(
            &prototype,
            PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::AuthoredClosedShape),
        )
        .unwrap();
    assert_transition_fields(&authored, &[PropertyFieldId::OutputAuthoredClosedShape]);
}

/// Proves transition payload edits finalize through existing commands and reject invalid updates atomically.
#[test]
fn current_transition_leaf_updates_finalize_and_reject_atomically() {
    let document = transition_fixture();
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60));
    let random = selector(&document, PropertyFieldId::RandomCharacter, target);
    let even = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    let update = VariantTransitionFieldUpdate {
        field: PropertyFieldId::RandomEvenMinimumCenterDistance,
        target,
        value: VariantTransitionValue::FiniteF64(2.5),
    };
    let edited = even.with_updates(std::slice::from_ref(&update)).unwrap();
    assert_eq!(
        edited.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 2.5,
            },
        }
    );
    for updates in [
        vec![update.clone(), update.clone()],
        vec![VariantTransitionFieldUpdate {
            value: VariantTransitionValue::FiniteF64(f64::NAN),
            ..update.clone()
        }],
        vec![VariantTransitionFieldUpdate {
            value: VariantTransitionValue::FiniteF64(-0.1),
            ..update.clone()
        }],
        vec![VariantTransitionFieldUpdate {
            value: VariantTransitionValue::U32(2),
            ..update.clone()
        }],
        vec![VariantTransitionFieldUpdate {
            target: PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(999)),
            ..update.clone()
        }],
    ] {
        assert!(even.with_updates(&updates).is_err());
        assert_eq!(
            even.finalize(&document).unwrap(),
            PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id: PatternMechanismId(60),
                character: RandomSiteCharacter::Even {
                    minimum_center_distance: 1.0,
                },
            }
        );
    }

    let modulation = selector(
        &document,
        PropertyFieldId::RandomDensityModulation,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
    );
    let artwork = document
        .variant_transition_draft(
            &modulation,
            PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
        )
        .unwrap();
    assert_eq!(
        artwork.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 1.0,
                response: ArtworkWeightResponse::Linear,
            },
        }
    );

    let exclusion = selector(
        &document,
        PropertyFieldId::RandomExclusion,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
    );
    let minimum = document
        .variant_transition_draft(
            &exclusion,
            PropertyEnumChoice::Exclusion(ExclusionKind::MinimumCenterDistance),
        )
        .unwrap();
    assert_eq!(
        minimum.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.0 },
        }
    );
    let visible = document
        .variant_transition_draft(
            &exclusion,
            PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
        )
        .unwrap()
        .with_updates(&[VariantTransitionFieldUpdate {
            field: PropertyFieldId::VisibleMarkMargin,
            target: exclusion.target,
            value: VariantTransitionValue::FiniteF64(0.75),
        }])
        .unwrap();
    assert_eq!(
        visible.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::VisibleMarkMargin { margin: 0.75 },
        }
    );

    let oriented = orientation_fixture();
    let orientation_target =
        PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1));
    let orientation = selector(
        &oriented,
        PropertyFieldId::OutputOrientation,
        orientation_target,
    );
    for (choice, dimension_id, expected) in [
        (
            MarkOrientationKind::GuideTangent,
            toniator_domain::GuideDimensionId(80),
            MarkOrientation::GuideTangent {
                dimension_id: toniator_domain::GuideDimensionId(80),
            },
        ),
        (
            MarkOrientationKind::GuideNormal,
            toniator_domain::GuideDimensionId(81),
            MarkOrientation::GuideNormal {
                dimension_id: toniator_domain::GuideDimensionId(81),
            },
        ),
    ] {
        let draft = oriented
            .variant_transition_draft(&orientation, PropertyEnumChoice::MarkOrientation(choice))
            .unwrap()
            .with_updates(&[VariantTransitionFieldUpdate {
                field: PropertyFieldId::OutputOrientationDimension,
                target: orientation_target,
                value: VariantTransitionValue::StableReference(Some(
                    toniator_domain::PropertyReferenceValue::GuideDimension(dimension_id),
                )),
            }])
            .unwrap();
        assert_eq!(
            draft.finalize(&oriented).unwrap(),
            PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(1),
                orientation: expected,
            }
        );
    }
}

/// Proves leafless selector fallbacks finalize exactly while unchanged choices remain no-ops.
#[test]
fn current_transition_leafless_fallbacks_and_same_choices_are_explicit() {
    let base = transition_fixture();
    for (field, target, choice) in [
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
        ),
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
            PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
        ),
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
            PropertyEnumChoice::Exclusion(ExclusionKind::None),
        ),
    ] {
        let selector = selector(&base, field, target);
        let draft = base.variant_transition_draft(&selector, choice).unwrap();
        assert!(draft.fields().is_empty());
        assert!(draft.finalize(&base).is_err());
    }

    let even = edited_document(
        transition_fixture(),
        PatternDefinitionId(50),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 2.0,
            },
        },
    );
    let raw = even
        .variant_transition_draft(
            &selector(
                &even,
                PropertyFieldId::RandomCharacter,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            ),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
        )
        .unwrap();
    assert!(raw.fields().is_empty());
    assert_eq!(
        raw.finalize(&even).unwrap(),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::RawUniform,
        }
    );

    let artwork = edited_document(
        transition_fixture(),
        PatternDefinitionId(50),
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 0.5,
                response: ArtworkWeightResponse::Smoothstep,
            },
        },
    );
    let uniform = artwork
        .variant_transition_draft(
            &selector(
                &artwork,
                PropertyFieldId::RandomDensityModulation,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
            ),
            PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
        )
        .unwrap();
    assert!(uniform.fields().is_empty());
    assert_eq!(
        uniform.finalize(&artwork).unwrap(),
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::Uniform,
        }
    );

    let excluded = edited_document(
        transition_fixture(),
        PatternDefinitionId(50),
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::VisibleMarkMargin { margin: 0.5 },
        },
    );
    let none = excluded
        .variant_transition_draft(
            &selector(
                &excluded,
                PropertyFieldId::RandomExclusion,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
            ),
            PropertyEnumChoice::Exclusion(ExclusionKind::None),
        )
        .unwrap();
    assert!(none.fields().is_empty());
    assert_eq!(
        none.finalize(&excluded).unwrap(),
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::None,
        }
    );

    let tangent = edited_document(
        orientation_fixture(),
        PatternDefinitionId(1),
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(1),
            orientation: MarkOrientation::GuideTangent {
                dimension_id: toniator_domain::GuideDimensionId(80),
            },
        },
    );
    let fixed = tangent
        .variant_transition_draft(
            &selector(
                &tangent,
                PropertyFieldId::OutputOrientation,
                PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1)),
            ),
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed),
        )
        .unwrap();
    assert!(fixed.fields().is_empty());
    assert_eq!(
        fixed.finalize(&tangent).unwrap(),
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(1),
            orientation: MarkOrientation::Fixed,
        }
    );
}

/// Proves finalized drafts reject selector or same-definition staleness before command publication.
#[test]
fn current_transition_drafts_reject_stale_bases() {
    let mut history = DocumentHistory::new(DocumentSession::new(transition_fixture()).unwrap());
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60));
    let selector = selector(history.document(), PropertyFieldId::RandomCharacter, target);
    let draft = history
        .document()
        .variant_transition_draft(
            &selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        )
        .unwrap();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == PatternDefinitionId(50))
        .map(|bundle| &bundle.definition)
        .unwrap()
        .clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition,
            edit: PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: PatternMechanismId(61),
                modulation: SiteDensityModulation::ArtworkWeighted {
                    mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                    strength: 0.75,
                    response: ArtworkWeightResponse::Smoothstep,
                },
            },
        })
        .unwrap();
    assert!(draft.finalize(history.document()).is_err());
}
