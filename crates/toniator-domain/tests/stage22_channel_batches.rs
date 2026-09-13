use toniator_domain::*;

/// Keeps an explicitly assigned equal End alpha independent of Start while hue owns only RGB.
///
/// # Panics
/// Panics if hue ownership swallows Alpha initialization or changes the hue transition.
#[test]
fn equal_all_end_alpha_initializes_beside_hue_rotation() {
    let mut history = history(HalftoneChannelModel::Rgb);
    let command = history
        .document()
        .edit_color_animation_command(&[ColorAnimationEdit {
            channel_id: ChannelId(1),
            mode: Some(ColorEndMode::HueRotation { end_degrees: 180.0 }),
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let command = history
        .document()
        .edit_all_channel_end_command(&[(PropertyFieldId::ColorAlpha, 1.0)])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    start_batch(&mut history, &[(PropertyFieldId::ColorAlpha, 0.3)]);
    assert_eq!(
        history
            .document()
            .materialize_frame(2)
            .unwrap()
            .solid_paint(ChannelId(1))
            .unwrap()
            .alpha,
        1.0
    );
    assert!(history.document().temporal_end_overrides().iter().any(|entry| matches!(entry, TemporalEndOverride::Color(value) if value.channel_id == ChannelId(1) && value.mode == ColorEndMode::HueRotation { end_degrees: 180.0 } && value.easing == Easing::SmoothStep)));
}

/// Initializes an equal-valued channel when an explicit ALL End assignment follows a partial reset.
///
/// # Panics
/// Panics if a later Start edit leaks into the explicitly assigned End value.
#[test]
fn all_end_assignment_initializes_equal_valued_missing_targets() {
    let mut history = history(HalftoneChannelModel::Rgb);
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let overrides = history.document().temporal_end_overrides().iter().filter(|entry| {
        !matches!(entry, TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::Channel(ChannelId(1)) && value.field == PropertyFieldId::RotationDegrees)
    }).cloned().collect();
    let command = history
        .document()
        .replace_temporal_authority_command(history.document().project_timing().clone(), overrides);
    history.apply_temporal(&command).unwrap();
    let command = history
        .document()
        .edit_all_channel_end_command(&[(PropertyFieldId::RotationDegrees, 0.0)])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    start_batch(&mut history, &[(PropertyFieldId::RotationDegrees, 20.0)]);
    let end = history.document().materialize_frame(2).unwrap();
    assert!(
        end.channel_scalar_batch(PropertyFieldId::RotationDegrees)
            .unwrap()
            .values
            .iter()
            .all(|value| value.value == 0.0)
    );
}

/// Creates a three-frame RGB or CMYK history with canonical channels and no media dependency.
///
/// # Panics
/// Panics if the canonical topology or exact timing fixture fails validation.
fn history(model: HalftoneChannelModel) -> DocumentHistory {
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

/// Publishes one prevalidated Start batch through the canonical stale-aware history boundary.
///
/// # Panics
/// Panics if the new configuration cannot publish atomically against its exact source document.
fn start_batch(history: &mut DocumentHistory, edits: &[(PropertyFieldId, f64)]) {
    let base = history.document().clone();
    let configuration = base.edit_all_channel_start_configuration(edits).unwrap();
    history
        .apply_document_configuration(&base, history.revision(), &configuration)
        .unwrap();
}

/// Compares finite expected effective values without mistaking harmless arithmetic rounding for drift.
///
/// # Panics
/// Panics if the scalar values differ beyond the fixed 1e-12 test tolerance.
fn near(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-12, "{left} != {right}");
}

/// Verifies absolute coupled Start batches preserve End values and one exact undo boundary.
///
/// # Panics
/// Panics if a partial range publishes, a compatible channel is lost or history loses exact state.
#[test]
fn coupled_start_batches_equalize_values_and_keep_initialized_end() {
    let mut history = history(HalftoneChannelModel::Rgb);
    for channel in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        let command = history
            .document()
            .set_channel_output_response_for_effective(
                channel,
                PatternOutputLayerId(1),
                PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: channel.0 as f64 / 10.0,
                    maximum_fill: 0.5 + channel.0 as f64 / 10.0,
                }),
            )
            .unwrap();
        history.apply(&command).unwrap();
    }
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let batch = before
        .channel_scalar_batch(PropertyFieldId::MarkMinimumFill)
        .unwrap();
    assert_eq!(batch.values.len(), 3);
    near(batch.minimum, 0.1);
    near(batch.maximum, 0.3);
    near(batch.average, 0.2);
    assert!(
        before
            .edit_all_channel_start_configuration(&[(PropertyFieldId::MarkMinimumFill, 1.2)])
            .is_err()
    );
    start_batch(
        &mut history,
        &[
            (PropertyFieldId::MarkMinimumFill, 1.2),
            (PropertyFieldId::MarkMaximumFill, 1.7),
        ],
    );
    for value in history
        .document()
        .channel_scalar_batch(PropertyFieldId::MarkMinimumFill)
        .unwrap()
        .values
        .iter()
    {
        near(value.value, 1.2);
    }
    assert_eq!(
        history.document().pattern_definition_bundles(),
        before.pattern_definition_bundles()
    );
    assert_eq!(
        history.document().temporal_end_overrides(),
        before.temporal_end_overrides()
    );
    assert_eq!(
        history.document().materialize_frame(2).unwrap(),
        before.materialize_frame(2).unwrap()
    );
    let after = history.document().clone();
    history.undo().unwrap().unwrap();
    assert_eq!(history.document(), &before);
    history.redo().unwrap().unwrap();
    assert_eq!(history.document(), &after);
}

/// Verifies All resolves compatible outputs independently after a named channel changes its pattern.
///
/// # Panics
/// Panics if an incompatible region receives a mark edit or a definition/output identity is collapsed.
#[test]
fn end_batches_skip_incompatible_outputs_and_preserve_interpolation() {
    let mut history = history(HalftoneChannelModel::Rgb);
    let before = history.document().clone();
    let base = before.pattern_settings().clone();
    let recipe = before
        .reconstruct_pattern_definition_recipe(base.definition_id)
        .unwrap();
    let base_definition = before.pattern_definition_bundles()[0].definition.clone();
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                channel_id: ChannelId(1),
                base_definition,
                recipe: recipe
                    .with_output_kind(0, PatternRecipeOutputKind::VoronoiRegions)
                    .unwrap(),
            },
        )
        .unwrap();
    let command = history
        .document()
        .edit_effective_end_command(&[
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(2), PatternOutputLayerId(1)),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 0.5,
                easing: Easing::QuadraticIn,
            },
            TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(ChannelId(3), PatternOutputLayerId(1)),
                field: PropertyFieldId::MarkMaximumFill,
                effective_end: 0.7,
                easing: Easing::SmoothStep,
            },
        ])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let projection = before
        .materialize_frame(2)
        .unwrap()
        .channel_scalar_batch(PropertyFieldId::MarkMaximumFill)
        .unwrap();
    assert_eq!(projection.values.len(), 2);
    let command = before
        .edit_all_channel_end_command(&[(
            PropertyFieldId::MarkMaximumFill,
            projection.average + 0.2,
        )])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let after = history.document().materialize_frame(2).unwrap();
    let values = after
        .channel_scalar_batch(PropertyFieldId::MarkMaximumFill)
        .unwrap();
    near(values.values[0].value, 0.8);
    near(values.values[1].value, 0.8);
    assert_eq!(
        after.effective_channel_pattern(ChannelId(1)).unwrap(),
        before
            .materialize_frame(2)
            .unwrap()
            .effective_channel_pattern(ChannelId(1))
            .unwrap()
    );
    for entry in history.document().temporal_end_overrides() {
        let TemporalEndOverride::Scalar(value) = entry else {
            continue;
        };
        assert_eq!(
            value.easing,
            if value.target == PropertyTarget::ChannelOutput(ChannelId(2), PatternOutputLayerId(1))
            {
                Easing::QuadraticIn
            } else {
                Easing::SmoothStep
            }
        );
    }
    assert_eq!(
        history.document().materialize_frame(0).unwrap(),
        before.materialize_frame(0).unwrap()
    );
    history.undo().unwrap().unwrap();
    assert_eq!(history.document(), &before);
}

/// Verifies All alpha editing preserves different CMYK colors, hue ownership and per-channel easing.
///
/// # Panics
/// Panics on color collapse, competing alpha authority, changed Start or a non-atomic bounds failure.
#[test]
fn cmyk_alpha_batch_keeps_colors_hue_and_easing() {
    let mut history = history(HalftoneChannelModel::Cmyk);
    let channels = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    let colors = ["#FF800080", "#40FF8080", "#2040FF80", "#FFC0CB80"];
    let edits = colors
        .iter()
        .enumerate()
        .map(|(index, hex)| ColorAnimationEdit {
            channel_id: channels[index],
            mode: Some(if index == 1 {
                ColorEndMode::HueRotation { end_degrees: 180.0 }
            } else {
                ColorEndMode::LinearColor {
                    end: ColorValue {
                        alpha: 0.2 + index as f64 / 10.0,
                        ..ColorValue::from_srgb_hex(hex).unwrap()
                    },
                }
            }),
            easing: if index % 2 == 0 {
                Easing::QuadraticIn
            } else {
                Easing::SmoothStep
            },
        })
        .collect::<Vec<_>>();
    let command = history
        .document()
        .edit_color_animation_command(&edits)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let command = history
        .document()
        .edit_paint_component_end_command(&[channels[1]], ColorComponent::Alpha, 0.3)
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let before = history.document().clone();
    let old_end = before.materialize_frame(2).unwrap();
    let old_alpha = old_end
        .channel_scalar_batch(PropertyFieldId::ColorAlpha)
        .unwrap();
    let command = before
        .edit_all_channel_end_command(&[(PropertyFieldId::ColorAlpha, old_alpha.average + 0.1)])
        .unwrap();
    history.apply_temporal(&command).unwrap();
    let new_end = history.document().materialize_frame(2).unwrap();
    for channel in channels {
        let old = old_end.solid_paint(channel).unwrap();
        let new = new_end.solid_paint(channel).unwrap();
        assert_eq!(
            (old.red, old.green, old.blue),
            (new.red, new.green, new.blue)
        );
        near(new.alpha, old_alpha.average + 0.1);
    }
    for old in before.temporal_end_overrides() {
        if let TemporalEndOverride::Color(old) = old {
            let new = history
                .document()
                .temporal_end_overrides()
                .iter()
                .find_map(|entry| match entry {
                    TemporalEndOverride::Color(value) if value.channel_id == old.channel_id => {
                        Some(value)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(new.easing, old.easing);
            if matches!(old.mode, ColorEndMode::HueRotation { .. }) {
                assert_eq!(new, old);
            }
        }
    }
    let unchanged = history.document().clone();
    assert!(
        unchanged
            .edit_all_channel_end_command(&[(PropertyFieldId::ColorAlpha, 1.1)])
            .is_err()
    );
    assert_eq!(history.document(), &unchanged);
    assert_eq!(
        history.document().materialize_frame(0).unwrap(),
        before.materialize_frame(0).unwrap()
    );
    history.undo().unwrap().unwrap();
    assert_eq!(history.document(), &before);
}

/// Verifies no-op, empty, duplicate, inactive and nonfinite requests do not change history.
///
/// # Panics
/// Panics if an invalid batch is accepted, identity-preserving edits normalize state, or timing changes.
#[test]
fn batch_validation_and_noop_preserve_exact_authority() {
    let mut history = history(HalftoneChannelModel::Rgb);
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let document = history.document();
    for edits in [
        vec![],
        vec![
            (PropertyFieldId::Opacity, 0.5),
            (PropertyFieldId::Opacity, 0.7),
        ],
        vec![(PropertyFieldId::Visibility, 1.0)],
        vec![(PropertyFieldId::Opacity, f64::NAN)],
        vec![(PropertyFieldId::Opacity, 2.0)],
    ] {
        assert!(
            document
                .edit_all_channel_start_configuration(&edits)
                .is_err()
        );
        assert!(document.edit_all_channel_end_command(&edits).is_err());
    }
    let gain = document
        .channel_scalar_batch(PropertyFieldId::ModeledMappingGain)
        .unwrap();
    assert_eq!(
        document
            .edit_all_channel_start_configuration(&[(gain.field, gain.average)])
            .unwrap(),
        DocumentConfiguration::capture(document)
    );
    assert_eq!(
        document
            .edit_all_channel_end_command(&[(gain.field, gain.average)])
            .unwrap()
            .replacement(),
        &document.temporal_authority()
    );
}

/// Equalizes mixed RGB/CMYK tone offsets even when the entered value equals their old average.
/// Start/End isolation and exact Undo/Redo hold through repeated individual and ALL edits.
///
/// # Panics
/// Panics if a batch misses a channel, changes an initialized endpoint or loses history state.
#[test]
fn repeated_named_and_all_bias_edits_equalize_each_selected_endpoint() {
    for model in [HalftoneChannelModel::Rgb, HalftoneChannelModel::Cmyk] {
        let mut history = history(model);
        let ids = history
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        let command = history.document().initialize_end_command().unwrap();
        history.apply_temporal(&command).unwrap();
        for round in 0..12 {
            let id = ids[round % ids.len()];
            history
                .apply(&DocumentCommand::SetModeledMappingField {
                    channel_id: id,
                    edit: ModeledMappingFieldEdit::Bias(0.1 + round as f64 * 0.01),
                })
                .unwrap();
            let before = history.document().clone();
            let old_end = before.materialize_frame(2).unwrap();
            let field = PropertyFieldId::ModeledMappingBias;
            let desired = before.channel_scalar_batch(field).unwrap().average;
            start_batch(&mut history, &[(field, desired)]);
            for value in history
                .document()
                .channel_scalar_batch(field)
                .unwrap()
                .values
            {
                near(value.value, desired);
            }
            assert_eq!(history.document().materialize_frame(2).unwrap(), old_end);
            let after = history.document().clone();
            history.undo().unwrap().unwrap();
            assert_eq!(history.document(), &before);
            history.redo().unwrap().unwrap();
            assert_eq!(history.document(), &after);
            let command = history
                .document()
                .edit_effective_end_command(&[TemporalEndpointEdit {
                    target: PropertyTarget::Channel(id),
                    field,
                    effective_end: -0.2,
                    easing: Easing::SmoothStep,
                }])
                .unwrap();
            history.apply_temporal(&command).unwrap();
            let before_start = history.document().materialize_frame(0).unwrap();
            let desired_end = history
                .document()
                .materialize_frame(2)
                .unwrap()
                .channel_scalar_batch(field)
                .unwrap()
                .average;
            let command = history
                .document()
                .edit_all_channel_end_command(&[(field, desired_end)])
                .unwrap();
            history.apply_temporal(&command).unwrap();
            for value in history
                .document()
                .materialize_frame(2)
                .unwrap()
                .channel_scalar_batch(field)
                .unwrap()
                .values
            {
                near(value.value, desired_end);
            }
            assert_eq!(
                history.document().materialize_frame(0).unwrap(),
                before_start
            );
        }
    }
}

/// Assigns effective layout values without confusing persisted channel deltas with absolutes.
///
/// # Panics
/// Panics if the ALL assignment changes initialized End values or leaves unequal Start values.
#[test]
fn all_layout_assignments_equalize_effective_values_and_keep_end() {
    let mut history = history(HalftoneChannelModel::Rgb);
    for channel_id in [ChannelId(1), ChannelId(2), ChannelId(3)] {
        let command = history
            .document()
            .set_channel_pattern_rotation_for_effective(channel_id, channel_id.0 as f64 * 10.0)
            .unwrap();
        history.apply(&command).unwrap();
    }
    let command = history.document().initialize_end_command().unwrap();
    history.apply_temporal(&command).unwrap();
    let old_end = history.document().materialize_frame(2).unwrap();
    start_batch(&mut history, &[(PropertyFieldId::RotationDegrees, 20.0)]);
    for value in history
        .document()
        .channel_scalar_batch(PropertyFieldId::RotationDegrees)
        .unwrap()
        .values
    {
        near(value.value, 20.0);
    }
    assert_eq!(history.document().materialize_frame(2).unwrap(), old_end);
}
