//! Scoped Advanced defaults use domain values and the dialog's private endpoint history.

use super::*;

/// Restores Advanced source, paint, opacity and channel response for the requested scope.
/// Work is prepared on a disposable history and published atomically; structural Pattern settings
/// and other channels remain unchanged. Start edits retain End intent; ALL range pairs use the
/// domain batch boundary, while region sampling and mark-density mode remain typed ALL fields.
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
    let all_scope = target == InspectorTarget::DocumentAll;
    let response_defaults = DocumentResponseSettings::default();
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
            if all_scope
                && (advanced_channel_response_field(current.descriptor.field)
                    || current.descriptor.field == PropertyFieldId::MarkSizeScale)
            {
                continue;
            }
            let Some(value) = response_default(&response_defaults, current.descriptor.field)
                .or_else(|| display.channel_property_default(channel, current.descriptor.field))
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
    if all_scope {
        let mut response_edits = Vec::new();
        for (minimum, maximum, minimum_default, maximum_default) in [
            (
                PropertyFieldId::MarkMinimumFill,
                PropertyFieldId::MarkMaximumFill,
                response_defaults.marks.minimum_fill,
                response_defaults.marks.maximum_fill,
            ),
            (
                PropertyFieldId::ConnectedMinimumThickness,
                PropertyFieldId::ConnectedMaximumThickness,
                response_defaults.connected_minimum_thickness,
                response_defaults.connected_maximum_thickness,
            ),
            (
                PropertyFieldId::RegionMinimumFill,
                PropertyFieldId::RegionMaximumFill,
                response_defaults.regions_minimum_fill,
                response_defaults.regions_maximum_fill,
            ),
        ] {
            if display.channel_scalar_batch(minimum).is_ok()
                && display.channel_scalar_batch(maximum).is_ok()
            {
                response_edits.extend([(minimum, minimum_default), (maximum, maximum_default)]);
            }
        }
        advanced_batches::apply_fields(&mut pending, endpoint, &response_edits)?;
        if display
            .channel_scalar_batch(PropertyFieldId::MarkSizeScale)
            .is_ok()
        {
            advanced_batches::apply_fields(
                &mut pending,
                endpoint,
                &[(
                    PropertyFieldId::MarkSizeScale,
                    response_defaults.mark_size_scale,
                )],
            )?;
        }
        advanced_batches::apply_all_mark_density_mode(
            &mut pending,
            response_defaults.mark_density_mode,
        )?;
        advanced_batches::apply_region_sampling(&mut pending, response_defaults.region_sampling)?;
    }
    advanced_temporal::apply_pending_inputs(&mut pending, endpoint, &inputs)?;
    if !all_scope && let InspectorTarget::Channel(channel) = target {
        for (output, _) in weighted_mark_density_states(&display, channel) {
            let current = pending.document();
            let named_mode = current
                .channel_response_overrides()
                .iter()
                .find(|entry| entry.channel_id == channel && entry.output_layer_id == output)
                .and_then(|entry| match &entry.response {
                    toniator_domain::ChannelOutputResponse::Marks { density_mode, .. } => {
                        *density_mode
                    }
                    _ => None,
                });
            let current_mode = named_mode.unwrap_or(current.response_settings().mark_density_mode);
            if current_mode == response_defaults.mark_density_mode {
                continue;
            }
            let command = current
                .set_channel_mark_density_mode_for_effective(
                    channel,
                    output,
                    response_defaults.mark_density_mode,
                )
                .map_err(|error| error.to_string())?;
            pending.apply(&command).map_err(|error| error.to_string())?;
        }
    }
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

/// Projects domain-owned response defaults absent from channel-property defaults.
///
/// The domain's typed baseline remains the sole authority for range and sampling defaults.
fn response_default(
    defaults: &DocumentResponseSettings,
    field: PropertyFieldId,
) -> Option<PropertyCurrentValueKind> {
    let value = match field {
        PropertyFieldId::MarkMinimumFill => defaults.marks.minimum_fill,
        PropertyFieldId::MarkMaximumFill => defaults.marks.maximum_fill,
        PropertyFieldId::MarkSizeScale => defaults.mark_size_scale,
        PropertyFieldId::ConnectedMinimumThickness => defaults.connected_minimum_thickness,
        PropertyFieldId::ConnectedMaximumThickness => defaults.connected_maximum_thickness,
        PropertyFieldId::RegionMinimumFill => defaults.regions_minimum_fill,
        PropertyFieldId::RegionMaximumFill => defaults.regions_maximum_fill,
        _ => return None,
    };
    Some(PropertyCurrentValueKind::FiniteF64(value))
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

    /// Resets named region ranges and sampling while preserving structural resizing and other channels.
    /// Exercises the response owner, selected-copy scope and atomic Undo boundary.
    ///
    /// # Panics
    /// Panics if reset loses authored bounds, changes another channel or cannot resolve renewed outputs.
    #[test]
    fn advanced_defaults_reset_region_response_and_preserve_structure() {
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
        let PatternGeometryResponse::Regions(mut structural) = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .response
            .clone()
        else {
            panic!("the selected Pattern exposes a region response");
        };
        structural.algorithm = toniator_domain::RegionResizeAlgorithm::UniformOffset;
        let command = history
            .document()
            .set_selected_channel_region_response_for_effective(channel, output, structural)
            .unwrap();
        history.apply(&command).unwrap();
        let PatternGeometryResponse::Regions(mut authored_response) = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .response
            .clone()
        else {
            panic!("the selected Pattern keeps its region response");
        };
        authored_response.sampling = toniator_domain::RegionSamplingStrategy::ReferencePoint;
        authored_response.minimum_fill = 0.2;
        authored_response.maximum_fill = 0.8;
        let output = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        let command = history
            .document()
            .set_selected_channel_region_response_for_effective(channel, output, authored_response)
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
            toniator_domain::RegionResizeAlgorithm::UniformOffset
        );
        assert_eq!(
            response.sampling,
            toniator_domain::RegionSamplingStrategy::AreaAverage
        );
        assert_eq!((response.minimum_fill, response.maximum_fill), (0.0, 1.0));
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

    /// Resets ALL Mark ranges, density mode, and scale after named members have overrides.
    /// Domain response defaults own the values; paired bounds and field-specific overrides reset atomically.
    ///
    /// # Panics
    /// Panics when a single-output Mark fixture cannot be prepared or reset retains nondefault range intent.
    #[test]
    fn advanced_defaults_reset_all_mark_response_clears_named_overrides() {
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
            .apply_to_document_base(&mut history, "round-spiral-marks")
            .unwrap();
        let channel = ChannelId(1);
        let pattern = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap();
        assert_eq!(pattern.output_settings.len(), 1);
        let output = pattern.output_settings[0].output_layer_id;

        let base = history.document().clone();
        let configuration = base
            .edit_all_channel_start_configuration(&[
                (PropertyFieldId::MarkMinimumFill, 0.1),
                (PropertyFieldId::MarkMaximumFill, 1.5),
                (PropertyFieldId::MarkSizeScale, 1.4),
            ])
            .unwrap();
        history
            .apply_document_configuration(&base, history.revision(), &configuration)
            .unwrap();
        advanced_batches::apply_all_mark_density_mode(
            &mut history,
            toniator_domain::MarkDensityMode::Uniform,
        )
        .unwrap();
        history
            .apply(&DocumentCommand::SetChannelResponse {
                channel_id: channel,
                output_layer_id: output,
                response: toniator_domain::ChannelOutputResponse::Marks {
                    minimum_fill: Some(0.2),
                    maximum_fill: Some(1.75),
                    density_mode: Some(toniator_domain::MarkDensityMode::Variable),
                    size_scale: Some(1.85),
                },
            })
            .unwrap();
        assert_eq!(
            (
                history.document().response_settings().marks.minimum_fill,
                history.document().response_settings().marks.maximum_fill,
            ),
            (0.1, 1.5)
        );
        assert_eq!(
            history.document().response_settings().mark_density_mode,
            toniator_domain::MarkDensityMode::Uniform
        );
        assert_eq!(history.document().response_settings().mark_size_scale, 1.4);
        assert_eq!(history.document().channel_response_overrides().len(), 1);
        assert!(
            history
                .document()
                .channel_scalar_batch(PropertyFieldId::MarkMinimumFill)
                .is_ok()
        );
        assert!(
            history
                .document()
                .channel_scalar_batch(PropertyFieldId::MarkMaximumFill)
                .is_ok()
        );

        assert!(
            apply(
                &mut history,
                temporal_preview::Endpoint::Start,
                InspectorTarget::DocumentAll,
            )
            .unwrap()
        );

        let defaults = DocumentResponseSettings::default();
        assert_eq!(history.document().response_settings(), &defaults);
        assert!(history.document().channel_response_overrides().is_empty());
    }

    /// Restores the named default mode without inheriting an ALL Variable choice or dropping End scale.
    /// The normal scoped draft resets paired ranges and Start scale, then writes only the typed mode field.
    ///
    /// # Panics
    /// Panics if a weighted Mark response cannot be authored, defaulted atomically, or retain its End easing.
    #[test]
    fn advanced_defaults_reset_named_mark_mode_and_preserve_temporal_siblings() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let mut recipe =
            PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Dispersion);
        let PatternStructureRecipe::RandomSites {
            density_modulation, ..
        } = &mut recipe.structure
        else {
            panic!("dispersion uses random placement");
        };
        *density_modulation = toniator_domain::SiteDensityModulation::ArtworkWeighted;
        let recipe = recipe
            .with_output_kind(0, PatternRecipeOutputKind::Marks)
            .unwrap();
        apply_wizard_recipe(&mut history, InspectorTarget::DocumentAll, recipe).unwrap();
        let channel = ChannelId(1);
        let pattern = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap();
        assert_eq!(pattern.output_settings.len(), 1);
        let output = pattern.output_settings[0].output_layer_id;

        let base = history.document().clone();
        let configuration = base
            .edit_all_channel_start_configuration(&[
                (PropertyFieldId::MarkMinimumFill, 0.1),
                (PropertyFieldId::MarkMaximumFill, 1.4),
                (PropertyFieldId::MarkSizeScale, 1.5),
            ])
            .unwrap();
        history
            .apply_document_configuration(&base, history.revision(), &configuration)
            .unwrap();
        advanced_batches::apply_all_mark_density_mode(
            &mut history,
            toniator_domain::MarkDensityMode::Variable,
        )
        .unwrap();
        history
            .apply(&DocumentCommand::SetChannelResponse {
                channel_id: channel,
                output_layer_id: output,
                response: toniator_domain::ChannelOutputResponse::Marks {
                    minimum_fill: Some(0.2),
                    maximum_fill: Some(1.7),
                    density_mode: Some(toniator_domain::MarkDensityMode::Variable),
                    size_scale: Some(1.8),
                },
            })
            .unwrap();
        let end = history
            .document()
            .edit_effective_end_command(&[toniator_domain::TemporalEndpointEdit {
                target: PropertyTarget::ChannelOutput(channel, output),
                field: PropertyFieldId::MarkSizeScale,
                effective_end: 0.65,
                easing: toniator_domain::Easing::SmoothStep,
            }])
            .unwrap();
        history.apply_temporal(&end).unwrap();
        assert!(
            history
                .document()
                .temporal_end_overrides()
                .iter()
                .any(|entry| {
                    matches!(entry, toniator_domain::TemporalEndOverride::Scalar(value)
                    if value.target == PropertyTarget::ChannelOutput(channel, output)
                        && value.field == PropertyFieldId::MarkSizeScale
                        && (value.end - (0.65 - 1.5)).abs() < 1e-10
                        && value.easing == toniator_domain::Easing::SmoothStep)
                }),
            "initialized End scale is present before the reset: {:?}",
            history.document().temporal_end_overrides()
        );

        assert!(
            apply(
                &mut history,
                temporal_preview::Endpoint::Start,
                InspectorTarget::Channel(channel),
            )
            .unwrap()
        );

        assert_eq!(
            history.document().response_settings().mark_density_mode,
            toniator_domain::MarkDensityMode::Variable,
            "named defaults do not change the ALL response choice",
        );
        let named = history
            .document()
            .channel_response_overrides()
            .iter()
            .find(|entry| entry.channel_id == channel && entry.output_layer_id == output)
            .unwrap();
        assert_eq!(
            named.response,
            toniator_domain::ChannelOutputResponse::Marks {
                minimum_fill: Some(0.0),
                maximum_fill: Some(1.0),
                density_mode: Some(toniator_domain::MarkDensityMode::Contextual),
                size_scale: Some(1.0),
            },
            "named defaults set the mode field explicitly while restoring response siblings",
        );
        assert!(
            history
                .document()
                .temporal_end_overrides()
                .iter()
                .any(|entry| {
                    matches!(entry, toniator_domain::TemporalEndOverride::Scalar(value)
                if value.target == PropertyTarget::ChannelOutput(channel, output)
                    && value.field == PropertyFieldId::MarkSizeScale
                    && (value.end - (0.65 - 1.5)).abs() < 1e-10
                    && value.easing == toniator_domain::Easing::SmoothStep)
                }),
            "Start reset retains the existing End scale and easing: {:?}",
            history.document().temporal_end_overrides()
        );
        assert_eq!(
            history
                .document()
                .effective_mark_density(channel, output)
                .unwrap()
                .mode,
            toniator_domain::MarkDensityMode::Uniform,
            "Contextual named default remains independent from ALL Variable intent",
        );
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
