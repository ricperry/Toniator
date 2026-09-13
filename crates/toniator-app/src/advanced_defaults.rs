//! Scoped Advanced defaults use domain values and the dialog's private endpoint history.

use super::*;

/// Restores Advanced source, paint, opacity and region interpretation for the requested channels.
/// Work is prepared on a disposable history and published atomically; other channels, pattern
/// layout and inline fill bounds remain unchanged. Start edits retain End intent. End color resets
/// replace hue/RGBA bindings with a canonical color endpoint; static source choices remain shared.
///
/// # Errors
/// Returns stale target, domain validation or endpoint errors without mutating the supplied history.
pub(super) fn apply(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    target: InspectorTarget,
) -> Result<bool, String> {
    let display = advanced_temporal::display_document(history.document(), endpoint)?;
    let channels = match target {
        InspectorTarget::DocumentAll => authoritative_channel_ids(&display),
        InspectorTarget::Channel(channel) => vec![channel],
    };
    let mut inputs = Vec::new();
    let mut colors = Vec::new();
    for channel in channels {
        let target = InspectorTarget::Channel(channel);
        let values = advanced_settings_values(&display, target);
        if values.is_empty() {
            return Err("This channel is no longer available.".into());
        }
        if display.solid_paint(channel).is_ok() {
            let component = |field| match display.channel_property_default(channel, field) {
                Some(PropertyCurrentValueKind::FiniteF64(value)) => Ok(value),
                _ => Err("This channel has no default paint color.".to_owned()),
            };
            colors.push((
                channel,
                ColorValue {
                    red: component(PropertyFieldId::ColorRed)?,
                    green: component(PropertyFieldId::ColorGreen)?,
                    blue: component(PropertyFieldId::ColorBlue)?,
                    alpha: component(PropertyFieldId::ColorAlpha)?,
                },
            ));
        }
        for current in values {
            if paint_editor::component(current.descriptor.field).is_some() {
                continue;
            }
            let Some(value) = display.channel_property_default(channel, current.descriptor.field)
            else {
                continue;
            };
            let input = match value {
                PropertyCurrentValueKind::FiniteF64(value) => InspectorInput::FiniteF64(value),
                PropertyCurrentValueKind::Boolean(value) => InspectorInput::Boolean(value),
                PropertyCurrentValueKind::EnumChoice(value) => InspectorInput::EnumChoice(value),
                _ => continue,
            };
            let locator = advanced_descriptor_locator(&display, target, &current.descriptor)
                .ok_or("This Advanced setting is no longer available.")?;
            inputs.push((target, locator, input));
        }
    }
    let mut pending = DocumentHistory::new_draft(history);
    advanced_temporal::apply_pending_inputs(&mut pending, endpoint, &inputs)?;
    if !colors.is_empty() {
        match endpoint {
            temporal_preview::Endpoint::Start => {
                let base = pending.document().clone();
                let configuration = base
                    .edit_start_colors_configuration(&colors)
                    .map_err(|error| error.to_string())?;
                pending
                    .apply_document_configuration(&base, pending.revision(), &configuration)
                    .map_err(|error| error.to_string())?;
            }
            temporal_preview::Endpoint::End => {
                let edits = colors
                    .into_iter()
                    .map(|(channel_id, end)| toniator_domain::ColorAnimationEdit {
                        channel_id,
                        mode: Some(toniator_domain::ColorEndMode::LinearColor { end }),
                        easing: toniator_domain::Easing::Linear,
                    })
                    .collect::<Vec<_>>();
                let command = pending
                    .document()
                    .edit_color_animation_command(&edits)
                    .map_err(|error| error.to_string())?;
                pending
                    .apply_temporal(&command)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    history
        .squash_draft(&pending)
        .map(|result| !result.unchanged)
        .map_err(|error| error.to_string())
}

/// Resets only the live dialog's captured channel scope, then refreshes controls and preview in place.
/// Raw pending entries in that scope are discarded; Apply and Cancel keep their existing authority.
pub(super) fn reset(state: &Rc<RefCell<AppState>>, epoch: u64, button: &gtk::Button) {
    let Some((draft, endpoint, target, guard, status)) = state
        .borrow()
        .advanced_settings
        .as_ref()
        .filter(|surface| surface.epoch == epoch)
        .map(|surface| {
            (
                surface.draft.clone(),
                surface.endpoint,
                surface.target,
                surface.refreshing.clone(),
                surface.status.clone(),
            )
        })
    else {
        return;
    };
    guard.set(true);
    button.grab_focus();
    guard.set(false);
    let result = apply(&mut draft.borrow_mut(), endpoint, target);
    match result {
        Ok(_) => {
            advanced_temporal::refresh_after_reset(state, epoch);
            submit_advanced_preview(state, epoch);
        }
        Err(error) => status.set_label(&format!("Couldn’t reset these settings: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resets current Advanced region selectors while retaining inline fill intent and other channels.
    /// Exercises selected-copy locator renewal across two selector edits and atomic Undo.
    ///
    /// # Panics
    /// Panics if reset loses authored bounds, changes another channel or cannot resolve renewed outputs.
    #[test]
    fn advanced_defaults_region_selectors_preserve_inline_bounds() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, "grid-voronoi-scale")
            .unwrap();
        let channel = ChannelId(1);
        let output = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        let command = history
            .document()
            .set_selected_channel_region_response_for_effective(
                channel,
                output,
                toniator_domain::RegionGeometryResponse {
                    algorithm: toniator_domain::RegionResizeAlgorithm::UniformOffset,
                    sampling: toniator_domain::RegionSamplingStrategy::AreaAverage,
                    minimum_fill: 0.2,
                    maximum_fill: 0.8,
                },
            )
            .unwrap();
        history.apply(&command).unwrap();
        let before = history.document().clone();
        assert!(
            apply(
                &mut history,
                temporal_preview::Endpoint::Start,
                InspectorTarget::Channel(channel)
            )
            .unwrap()
        );
        let result = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap();
        let PatternGeometryResponse::Regions(response) = &result.output_settings[0].response else {
            panic!("region output remains a region");
        };
        assert_eq!(
            response.algorithm,
            toniator_domain::RegionResizeAlgorithm::Scale
        );
        assert_eq!(
            response.sampling,
            toniator_domain::RegionSamplingStrategy::ReferencePoint
        );
        assert_eq!((response.minimum_fill, response.maximum_fill), (0.2, 0.8));
        assert_eq!(
            history
                .document()
                .effective_channel_pattern(ChannelId(2))
                .unwrap(),
            before.effective_channel_pattern(ChannelId(2)).unwrap()
        );
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
    }

    /// Verifies named/ALL defaults across model roles, private publication and immutable source kinds.
    /// Coupled source levels reset together; paint/opacity reset while other channels and patterns stay intact.
    ///
    /// # Panics
    /// Panics when source loading, domain edits, canonical defaults or Undo/Redo boundaries regress.
    #[test]
    fn advanced_defaults_scope_and_history() {
        for source in ["raster-sample.png", "vector-sample.svg"] {
            for model in [
                HalftoneChannelModel::Rgb,
                HalftoneChannelModel::Cmyk,
                HalftoneChannelModel::SourceColorAlpha,
            ] {
                let mut workspace = load_workspace(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../assets")
                        .join(source),
                )
                .unwrap();
                let document = workspace.document();
                let template = toniator_domain::ChannelTopologyTemplate {
                    pattern_instance: document
                        .channel_pattern_instance(ChannelId(1))
                        .unwrap()
                        .clone(),
                };
                let topology = document
                    .canonical_channel_topology(model, template)
                    .unwrap();
                if workspace.document().channel_model() != Some(model) {
                    workspace
                        .history
                        .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
                        .unwrap();
                }
                let canonical = workspace.document().clone();
                let channels = authoritative_channel_ids(&canonical);
                for &channel in &channels {
                    for command in [
                        DocumentCommand::SetModeledMappingField {
                            channel_id: channel,
                            edit: toniator_domain::ModeledMappingFieldEdit::Component(
                                SourceMappingComponent::Luminance,
                            ),
                        },
                        DocumentCommand::SetSourceWeightingField {
                            channel_id: channel,
                            edit: toniator_domain::SourceWeightingFieldEdit::Component(
                                SourceMappingComponent::Luminance,
                            ),
                        },
                        DocumentCommand::SetModeledMappingField {
                            channel_id: channel,
                            edit: toniator_domain::ModeledMappingFieldEdit::WhitePoint(0.4),
                        },
                        DocumentCommand::SetSourceWeightingField {
                            channel_id: channel,
                            edit: toniator_domain::SourceWeightingFieldEdit::Inverted(true),
                        },
                    ] {
                        workspace.history.apply(&command).unwrap();
                    }
                }
                let base = workspace.document().clone();
                let edits = channels
                    .iter()
                    .map(|channel| {
                        (
                            PropertyTarget::Channel(*channel),
                            PropertyFieldId::Opacity,
                            0.3,
                        )
                    })
                    .collect::<Vec<_>>();
                let configuration = base.edit_channel_start_configuration(&edits).unwrap();
                workspace
                    .history
                    .apply_document_configuration(
                        &base,
                        workspace.history.revision(),
                        &configuration,
                    )
                    .unwrap();
                let colors = channels
                    .iter()
                    .filter(|channel| workspace.document().solid_paint(**channel).is_ok())
                    .map(|channel| (*channel, ColorValue::from_srgb_hex("#34567880").unwrap()))
                    .collect::<Vec<_>>();
                if !colors.is_empty() {
                    let base = workspace.document().clone();
                    let configuration = base.edit_start_colors_configuration(&colors).unwrap();
                    workspace
                        .history
                        .apply_document_configuration(
                            &base,
                            workspace.history.revision(),
                            &configuration,
                        )
                        .unwrap();
                }
                let before = workspace.document().clone();
                let mut draft = DocumentHistory::new_draft(&workspace.history);
                assert!(
                    apply(
                        &mut draft,
                        temporal_preview::Endpoint::Start,
                        InspectorTarget::Channel(channels[0])
                    )
                    .unwrap()
                );
                assert_eq!(
                    draft.document().modeled_channel(channels[0]),
                    canonical.modeled_channel(channels[0])
                );
                for &channel in &channels[1..] {
                    assert_eq!(
                        draft.document().modeled_channel(channel),
                        before.modeled_channel(channel)
                    );
                }
                assert_eq!(workspace.document(), &before);
                apply(
                    &mut draft,
                    temporal_preview::Endpoint::Start,
                    InspectorTarget::DocumentAll,
                )
                .unwrap();
                assert_eq!(draft.document(), &canonical);
                assert!(
                    !apply(
                        &mut draft,
                        temporal_preview::Endpoint::Start,
                        InspectorTarget::DocumentAll
                    )
                    .unwrap()
                );
                workspace.history.squash_draft(&draft).unwrap();
                workspace.history.undo().unwrap();
                assert_eq!(workspace.document(), &before);
                workspace.history.redo().unwrap();
                assert_eq!(workspace.document(), &canonical);
            }
        }
    }

    /// Restores a canonical End color even under hue animation without changing Start or another channel.
    ///
    /// # Panics
    /// Panics if reset conflicts with grouped color ownership or changes unrelated endpoint intent.
    #[test]
    fn advanced_defaults_end_replaces_hue_without_changing_start() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap()
        .with_temporal_authority(
            toniator_domain::ProjectTiming::new(
                toniator_domain::FrameRate::new(30, 1).unwrap(),
                toniator_domain::FrameRange::new(0, 3).unwrap(),
            ),
            vec![],
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let base = history.document().clone();
        let configuration = base
            .edit_start_colors_configuration(&[(
                ChannelId(1),
                ColorValue::from_srgb_hex("#34567880").unwrap(),
            )])
            .unwrap();
        history
            .apply_document_configuration(&base, history.revision(), &configuration)
            .unwrap();
        let command = history
            .document()
            .edit_color_animation_command(&[toniator_domain::ColorAnimationEdit {
                channel_id: ChannelId(1),
                mode: Some(toniator_domain::ColorEndMode::HueRotation { end_degrees: 120.0 }),
                easing: toniator_domain::Easing::Linear,
            }])
            .unwrap();
        history.apply_temporal(&command).unwrap();
        let before = history.document().clone();
        apply(
            &mut history,
            temporal_preview::Endpoint::End,
            InspectorTarget::Channel(ChannelId(1)),
        )
        .unwrap();
        assert_eq!(
            history.document().solid_paint(ChannelId(1)).unwrap(),
            before.solid_paint(ChannelId(1)).unwrap()
        );
        let end = history.document().materialize_frame(2).unwrap();
        assert_eq!(
            end.solid_paint(ChannelId(1))
                .unwrap()
                .to_srgb_hex()
                .unwrap(),
            "#FF0000FF"
        );
        assert_eq!(
            end.modeled_channel(ChannelId(2)),
            before
                .materialize_frame(2)
                .unwrap()
                .modeled_channel(ChannelId(2))
        );
        assert_eq!(
            history.document().pattern_definition_bundles(),
            before.pattern_definition_bundles()
        );
    }
}
