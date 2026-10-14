//! Independent calibration ownership and immutable continuous-frame authority regressions.
use toniator_domain::*;

/// Builds a validated current document with independent near-equal numeric assignments.
///
/// # Panics
/// Panics if defaults or independently valid equal-binding artifacts fail domain validation.
fn independent_document() -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 128.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let original = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap();
    let binding = original.binding().clone();
    let make = |coverage| {
        SavedCalibration::new(
            binding.clone(),
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 1.0,
                    coverage,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
        )
        .unwrap()
    };
    document
        .with_saved_calibration_authority(
            vec![
                AttachedCalibration {
                    channel_id: ChannelId(1),
                    artifact: make(0.4),
                },
                AttachedCalibration {
                    channel_id: ChannelId(2),
                    artifact: make(0.4 + 1e-13),
                },
                AttachedCalibration {
                    channel_id: ChannelId(3),
                    artifact: make(0.6),
                },
            ],
            Some(make(0.5)),
        )
        .unwrap()
}

/// Checks exact numeric assignments and measured provenance without an epsilon.
///
/// # Panics
/// Panics when any stored numeric knot or measured binding differs.
fn assert_exact(a: &SavedCalibration, b: &SavedCalibration) {
    assert_eq!(a.binding(), b.binding());
    assert_eq!(a.profile(), b.profile());
    assert_eq!(a.uniform(), b.uniform());
    assert_eq!(a.variation(), b.variation());
    assert_eq!(a.raw().len(), b.raw().len());
    for (a, b) in a.raw().iter().zip(b.raw()) {
        assert_eq!(a.input.to_bits(), b.input.to_bits());
        assert_eq!(a.coverage.to_bits(), b.coverage.to_bits());
    }
}

/// Keeps near-equal same-binding assignments independent through named publication and history.
///
/// # Panics
/// Panics on fanout, baseline replacement, stale acceptance, or inexact Undo/Redo restoration.
#[test]
fn named_assignment_and_exact_history_never_fan_out() {
    let document = independent_document();
    let channels = document.saved_calibrations().to_vec();
    let baseline = document
        .baseline_saved_calibration()
        .unwrap()
        .unwrap()
        .clone();
    let replacement = channels[2].artifact.clone();
    let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
    let revision = history.revision();
    history
        .attach_saved_calibration(&document, revision, ChannelId(1), replacement.clone())
        .unwrap();
    assert_exact(
        history
            .document()
            .saved_calibration_for(ChannelId(1))
            .unwrap()
            .unwrap(),
        &replacement,
    );
    for old in &channels[1..] {
        assert_exact(
            history
                .document()
                .saved_calibration_for(old.channel_id)
                .unwrap()
                .unwrap(),
            &old.artifact,
        );
    }
    assert_exact(
        history
            .document()
            .baseline_saved_calibration()
            .unwrap()
            .unwrap(),
        &baseline,
    );
    assert!(
        history
            .attach_saved_calibration(&document, revision, ChannelId(2), replacement)
            .is_err()
    );
    let applied = history.document().clone();
    history.undo().unwrap();
    assert_eq!(history.document(), &document);
    history.redo().unwrap();
    assert_eq!(history.document(), &applied);
    let restored = document
        .with_saved_calibration_authority(channels.clone(), Some(baseline))
        .unwrap();
    assert_eq!(restored.saved_calibrations(), channels);
    let duplicate = vec![channels[0].clone(), channels[0].clone()];
    assert!(document.with_saved_calibrations(duplicate).is_err());
}

/// Keeps each own artifact when static artistic angles cross and preserves measured provenance.
///
/// # Panics
/// Panics if rotation drops/swaps an assignment or static aspect gains frame privileges.
#[test]
fn static_rotation_reuses_own_assignment_while_aspect_stays_structural() {
    let before = independent_document();
    let old = before.saved_calibration_for(ChannelId(2)).unwrap().unwrap();
    let measured45 = SavedCalibration::with_profile(
        old.profile().into(),
        CalibrationBinding::new(
            old.binding().recipe().clone(),
            old.binding().density_aspect(),
            45.0,
        )
        .unwrap(),
        old.raw().to_vec(),
        old.uniform().cloned(),
    )
    .unwrap();
    let before = before
        .with_saved_calibration(ChannelId(2), measured45)
        .unwrap();
    let config = before
        .edit_channel_start_configuration(&[
            (
                PropertyTarget::Channel(ChannelId(1)),
                PropertyFieldId::ShapeRotationDegrees,
                45.0,
            ),
            (
                PropertyTarget::Channel(ChannelId(2)),
                PropertyFieldId::ShapeRotationDegrees,
                -45.0,
            ),
        ])
        .unwrap();
    let changed = config.bind(&before).unwrap();
    let config = changed
        .edit_channel_start_configuration(&[
            (
                PropertyTarget::Channel(ChannelId(1)),
                PropertyFieldId::ShapeRotationDegrees,
                -45.0,
            ),
            (
                PropertyTarget::Channel(ChannelId(2)),
                PropertyFieldId::ShapeRotationDegrees,
                45.0,
            ),
        ])
        .unwrap();
    let crossed = config.bind(&changed).unwrap();
    for channel in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        assert_exact(
            crossed.saved_calibration_for(channel).unwrap().unwrap(),
            before.saved_calibration_for(channel).unwrap().unwrap(),
        );
        assert_eq!(
            crossed
                .saved_calibration_for(channel)
                .unwrap()
                .unwrap()
                .binding()
                .shape_rotation_degrees()
                .to_bits(),
            before
                .saved_calibration_for(channel)
                .unwrap()
                .unwrap()
                .binding()
                .shape_rotation_degrees()
                .to_bits()
        );
    }
    let config = crossed
        .edit_channel_start_configuration(&[(
            PropertyTarget::Channel(ChannelId(1)),
            PropertyFieldId::DensityAspect,
            1.4,
        )])
        .unwrap();
    let structural = config.bind(&crossed).unwrap();
    assert!(
        structural
            .saved_calibration_for(ChannelId(1))
            .unwrap()
            .is_none()
    );
    assert!(structural.require_saved_calibration_authority().is_err());
    let stale = crossed.saved_calibrations().to_vec();
    assert!(structural.with_saved_calibrations(stale).is_err());
}

/// Retains Start's independent companions throughout continuous aspect/rotation without clone privilege.
///
/// # Panics
/// Panics if frames fit/fill/swap artifacts, static clones validate stale aspect, or snapshot authority fails.
#[test]
fn continuous_frame_authority_is_immutable_and_does_not_escape_through_document() {
    let start = independent_document();
    let overrides = vec![
        TemporalEndOverride::Scalar(ScalarEndOverride {
            target: PropertyTarget::Document,
            field: PropertyFieldId::DensityAspect,
            end: 1.7,
            easing: Easing::Linear,
        }),
        TemporalEndOverride::Scalar(ScalarEndOverride {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::ShapeRotationDegrees,
            end: 360.0,
            easing: Easing::SmoothStep,
        }),
    ];
    let animated = start
        .clone()
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(24, 1).unwrap(),
                FrameRange::new(0, 9).unwrap(),
            ),
            overrides,
        )
        .unwrap();
    let session = DocumentSession::new(animated.clone()).unwrap();
    for number in [0, 1, 4, 7, 8] {
        let frame = animated.materialize_frame(number).unwrap();
        frame.validate().unwrap();
        assert_eq!(
            DocumentConfiguration::capture(frame.document()).baseline_saved_calibration(),
            start.baseline_saved_calibration().unwrap()
        );
        assert_eq!(
            frame.document().saved_calibrations(),
            start.saved_calibrations()
        );
        for entry in start.saved_calibrations() {
            assert_exact(
                frame
                    .saved_calibration_for(entry.channel_id)
                    .unwrap()
                    .unwrap(),
                &entry.artifact,
            );
        }
        let snapshot = session
            .document_evaluation_snapshot_at_frame(number)
            .unwrap();
        snapshot.validate().unwrap();
        assert_eq!(snapshot.token(), session.document_evaluation_token());
        if number > 0 {
            assert!(frame.document().clone().validate().is_err());
            assert!(DocumentSession::new(snapshot.document().clone()).is_err());
            assert!(
                DocumentConfiguration::capture(frame.document())
                    .bind(&start)
                    .is_err()
            );
        }
    }
    let missing = start
        .with_saved_calibration_authority(Vec::new(), None)
        .unwrap();
    let frame = missing.materialize_frame(0).unwrap();
    assert!(frame.saved_calibration_for(ChannelId(1)).unwrap().is_none());
}

/// Evaluates warning trajectories using authored base/delta fields and their real easing.
///
/// # Panics
/// Panics on invalid finite trajectory construction or an incorrect warning classification.
fn warning_case(
    base_start: f64,
    base_end: f64,
    named_start: f64,
    named_end: f64,
    a: Easing,
    b: Easing,
) -> bool {
    let mut document = independent_document();
    let base = document.pattern_settings().clone();
    let mut settings = base.clone();
    settings.shape_rotation_degrees = base_start;
    if settings != base {
        document = document
            .apply_command(&DocumentCommand::SetDocumentPatternSettings { base, settings })
            .unwrap()
            .0;
    }
    // Set all named deltas so other active channels do not introduce uncancelled base motion.
    let edits = [1, 2, 3].map(|id| {
        (
            PropertyTarget::Channel(ChannelId(id)),
            PropertyFieldId::ShapeRotationDegrees,
            base_start + named_start,
        )
    });
    document = document
        .edit_channel_start_configuration(&edits)
        .unwrap()
        .bind(&document)
        .unwrap();
    let mut overrides = vec![TemporalEndOverride::Scalar(ScalarEndOverride {
        target: PropertyTarget::Document,
        field: PropertyFieldId::ShapeRotationDegrees,
        end: base_end,
        easing: a,
    })];
    for id in [1, 2, 3] {
        overrides.push(TemporalEndOverride::Scalar(ScalarEndOverride {
            target: PropertyTarget::Channel(ChannelId(id)),
            field: PropertyFieldId::ShapeRotationDegrees,
            end: named_end,
            easing: b,
        }));
    }
    document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(24, 1).unwrap(),
                FrameRange::new(0, 3).unwrap(),
            ),
            overrides,
        )
        .unwrap()
        .has_animated_shape_rotation()
        .unwrap()
}

/// Detects full turns, Hold, tiny residuals, and unequal easing while suppressing exact cancellation.
///
/// # Panics
/// Panics if warning detection uses wrapping, sampled frames, rounded totals, or epsilon cancellation.
#[test]
fn warning_uses_effective_exact_trajectories() {
    assert!(warning_case(
        0.0,
        360.0,
        0.0,
        0.0,
        Easing::Linear,
        Easing::Linear
    ));
    assert!(warning_case(
        0.0,
        360.0,
        0.0,
        0.0,
        Easing::Hold,
        Easing::Linear
    ));
    assert!(!warning_case(
        0.0,
        45.0,
        0.0,
        -45.0,
        Easing::SmoothStep,
        Easing::SmoothStep
    ));
    assert!(warning_case(
        0.0,
        45.0,
        0.0,
        -45.0,
        Easing::Linear,
        Easing::QuadraticIn
    ));
    assert!(!warning_case(
        0.0,
        0.0,
        0.0,
        0.0,
        Easing::Hold,
        Easing::Linear
    ));
    assert!(warning_case(
        1e16,
        1e16 + 2.0,
        0.0,
        -1.0,
        Easing::Linear,
        Easing::Linear
    ));
    assert!(!warning_case(
        0.0,
        f64::MAX / 2.0,
        0.0,
        -f64::MAX / 2.0,
        Easing::Linear,
        Easing::Linear
    ));
    assert!(warning_case(
        0.0,
        1.0,
        0.0,
        -1.0 + f64::EPSILON,
        Easing::Linear,
        Easing::Linear
    ));
}

/// Reconciles model proposals and publication with last-ALL inheritance and exact own assignments.
///
/// # Panics
/// Panics if foreign old IDs invalidate a proposal, own IDs swap payloads, or new IDs lose ALL intent.
#[test]
fn model_rebinding_validates_proposals_and_retains_last_all_intent() {
    let original = independent_document();
    let document = original
        .apply_command(
            &original
                .set_channel_shape_rotation_for_effective(ChannelId(1), 45.0)
                .unwrap(),
        )
        .unwrap()
        .0;
    let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
    let topology = document
        .canonical_channel_topology_with_appearance_baseline(
            HalftoneChannelModel::Rgb,
            ChannelTopologyTemplate::document_base(),
        )
        .unwrap();
    history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology,
        })
        .unwrap();
    for old in original.saved_calibrations() {
        assert_exact(
            history
                .document()
                .saved_calibration_for(old.channel_id)
                .unwrap()
                .unwrap(),
            &old.artifact,
        );
    }
    let baseline = original.baseline_saved_calibration().unwrap().unwrap();
    for model in [
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::SourceColorAlpha,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
        HalftoneChannelModel::Rgb,
    ] {
        let topology = history
            .document()
            .canonical_channel_topology_with_appearance_baseline(
                model,
                ChannelTopologyTemplate::document_base(),
            )
            .unwrap();
        history
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .unwrap();
        history
            .document()
            .require_saved_calibration_authority()
            .unwrap();
        assert_exact(
            history
                .document()
                .baseline_saved_calibration()
                .unwrap()
                .unwrap(),
            baseline,
        );
        for channel in history.document().channel_topology().unwrap().channels() {
            assert_exact(
                history
                    .document()
                    .saved_calibration_for(channel.id)
                    .unwrap()
                    .unwrap(),
                baseline,
            );
        }
    }
}
