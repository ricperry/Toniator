use std::collections::HashSet;

use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, ChannelAppearance, ChannelId, ChannelPaint,
    ChannelPatternLayout, ChannelSourceMapping, ChannelState, ColorComponent, ColorValue,
    CoveragePolicy, DensityEditedAxis, DensityMetric2D, DensityModulationKind, Document,
    DocumentCommand, DocumentCommandFieldClassification, DocumentHistory, DocumentId,
    DocumentSession, ExclusionKind, GuideDimensionId, InvalidationLevel, LegacyMappingFieldEdit,
    MarkGeometryFieldEdit, MarkGeometryResponse, MarkOrientation, MarkOrientationKind,
    MarkPrototype, ModeledMappingFieldEdit, NonFieldCommandOperation, PROPERTY_FIELD_IDS,
    PatternDefinition, PatternDefinitionEdit, PatternDefinitionId, PatternMechanismId,
    PatternOutputLayerId, PropertyApplicability, PropertyCommandKind, PropertyCurrentValueKind,
    PropertyEnumChoice, PropertyFieldId, PropertyReferenceValue, PropertyTarget,
    RandomCharacterKind, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference,
    SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
    StructuralSupportConstraint, TranslationEditedAxis, VariantTransitionDraft,
    VariantTransitionFieldUpdate, VariantTransitionValue, VisibleMarkSizingPolicy,
    property_field_contract, property_field_contracts,
};

fn generalized_document(
    product: toniator_domain::GeneralizedSiteProduct,
    orientation: MarkOrientation,
) -> Document {
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(10),
        "guides",
        PatternMechanismId(20),
        PatternMechanismId(21),
        PatternOutputLayerId(30),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(40),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(41),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(42),
                baseline_angle_degrees: 45.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        product,
        orientation,
        CoveragePolicy {
            guard_steps: 2,
            maximum_support_radius: 8.0,
        },
    );
    let channel = |id| ChannelState {
        id: ChannelId(id),
        pattern_definition_id: PatternDefinitionId(10),
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: 90.0,
                across_y: 60.0,
                aspect_locked: true,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
        },
        appearance: ChannelAppearance {
            visible: true,
            color: ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity: 1.0,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_size: 0.0,
            maximum_size: 8.0,
        },
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    };
    Document::new(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![definition],
        vec![channel(1), channel(2)],
    )
    .unwrap()
}

fn shared_document() -> Document {
    generalized_document(
        toniator_domain::GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(41)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
    )
}

fn shared_copy_exhaustion_document(
    definition_id: PatternDefinitionId,
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    output_id: PatternOutputLayerId,
    dimensions: Vec<StraightGuideDimension>,
    orientation: MarkOrientation,
) -> Document {
    let product_dimensions = dimensions
        .iter()
        .take(2)
        .map(|dimension| dimension.id)
        .collect();
    let definition = PatternDefinition::generalized_straight_guides(
        definition_id,
        "copy exhaustion",
        guide_id,
        site_id,
        output_id,
        dimensions,
        toniator_domain::GeneralizedSiteProduct::Intersections {
            dimensions: product_dimensions,
            merge_epsilon: 0.0,
        },
        orientation,
        CoveragePolicy {
            guard_steps: 2,
            maximum_support_radius: 8.0,
        },
    );
    let mut channels = shared_document().channels().unwrap().to_vec();
    for channel in &mut channels {
        channel.pattern_definition_id = definition_id;
    }
    Document::new(
        DocumentId(99),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![definition],
        channels,
    )
    .unwrap()
}

fn copy_exhaustion_dimensions(last: GuideDimensionId) -> Vec<StraightGuideDimension> {
    vec![
        StraightGuideDimension {
            id: GuideDimensionId(last.0 - 1),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        },
        StraightGuideDimension {
            id: last,
            baseline_angle_degrees: 90.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        },
    ]
}

fn along_guide_document() -> Document {
    generalized_document(
        toniator_domain::GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(40)],
            interval_multiplier: 0.75,
            phase: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(40),
        },
    )
}

#[test]
fn descriptors_are_deterministic_duplicate_free_and_backed_by_commands() {
    let document = shared_document();
    let first = document.property_descriptors();
    assert_eq!(first, document.property_descriptors());
    assert!(document.validate_property_descriptors().is_ok());
    let mut unique = HashSet::new();
    assert!(
        first
            .iter()
            .all(|descriptor| unique.insert((descriptor.field, descriptor.target)))
    );
    assert!(first.iter().any(|descriptor| {
        descriptor.copy_on_edit_escalates_to_family
            && descriptor.invalidation == InvalidationLevel::Realization
    }));
    let definition_target = PropertyTarget::Definition(PatternDefinitionId(10));
    assert_eq!(
        first
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::CoverageMaximumSupportRadius
                    && descriptor.target == definition_target
            })
            .unwrap()
            .structural_support,
        StructuralSupportConstraint::DefinesMaximumMarkSupportRadius
    );
    assert_eq!(
        first
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::CoverageGuardSteps
                    && descriptor.target == definition_target
            })
            .unwrap()
            .structural_support,
        StructuralSupportConstraint::None
    );
    let guide_target = PropertyTarget::GuideDimension(
        PatternDefinitionId(10),
        PatternMechanismId(20),
        GuideDimensionId(40),
    );
    assert_eq!(
        first
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::GuideBaselineAngle
                    && descriptor.target == guide_target
            })
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetGuideBaselineAngle
    );
    assert_eq!(
        first
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::GuidePhase && descriptor.target == guide_target
            })
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetGuidePhase
    );
    assert_eq!(
        first
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::GuideSpacingMultiplier
                    && descriptor.target == guide_target
            })
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetGuideSpacingMultiplier
    );
}

#[test]
fn active_descriptor_values_are_typed_separate_and_exhaustive() {
    let document = shared_document();
    let descriptors = document.property_descriptors();
    let values = document.property_values();
    assert_eq!(values.len(), descriptors.len());
    assert!(values.iter().zip(descriptors).all(|(value, descriptor)| {
        value.descriptor == descriptor
            && matches!(
                (&value.descriptor.value_kind, &value.value),
                (
                    toniator_domain::PropertyValueKind::FiniteF64,
                    PropertyCurrentValueKind::FiniteF64(_)
                ) | (
                    toniator_domain::PropertyValueKind::U32,
                    PropertyCurrentValueKind::U32(_)
                ) | (
                    toniator_domain::PropertyValueKind::Boolean,
                    PropertyCurrentValueKind::Boolean(_)
                ) | (
                    toniator_domain::PropertyValueKind::EnumChoice,
                    PropertyCurrentValueKind::EnumChoice(_)
                ) | (
                    toniator_domain::PropertyValueKind::StableIdReference,
                    PropertyCurrentValueKind::Reference(_)
                ) | (
                    toniator_domain::PropertyValueKind::StableIdReference,
                    PropertyCurrentValueKind::ReferenceCollection(_)
                )
            )
    }));
    assert_eq!(document.property_values(), values);
}

fn transition_fixture() -> Document {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
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
                    maximum_support_radius: 8.0,
                },
            ),
        })
        .unwrap();
    history.document().clone()
}

fn selector(
    document: &Document,
    field: PropertyFieldId,
    target: PropertyTarget,
) -> toniator_domain::PropertyDescriptor {
    document
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == field && descriptor.target == target)
        .unwrap()
}

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
        assert_eq!(
            field.contract,
            property_field_contract(field.field),
            "transition field {:?} must copy its authoritative contract",
            field.field
        );
    }
}

#[test]
fn compound_variant_drafts_are_exhaustive_explicit_and_finalize_to_existing_edits() {
    let document = transition_fixture();
    let random = selector(
        &document,
        PropertyFieldId::RandomCharacter,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
    );
    let even = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    assert_transition_fields(&even, &[PropertyFieldId::RandomEvenMinimumCenterDistance]);
    assert_eq!(
        even.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 1.0,
            },
        }
    );
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
    assert_eq!(
        clustered.finalize(&document).unwrap(),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Clustered {
                cluster_density: 1.0,
                cluster_spread: 1.0,
                cluster_strength: 1.0,
            },
        }
    );

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
    for (choice, fields, expected) in [
        (
            ExclusionKind::MinimumCenterDistance,
            vec![PropertyFieldId::ExclusionMinimumCenterDistance],
            PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(62),
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.0 },
            },
        ),
        (
            ExclusionKind::VisibleMarkMargin,
            vec![
                PropertyFieldId::VisibleMarkMargin,
                PropertyFieldId::VisibleMarkSizingPolicy,
            ],
            PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(62),
                policy: SiteExclusionPolicy::VisibleMarkMargin {
                    margin: 0.0,
                    sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
                },
            },
        ),
    ] {
        let draft = document
            .variant_transition_draft(&exclusion, PropertyEnumChoice::Exclusion(choice))
            .unwrap();
        assert_transition_fields(&draft, &fields);
        assert_eq!(draft.finalize(&document).unwrap(), expected);
    }

    let oriented = along_guide_document();
    let orientation = selector(
        &oriented,
        PropertyFieldId::OutputOrientation,
        PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
    );
    let normal = oriented
        .variant_transition_draft(
            &orientation,
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal),
        )
        .unwrap();
    let dimension = normal.fields().first().unwrap();
    assert_transition_fields(&normal, &[PropertyFieldId::OutputOrientationDimension]);
    assert_eq!(dimension.field, PropertyFieldId::OutputOrientationDimension);
    assert!(matches!(
        dimension.value,
        VariantTransitionValue::StableReference(None)
    ));
    assert_eq!(
        dimension.reference_choices,
        vec![
            PropertyReferenceValue::GuideDimension(GuideDimensionId(40)),
            PropertyReferenceValue::GuideDimension(GuideDimensionId(41)),
            PropertyReferenceValue::GuideDimension(GuideDimensionId(42)),
        ]
    );
    assert!(
        normal.finalize(&oriented).is_err(),
        "guided orientation cannot silently choose a guide"
    );
    let normal = normal
        .with_updates(&[VariantTransitionFieldUpdate {
            field: PropertyFieldId::OutputOrientationDimension,
            target: PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
            value: VariantTransitionValue::StableReference(Some(
                PropertyReferenceValue::GuideDimension(GuideDimensionId(41)),
            )),
        }])
        .unwrap();
    assert_eq!(
        normal.finalize(&oriented).unwrap(),
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::GuideNormal {
                dimension_id: GuideDimensionId(41)
            },
        }
    );

    let fixed = oriented
        .variant_transition_draft(
            &orientation,
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed),
        )
        .unwrap();
    assert_transition_fields(&fixed, &[]);
    assert_eq!(
        fixed.finalize(&oriented).unwrap(),
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::Fixed,
        }
    );
}

#[test]
fn compound_transition_finalization_reaches_every_selector_alternative_exactly() {
    let mut history = DocumentHistory::new(DocumentSession::new(transition_fixture()).unwrap());
    let apply = |history: &mut DocumentHistory, edit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(50))
            .unwrap()
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(50),
                base_definition: base,
                edit,
            })
            .unwrap();
    };

    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 3.25,
            },
        },
    );
    let random = selector(
        history.document(),
        PropertyFieldId::RandomCharacter,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
    );
    let raw = history
        .document()
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
        )
        .unwrap();
    assert_transition_fields(&raw, &[]);
    assert_eq!(
        raw.finalize(history.document()).unwrap(),
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::RawUniform,
        }
    );

    apply(
        &mut history,
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 0.75,
                response: ArtworkWeightResponse::Smoothstep,
            },
        },
    );
    let modulation = selector(
        history.document(),
        PropertyFieldId::RandomDensityModulation,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
    );
    let uniform = history
        .document()
        .variant_transition_draft(
            &modulation,
            PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
        )
        .unwrap();
    assert_transition_fields(&uniform, &[]);
    assert_eq!(
        uniform.finalize(history.document()).unwrap(),
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::Uniform,
        }
    );

    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 1.5 },
        },
    );
    let exclusion = selector(
        history.document(),
        PropertyFieldId::RandomExclusion,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
    );
    let none = history
        .document()
        .variant_transition_draft(
            &exclusion,
            PropertyEnumChoice::Exclusion(ExclusionKind::None),
        )
        .unwrap();
    assert_transition_fields(&none, &[]);
    assert_eq!(
        none.finalize(history.document()).unwrap(),
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::None,
        }
    );

    let oriented = shared_document();
    let orientation = selector(
        &oriented,
        PropertyFieldId::OutputOrientation,
        PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
    );
    let tangent = oriented
        .variant_transition_draft(
            &orientation,
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent),
        )
        .unwrap();
    assert_transition_fields(&tangent, &[PropertyFieldId::OutputOrientationDimension]);
    let tangent = tangent
        .with_updates(&[VariantTransitionFieldUpdate {
            field: PropertyFieldId::OutputOrientationDimension,
            target: PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
            value: VariantTransitionValue::StableReference(Some(
                PropertyReferenceValue::GuideDimension(GuideDimensionId(40)),
            )),
        }])
        .unwrap();
    assert_eq!(
        tangent.finalize(&oriented).unwrap(),
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(40),
            },
        }
    );
}

#[test]
fn compound_transition_drafts_retain_same_choice_and_reject_bad_or_stale_updates_atomically() {
    let document = transition_fixture();
    let random = selector(
        &document,
        PropertyFieldId::RandomCharacter,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
    );
    let same = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
        )
        .unwrap();
    assert!(same.fields().is_empty());
    assert!(
        same.finalize(&document).is_err(),
        "same choice must remain a semantic no-op"
    );
    let even = document
        .variant_transition_draft(
            &random,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    let update = VariantTransitionFieldUpdate {
        field: PropertyFieldId::RandomEvenMinimumCenterDistance,
        target: PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
        value: VariantTransitionValue::FiniteF64(2.5),
    };
    let edited = even.with_updates(std::slice::from_ref(&update)).unwrap();
    assert_eq!(
        edited.finalize(&document).unwrap().field_projection().field,
        PropertyFieldId::RandomCharacter
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
                    minimum_center_distance: 1.0
                }
            }
        );
    }
    let oriented = along_guide_document();
    let orientation = selector(
        &oriented,
        PropertyFieldId::OutputOrientation,
        PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
    );
    let normal = oriented
        .variant_transition_draft(
            &orientation,
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal),
        )
        .unwrap();
    let reference_update = VariantTransitionFieldUpdate {
        field: PropertyFieldId::OutputOrientationDimension,
        target: PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
        value: VariantTransitionValue::StableReference(Some(PropertyReferenceValue::Mechanism(
            PatternMechanismId(20),
        ))),
    };
    assert!(normal.with_updates(&[reference_update]).is_err());
    let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
    let base = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(50))
        .unwrap()
        .clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: base,
            edit: PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id: PatternMechanismId(60),
                character: RandomSiteCharacter::Even {
                    minimum_center_distance: 2.0,
                },
            },
        })
        .unwrap();
    assert!(
        edited.finalize(history.document()).is_err(),
        "draft base must reject a changed selector"
    );
}

#[test]
fn compound_transition_drafts_reject_same_discriminant_and_unrelated_definition_staleness() {
    let mut history = DocumentHistory::new(DocumentSession::new(transition_fixture()).unwrap());
    let apply = |history: &mut DocumentHistory, edit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(50))
            .unwrap()
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(50),
                base_definition: base,
                edit,
            })
            .unwrap();
    };
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 3.25,
            },
        },
    );
    let random_selector = selector(
        history.document(),
        PropertyFieldId::RandomCharacter,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
    );
    let same_even = history
        .document()
        .variant_transition_draft(
            &random_selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 4.0,
            },
        },
    );
    assert!(
        same_even.finalize(history.document()).is_err(),
        "same enum discriminant cannot overwrite newer payload"
    );
    let fresh = history
        .document()
        .variant_transition_draft(
            &selector(
                history.document(),
                PropertyFieldId::RandomCharacter,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            ),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        )
        .unwrap();
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(60),
            seed: 18,
        },
    );
    assert!(
        fresh.finalize(history.document()).is_err(),
        "an unrelated same-definition structural edit makes the draft stale"
    );
}

#[test]
fn compound_transition_same_choice_drafts_retain_authoritative_payloads() {
    let mut history = DocumentHistory::new(DocumentSession::new(transition_fixture()).unwrap());
    let apply = |history: &mut DocumentHistory, edit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(50))
            .unwrap()
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(50),
                base_definition: base,
                edit,
            })
            .unwrap();
    };
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 3.25,
            },
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping {
                    component: SourceMappingComponent::Alpha,
                    placement: SourcePlacement::StretchToCanvas,
                    inverted: true,
                    gain: 0.75,
                    bias: -0.25,
                },
                strength: 0.5,
                response: ArtworkWeightResponse::Smoothstep,
            },
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::VisibleMarkMargin {
                margin: 1.5,
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
        },
    );
    let document = history.document();
    for (field, target, choice, expected) in [
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
            VariantTransitionValue::FiniteF64(3.25),
        ),
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61)),
            PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                SourceMappingComponent::Alpha,
            )),
        ),
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62)),
            PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
            VariantTransitionValue::FiniteF64(1.5),
        ),
    ] {
        let draft = document
            .variant_transition_draft(&selector(document, field, target), choice)
            .unwrap();
        assert!(
            draft
                .fields()
                .iter()
                .any(|candidate| candidate.value == expected)
        );
        assert!(
            draft.finalize(document).is_err(),
            "same choice remains a semantic no-op"
        );
    }
    let oriented = along_guide_document();
    let orientation = selector(
        &oriented,
        PropertyFieldId::OutputOrientation,
        PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30)),
    );
    let same = oriented
        .variant_transition_draft(
            &orientation,
            PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent),
        )
        .unwrap();
    assert_eq!(
        same.fields()[0].value,
        VariantTransitionValue::StableReference(Some(PropertyReferenceValue::GuideDimension(
            GuideDimensionId(40)
        )))
    );
    assert!(same.finalize(&oriented).is_err());
}

#[test]
fn finalized_transition_edit_preserves_existing_history_results_and_redo_branch_rules() {
    let mut history = DocumentHistory::new(DocumentSession::new(transition_fixture()).unwrap());
    for channel_id in [ChannelId(1), ChannelId(2)] {
        history
            .apply(&DocumentCommand::RetargetChannelPatternDefinition {
                channel_id,
                definition_id: PatternDefinitionId(50),
            })
            .unwrap();
    }
    let random_selector = selector(
        history.document(),
        PropertyFieldId::RandomCharacter,
        PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
    );
    let draft = history
        .document()
        .variant_transition_draft(
            &random_selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    let base = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(50))
        .unwrap()
        .clone();
    let result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: base,
            edit: draft.finalize(history.document()).unwrap(),
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let applied = history.document().clone();
    let revision = history.revision();
    assert!(history.can_undo());
    history.undo().unwrap();
    assert!(history.can_redo());
    history.redo().unwrap();
    assert_eq!(history.document(), &applied);
    assert_eq!(history.revision().0, revision.0 + 2);
    history.undo().unwrap();
    let base = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(50))
        .unwrap()
        .clone();
    let clustered = history
        .document()
        .variant_transition_draft(
            &selector(
                history.document(),
                PropertyFieldId::RandomCharacter,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            ),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        )
        .unwrap();
    let branch_revision = history.revision();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: base,
            edit: clustered.finalize(history.document()).unwrap(),
        })
        .unwrap();
    assert_eq!(history.revision().0, branch_revision.0 + 1);
    assert!(
        !history.can_redo(),
        "new finalized transition truncates redo"
    );
    let no_op = history
        .document()
        .variant_transition_draft(
            &selector(
                history.document(),
                PropertyFieldId::RandomCharacter,
                PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60)),
            ),
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
        )
        .unwrap();
    let no_op_document = history.document().clone();
    let no_op_revision = history.revision();
    assert!(no_op.finalize(history.document()).is_err());
    assert_eq!(history.document(), &no_op_document);
    assert_eq!(history.revision(), no_op_revision);
}

#[test]
fn field_contracts_are_exhaustive_and_descriptors_emit_contract_metadata() {
    let contracts: Vec<_> = property_field_contracts().collect();
    assert_eq!(contracts.len(), PROPERTY_FIELD_IDS.len());
    let mut fields = HashSet::new();
    assert!(
        contracts
            .iter()
            .all(|contract| fields.insert(contract.field))
    );
    assert_eq!(fields.len(), PROPERTY_FIELD_IDS.len());

    for document in [shared_document(), along_guide_document()] {
        let descriptors = document.property_descriptors();
        for descriptor in &descriptors {
            let contract = property_field_contract(descriptor.field);
            assert_eq!(descriptor.command_kind(), contract.command_kind);
            assert_eq!(descriptor.value_kind, contract.value_kind);
            assert_eq!(descriptor.bounds, contract.bounds);
            assert_eq!(descriptor.unit, contract.unit);
            assert_eq!(descriptor.invalidation, contract.invalidation);
            assert_eq!(
                descriptor.reference_constraint,
                contract.reference_constraint
            );
            assert_eq!(descriptor.choice_policy, contract.choice_policy);
            assert_eq!(
                descriptor.copy_on_edit_escalates_to_family,
                contract.copy_on_edit_escalates_to_family
            );
            match contract.applicability {
                PropertyApplicability::CurrentPaint
                | PropertyApplicability::CurrentDensityModulation
                | PropertyApplicability::CurrentExclusion => {}
                _ => assert_eq!(
                    descriptor.choices, contract.choices,
                    "static choices must come from the field contract for {:?}",
                    descriptor.field
                ),
            }
        }
        assert!(document.validate_property_descriptors().is_ok());
    }
}

fn representative_descriptor_command(field: PropertyFieldId) -> DocumentCommand {
    let straight_base = shared_document().pattern_definitions()[0].clone();
    let along_base = along_guide_document().pattern_definitions()[0].clone();
    let structural = |edit| DocumentCommand::EditSharedPatternDefinition {
        definition_id: PatternDefinitionId(10),
        base_definition: straight_base.clone(),
        edit,
    };
    match field {
        PropertyFieldId::SourceReference => DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(SourceReferenceId::new("projection-source").unwrap()),
        },
        PropertyFieldId::DensityAcrossX => DocumentCommand::SetDensityAxis {
            channel_id: ChannelId(1),
            edited_axis: DensityEditedAxis::AcrossX,
            value: 91.0,
        },
        PropertyFieldId::DensityAcrossY => DocumentCommand::SetDensityAxis {
            channel_id: ChannelId(1),
            edited_axis: DensityEditedAxis::AcrossY,
            value: 61.0,
        },
        PropertyFieldId::DensityAspectLocked => DocumentCommand::SetDensityAspectLock {
            channel_id: ChannelId(1),
            aspect_locked: false,
        },
        PropertyFieldId::RotationDegrees => DocumentCommand::SetRotation {
            channel_id: ChannelId(1),
            rotation_degrees: 1.0,
        },
        PropertyFieldId::TranslationX => DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::X,
            value: 1.0,
        },
        PropertyFieldId::TranslationY => DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::Y,
            value: 1.0,
        },
        PropertyFieldId::MarkMinimumSize => DocumentCommand::SetMarkGeometryField {
            channel_id: ChannelId(1),
            edit: MarkGeometryFieldEdit::MinimumSize(1.0),
        },
        PropertyFieldId::MarkMaximumSize => DocumentCommand::SetMarkGeometryField {
            channel_id: ChannelId(1),
            edit: MarkGeometryFieldEdit::MaximumSize(7.0),
        },
        PropertyFieldId::LegacyMappingComponent => DocumentCommand::SetLegacyMappingField {
            channel_id: ChannelId(1),
            edit: LegacyMappingFieldEdit::Component(SourceComponent::Alpha),
        },
        PropertyFieldId::LegacyMappingPlacement => DocumentCommand::SetLegacyMappingField {
            channel_id: ChannelId(1),
            edit: LegacyMappingFieldEdit::Placement(SourcePlacement::StretchToCanvas),
        },
        PropertyFieldId::ModeledMappingComponent => DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Component(SourceMappingComponent::Red),
        },
        PropertyFieldId::ModeledMappingPlacement => DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Placement(SourcePlacement::StretchToCanvas),
        },
        PropertyFieldId::ModeledMappingInverted => DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Inverted(true),
        },
        PropertyFieldId::ModeledMappingGain => DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Gain(0.5),
        },
        PropertyFieldId::ModeledMappingBias => DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Bias(0.1),
        },
        PropertyFieldId::Paint => DocumentCommand::SetChannelPaint {
            channel_id: ChannelId(1),
            paint: ChannelPaint::SampledSource,
        },
        PropertyFieldId::ColorRed => color_projection(ColorComponent::Red),
        PropertyFieldId::ColorGreen => color_projection(ColorComponent::Green),
        PropertyFieldId::ColorBlue => color_projection(ColorComponent::Blue),
        PropertyFieldId::ColorAlpha => color_projection(ColorComponent::Alpha),
        PropertyFieldId::Opacity => DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        },
        PropertyFieldId::Visibility => DocumentCommand::SetVisibility {
            channel_id: ChannelId(1),
            visible: false,
        },
        PropertyFieldId::DefinitionSelection => DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(1),
            definition_id: PatternDefinitionId(10),
        },
        PropertyFieldId::CoverageGuardSteps => {
            structural(PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 })
        }
        PropertyFieldId::CoverageMaximumSupportRadius => {
            structural(PatternDefinitionEdit::SetCoverageMaximumSupportRadius {
                maximum_support_radius: 7.0,
            })
        }
        PropertyFieldId::GuideBaselineAngle => {
            structural(PatternDefinitionEdit::SetGuideBaselineAngle {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(40),
                baseline_angle_degrees: 1.0,
            })
        }
        PropertyFieldId::GuidePhase => structural(PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: PatternMechanismId(20),
            dimension_id: GuideDimensionId(40),
            phase: 0.1,
        }),
        PropertyFieldId::GuideSpacingMultiplier => {
            structural(PatternDefinitionEdit::SetGuideSpacingMultiplier {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(40),
                spacing_multiplier: 1.1,
            })
        }
        PropertyFieldId::IntersectionDimensions => {
            structural(PatternDefinitionEdit::SetIntersectionDimensions {
                mechanism_id: PatternMechanismId(21),
                dimensions: vec![GuideDimensionId(41), GuideDimensionId(40)],
            })
        }
        PropertyFieldId::IntersectionMergeEpsilon => {
            structural(PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: PatternMechanismId(21),
                merge_epsilon: 0.1,
            })
        }
        PropertyFieldId::AlongGuideDimensions => DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: along_base.clone(),
            edit: PatternDefinitionEdit::SetAlongGuideDimensions {
                mechanism_id: PatternMechanismId(21),
                dimensions: vec![GuideDimensionId(41)],
            },
        },
        PropertyFieldId::AlongGuideIntervalMultiplier => {
            DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(10),
                base_definition: along_base.clone(),
                edit: PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
                    mechanism_id: PatternMechanismId(21),
                    interval_multiplier: 1.1,
                },
            }
        }
        PropertyFieldId::AlongGuidePhase => DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: along_base,
            edit: PatternDefinitionEdit::SetAlongGuidePhase {
                mechanism_id: PatternMechanismId(21),
                phase: 0.1,
            },
        },
        PropertyFieldId::RandomCharacter => structural(PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::RawUniform,
        }),
        PropertyFieldId::RandomEvenMinimumCenterDistance => {
            structural(PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
                mechanism_id: PatternMechanismId(60),
                minimum_center_distance: 2.0,
            })
        }
        PropertyFieldId::RandomClusterDensity => {
            structural(PatternDefinitionEdit::SetRandomClusterDensity {
                mechanism_id: PatternMechanismId(60),
                cluster_density: 1.0,
            })
        }
        PropertyFieldId::RandomClusterSpread => {
            structural(PatternDefinitionEdit::SetRandomClusterSpread {
                mechanism_id: PatternMechanismId(60),
                cluster_spread: 2.0,
            })
        }
        PropertyFieldId::RandomClusterStrength => {
            structural(PatternDefinitionEdit::SetRandomClusterStrength {
                mechanism_id: PatternMechanismId(60),
                cluster_strength: 0.5,
            })
        }
        PropertyFieldId::RandomSeed => structural(PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(60),
            seed: 1,
        }),
        PropertyFieldId::RandomDensityModulation => {
            structural(PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: PatternMechanismId(61),
                modulation: SiteDensityModulation::Uniform,
            })
        }
        PropertyFieldId::ArtworkWeightMappingComponent => {
            structural(PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                mechanism_id: PatternMechanismId(61),
                component: SourceMappingComponent::Red,
            })
        }
        PropertyFieldId::ArtworkWeightMappingPlacement => {
            structural(PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
                mechanism_id: PatternMechanismId(61),
                placement: SourcePlacement::StretchToCanvas,
            })
        }
        PropertyFieldId::ArtworkWeightMappingInverted => {
            structural(PatternDefinitionEdit::SetArtworkWeightMappingInverted {
                mechanism_id: PatternMechanismId(61),
                inverted: true,
            })
        }
        PropertyFieldId::ArtworkWeightMappingGain => {
            structural(PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: PatternMechanismId(61),
                gain: 0.5,
            })
        }
        PropertyFieldId::ArtworkWeightMappingBias => {
            structural(PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: PatternMechanismId(61),
                bias: 0.1,
            })
        }
        PropertyFieldId::ArtworkWeightStrength => {
            structural(PatternDefinitionEdit::SetArtworkWeightStrength {
                mechanism_id: PatternMechanismId(61),
                strength: 0.5,
            })
        }
        PropertyFieldId::ArtworkWeightResponse => {
            structural(PatternDefinitionEdit::SetArtworkWeightResponse {
                mechanism_id: PatternMechanismId(61),
                response: ArtworkWeightResponse::Linear,
            })
        }
        PropertyFieldId::RandomExclusion => {
            structural(PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(62),
                policy: SiteExclusionPolicy::None,
            })
        }
        PropertyFieldId::ExclusionMinimumCenterDistance => {
            structural(PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
                mechanism_id: PatternMechanismId(62),
                minimum_center_distance: 2.0,
            })
        }
        PropertyFieldId::VisibleMarkMargin => {
            structural(PatternDefinitionEdit::SetVisibleMarkMargin {
                mechanism_id: PatternMechanismId(62),
                margin: 0.5,
            })
        }
        PropertyFieldId::VisibleMarkSizingPolicy => {
            structural(PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
                mechanism_id: PatternMechanismId(62),
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            })
        }
        PropertyFieldId::RandomMaximumAttempts => {
            structural(PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(63),
                maximum_attempts: 1,
            })
        }
        PropertyFieldId::RandomMaximumNeighborChecks => {
            structural(PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                mechanism_id: PatternMechanismId(63),
                maximum_neighbor_checks: 1,
            })
        }
        PropertyFieldId::OutputSiteProduct => {
            structural(PatternDefinitionEdit::SetOutputSiteProduct {
                output_layer_id: PatternOutputLayerId(30),
                site_mechanism_id: PatternMechanismId(21),
            })
        }
        PropertyFieldId::OutputPrototype => {
            structural(PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: PatternOutputLayerId(30),
                prototype: MarkPrototype::Circle,
            })
        }
        PropertyFieldId::OutputOrientation => {
            structural(PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(30),
                orientation: MarkOrientation::GuideTangent {
                    dimension_id: GuideDimensionId(40),
                },
            })
        }
        PropertyFieldId::OutputOrientationDimension => {
            structural(PatternDefinitionEdit::SetOutputOrientationDimension {
                output_layer_id: PatternOutputLayerId(30),
                dimension_id: GuideDimensionId(40),
            })
        }
    }
}

fn color_projection(component: ColorComponent) -> DocumentCommand {
    DocumentCommand::SetColorComponent {
        channel_id: ChannelId(1),
        component,
        value: 0.5,
    }
}

#[test]
fn every_contract_field_has_one_real_leaf_projection_and_non_fields_are_explicit() {
    let mut projected = std::collections::BTreeMap::new();
    for field in PROPERTY_FIELD_IDS.iter().copied() {
        let command = representative_descriptor_command(field);
        let DocumentCommandFieldClassification::DescriptorBacked(projections) =
            command.field_classification()
        else {
            panic!("descriptor field {field:?} was classified as non-field");
        };
        assert_eq!(projections.len(), 1, "{field:?}");
        let projection = projections[0];
        assert_eq!(projection.field, field);
        assert_eq!(
            property_field_contract(field).command_kind,
            property_field_contract(projection.field).command_kind
        );
        *projected.entry(field).or_insert(0_usize) += 1;
    }
    assert_eq!(projected.len(), PROPERTY_FIELD_IDS.len());
    assert!(projected.values().all(|count| *count == 1));

    let base = shared_document().pattern_definitions()[0].clone();
    for (command, operation) in [
        (
            DocumentCommand::AddPatternDefinition {
                definition: toniator_domain::PatternDefinitionDraft {
                    name: "new".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        maximum_support_radius: 1.0,
                    },
                },
            },
            NonFieldCommandOperation::AddPatternDefinition,
        ),
        (
            DocumentCommand::AddTypedPatternDefinition {
                definition: base.clone(),
            },
            NonFieldCommandOperation::AddTypedPatternDefinition,
        ),
        (
            DocumentCommand::ReplaceSelectedChannelDefinitionTopology {
                channel_id: ChannelId(1),
                base_definition: base.clone(),
                definition: base.clone(),
            },
            NonFieldCommandOperation::ReplaceSelectedChannelDefinitionTopology,
        ),
        (
            DocumentCommand::DuplicatePatternDefinition {
                definition_id: PatternDefinitionId(10),
            },
            NonFieldCommandOperation::DuplicatePatternDefinition,
        ),
        (
            DocumentCommand::RemoveUnreferencedPatternDefinition {
                definition_id: PatternDefinitionId(10),
            },
            NonFieldCommandOperation::RemoveUnreferencedPatternDefinition,
        ),
        (
            DocumentCommand::ReplaceChannelTopology {
                model: toniator_domain::HalftoneChannelModel::Rgb,
                topology: toniator_domain::ChannelTopology::new(Vec::new()),
            },
            NonFieldCommandOperation::ReplaceChannelTopology,
        ),
    ] {
        assert_eq!(
            command.field_classification(),
            DocumentCommandFieldClassification::NonField(operation)
        );
        assert!(command.field_projections().is_empty());
    }
}

#[test]
fn translation_axes_are_independent_contract_leaves_and_history_atomic() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let original = history.document().clone();
    let x = DocumentCommand::SetTranslationAxis {
        channel_id: ChannelId(1),
        edited_axis: TranslationEditedAxis::X,
        value: 3.0,
    };
    let result = history.apply(&x).unwrap();
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    assert_eq!(
        (
            history
                .document()
                .channel(ChannelId(1))
                .unwrap()
                .layout
                .translation_x,
            history
                .document()
                .channel(ChannelId(1))
                .unwrap()
                .layout
                .translation_y,
        ),
        (3.0, 0.0)
    );
    history
        .apply(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::Y,
            value: -2.0,
        })
        .unwrap();
    let edited = history.document().clone();
    let revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::SetTranslationAxis {
                channel_id: ChannelId(1),
                edited_axis: TranslationEditedAxis::X,
                value: f64::NAN,
            })
            .is_err()
    );
    assert_eq!(history.document(), &edited);
    assert_eq!(history.revision(), revision);
    assert!(
        history.apply(&x).is_err(),
        "same axis value is a semantic no-op"
    );
    assert_eq!(history.document(), &edited);
    history.undo().unwrap();
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    history.redo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &edited);
}

#[test]
fn guide_field_edits_are_id_addressed_and_fail_atomically() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let stale_base = history.document().pattern_definitions()[0].clone();
    let result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: stale_base.clone(),
            edit: PatternDefinitionEdit::SetGuideBaselineAngle {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(40),
                baseline_angle_degrees: 15.0,
            },
        })
        .unwrap();
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    let guide = match &history.document().pattern_definitions()[0].mechanisms[0] {
        toniator_domain::PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
            &dimensions[0]
        }
        _ => panic!("fixture must retain a guide mechanism"),
    };
    assert_eq!(guide.baseline_angle_degrees, 15.0);
    history.undo().unwrap();
    assert_eq!(history.document().pattern_definitions()[0], stale_base);
    history.redo().unwrap();

    let before = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: PatternMechanismId(20),
            dimension_id: GuideDimensionId(999),
            phase: 0.5,
        },
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: PatternMechanismId(20),
            dimension_id: GuideDimensionId(40),
            phase: f64::NAN,
        },
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id: PatternMechanismId(20),
            dimension_id: GuideDimensionId(40),
            spacing_multiplier: 0.0,
        },
    ] {
        assert!(
            history
                .apply(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: PatternDefinitionId(10),
                    base_definition: history.document().pattern_definitions()[0].clone(),
                    edit,
                })
                .is_err()
        );
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
    }
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(10),
                base_definition: stale_base,
                edit: PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id: PatternMechanismId(20),
                    dimension_id: GuideDimensionId(40),
                    phase: 0.5,
                },
            })
            .is_err()
    );
    assert_eq!(history.document(), &before);
    assert_eq!(history.revision(), revision);
    assert!(
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(10),
                base_definition: history.document().pattern_definitions()[0].clone(),
                edit: PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id: PatternMechanismId(20),
                    dimension_id: GuideDimensionId(40),
                    phase: 0.0,
                },
            })
            .is_err()
    );
    assert_eq!(history.document(), &before);
    assert_eq!(history.revision(), revision);
    let phase_result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit: PatternDefinitionEdit::SetGuidePhase {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(40),
                phase: 0.5,
            },
        })
        .unwrap();
    assert_eq!(phase_result.invalidation, InvalidationLevel::Family);
    let spacing_result = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit: PatternDefinitionEdit::SetGuideSpacingMultiplier {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(40),
                spacing_multiplier: 0.75,
            },
        })
        .unwrap();
    assert_eq!(spacing_result.invalidation, InvalidationLevel::Family);
    let guide = match &history.document().pattern_definitions()[0].mechanisms[0] {
        toniator_domain::PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
            &dimensions[0]
        }
        _ => panic!("fixture must retain a guide mechanism"),
    };
    assert_eq!(guide.phase, 0.5);
    assert_eq!(guide.repetition.spacing_multiplier, 0.75);
    history.undo().unwrap();
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
}

#[test]
fn guide_product_fields_preserve_order_and_validate_each_leaf_atomically() {
    let mut intersections = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let target = PropertyTarget::Mechanism(PatternDefinitionId(10), PatternMechanismId(21));
    for (field, kind) in [
        (
            PropertyFieldId::IntersectionDimensions,
            PropertyCommandKind::SetIntersectionDimensions,
        ),
        (
            PropertyFieldId::IntersectionMergeEpsilon,
            PropertyCommandKind::SetIntersectionMergeEpsilon,
        ),
    ] {
        assert_eq!(
            intersections
                .document()
                .property_descriptors()
                .into_iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    let apply_intersection = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit,
        })
    };
    assert_eq!(
        apply_intersection(
            &mut intersections,
            PatternDefinitionEdit::SetIntersectionDimensions {
                mechanism_id: PatternMechanismId(21),
                dimensions: vec![GuideDimensionId(40), GuideDimensionId(42)],
            },
        )
        .unwrap()
        .invalidation,
        InvalidationLevel::Family
    );
    apply_intersection(
        &mut intersections,
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id: PatternMechanismId(21),
            merge_epsilon: 0.25,
        },
    )
    .unwrap();
    match &intersections.document().pattern_definitions()[0].mechanisms[1] {
        toniator_domain::PatternMechanism::SelectedGuideIntersections {
            dimensions,
            merge_epsilon,
            ..
        } => {
            assert_eq!(
                dimensions,
                &vec![GuideDimensionId(40), GuideDimensionId(42)]
            );
            assert_eq!(*merge_epsilon, 0.25);
        }
        _ => panic!("fixture must retain an intersection mechanism"),
    }
    let before_failures = intersections.document().clone();
    let revision = intersections.revision();
    for edit in [
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(40)],
        },
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(40)],
        },
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(42), GuideDimensionId(40)],
        },
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(999)],
        },
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id: PatternMechanismId(21),
            merge_epsilon: f64::NAN,
        },
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id: PatternMechanismId(999),
            merge_epsilon: 0.5,
        },
    ] {
        assert!(apply_intersection(&mut intersections, edit).is_err());
        assert_eq!(intersections.document(), &before_failures);
        assert_eq!(intersections.revision(), revision);
    }
    assert!(
        apply_intersection(
            &mut intersections,
            PatternDefinitionEdit::SetIntersectionDimensions {
                mechanism_id: PatternMechanismId(21),
                dimensions: vec![GuideDimensionId(40), GuideDimensionId(42)],
            },
        )
        .is_err()
    );
    assert_eq!(intersections.document(), &before_failures);
    assert_eq!(intersections.revision(), revision);
    intersections.undo().unwrap();
    intersections.undo().unwrap();
    assert_eq!(intersections.document(), &shared_document());

    let mut along = DocumentHistory::new(DocumentSession::new(along_guide_document()).unwrap());
    for (field, kind) in [
        (
            PropertyFieldId::AlongGuideDimensions,
            PropertyCommandKind::SetAlongGuideDimensions,
        ),
        (
            PropertyFieldId::AlongGuideIntervalMultiplier,
            PropertyCommandKind::SetAlongGuideIntervalMultiplier,
        ),
        (
            PropertyFieldId::AlongGuidePhase,
            PropertyCommandKind::SetAlongGuidePhase,
        ),
    ] {
        assert_eq!(
            along
                .document()
                .property_descriptors()
                .into_iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    let apply_along = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit,
        })
    };
    apply_along(
        &mut along,
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(42)],
        },
    )
    .unwrap();
    apply_along(
        &mut along,
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id: PatternMechanismId(21),
            interval_multiplier: 0.5,
        },
    )
    .unwrap();
    apply_along(
        &mut along,
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: PatternMechanismId(21),
            phase: 0.25,
        },
    )
    .unwrap();
    let before_failures = along.document().clone();
    let revision = along.revision();
    for edit in [
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![],
        },
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(40), GuideDimensionId(40)],
        },
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(42), GuideDimensionId(40)],
        },
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: PatternMechanismId(21),
            dimensions: vec![GuideDimensionId(999)],
        },
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id: PatternMechanismId(21),
            interval_multiplier: 0.0,
        },
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: PatternMechanismId(21),
            phase: f64::NAN,
        },
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: PatternMechanismId(999),
            phase: 0.5,
        },
    ] {
        assert!(apply_along(&mut along, edit).is_err());
        assert_eq!(along.document(), &before_failures);
        assert_eq!(along.revision(), revision);
    }
    assert!(
        apply_along(
            &mut along,
            PatternDefinitionEdit::SetAlongGuidePhase {
                mechanism_id: PatternMechanismId(21),
                phase: 0.25,
            },
        )
        .is_err()
    );
    assert_eq!(along.document(), &before_failures);
    assert_eq!(along.revision(), revision);
    along.undo().unwrap();
    along.undo().unwrap();
    along.undo().unwrap();
    assert_eq!(along.document(), &along_guide_document());

    let mut selected = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let base = selected.document().pattern_definitions()[0].clone();
    let result = selected
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: base,
            edit: PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: PatternMechanismId(21),
                merge_epsilon: 0.5,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
}

#[test]
fn density_axis_commands_preserve_authoritative_metric_and_reject_noops() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let result = history
        .apply(&DocumentCommand::SetDensityAxis {
            channel_id: ChannelId(1),
            edited_axis: DensityEditedAxis::AcrossX,
            value: 180.0,
        })
        .unwrap();
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let density = &history.document().channels().unwrap()[0].layout.density;
    assert_eq!(density.across_x, 180.0);
    assert_eq!(density.across_y, 120.0);
    let before = history.document().clone();
    let revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::SetDensityAxis {
                channel_id: ChannelId(1),
                edited_axis: DensityEditedAxis::AcrossX,
                value: 180.0,
            })
            .is_err()
    );
    assert_eq!(history.document(), &before);
    assert_eq!(history.revision(), revision);
}

#[test]
fn selected_output_edit_escalates_only_for_copy_on_edit_and_undo_restores_ids() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let base = history.document().pattern_definitions()[0].clone();
    let command = DocumentCommand::EditSelectedChannelPatternDefinition {
        channel_id: ChannelId(1),
        base_definition: base.clone(),
        edit: PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(40),
            },
        },
    };
    let before = history.document().clone();
    let result = history.apply(&command).unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    assert_ne!(
        history.document().channels().unwrap()[0].pattern_definition_id,
        PatternDefinitionId(10)
    );
    let copied_id = history.document().channels().unwrap()[0].pattern_definition_id;
    let copied = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == copied_id)
        .unwrap();
    let copied_dimension = match &copied.mechanisms[0] {
        toniator_domain::PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
            dimensions[0].id
        }
        _ => panic!("copy must retain the generalized guide root"),
    };
    assert!(matches!(
        copied.output_layers[0],
        toniator_domain::PatternOutputLayer::MarkPrototype {
            orientation: MarkOrientation::GuideTangent { dimension_id },
            ..
        } if dimension_id == copied_dimension
    ));
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
    let shared = DocumentCommand::EditSharedPatternDefinition {
        definition_id: PatternDefinitionId(10),
        base_definition: base,
        edit: PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::GuideNormal {
                dimension_id: GuideDimensionId(40),
            },
        },
    };
    let result = history.apply(&shared).unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    assert_eq!(result.invalidation, InvalidationLevel::Realization);
}

#[test]
fn selected_shared_copy_on_edit_id_exhaustion_is_atomic_for_every_allocated_id_kind() {
    let max = u64::MAX;
    let cases = [
        (
            "pattern_definitions.id",
            shared_copy_exhaustion_document(
                PatternDefinitionId(max),
                PatternMechanismId(20),
                PatternMechanismId(21),
                PatternOutputLayerId(30),
                copy_exhaustion_dimensions(GuideDimensionId(41)),
                MarkOrientation::Fixed,
            ),
            PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: PatternMechanismId(21),
                merge_epsilon: 0.25,
            },
        ),
        (
            "pattern_definitions.mechanisms.id",
            shared_copy_exhaustion_document(
                PatternDefinitionId(10),
                PatternMechanismId(max - 1),
                PatternMechanismId(max),
                PatternOutputLayerId(30),
                copy_exhaustion_dimensions(GuideDimensionId(41)),
                MarkOrientation::Fixed,
            ),
            PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: PatternMechanismId(max),
                merge_epsilon: 0.25,
            },
        ),
        (
            "pattern_definitions.output_layers.id",
            shared_copy_exhaustion_document(
                PatternDefinitionId(10),
                PatternMechanismId(20),
                PatternMechanismId(21),
                PatternOutputLayerId(max),
                copy_exhaustion_dimensions(GuideDimensionId(41)),
                MarkOrientation::Fixed,
            ),
            PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(max),
                orientation: MarkOrientation::GuideTangent {
                    dimension_id: GuideDimensionId(40),
                },
            },
        ),
        (
            "pattern_definitions.mechanisms.dimensions.id",
            shared_copy_exhaustion_document(
                PatternDefinitionId(10),
                PatternMechanismId(20),
                PatternMechanismId(21),
                PatternOutputLayerId(30),
                copy_exhaustion_dimensions(GuideDimensionId(max)),
                MarkOrientation::Fixed,
            ),
            PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(30),
                orientation: MarkOrientation::GuideTangent {
                    dimension_id: GuideDimensionId(max),
                },
            },
        ),
        (
            "pattern_definitions.mechanisms.dimensions.id",
            shared_copy_exhaustion_document(
                PatternDefinitionId(10),
                PatternMechanismId(20),
                PatternMechanismId(21),
                PatternOutputLayerId(30),
                copy_exhaustion_dimensions(GuideDimensionId(max)),
                MarkOrientation::GuideNormal {
                    dimension_id: GuideDimensionId(max),
                },
            ),
            PatternDefinitionEdit::SetOutputOrientationDimension {
                output_layer_id: PatternOutputLayerId(30),
                dimension_id: GuideDimensionId(max - 1),
            },
        ),
    ];

    for (path, document, edit) in cases {
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let before = history.document().clone();
        let revision = history.revision();
        let base = before.pattern_definitions()[0].clone();
        let error = history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: base,
                edit,
            })
            .unwrap_err();
        assert!(error.to_string().contains(path), "{path}: {error}");
        assert_eq!(history.document(), &before, "{path}");
        assert_eq!(history.revision(), revision, "{path}");
        assert!(!history.can_undo(), "{path}");
        assert!(!history.can_redo(), "{path}");
    }
}

#[test]
fn output_layer_leaves_are_variant_aware_and_keep_realization_identity_in_place() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let target = PropertyTarget::OutputLayer(PatternDefinitionId(10), PatternOutputLayerId(30));
    let descriptors = history.document().property_descriptors();
    for (field, kind) in [
        (
            PropertyFieldId::OutputSiteProduct,
            PropertyCommandKind::SetOutputSiteProduct,
        ),
        (
            PropertyFieldId::OutputPrototype,
            PropertyCommandKind::SetOutputMarkPrototype,
        ),
        (
            PropertyFieldId::OutputOrientation,
            PropertyCommandKind::SetOutputOrientation,
        ),
    ] {
        assert_eq!(
            descriptors
                .iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.target == target
            && descriptor.field == PropertyFieldId::OutputOrientationDimension
    }));
    let apply = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(10),
            base_definition: history.document().pattern_definitions()[0].clone(),
            edit,
        })
    };
    let result = apply(
        &mut history,
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(30),
            orientation: MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(40),
            },
        },
    )
    .unwrap();
    assert_eq!(result.invalidation, InvalidationLevel::Realization);
    assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::OutputOrientationDimension
                    && descriptor.target == target
            })
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetOutputOrientationDimension
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id: PatternOutputLayerId(30),
            dimension_id: GuideDimensionId(41),
        },
    )
    .unwrap();
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id: PatternOutputLayerId(30),
            dimension_id: GuideDimensionId(999),
        },
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: PatternOutputLayerId(999),
            orientation: MarkOrientation::Fixed,
        },
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id: PatternOutputLayerId(30),
            site_mechanism_id: PatternMechanismId(999),
        },
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id: PatternOutputLayerId(30),
            site_mechanism_id: PatternMechanismId(20),
        },
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id: PatternOutputLayerId(30),
            dimension_id: GuideDimensionId(41),
        },
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: PatternOutputLayerId(30),
            prototype: MarkPrototype::Circle,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    let after = history.document().clone();
    history.undo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &after);
}

#[test]
fn typed_random_definition_construction_and_each_mechanism_edit_are_history_atomic() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let random = PatternDefinition::random_sites(
        PatternDefinitionId(50),
        "random",
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
            maximum_support_radius: 8.0,
        },
    );
    assert!(
        history
            .apply(&DocumentCommand::AddTypedPatternDefinition {
                definition: random.clone(),
            })
            .unwrap()
            .affected_channels
            .is_empty()
    );
    let apply = |history: &mut DocumentHistory, edit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(50))
            .unwrap()
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(50),
                base_definition: base,
                edit,
            })
            .unwrap()
    };
    assert_eq!(
        apply(
            &mut history,
            PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id: PatternMechanismId(60),
                character: RandomSiteCharacter::Even {
                    minimum_center_distance: 3.0,
                },
            },
        )
        .invalidation,
        InvalidationLevel::Family
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(60),
            seed: u32::MAX,
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 0.5,
                response: ArtworkWeightResponse::Smoothstep,
            },
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 2.0 },
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: PatternMechanismId(63),
            maximum_attempts: 1_001,
        },
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(63),
            maximum_neighbor_checks: 2_001,
        },
    );
    assert!(history.document().validate_property_descriptors().is_ok());
}

#[test]
fn random_process_leaves_follow_the_active_character_and_preserve_history() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let random = PatternDefinition::random_sites(
        PatternDefinitionId(50),
        "random",
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
            maximum_support_radius: 8.0,
        },
    );
    history
        .apply(&DocumentCommand::AddTypedPatternDefinition { definition: random })
        .unwrap();
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(60));
    let descriptors = history.document().property_descriptors();
    for (field, kind) in [
        (
            PropertyFieldId::RandomCharacter,
            PropertyCommandKind::SetRandomCharacter,
        ),
        (
            PropertyFieldId::RandomSeed,
            PropertyCommandKind::SetRandomSeed,
        ),
    ] {
        assert_eq!(
            descriptors
                .iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.target == target
            && matches!(
                descriptor.field,
                PropertyFieldId::RandomEvenMinimumCenterDistance
                    | PropertyFieldId::RandomClusterDensity
                    | PropertyFieldId::RandomClusterSpread
                    | PropertyFieldId::RandomClusterStrength
            )
    }));
    let apply = |history: &mut DocumentHistory, edit| {
        let base = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|definition| definition.id == PatternDefinitionId(50))
            .unwrap()
            .clone();
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: base,
            edit,
        })
    };
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Even {
                minimum_center_distance: 2.0,
            },
        },
    )
    .unwrap();
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .find(|descriptor| {
                descriptor.field == PropertyFieldId::RandomEvenMinimumCenterDistance
                    && descriptor.target == target
            })
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetRandomEvenMinimumCenterDistance
    );
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.target == target
            && matches!(
                descriptor.field,
                PropertyFieldId::RandomClusterDensity
                    | PropertyFieldId::RandomClusterSpread
                    | PropertyFieldId::RandomClusterStrength
            )
    }));
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: PatternMechanismId(60),
            minimum_center_distance: 3.0,
        },
    )
    .unwrap();
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(60),
            seed: 0,
        },
    )
    .unwrap();
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(60),
            seed: u32::MAX,
        },
    )
    .unwrap();
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: PatternMechanismId(60),
            minimum_center_distance: 0.0,
        },
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: PatternMechanismId(60),
            minimum_center_distance: f64::NAN,
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: PatternMechanismId(60),
            cluster_density: 1.0,
        },
        PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(999),
            seed: 0,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: PatternMechanismId(60),
            character: RandomSiteCharacter::Clustered {
                cluster_density: 1.0,
                cluster_spread: 2.0,
                cluster_strength: 0.5,
            },
        },
    )
    .unwrap();
    let descriptors = history.document().property_descriptors();
    for (field, kind) in [
        (
            PropertyFieldId::RandomClusterDensity,
            PropertyCommandKind::SetRandomClusterDensity,
        ),
        (
            PropertyFieldId::RandomClusterSpread,
            PropertyCommandKind::SetRandomClusterSpread,
        ),
        (
            PropertyFieldId::RandomClusterStrength,
            PropertyCommandKind::SetRandomClusterStrength,
        ),
    ] {
        assert_eq!(
            descriptors
                .iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    assert!(!descriptors.iter().any(|descriptor| {
        descriptor.target == target
            && descriptor.field == PropertyFieldId::RandomEvenMinimumCenterDistance
    }));
    for edit in [
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: PatternMechanismId(60),
            cluster_density: 1.5,
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id: PatternMechanismId(60),
            cluster_spread: 2.5,
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id: PatternMechanismId(60),
            cluster_strength: 0.75,
        },
    ] {
        assert_eq!(
            apply(&mut history, edit).unwrap().invalidation,
            InvalidationLevel::Family
        );
    }
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: PatternMechanismId(60),
            minimum_center_distance: 2.0,
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: PatternMechanismId(60),
            cluster_density: 0.0,
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id: PatternMechanismId(60),
            cluster_spread: f64::NAN,
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id: PatternMechanismId(60),
            cluster_strength: 1.1,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    assert!(
        apply(
            &mut history,
            PatternDefinitionEdit::SetRandomClusterStrength {
                mechanism_id: PatternMechanismId(60),
                cluster_strength: 0.75,
            },
        )
        .is_err()
    );
    assert_eq!(history.document(), &before_failures);
    assert_eq!(history.revision(), revision);
    let after = history.document().clone();
    history.undo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &after);

    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(1),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(2),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    let before_copy = history.document().clone();
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(60),
                seed: 0,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let copied_id = history.document().channels().unwrap()[0].pattern_definition_id;
    assert_ne!(copied_id, PatternDefinitionId(50));
    let copied = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == copied_id)
        .unwrap();
    assert!(matches!(
        copied.mechanisms[0],
        toniator_domain::PatternMechanism::RandomSiteProcess { seed: 0, .. }
    ));
    history.undo().unwrap();
    assert_eq!(history.document(), &before_copy);
}

#[test]
fn artwork_weight_leaves_are_active_only_for_weighted_modulation() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let random = PatternDefinition::random_sites(
        PatternDefinitionId(50),
        "random",
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
            maximum_support_radius: 8.0,
        },
    );
    history
        .apply(&DocumentCommand::AddTypedPatternDefinition { definition: random })
        .unwrap();
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(61));
    let apply = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit,
        })
    };
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .filter(|descriptor| descriptor.target == target)
            .map(|descriptor| descriptor.field)
            .collect::<Vec<_>>(),
        vec![PropertyFieldId::RandomDensityModulation]
    );
    assert_eq!(
        descriptors
            .iter()
            .find(|descriptor| descriptor.target == target)
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetDensityModulationVariant
    );
    let before = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetArtworkWeightMappingGain {
            mechanism_id: PatternMechanismId(61),
            gain: 0.5,
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(999),
            modulation: SiteDensityModulation::Uniform,
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(60),
            modulation: SiteDensityModulation::Uniform,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
    }
    apply(
        &mut history,
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: PatternMechanismId(61),
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 0.5,
                response: ArtworkWeightResponse::Linear,
            },
        },
    )
    .unwrap();
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .filter(|descriptor| descriptor.target == target)
            .map(|descriptor| descriptor.field)
            .collect::<Vec<_>>(),
        vec![
            PropertyFieldId::RandomDensityModulation,
            PropertyFieldId::ArtworkWeightMappingComponent,
            PropertyFieldId::ArtworkWeightMappingPlacement,
            PropertyFieldId::ArtworkWeightMappingInverted,
            PropertyFieldId::ArtworkWeightMappingGain,
            PropertyFieldId::ArtworkWeightMappingBias,
            PropertyFieldId::ArtworkWeightStrength,
            PropertyFieldId::ArtworkWeightResponse,
        ]
    );
    for (field, kind) in [
        (
            PropertyFieldId::ArtworkWeightMappingComponent,
            PropertyCommandKind::SetArtworkWeightMappingComponent,
        ),
        (
            PropertyFieldId::ArtworkWeightMappingPlacement,
            PropertyCommandKind::SetArtworkWeightMappingPlacement,
        ),
        (
            PropertyFieldId::ArtworkWeightMappingInverted,
            PropertyCommandKind::SetArtworkWeightMappingInverted,
        ),
        (
            PropertyFieldId::ArtworkWeightMappingGain,
            PropertyCommandKind::SetArtworkWeightMappingGain,
        ),
        (
            PropertyFieldId::ArtworkWeightMappingBias,
            PropertyCommandKind::SetArtworkWeightMappingBias,
        ),
        (
            PropertyFieldId::ArtworkWeightStrength,
            PropertyCommandKind::SetArtworkWeightStrength,
        ),
        (
            PropertyFieldId::ArtworkWeightResponse,
            PropertyCommandKind::SetArtworkWeightResponse,
        ),
    ] {
        assert_eq!(
            descriptors
                .iter()
                .find(|descriptor| descriptor.field == field && descriptor.target == target)
                .unwrap()
                .command_kind(),
            kind
        );
    }
    for edit in [
        PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id: PatternMechanismId(61),
            component: SourceMappingComponent::Alpha,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id: PatternMechanismId(61),
            inverted: true,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingGain {
            mechanism_id: PatternMechanismId(61),
            gain: 0.75,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingBias {
            mechanism_id: PatternMechanismId(61),
            bias: 0.25,
        },
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id: PatternMechanismId(61),
            strength: 0.75,
        },
        PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id: PatternMechanismId(61),
            response: ArtworkWeightResponse::Smoothstep,
        },
    ] {
        assert_eq!(
            apply(&mut history, edit).unwrap().invalidation,
            InvalidationLevel::Family
        );
    }
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetArtworkWeightMappingGain {
            mechanism_id: PatternMechanismId(61),
            gain: -0.1,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingGain {
            mechanism_id: PatternMechanismId(61),
            gain: f64::NAN,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingBias {
            mechanism_id: PatternMechanismId(61),
            bias: f64::NAN,
        },
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id: PatternMechanismId(61),
            strength: 1.1,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    assert!(
        apply(
            &mut history,
            PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
                mechanism_id: PatternMechanismId(61),
                placement: SourcePlacement::StretchToCanvas,
            },
        )
        .is_err()
    );
    assert_eq!(history.document(), &before_failures);
    assert_eq!(history.revision(), revision);
    let after = history.document().clone();
    history.undo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &after);
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(1),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(2),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    let before_copy = history.document().clone();
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit: PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: PatternMechanismId(61),
                gain: 0.5,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let copied_id = history.document().channels().unwrap()[0].pattern_definition_id;
    let copied = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == copied_id)
        .unwrap();
    assert!(matches!(
        copied.mechanisms[1],
        toniator_domain::PatternMechanism::SiteDensityModulation {
            modulation: SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping { gain: 0.5, .. },
                ..
            },
            ..
        }
    ));
    history.undo().unwrap();
    assert_eq!(history.document(), &before_copy);
}

#[test]
fn exclusion_leaves_expose_the_conservative_visible_mark_contract() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    let random = PatternDefinition::random_sites(
        PatternDefinitionId(50),
        "random",
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
            maximum_support_radius: 8.0,
        },
    );
    history
        .apply(&DocumentCommand::AddTypedPatternDefinition { definition: random })
        .unwrap();
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(62));
    let apply = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit,
        })
    };
    assert_eq!(
        history
            .document()
            .property_descriptors()
            .into_iter()
            .filter(|descriptor| descriptor.target == target)
            .map(|descriptor| descriptor.field)
            .collect::<Vec<_>>(),
        vec![PropertyFieldId::RandomExclusion]
    );
    let before = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: PatternMechanismId(62),
            minimum_center_distance: 2.0,
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: PatternMechanismId(62),
            margin: 0.5,
        },
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(999),
            policy: SiteExclusionPolicy::None,
        },
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(60),
            policy: SiteExclusionPolicy::None,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
    }
    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 2.0 },
        },
    )
    .unwrap();
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .filter(|descriptor| descriptor.target == target)
            .map(|descriptor| descriptor.field)
            .collect::<Vec<_>>(),
        vec![
            PropertyFieldId::RandomExclusion,
            PropertyFieldId::ExclusionMinimumCenterDistance,
        ]
    );
    assert_eq!(
        descriptors
            .iter()
            .find(
                |descriptor| descriptor.field == PropertyFieldId::RandomExclusion
                    && descriptor.target == target
            )
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetExclusionVariant
    );
    assert_eq!(
        descriptors
            .iter()
            .find(
                |descriptor| descriptor.field == PropertyFieldId::ExclusionMinimumCenterDistance
                    && descriptor.target == target
            )
            .unwrap()
            .command_kind(),
        PropertyCommandKind::SetExclusionMinimumCenterDistance
    );
    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: PatternMechanismId(62),
            minimum_center_distance: 3.0,
        },
    )
    .unwrap();
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: PatternMechanismId(62),
            minimum_center_distance: 0.0,
        },
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: PatternMechanismId(62),
            minimum_center_distance: f64::NAN,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    apply(
        &mut history,
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: PatternMechanismId(62),
            policy: SiteExclusionPolicy::VisibleMarkMargin {
                margin: 0.5,
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
        },
    )
    .unwrap();
    let descriptors = history.document().property_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .filter(|descriptor| descriptor.target == target)
            .map(|descriptor| descriptor.field)
            .collect::<Vec<_>>(),
        vec![
            PropertyFieldId::RandomExclusion,
            PropertyFieldId::VisibleMarkMargin,
            PropertyFieldId::VisibleMarkSizingPolicy,
        ]
    );
    for (field, kind) in [
        (
            PropertyFieldId::VisibleMarkMargin,
            PropertyCommandKind::SetVisibleMarkMargin,
        ),
        (
            PropertyFieldId::VisibleMarkSizingPolicy,
            PropertyCommandKind::SetVisibleMarkSizingPolicy,
        ),
    ] {
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.field == field && descriptor.target == target)
            .unwrap();
        assert_eq!(descriptor.command_kind(), kind);
        assert_eq!(
            descriptor.structural_support,
            StructuralSupportConstraint::VisibleMarkMarginUsesMaximumSupportRadius
        );
    }
    apply(
        &mut history,
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: PatternMechanismId(62),
            margin: 0.0,
        },
    )
    .unwrap();
    let before_failures = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: PatternMechanismId(62),
            margin: -0.1,
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: PatternMechanismId(62),
            margin: f64::NAN,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before_failures);
        assert_eq!(history.revision(), revision);
    }
    assert!(
        apply(
            &mut history,
            PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
                mechanism_id: PatternMechanismId(62),
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
        )
        .is_err()
    );
    assert_eq!(history.document(), &before_failures);
    assert_eq!(history.revision(), revision);
    let after = history.document().clone();
    history.undo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &after);
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(1),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id: ChannelId(2),
            definition_id: PatternDefinitionId(50),
        })
        .unwrap();
    let before_copy = history.document().clone();
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit: PatternDefinitionEdit::SetVisibleMarkMargin {
                mechanism_id: PatternMechanismId(62),
                margin: 0.25,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(result.invalidation, InvalidationLevel::Family);
    let copied_id = history.document().channels().unwrap()[0].pattern_definition_id;
    let copied = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == copied_id)
        .unwrap();
    assert!(matches!(
        copied.mechanisms[2],
        toniator_domain::PatternMechanism::SiteExclusion {
            policy: SiteExclusionPolicy::VisibleMarkMargin { margin: 0.25, .. },
            ..
        }
    ));
    history.undo().unwrap();
    assert_eq!(history.document(), &before_copy);
}

#[test]
fn random_product_work_leaves_have_nonzero_discrete_contracts() {
    let mut history = DocumentHistory::new(DocumentSession::new(shared_document()).unwrap());
    history
        .apply(&DocumentCommand::AddTypedPatternDefinition {
            definition: PatternDefinition::random_sites(
                PatternDefinitionId(50),
                "random",
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
                    maximum_support_radius: 8.0,
                },
            ),
        })
        .unwrap();
    let target = PropertyTarget::Mechanism(PatternDefinitionId(50), PatternMechanismId(63));
    let descriptors = history.document().property_descriptors();
    for (field, kind) in [
        (
            PropertyFieldId::RandomMaximumAttempts,
            PropertyCommandKind::SetRandomMaximumAttempts,
        ),
        (
            PropertyFieldId::RandomMaximumNeighborChecks,
            PropertyCommandKind::SetRandomMaximumNeighborChecks,
        ),
    ] {
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.field == field && descriptor.target == target)
            .unwrap();
        assert_eq!(descriptor.command_kind(), kind);
        assert_eq!(descriptor.bounds.unwrap().minimum, Some(0.0));
        assert!(!descriptor.bounds.unwrap().minimum_inclusive);
    }
    let apply = |history: &mut DocumentHistory, edit| {
        history.apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(50),
            base_definition: history
                .document()
                .pattern_definitions()
                .iter()
                .find(|definition| definition.id == PatternDefinitionId(50))
                .unwrap()
                .clone(),
            edit,
        })
    };
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: PatternMechanismId(63),
            maximum_attempts: u32::MAX,
        },
    )
    .unwrap();
    apply(
        &mut history,
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(63),
            maximum_neighbor_checks: u32::MAX,
        },
    )
    .unwrap();
    let before = history.document().clone();
    let revision = history.revision();
    for edit in [
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: PatternMechanismId(63),
            maximum_attempts: 0,
        },
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(63),
            maximum_neighbor_checks: 0,
        },
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: PatternMechanismId(999),
            maximum_attempts: 1,
        },
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(60),
            maximum_neighbor_checks: 1,
        },
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: PatternMechanismId(63),
            maximum_attempts: u32::MAX,
        },
    ] {
        assert!(apply(&mut history, edit).is_err());
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
    }
    history.undo().unwrap();
    history.redo().unwrap();
    assert_eq!(history.document(), &before);
}
