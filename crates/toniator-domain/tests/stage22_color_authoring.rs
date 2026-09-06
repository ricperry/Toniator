use toniator_domain::*;

/// Preserves achromatic paint exactly during positive, negative and multi-turn hue animation.
///
/// # Panics
/// Panics if valid paint/timing commands fail or hue changes achromatic RGB or independent alpha.
#[test]
fn achromatic_hue_animation_preserves_rgb_and_interpolates_alpha() {
    for hex in ["#000000", "#808080", "#FFFFFF"] {
        for degrees in [-360.0, 120.0, 720.0] {
            let mut history = timed_history(HalftoneChannelModel::Rgb);
            let start = ColorValue::from_srgb_hex(hex).unwrap();
            let before = history.document().clone();
            let configuration = before
                .edit_start_colors_configuration(&[(ChannelId(1), start.clone())])
                .unwrap();
            history
                .apply_document_configuration(&before, history.revision(), &configuration)
                .unwrap();
            let command = history
                .document()
                .edit_color_end_command(
                    ChannelId(1),
                    Some((
                        ColorEndMode::HueRotation {
                            end_degrees: degrees,
                        },
                        Easing::Linear,
                    )),
                )
                .unwrap();
            history.apply_temporal(&command).unwrap();
            let command = history
                .document()
                .edit_paint_component_end_command(&[ChannelId(1)], ColorComponent::Alpha, 0.0)
                .unwrap();
            history.apply_temporal(&command).unwrap();
            for (frame, alpha) in [(0, 1.0), (1, 0.5), (2, 0.0)] {
                let materialized = history.document().materialize_frame(frame).unwrap();
                let paint = materialized.solid_paint(ChannelId(1)).unwrap();
                assert_eq!(
                    (paint.red, paint.green, paint.blue),
                    (start.red, start.green, start.blue)
                );
                assert_eq!(paint.alpha, alpha);
            }
        }
    }
}

/// Creates a three-frame modeled document without media or frontend dependencies.
///
/// # Panics
/// Panics if the canonical topology or exact test timing cannot be installed.
fn timed_history(model: HalftoneChannelModel) -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let topology = ChannelTopology::canonical(
        model,
        ChannelTopologyTemplate {
            pattern_instance: document
                .channel_pattern_instance(ChannelId(1))
                .unwrap()
                .clone(),
        },
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    if model != HalftoneChannelModel::Rgb {
        history
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .unwrap();
    }
    let timing = ProjectTiming::new(
        FrameRate::new(24, 1).unwrap(),
        FrameRange::new(0, 3).unwrap(),
    );
    let command = history
        .document()
        .replace_temporal_authority_command(timing, vec![]);
    history.apply_temporal(&command).unwrap();
    history
}

/// Checks encoded HEX/linear conversion, transparent hidden RGB and display-only quantization.
///
/// # Panics
/// Panics if parsing, transfer conversion or immutable display expectations fail.
#[test]
fn hex_preserves_straight_alpha_and_canonical_precision() {
    let picker = ColorValue::from_srgb_rgba([0.5, 0.25, 1.0, 0.0]).unwrap();
    let projected = picker.srgb_rgba().unwrap();
    for (actual, expected) in projected.into_iter().zip([0.5, 0.25, 1.0, 0.0]) {
        assert!((actual - expected).abs() < 1e-14);
    }
    assert!(ColorValue::from_srgb_rgba([f64::NAN, 0.0, 0.0, 1.0]).is_err());
    let color = ColorValue::from_srgb_hex(" #808000a3 ").unwrap();
    assert!((color.red - 0.215_860_500_113_899_26).abs() < 1e-14);
    assert_eq!(color.alpha, 163.0 / 255.0);
    assert_eq!(color.to_srgb_hex().unwrap(), "#808000A3");
    assert_eq!(
        ColorValue::from_srgb_hex("#FFA50000")
            .unwrap()
            .to_srgb_hex()
            .unwrap(),
        "#FFA50000"
    );
    assert_eq!(ColorValue::from_srgb_hex("#FFFFFF").unwrap().alpha, 1.0);
    for invalid in ["", "123456", "#fff", "#1234567", "#GG0000", "#１２３"] {
        assert!(ColorValue::from_srgb_hex(invalid).is_err());
    }
    let precise = ColorValue {
        red: 0.1234567890123,
        green: 0.0,
        blue: 0.4,
        alpha: 0.3141592653589,
    };
    let before = precise.clone();
    let _ = precise.to_srgb_hex().unwrap();
    assert_eq!(precise, before, "display does not quantize authority");
}

/// Applies all CMYK Start paints and distinct End colors as atomic presentation-only batches.
///
/// # Panics
/// Panics on invalid canonical fixtures or changed identity, alpha, invalidation or history bounds.
#[test]
fn cmyk_color_batches_preserve_identity_start_and_one_undo_boundary() {
    let mut history = timed_history(HalftoneChannelModel::Cmyk);
    let before = history.document().clone();
    let edits = before
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .zip(["#FF800080", "#40FF80FF", "#2040FFFF", "#FFC0CBFF"])
        .map(|(channel, hex)| (channel.id, ColorValue::from_srgb_hex(hex).unwrap()))
        .collect::<Vec<_>>();
    let configuration = before.edit_start_colors_configuration(&edits).unwrap();
    let result = history
        .apply_document_configuration(&before, history.revision(), &configuration)
        .unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Presentation));
    assert_eq!(result.affected_channels.len(), 4);
    let start = history.document().clone();
    let ends = edits
        .iter()
        .map(|(channel_id, color)| ColorAnimationEdit {
            channel_id: *channel_id,
            mode: Some(ColorEndMode::LinearColor {
                end: ColorValue {
                    alpha: 0.0,
                    ..color.clone()
                },
            }),
            easing: Easing::Linear,
        })
        .collect::<Vec<_>>();
    let command = start.edit_color_animation_command(&ends).unwrap();
    let result = history.apply_temporal(&command).unwrap();
    assert_eq!(result.invalidation, Some(InvalidationLevel::Presentation));
    let end = history.document().materialize_frame(2).unwrap();
    let middle = history.document().materialize_frame(1).unwrap();
    for (channel, color) in &edits {
        assert_eq!(history.document().solid_paint(*channel).unwrap(), color);
        assert_eq!(end.solid_paint(*channel).unwrap().alpha, 0.0);
        assert!((middle.solid_paint(*channel).unwrap().alpha - color.alpha / 2.0).abs() < 1e-14);
        assert_eq!(
            start.modeled_channel(*channel).unwrap().mapping,
            end.modeled_channel(*channel).unwrap().mapping
        );
    }
    history.undo().unwrap();
    assert_eq!(history.document(), &start);
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
    let invalid = vec![
        (ChannelId(1), edits[0].1.clone()),
        (ChannelId(99), edits[1].1.clone()),
    ];
    assert!(before.edit_start_colors_configuration(&invalid).is_err());
    assert_eq!(history.document(), &before);
}

/// Replaces conflicting color bindings explicitly while preserving hue's independent alpha path.
///
/// # Panics
/// Panics on invalid fixtures, conflicting writers, lost alpha or non-atomic rejection.
#[test]
fn mode_switch_keeps_alpha_and_rejects_invalid_batches_atomically() {
    let mut history = timed_history(HalftoneChannelModel::Rgb);
    let command = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::ColorBlue,
            effective_end: 0.5,
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let linear = ColorAnimationEdit {
        channel_id: ChannelId(1),
        mode: Some(ColorEndMode::LinearColor {
            end: ColorValue::from_srgb_hex("#00FF0080").unwrap(),
        }),
        easing: Easing::QuadraticIn,
    };
    let command = history
        .document()
        .edit_color_animation_command(std::slice::from_ref(&linear))
        .unwrap();
    history.apply_temporal(&command).unwrap();
    assert_eq!(history.document().temporal_end_overrides().len(), 1);
    let hue = ColorAnimationEdit {
        channel_id: ChannelId(1),
        mode: Some(ColorEndMode::HueRotation { end_degrees: 720.0 }),
        easing: Easing::Linear,
    };
    let command = history
        .document()
        .edit_color_animation_command(std::slice::from_ref(&hue))
        .unwrap();
    history.apply_temporal(&command).unwrap();
    assert_eq!(history.document().temporal_end_overrides().len(), 2);
    let end = history.document().materialize_frame(2).unwrap();
    assert!((end.solid_paint(ChannelId(1)).unwrap().alpha - 128.0 / 255.0).abs() < 1e-14);
    assert!(history.document().temporal_end_overrides().iter().any(|value| matches!(value,
        TemporalEndOverride::Scalar(value) if value.field == PropertyFieldId::ColorAlpha && value.easing == Easing::QuadraticIn)));
    let before = history.document().clone();
    assert!(
        before
            .edit_color_animation_command(&[hue.clone(), hue])
            .is_err()
    );
    assert_eq!(history.document(), &before);
    let source = timed_history(HalftoneChannelModel::SourceColorAlpha);
    assert!(
        source
            .document()
            .edit_color_animation_command(&[linear])
            .is_err()
    );
}

/// Edits alpha once across grouped colors, hue and components without adding a second alpha writer.
///
/// # Panics
/// Panics if the batch changes Start, loses grouped intent, or admits hue-owned RGB component edits.
#[test]
fn mixed_color_modes_share_one_alpha_batch() {
    let mut history = timed_history(HalftoneChannelModel::Rgb);
    let command = history
        .document()
        .edit_color_animation_command(&[
            ColorAnimationEdit {
                channel_id: ChannelId(1),
                mode: Some(ColorEndMode::LinearColor {
                    end: ColorValue::from_srgb_hex("#12345680").unwrap(),
                }),
                easing: Easing::SmoothStep,
            },
            ColorAnimationEdit {
                channel_id: ChannelId(2),
                mode: Some(ColorEndMode::HueRotation {
                    end_degrees: -360.0,
                }),
                easing: Easing::Linear,
            },
        ])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let channels = [ChannelId(1), ChannelId(2), ChannelId(3)];
    let command = before
        .edit_paint_component_end_command(&channels, ColorComponent::Alpha, 0.4)
        .unwrap();
    assert_eq!(
        history.apply_temporal(&command).unwrap().invalidation,
        Some(InvalidationLevel::Presentation)
    );
    let end = history.document().materialize_frame(2).unwrap();
    for channel in channels {
        assert_eq!(end.solid_paint(channel).unwrap().alpha, 0.4);
        assert_eq!(history.document().solid_paint(channel).unwrap().alpha, 1.0);
    }
    assert_eq!(history.document().temporal_end_overrides().len(), 4);
    assert!(
        history
            .document()
            .edit_paint_component_end_command(&[ChannelId(2)], ColorComponent::Red, 0.2)
            .is_err()
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
}

/// Gives alpha created after a hue transition the same easing without replacing authored alpha easing.
///
/// # Panics
/// Panics if midpoint alpha uses Linear instead of the displayed hue easing or edits change Start.
#[test]
fn alpha_added_after_hue_inherits_its_easing() {
    let mut history = timed_history(HalftoneChannelModel::Rgb);
    let command = history
        .document()
        .edit_color_animation_command(&[ColorAnimationEdit {
            channel_id: ChannelId(1),
            mode: Some(ColorEndMode::HueRotation { end_degrees: 120.0 }),
            easing: Easing::QuadraticIn,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let command = before
        .edit_paint_component_end_command(&[ChannelId(1)], ColorComponent::Alpha, 0.2)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    assert!(
        (history
            .document()
            .materialize_frame(1)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap()
            .alpha
            - 0.8)
            .abs()
            < 1e-12
    );
    assert_eq!(
        history.document().solid_paint(ChannelId(1)).unwrap(),
        before.solid_paint(ChannelId(1)).unwrap()
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &before);
    let command = history
        .document()
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(1)),
            field: PropertyFieldId::ColorAlpha,
            effective_end: 0.2,
            easing: Easing::Hold,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let command = history
        .document()
        .edit_paint_component_end_command(&[ChannelId(1)], ColorComponent::Alpha, 0.4)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    assert_eq!(
        history
            .document()
            .materialize_frame(1)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap()
            .alpha,
        1.0
    );
}
