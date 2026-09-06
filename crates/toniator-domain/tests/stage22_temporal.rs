use std::collections::HashSet;

use toniator_domain::{
    ANIMATABLE_SCALAR_FIELD_IDS, ArtworkWeightResponse, CanvasSpec, ChannelId, ChannelPaint,
    ChannelTopology, ChannelTopologyTemplate, ColorEndMode, ColorValue, CoveragePolicy, Document,
    DocumentCommand, DocumentConfiguration, DocumentHistory, DocumentSession, Easing, FrameRange,
    FrameRate, HalftoneChannelModel, InvalidationLevel, PROPERTY_FIELD_IDS, PatternDefinition,
    PatternDefinitionBundle, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputSettings, ProjectTiming, PropertyFieldId, PropertyTarget, RandomSiteCharacter,
    RationalTime, SiteDensityModulation, SiteExclusionPolicy, SourceMapping,
    SourceMappingComponent, SourceReference, TemporalCapability, TemporalEndOverride,
    TemporalEndpointEdit, TimeRange, TranslationEditedAxis, temporal_capability,
};

/// Builds current mark, guide-path and region documents covering every scalar capability.
///
/// # Panics
/// Panics if a current recipe or homogeneous response fixture violates domain validation.
fn scalar_capability_documents() -> [Document; 3] {
    use toniator_domain::{
        ConnectedGeometryResponse, GeneralizedSiteProduct, GuideDimensionId, MarkOrientation,
        PathStrokeStyle, PatternDefinitionDraft, PatternDefinitionRecipe, PatternOutputLayer,
        PatternOutputRealization, PatternStructureRecipe, StraightGuideDimension,
        StraightGuideRepetition,
    };
    let marks = document();
    let guide_id = PatternMechanismId(31);
    let output_id = PatternOutputLayerId(33);
    let mut definition = PatternDefinition::generalized_straight_guides(
        marks.pattern_settings().definition_id,
        "Temporal guide-path witness",
        guide_id,
        PatternMechanismId(32),
        output_id,
        vec![StraightGuideDimension {
            id: GuideDimensionId(34),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(34)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::all(
        output_id,
        PatternOutputRealization::GuidePaths {
            guide_mechanism_id: guide_id,
            style: PathStrokeStyle::default(),
        },
    )];
    let connected = with_bundles(
        &marks,
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: output_id,
                response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.25,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                }),
            }],
        }],
    );
    let (regions, _) = marks
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: marks.pattern_settings().clone(),
            base_definition: marks.pattern_definition_bundles()[0].definition.clone(),
            recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "Temporal region witness".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 2,
                        additional_margin: 0.0,
                    },
                },
            )),
        })
        .expect("region recipe validates");
    [marks, connected, regions]
}

/// Reads an active scalar through the same effective descriptor surface used by frontends.
///
/// # Panics
/// Panics if the fixture's promised descriptor is missing or is not a finite scalar.
fn displayed_scalar(document: &Document, target: PropertyTarget, field: PropertyFieldId) -> f64 {
    let current = document
        .property_values()
        .into_iter()
        .find(|value| value.descriptor.target == target && value.descriptor.field == field)
        .expect("active scalar descriptor");
    let toniator_domain::PropertyCurrentValueKind::FiniteF64(value) = current.value else {
        panic!("active temporal descriptor is a finite scalar");
    };
    value
}

/// Verifies every scalar field through real capabilities at both endpoints and three interior frames.
///
/// All six easing curves use independently specified expected weights; shared recipes and
/// authored Start remain unchanged. Connected and region responses use current active outputs.
///
/// # Panics
/// Panics on a missing capability, wrong frame value, changed Start/definition, or incomplete coverage.
#[test]
fn every_scalar_field_materializes_all_easings_through_active_descriptors() {
    use PropertyFieldId::*;
    let documents = scalar_capability_documents();
    for descriptor in documents[1]
        .property_descriptors()
        .into_iter()
        .filter(|descriptor| descriptor.field == CurveResponseBias)
    {
        assert_eq!(
            descriptor.reset_capable,
            matches!(descriptor.target, PropertyTarget::ChannelOutput(_, _))
        );
    }
    let curves = [
        (Easing::Hold, [0.0, 0.0, 0.0, 0.0, 1.0]),
        (Easing::Linear, [0.0, 0.25, 0.5, 0.75, 1.0]),
        (Easing::QuadraticIn, [0.0, 0.0625, 0.25, 0.5625, 1.0]),
        (Easing::QuadraticOut, [0.0, 0.4375, 0.75, 0.9375, 1.0]),
        (Easing::SmoothStep, [0.0, 0.15625, 0.5, 0.84375, 1.0]),
        (Easing::SmoothInOut, [0.0, 0.125, 0.5, 0.875, 1.0]),
    ];
    let mut covered = HashSet::new();
    for &field in ANIMATABLE_SCALAR_FIELD_IDS {
        let fixture = match field {
            ConnectedMinimumThickness | ConnectedMaximumThickness | CurveResponseBias => {
                &documents[1]
            }
            RegionMinimumFill | RegionMaximumFill => &documents[2],
            _ => &documents[0],
        };
        let descriptor = fixture
            .property_descriptors()
            .into_iter()
            .find(|descriptor| {
                descriptor.field == field
                    && match field {
                        Density | DensityAspect | RotationDegrees | ShapeRotationDegrees => {
                            descriptor.target == PropertyTarget::Document
                        }
                        MarkMinimumFill
                        | MarkMaximumFill
                        | ConnectedMinimumThickness
                        | ConnectedMaximumThickness
                        | CurveResponseBias
                        | RegionMinimumFill
                        | RegionMaximumFill => matches!(
                            descriptor.target,
                            PropertyTarget::ChannelOutput(ChannelId(1), _)
                        ),
                        _ => descriptor.target == PropertyTarget::Channel(ChannelId(1)),
                    }
            })
            .expect("field has a real active descriptor");
        assert!(matches!(
            descriptor.temporal,
            TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
        ));
        if field == CurveResponseBias {
            assert!(descriptor.reset_capable);
        }
        let start = displayed_scalar(fixture, descriptor.target, field);
        let end = match field {
            Density => start + 7.0,
            DensityAspect => start + 0.6,
            RotationDegrees | ShapeRotationDegrees => start + 120.0,
            TranslationX => 5.0,
            TranslationY => -4.0,
            MarkMinimumFill | RegionMinimumFill => 0.2,
            MarkMaximumFill | RegionMaximumFill => 0.8,
            ConnectedMinimumThickness => 0.5,
            ConnectedMaximumThickness => 0.9,
            CurveResponseBias => -0.5,
            ModeledMappingGain => 0.3,
            ModeledMappingBias => -0.2,
            ColorRed | ColorGreen | ColorBlue | ColorAlpha => 0.35,
            Opacity => 0.25,
            _ => panic!("new scalar needs an explicit witness"),
        };
        assert_ne!(start, end);
        for (easing, weights) in curves {
            let mut history = timed_document_history(fixture.clone(), 5);
            let command = history
                .document()
                .edit_effective_end_command(&[TemporalEndpointEdit {
                    target: descriptor.target,
                    field,
                    effective_end: end,
                    easing,
                }])
                .expect("End edit validates against current capability");
            history.apply_temporal(&command).unwrap();
            for (frame, weight) in weights.into_iter().enumerate() {
                let materialized = history.document().materialize_frame(frame as u64).unwrap();
                let actual = displayed_scalar(&materialized, descriptor.target, field);
                let expected = start * (1.0 - weight) + end * weight;
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{field:?} {easing:?} frame {frame}: {actual} != {expected}"
                );
                assert_eq!(
                    materialized.pattern_definition_bundles(),
                    fixture.pattern_definition_bundles()
                );
            }
            assert_eq!(
                displayed_scalar(history.document(), descriptor.target, field),
                start
            );
            assert_eq!(
                history.document().pattern_definition_bundles(),
                fixture.pattern_definition_bundles()
            );
        }
        covered.insert(field);
    }
    assert_eq!(
        covered,
        ANIMATABLE_SCALAR_FIELD_IDS.iter().copied().collect()
    );
}

/// Builds one validated current RGB document for temporal tests.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates")
}

/// Installs exact output timing through one temporal history transition.
fn timed_history(frame_count: u64) -> DocumentHistory {
    timed_document_history(document(), frame_count)
}

/// Installs exact output timing on one supplied validated document.
fn timed_document_history(document: Document, frame_count: u64) -> DocumentHistory {
    let timing = ProjectTiming::new(
        FrameRate::new(30_000, 1_001).expect("NTSC-derived rate validates"),
        FrameRange::new(0, frame_count).expect("nonempty frame range validates"),
    );
    let command = document.replace_temporal_authority_command(timing, Vec::new());
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    history
        .apply_temporal(&command)
        .expect("timing transition applies");
    history
}

/// Rebuilds one modeled document with replacement pattern bundles.
///
/// # Panics
/// Panics if the replacement bundles violate current document authority.
fn with_bundles(base: &Document, bundles: Vec<PatternDefinitionBundle>) -> Document {
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        bundles,
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document"),
        base.channel_topology().expect("modeled document").clone(),
        base.authored_structures().to_vec(),
    )
    .expect("replacement bundles validate")
}

/// Builds two mark outputs whose structural order is the reverse of their IDs.
fn reverse_id_multi_output_document() -> Document {
    let base = document();
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    let mut second_output = bundle.definition.output_layers[0].clone();
    second_output.id = PatternOutputLayerId(2);
    bundle.definition.output_layers.insert(0, second_output);
    bundle.output_settings.insert(
        0,
        PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(2),
            response: bundle.output_settings[0].response.clone(),
        },
    );
    with_bundles(&base, vec![bundle])
}

/// Builds one random-site document where artwork weighting owns placement coordinates.
fn artwork_weighted_document() -> Document {
    let base = document();
    let definition = PatternDefinition::random_sites(
        base.pattern_settings().definition_id,
        "Artwork-weighted marks",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternMechanismId(5),
        PatternOutputLayerId(1),
        RandomSiteCharacter::RawUniform,
        19,
        SiteDensityModulation::ArtworkWeighted {
            mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
            strength: 0.8,
            response: ArtworkWeightResponse::Smoothstep,
        },
        SiteExclusionPolicy::None,
        20_000,
        20_000,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: base.pattern_definition_bundles()[0].output_settings.clone(),
    };
    with_bundles(&base, vec![bundle])
}

/// Returns one modeled channel's solid paint from the public topology projection.
fn solid_color(document: &Document, channel_id: ChannelId) -> &ColorValue {
    let channel = document
        .channel_topology()
        .expect("modeled topology")
        .channels()
        .iter()
        .find(|channel| channel.id == channel_id)
        .expect("channel exists");
    let ChannelPaint::Solid(color) = &channel.paint else {
        panic!("test channel has solid paint")
    };
    color
}

/// Proves rational timing remains reduced and exact, including N=1 and overflow rejection.
///
/// # Panics
/// Panics when exact timing, overflow rejection, or easing boundaries differ from the contract.
#[test]
fn rational_timing_and_easing_have_exact_boundaries() {
    assert_eq!(RationalTime::default(), RationalTime::new(0, 1).unwrap());
    let large = RationalTime::new(u64::MAX, u64::MAX - 1).unwrap();
    assert_eq!(
        large.checked_add(large).unwrap(),
        RationalTime::new(u64::MAX, (u64::MAX - 1) / 2).unwrap()
    );
    let rate = FrameRate::new(60_000, 2_002).expect("rate reduces");
    assert_eq!((rate.numerator(), rate.denominator()), (30_000, 1_001));
    let timing = ProjectTiming::new(rate, FrameRange::new(10, 13).expect("range"));
    assert_eq!(
        timing.time_for_frame(12).expect("exact time"),
        RationalTime::new(1_001, 15_000).expect("reduced expected time")
    );
    assert_eq!(
        timing.source_time_for_frame(12).unwrap(),
        RationalTime::new(1_001, 2_500).unwrap()
    );
    let trimmed = ProjectTiming::new(
        FrameRate::new(3, 1).unwrap(),
        FrameRange::new(10, 14).unwrap(),
    )
    .with_source_time_range(
        TimeRange::new(
            RationalTime::new(1, 2).unwrap(),
            RationalTime::new(3, 2).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        trimmed.source_time_for_frame(10).unwrap(),
        RationalTime::new(1, 2).unwrap()
    );
    assert_eq!(
        trimmed.source_time_for_frame(12).unwrap(),
        RationalTime::new(7, 6).unwrap()
    );
    assert!(trimmed.source_time_for_frame(13).is_err());
    assert!(trimmed.source_time_for_frame(9).is_err());
    assert!(
        RationalTime::new(u64::MAX, 1)
            .unwrap()
            .checked_add(RationalTime::new(1, 1).unwrap())
            .is_err()
    );
    let source_range = TimeRange::new(
        RationalTime::new(1, 3).expect("start"),
        RationalTime::new(5, 6).expect("end"),
    )
    .expect("time range");
    assert_eq!(
        source_range.duration().expect("duration"),
        RationalTime::new(1, 2).expect("half second")
    );
    assert_eq!(
        timing
            .frames_for_duration(source_range.duration().unwrap())
            .unwrap(),
        15
    );
    assert_eq!(FrameRange::new(4, 5).unwrap().progress(4).unwrap(), 0.0);
    assert_eq!(Easing::Hold.weight(0.999), 0.0);
    for easing in [
        Easing::Hold,
        Easing::Linear,
        Easing::QuadraticIn,
        Easing::QuadraticOut,
        Easing::SmoothStep,
        Easing::SmoothInOut,
    ] {
        assert_eq!(easing.weight(0.0), 0.0);
        assert_eq!(easing.weight(1.0), 1.0);
    }
    assert_eq!(Easing::Hold.weight(0.5), 0.0);
    assert_eq!(Easing::Linear.weight(0.5), 0.5);
    assert_eq!(Easing::QuadraticIn.weight(0.5), 0.25);
    assert_eq!(Easing::QuadraticOut.weight(0.5), 0.75);
    assert_eq!(Easing::SmoothStep.weight(0.5), 0.5);
    assert_eq!(Easing::SmoothInOut.weight(0.5), 0.5);
    assert_eq!(
        FrameRate::new(1, u32::MAX)
            .unwrap()
            .time_for_frame(u64::MAX)
            .unwrap_err()
            .path(),
        "temporal.frame_time"
    );
}

/// Proves the exact 20-field inventory and target-sensitive descriptor authority.
#[test]
fn descriptors_expose_exact_temporal_inventory_and_positive_aspect() {
    let authority_document = document();
    for descriptor in authority_document.property_descriptors() {
        if matches!(descriptor.target, PropertyTarget::OutputLayer(_, _)) {
            assert_eq!(
                descriptor.authority,
                toniator_domain::PropertyAuthority::StructuralDefinition
            );
        }
        if matches!(descriptor.target, PropertyTarget::ChannelOutput(_, _))
            && ANIMATABLE_SCALAR_FIELD_IDS.contains(&descriptor.field)
        {
            assert_eq!(
                descriptor.authority,
                toniator_domain::PropertyAuthority::ChannelDelta
            );
        }
    }
    assert_eq!(ANIMATABLE_SCALAR_FIELD_IDS.len(), 20);
    assert_eq!(
        ANIMATABLE_SCALAR_FIELD_IDS
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len(),
        20
    );
    for field in PROPERTY_FIELD_IDS {
        let target = match field {
            PropertyFieldId::Density
            | PropertyFieldId::DensityAspect
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees => PropertyTarget::Document,
            PropertyFieldId::TranslationX
            | PropertyFieldId::TranslationY
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity => PropertyTarget::Channel(ChannelId(1)),
            PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness
            | PropertyFieldId::CurveResponseBias
            | PropertyFieldId::RegionMinimumFill
            | PropertyFieldId::RegionMaximumFill => {
                PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(1))
            }
            _ => PropertyTarget::Document,
        };
        let capability = temporal_capability(*field, target);
        assert_eq!(
            matches!(
                capability,
                TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
            ),
            ANIMATABLE_SCALAR_FIELD_IDS.contains(field),
            "every field must classify explicitly as animatable or static: {field:?}"
        );
    }
    let document = document();
    let descriptors = document.property_descriptors();
    let aspect = descriptors
        .iter()
        .find(|value| {
            value.field == PropertyFieldId::DensityAspect
                && value.target == PropertyTarget::Document
        })
        .expect("document aspect descriptor");
    let bounds = aspect.bounds.expect("aspect has positive bounds");
    assert_eq!(bounds.minimum, Some(0.0));
    assert!(!bounds.minimum_inclusive);

    let channel_response = descriptors
        .iter()
        .find(|value| {
            value.field == PropertyFieldId::MarkMinimumFill
                && matches!(value.target, PropertyTarget::ChannelOutput(ChannelId(1), _))
        })
        .expect("channel response descriptor");
    assert_eq!(channel_response.temporal, TemporalCapability::Scalar);
    let definition_response = descriptors
        .iter()
        .find(|value| {
            value.field == PropertyFieldId::MarkMinimumFill
                && matches!(value.target, PropertyTarget::OutputLayer(_, _))
        })
        .expect("definition response descriptor");
    assert!(matches!(
        definition_response.temporal,
        TemporalCapability::Static(_)
    ));
    document
        .validate_property_descriptors()
        .expect("temporal metadata remains descriptor-complete");
}

/// Proves End editing stores deltas against the End base and preserves inheritance per frame.
#[test]
fn effective_end_edits_preserve_base_and_named_delta_semantics() {
    let mut history = timed_history(3);
    let start_density = history.document().pattern_settings().density.density;
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::Document,
                field: PropertyFieldId::Density,
                effective_end: start_density + 20.0,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::Channel(ChannelId(1)),
                field: PropertyFieldId::Density,
                effective_end: start_density + 30.0,
                easing: Easing::Linear,
            },
        ])
        .expect("effective End edits normalize");
    history
        .apply_temporal(&command)
        .expect("atomic temporal edit applies");

    let overrides = history.document().temporal_end_overrides();
    assert!(overrides.iter().any(|entry| matches!(
        entry,
        TemporalEndOverride::Scalar(value)
            if value.target == PropertyTarget::Channel(ChannelId(1))
                && value.field == PropertyFieldId::Density
                && value.end == 10.0
    )));
    let middle = history
        .document()
        .materialize_frame(1)
        .expect("middle frame");
    assert_eq!(
        middle
            .effective_channel_pattern(ChannelId(1))
            .unwrap()
            .density
            .density,
        start_density + 15.0
    );
    assert_eq!(
        middle
            .effective_channel_pattern(ChannelId(2))
            .unwrap()
            .density
            .density,
        start_density + 10.0
    );
    assert_eq!(
        history.document().pattern_settings().density.density,
        start_density
    );
}

/// Proves Start and zero-weight Hold frames preserve absent authored channel deltas.
#[test]
fn zero_weight_frames_preserve_authored_start_and_live_inheritance() {
    let mut history = timed_history(4);
    let start_density = history.document().pattern_settings().density.density;
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::Document,
                field: PropertyFieldId::Density,
                effective_end: start_density + 30.0,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::Channel(ChannelId(1)),
                field: PropertyFieldId::Density,
                effective_end: start_density + 40.0,
                easing: Easing::Hold,
            },
        ])
        .expect("base and inherited End edits normalize");
    history.apply_temporal(&command).expect("End edits apply");

    let start = history
        .document()
        .materialize_frame(0)
        .expect("Start frame");
    assert!(
        start
            .channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .layout_delta
            .density
            .is_none()
    );
    assert_eq!(
        start
            .effective_channel_pattern(ChannelId(1))
            .expect("effective Start")
            .density
            .density,
        start_density
    );

    let held = history.document().materialize_frame(1).expect("Hold frame");
    assert!(
        held.channel_pattern_instance(ChannelId(1))
            .expect("channel")
            .layout_delta
            .density
            .is_none()
    );
    assert_eq!(
        held.effective_channel_pattern(ChannelId(1))
            .expect("effective inherited Hold")
            .density
            .density,
        start_density + 10.0
    );
}

/// Proves an ordinary Start edit makes a previously captured temporal command stale.
#[test]
fn static_start_edit_stales_preexisting_temporal_command() {
    let mut history = timed_history(3);
    let temporal = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::Opacity,
            effective_end: 0.25,
            easing: Easing::Linear,
        }])
        .expect("temporal command builds");
    history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        })
        .expect("ordinary Start edit applies");
    let after_static = history.document().clone();
    let revision = history.revision();

    let failure = history
        .apply_temporal(&temporal)
        .expect_err("old temporal command is stale");
    assert!(failure.to_string().contains("temporal.command.base"));
    assert_eq!(history.document(), &after_static);
    assert_eq!(history.revision(), revision);
}

/// Proves equivalent End override sets have one canonical persisted ordering.
#[test]
fn equivalent_end_override_sets_canonicalize_identically() {
    let document = document();
    let opacity = TemporalEndOverride::Scalar(toniator_domain::ScalarEndOverride {
        target: PropertyTarget::Channel(ChannelId(1)),
        field: PropertyFieldId::Opacity,
        end: 0.5,
        easing: Easing::Linear,
    });
    let translation = TemporalEndOverride::Scalar(toniator_domain::ScalarEndOverride {
        target: PropertyTarget::Channel(ChannelId(1)),
        field: PropertyFieldId::TranslationX,
        end: 12.0,
        easing: Easing::SmoothStep,
    });
    let forward = document.replace_temporal_authority_command(
        document.project_timing().clone(),
        vec![opacity.clone(), translation.clone()],
    );
    let reverse = document.replace_temporal_authority_command(
        document.project_timing().clone(),
        vec![translation, opacity],
    );
    assert_eq!(forward.replacement(), reverse.replacement());
}

/// Proves paired response End edits remain channel-output deltas and validate requested frames.
#[test]
fn response_pairs_are_atomic_and_never_mutate_shared_output_defaults() {
    let mut history = timed_history(5);
    let output_id =
        history.document().pattern_definition_bundles()[0].output_settings()[0].output_layer_id;
    let original_bundle = history.document().pattern_definition_bundles()[0].clone();
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), output_id),
                field: PropertyFieldId::MarkMinimumFill,
                effective_end: 1.5,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), output_id),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 1.6,
                easing: Easing::Hold,
            },
        ])
        .expect("ordered End pair validates atomically");
    history.apply_temporal(&command).expect("pair applies");
    assert_eq!(
        history.document().pattern_definition_bundles()[0],
        original_bundle
    );
    let failure = history.document().materialize_frame(3).unwrap_err();
    assert_eq!(failure.path(), "channel.pattern.mark_geometry_response");
    let end = history
        .document()
        .materialize_frame(4)
        .expect("valid endpoint");
    let response = &end
        .effective_channel_pattern(ChannelId(1))
        .unwrap()
        .output_settings[0]
        .response;
    let PatternGeometryResponse::Marks(response) = response else {
        panic!("mark response")
    };
    assert_eq!((response.minimum_fill, response.maximum_fill), (1.5, 1.6));
}

/// Proves reverse-authored multi-output edits materialize deltas in structural order.
#[test]
fn reversed_multi_output_edits_preserve_structural_delta_order() {
    let mut history = timed_document_history(reverse_id_multi_output_document(), 3);
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(1)),
                field: PropertyFieldId::MarkMinimumFill,
                effective_end: 0.2,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(2)),
                field: PropertyFieldId::MarkMinimumFill,
                effective_end: 0.3,
                easing: Easing::Linear,
            },
        ])
        .expect("reverse structural edits normalize");
    history.apply_temporal(&command).expect("End edits apply");
    let middle = history
        .document()
        .materialize_frame(1)
        .expect("middle frame");
    let delta_ids = middle
        .channel_pattern_instance(ChannelId(1))
        .expect("channel")
        .output_response_deltas
        .iter()
        .map(|delta| delta.output_layer_id)
        .collect::<Vec<_>>();
    assert_eq!(
        delta_ids,
        vec![PatternOutputLayerId(2), PatternOutputLayerId(1)]
    );
}

/// Proves artwork-weighted placement rejects channel rotation End authority.
#[test]
fn artwork_weighted_channel_rotation_is_temporally_inapplicable() {
    let document = artwork_weighted_document();
    let failure = document
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::RotationDegrees,
            effective_end: 45.0,
            easing: Easing::Linear,
        }])
        .expect_err("artwork-weighted channel rotation stays inactive");
    assert_eq!(failure.path(), "temporal.descriptor");
}

/// Proves interpolation remains finite across opposite-sign extreme endpoints.
#[test]
fn interpolation_avoids_overflow_for_finite_extreme_endpoints() {
    let start = document()
        .apply_command(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::X,
            value: f64::MAX,
        })
        .expect("finite extreme Start applies")
        .0;
    let mut history = timed_document_history(start, 3);
    let command = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::TranslationX,
            effective_end: -f64::MAX,
            easing: Easing::Linear,
        }])
        .expect("finite extreme End applies");
    history.apply_temporal(&command).expect("End edit applies");
    let middle = history
        .document()
        .materialize_frame(1)
        .expect("middle frame");
    assert_eq!(
        middle
            .effective_channel_pattern(ChannelId(1))
            .expect("effective middle")
            .translation_x,
        0.0
    );
}

/// Proves grouped linear colors, hue paths, independent alpha, and conflict rejection.
#[test]
fn grouped_color_and_hue_modes_preserve_canonical_authority() {
    let mut history = timed_history(3);
    let start = solid_color(history.document(), ChannelId(1)).clone();
    let end = ColorValue {
        red: 0.25,
        green: 0.5,
        blue: 0.75,
        alpha: 0.4,
    };
    let command = history
        .document()
        .edit_color_end_command(
            ChannelId(1),
            Some((
                ColorEndMode::LinearColor { end: end.clone() },
                Easing::Linear,
            )),
        )
        .expect("linear color command");
    history
        .apply_temporal(&command)
        .expect("linear color applies");
    assert_eq!(
        solid_color(
            &history.document().materialize_frame(0).unwrap(),
            ChannelId(1)
        ),
        &start
    );
    assert_eq!(
        solid_color(
            &history.document().materialize_frame(2).unwrap(),
            ChannelId(1)
        ),
        &end
    );

    let clear = history
        .document()
        .edit_color_end_command(ChannelId(1), None)
        .expect("clear color command");
    history.apply_temporal(&clear).expect("clear applies");
    let alpha = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::ColorAlpha,
            effective_end: 0.2,
            easing: Easing::Linear,
        }])
        .expect("alpha End command");
    history.apply_temporal(&alpha).expect("alpha applies");
    let hue = history
        .document()
        .edit_color_end_command(
            ChannelId(1),
            Some((
                ColorEndMode::HueRotation { end_degrees: 360.0 },
                Easing::Linear,
            )),
        )
        .expect("hue and alpha coexist");
    history.apply_temporal(&hue).expect("hue applies");
    let hue_end = history.document().materialize_frame(2).unwrap();
    let hue_end = solid_color(&hue_end, ChannelId(1));
    assert_eq!(
        (hue_end.red, hue_end.green, hue_end.blue),
        (start.red, start.green, start.blue)
    );
    assert_eq!(hue_end.alpha, 0.2);

    let conflict = history.document().replace_temporal_authority_command(
        history.document().project_timing().clone(),
        vec![
            TemporalEndOverride::Color(toniator_domain::ColorEndOverride {
                channel_id: ChannelId(1),
                mode: ColorEndMode::LinearColor { end },
                easing: Easing::Linear,
            }),
            TemporalEndOverride::Scalar(toniator_domain::ScalarEndOverride {
                target: PropertyTarget::Channel(ChannelId(1)),
                field: PropertyFieldId::ColorRed,
                end: 0.1,
                easing: Easing::Linear,
            }),
        ],
    );
    assert_eq!(
        history.apply_temporal(&conflict).unwrap_err().to_string(),
        "temporal.color.conflict: grouped color and component RGB End overrides are mutually exclusive"
    );
}

/// Proves every CMYK semantic channel can carry an independent solid-paint End color.
#[test]
fn cmyk_channels_keep_separate_color_endpoints() {
    let document = document();
    let template = ChannelTopologyTemplate {
        pattern_instance: document
            .channel_pattern_instance(ChannelId(1))
            .expect("RGB channel instance")
            .clone(),
    };
    let topology = ChannelTopology::canonical(HalftoneChannelModel::Cmyk, template)
        .expect("CMYK topology validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology,
        })
        .expect("CMYK topology applies");
    let timing = ProjectTiming::new(
        FrameRate::new(24, 1).unwrap(),
        FrameRange::new(0, 2).unwrap(),
    );
    let endpoints = [
        ColorValue {
            red: 0.1,
            green: 0.2,
            blue: 0.3,
            alpha: 0.4,
        },
        ColorValue {
            red: 0.2,
            green: 0.3,
            blue: 0.4,
            alpha: 0.5,
        },
        ColorValue {
            red: 0.3,
            green: 0.4,
            blue: 0.5,
            alpha: 0.6,
        },
        ColorValue {
            red: 0.4,
            green: 0.5,
            blue: 0.6,
            alpha: 0.7,
        },
    ];
    let overrides = (4_u64..=7)
        .zip(endpoints.iter().cloned())
        .map(|(channel, end)| {
            TemporalEndOverride::Color(toniator_domain::ColorEndOverride {
                channel_id: ChannelId(channel),
                mode: ColorEndMode::LinearColor { end },
                easing: Easing::Linear,
            })
        })
        .collect();
    let command = history
        .document()
        .replace_temporal_authority_command(timing, Vec::new());
    let timing_result = history
        .apply_temporal(&command)
        .expect("CMYK timing initializes");
    assert_eq!(timing_result.invalidation, Some(InvalidationLevel::Source));
    let command = history
        .document()
        .replace_temporal_authority_command(history.document().project_timing().clone(), overrides);
    let result = history.apply_temporal(&command).expect("CMYK colors apply");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Presentation));
    let end_document = history.document().materialize_frame(1).unwrap();
    for (channel, expected) in (4_u64..=7).zip(&endpoints) {
        assert_eq!(solid_color(&end_document, ChannelId(channel)), expected);
    }
}

/// Proves project timing edits invalidate source sampling for every channel.
#[test]
fn timing_only_edit_requires_source_invalidation() {
    let document = document();
    let timing = ProjectTiming::new(
        FrameRate::new(24, 1).expect("frame rate"),
        FrameRange::new(0, 12).expect("frame range"),
    );
    let command = document.replace_temporal_authority_command(timing, Vec::new());
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    let result = history.apply_temporal(&command).expect("timing applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Source));
    assert_eq!(
        result.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
}

/// Proves a configuration carrying only reusable End data reports temporal invalidation.
#[test]
fn temporal_only_configuration_apply_reports_invalidation() {
    let destination = document();
    let command = destination
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::Opacity,
            effective_end: 0.5,
            easing: Easing::Linear,
        }])
        .expect("End opacity command");
    let source = destination
        .clone()
        .with_temporal_authority(
            command.replacement().project_timing.clone(),
            command.replacement().end_overrides.clone(),
        )
        .expect("temporal source validates");
    let configuration = DocumentConfiguration::capture(&source);
    let base = destination.clone();
    let mut history =
        DocumentHistory::new(DocumentSession::new(destination).expect("session validates"));
    let revision = history.revision();
    let result = history
        .apply_document_configuration(&base, revision, &configuration)
        .expect("temporal configuration applies");
    assert!(!result.unchanged);
    assert_eq!(result.invalidation, Some(InvalidationLevel::Presentation));
    assert_eq!(result.affected_channels, vec![ChannelId(1)]);
    assert_eq!(
        history.document().temporal_end_overrides(),
        source.temporal_end_overrides()
    );
}

/// Proves history, configuration capture, and frame snapshots retain their authority boundaries.
#[test]
fn history_configuration_and_frame_snapshot_preserve_boundaries() {
    let mut history = timed_history(3);
    let command = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::TranslationX,
            effective_end: 12.0,
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let transitioned = history.document().clone();
    let token = history.session().document_evaluation_token();
    let snapshot = history
        .session()
        .document_evaluation_snapshot_at_frame(1)
        .expect("frame snapshot");
    assert_eq!(snapshot.token(), token);
    assert!(snapshot.document().temporal_end_overrides().is_empty());
    assert_eq!(
        snapshot
            .document()
            .effective_channel_pattern(ChannelId(1))
            .unwrap()
            .translation_x,
        6.0
    );
    history.undo().unwrap().expect("temporal undo");
    assert!(history.document().temporal_end_overrides().is_empty());
    history.redo().unwrap().expect("temporal redo");
    assert_eq!(history.document(), &transitioned);

    let configuration = DocumentConfiguration::capture(history.document());
    let destination = document()
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(60, 1).unwrap(),
                FrameRange::new(5, 7).unwrap(),
            ),
            Vec::new(),
        )
        .unwrap();
    let rebound = configuration.bind(&destination).expect("preset bind");
    assert_eq!(rebound.project_timing(), destination.project_timing());
    assert_eq!(
        rebound.temporal_end_overrides(),
        transitioned.temporal_end_overrides()
    );
}

/// Proves first End activation freezes equal settings and repeated selection is idempotent.
///
/// # Panics
/// Panics if Start edits leak into initialized End or history fails to restore exact snapshots.
#[test]
fn initialized_end_preserves_equal_values_across_start_edits() {
    let mut history = timed_history(3);
    let initial = history.document().clone();
    let command = initial.initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let initialized = history.document().clone();
    assert_eq!(
        initialized.initialize_end_command().unwrap().replacement(),
        &initialized.temporal_authority()
    );
    let start = initialized.pattern_settings().clone();
    let mut settings = start.clone();
    settings.pattern_rotation_degrees = 27.0;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings {
            base: start,
            settings,
        })
        .unwrap();
    history
        .apply(&DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: toniator_domain::ColorComponent::Alpha,
            value: 0.37,
        })
        .unwrap();
    let end = history.document().materialize_frame(2).unwrap();
    for channel in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        assert_eq!(
            end.effective_channel_pattern(channel)
                .unwrap()
                .pattern_rotation_degrees,
            initial
                .effective_channel_pattern(channel)
                .unwrap()
                .pattern_rotation_degrees
        );
        assert_eq!(
            end.solid_paint(channel).unwrap(),
            initial.solid_paint(channel).unwrap()
        );
    }
    let command = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::ColorAlpha,
            effective_end: 0.37,
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    history
        .apply(&DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: toniator_domain::ColorComponent::Alpha,
            value: 0.81,
        })
        .unwrap();
    assert_eq!(
        history
            .document()
            .materialize_frame(2)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap()
            .alpha,
        0.37
    );
    for _ in 0..4 {
        history.undo().unwrap().unwrap();
    }
    assert_eq!(history.document(), &initialized);
    history.undo().unwrap().unwrap();
    assert_eq!(history.document(), &initial);
    history.redo().unwrap().unwrap();
    assert_eq!(history.document(), &initialized);
}

/// Proves copy-on-edit and shared response edits preserve the initialized effective End response.
///
/// # Panics
/// Panics if new output IDs lose End intent, rebasing drifts, or unaffected channels change.
#[test]
fn start_response_edit_remaps_and_rebases_initialized_end() {
    for shared in [false, true] {
        let mut history = timed_history(3);
        let command = history.document().initialize_end_command().unwrap();
        history.apply_temporal(&command).unwrap();
        let before = history.document().materialize_frame(2).unwrap();
        let base_bundle = history.document().pattern_definition_bundles()[0].clone();
        let PatternGeometryResponse::Marks(mut response) =
            base_bundle.output_settings[0].response.clone()
        else {
            panic!("mark fixture");
        };
        response.maximum_fill = 0.7;
        let edit = toniator_domain::PatternDefinitionBundleEdit::OutputSettings(
            toniator_domain::PatternOutputSettingsEdit::SetMarkResponse {
                output_layer_id: base_bundle.output_settings[0].output_layer_id,
                response,
            },
        );
        let command = if shared {
            DocumentCommand::EditSharedPatternDefinitionBundle {
                definition_id: base_bundle.definition.id,
                base_bundle,
                edit,
            }
        } else {
            DocumentCommand::EditSelectedChannelPatternDefinitionBundle {
                channel_id: ChannelId(1),
                base_bundle,
                edit,
            }
        };
        history.apply(&command).unwrap();
        let end = history.document().materialize_frame(2).unwrap();
        for channel in [ChannelId(1), ChannelId(2), ChannelId(3)] {
            let old = before.effective_channel_pattern(channel).unwrap();
            let new = end.effective_channel_pattern(channel).unwrap();
            assert_eq!(
                new.output_settings[0].response,
                old.output_settings[0].response
            );
        }
    }
}

/// Proves a named pattern replacement resets only its dependent End settings.
///
/// # Panics
/// Panics if copied output IDs lose their distinct End values when painter order changes.
#[test]
fn initialized_end_follows_copied_outputs_when_reordered() {
    let mut history = timed_document_history(reverse_id_multi_output_document(), 3);
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(2)),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 0.4,
                easing: Easing::Linear,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(1), PatternOutputLayerId(1)),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 0.7,
                easing: Easing::Linear,
            },
        ])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().materialize_frame(2).unwrap();
    let base_bundle = history.document().pattern_definition_bundles()[0].clone();
    history
        .apply(
            &DocumentCommand::EditSelectedChannelPatternDefinitionBundle {
                channel_id: ChannelId(1),
                base_bundle,
                edit: toniator_domain::PatternDefinitionBundleEdit::MoveOutputLayer {
                    output_layer_id: PatternOutputLayerId(2),
                    painter_index: 1,
                },
            },
        )
        .unwrap();
    let after = history.document().materialize_frame(2).unwrap();
    let old = before.effective_channel_pattern(ChannelId(1)).unwrap();
    let new = after.effective_channel_pattern(ChannelId(1)).unwrap();
    assert_eq!(
        new.output_settings[0].response,
        old.output_settings[1].response
    );
    assert_eq!(
        new.output_settings[1].response,
        old.output_settings[0].response
    );
    assert_eq!(
        after.effective_channel_pattern(ChannelId(2)).unwrap(),
        before.effective_channel_pattern(ChannelId(2)).unwrap()
    );
}

/// Proves a named pattern replacement resets only its dependent End settings.
///
/// # Panics
/// Panics if another channel, independent paint/mapping, or reversible history is changed.
#[test]
fn pattern_replacement_reinitializes_only_affected_end_dependencies() {
    let mut history = timed_history(3);
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let edits = [ChannelId(1), ChannelId(2), ChannelId(3)].map(|channel| TemporalEndpointEdit {
        target: PropertyTarget::Channel(channel),
        field: PropertyFieldId::RotationDegrees,
        effective_end: channel.0 as f64 * 11.0,
        easing: Easing::SmoothStep,
    });
    let command = history
        .document()
        .edit_effective_end_command(&edits)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let base = before.pattern_settings().clone();
    let definition = before
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .unwrap()
        .definition
        .clone();
    let recipe = before
        .reconstruct_pattern_definition_recipe(base.definition_id)
        .unwrap();
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                channel_id: ChannelId(1),
                base_definition: definition,
                recipe,
            },
        )
        .unwrap();
    let independent = |document: &Document| {
        document.temporal_end_overrides().iter().filter(|entry| {
        !matches!(entry, TemporalEndOverride::Scalar(value) if
            matches!(value.target, PropertyTarget::Channel(ChannelId(1)) | PropertyTarget::ChannelOutput(ChannelId(1), _))
            && !matches!(value.field, PropertyFieldId::ColorRed | PropertyFieldId::ColorGreen
                | PropertyFieldId::ColorBlue | PropertyFieldId::ColorAlpha | PropertyFieldId::Opacity
                | PropertyFieldId::TranslationX | PropertyFieldId::TranslationY
                | PropertyFieldId::ModeledMappingGain | PropertyFieldId::ModeledMappingBias))
    }).cloned().collect::<Vec<_>>()
    };
    assert_eq!(independent(&before), independent(history.document()));
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let end = history.document().materialize_frame(2).unwrap();
    assert_eq!(
        end.effective_channel_pattern(ChannelId(1))
            .unwrap()
            .pattern_rotation_degrees,
        history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .unwrap()
            .pattern_rotation_degrees
    );
    for channel in [ChannelId(2), ChannelId(3)] {
        assert_eq!(
            end.effective_channel_pattern(channel).unwrap(),
            before
                .materialize_frame(2)
                .unwrap()
                .effective_channel_pattern(channel)
                .unwrap()
        );
    }
    history.undo().unwrap().unwrap();
    history.undo().unwrap().unwrap();
    assert_eq!(history.document(), &before);
}

/// Reinitializes document-level End dependencies when the All wizard replaces its shared base recipe.
///
/// # Panics
/// Panics if inherited pattern fields survive replacement, independent values change, or Undo loses state.
#[test]
fn shared_base_recipe_reinitializes_document_end_dependencies() {
    let mut history = timed_history(3);
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let edits = [
        (PropertyFieldId::Density, 45.0),
        (PropertyFieldId::DensityAspect, 1.7),
        (PropertyFieldId::RotationDegrees, 31.0),
        (PropertyFieldId::TranslationX, 7.0),
    ]
    .map(|(field, effective_end)| TemporalEndpointEdit {
        target: if field == PropertyFieldId::TranslationX {
            PropertyTarget::Channel(ChannelId(1))
        } else {
            PropertyTarget::Document
        },
        field,
        effective_end,
        easing: Easing::QuadraticIn,
    });
    let command = history
        .document()
        .edit_effective_end_command(&edits)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let definition_id = before.pattern_settings().definition_id;
    let base_definition = before
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .unwrap()
        .definition
        .clone();
    let recipe = toniator_domain::PatternDefinitionRecipe::marks(
        toniator_domain::PatternStructureRecipe::StraightGrid(
            toniator_domain::PatternDefinitionDraft {
                name: "replacement for all channels".into(),
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    additional_margin: 0.0,
                },
            },
        ),
    );
    history
        .apply(&DocumentCommand::ReplaceSharedPatternDefinitionRecipe {
            definition_id,
            base_definition,
            recipe,
        })
        .unwrap();
    for field in [
        PropertyFieldId::Density,
        PropertyFieldId::DensityAspect,
        PropertyFieldId::RotationDegrees,
    ] {
        assert!(!history.document().temporal_end_overrides().iter().any(|entry| matches!(entry,
            TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::Document && value.field == field)));
    }
    let translation = |document: &Document| {
        document.temporal_end_overrides().iter().find(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::Channel(ChannelId(1)) && value.field == PropertyFieldId::TranslationX)).cloned()
    };
    assert_eq!(translation(history.document()), translation(&before));
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
}
