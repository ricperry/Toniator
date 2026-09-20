//! Focused source-editor and normal-inspector authority regressions for Gates 2 and 3.

use super::*;

/// Keeps the native inspector hierarchy alive until a numeric activation finishes unwinding.
/// Runs only in the private GTK harness; domain history changes immediately, but GTK child
/// replacement must wait for the idle boundary so native focus traversal retains valid widgets.
///
/// # Panics
/// Panics if GTK setup fails, an edit does not publish, or activation removes selector children.
#[test]
#[ignore = "requires the private GTK Wayland session"]
fn numeric_activation_defers_inspector_hierarchy_changes() {
    register_resources();
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.InspectorRegression")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg");
    let mut workspace = load_workspace(&path).unwrap();
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, 0.25).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    let rgb = authoritative_channel_ids(workspace.document());
    for (channel, rotation, edited_axis) in [
        (rgb[0], 30.0, toniator_domain::TranslationEditedAxis::X),
        (rgb[1], 60.0, toniator_domain::TranslationEditedAxis::Y),
    ] {
        let command = workspace
            .document()
            .set_channel_pattern_rotation_for_effective(channel, rotation)
            .unwrap();
        workspace.history.apply(&command).unwrap();
        workspace
            .history
            .apply(&DocumentCommand::SetTranslationAxis {
                channel_id: channel,
                edited_axis,
                value: 4.5,
            })
            .unwrap();
    }
    replace_model_topology(&mut workspace.history, PreviewModel::Cmyk).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    let cmyk = authoritative_channel_ids(workspace.document());
    let cyan = cmyk[0];
    for (channel_id, edited_axis, value) in [
        (cyan, toniator_domain::TranslationEditedAxis::X, 4.25),
        (cmyk[1], toniator_domain::TranslationEditedAxis::Y, 4.25),
        (cmyk[2], toniator_domain::TranslationEditedAxis::X, 3.18),
        (cmyk[2], toniator_domain::TranslationEditedAxis::Y, 3.18),
    ] {
        workspace
            .history
            .apply(&DocumentCommand::SetTranslationAxis {
                channel_id,
                edited_axis,
                value,
            })
            .unwrap();
    }
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/ton008");
    fs::create_dir_all(&output).unwrap();
    save_container(
        &output.join("before-cyan-rotation.toniator"),
        workspace.document(),
        &workspace.sources,
    )
    .unwrap();
    state.borrow_mut().workspace = Some(workspace);
    state.borrow_mut().inspector_runtime.target = InspectorTarget::Channel(cyan);
    rebuild_inspector(&state);
    let selector = state.borrow().channel_segments.first_child().unwrap();
    let entry = state
        .borrow()
        .descriptor_components
        .values()
        .find(|component| component.value.descriptor.field == PropertyFieldId::RotationDegrees)
        .unwrap()
        .control
        .clone()
        .downcast::<gtk::Entry>()
        .unwrap();
    entry.set_text("22.5");
    entry.emit_activate();
    assert_eq!(
        state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .document()
            .effective_channel_pattern(cyan)
            .unwrap()
            .pattern_rotation_degrees,
        22.5
    );
    assert!(
        selector.parent().is_some(),
        "numeric activation must not detach GTK traversal children synchronously"
    );
    assert!(state.borrow().inspector_rebuild_scheduled);
    for _ in 0..100 {
        if !state.borrow().inspector_rebuild_scheduled {
            break;
        }
        glib::MainContext::default().iteration(false);
    }
    assert!(!state.borrow().inspector_rebuild_scheduled);
    assert!(
        selector.parent().is_none(),
        "idle rebuild replaces the old hierarchy after activation"
    );
    state.borrow().window.destroy();
}

/// Verifies that model switching inherits the document base rather than the first channel's recipe.
/// Exercises both immutable sources, Undo/Redo and all new-role instances; writes a pre-switch GUI fixture.
///
/// # Panics
/// Panics when registry application, document-base inheritance, persistence or history boundaries regress.
#[test]
fn model_switch_uses_document_base_after_channel_override() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/review-advanced");
    fs::create_dir_all(&output).unwrap();
    for (name, source) in [
        ("raster", "raster-sample.png"),
        ("vector", "vector-sample.svg"),
    ] {
        let mut workspace = load_workspace(&root.join("assets").join(source)).unwrap();
        let base = workspace.document().pattern_settings().clone();
        PresetRegistry::bundled()
            .apply_to_selected(&mut workspace.history, ChannelId(1), "round-spiral-marks")
            .unwrap();
        assert_ne!(
            workspace
                .document()
                .effective_channel_pattern(ChannelId(1))
                .unwrap()
                .definition_id,
            base.definition_id
        );
        assert_eq!(
            workspace
                .document()
                .effective_channel_pattern(ChannelId(2))
                .unwrap()
                .definition_id,
            base.definition_id
        );
        let before = workspace.document().clone();
        let snapshot = workspace.snapshot();
        save_container(
            &output.join(format!("model-switch-{name}.toniator")),
            &snapshot.document,
            &snapshot.sources,
        )
        .unwrap();
        replace_model_topology(&mut workspace.history, PreviewModel::Cmyk).unwrap();
        assert_eq!(workspace.document().pattern_settings(), &base);
        for channel in workspace.document().channel_topology().unwrap().channels() {
            assert_eq!(
                channel.pattern_instance,
                ChannelTopologyTemplate::document_base().pattern_instance
            );
            assert_eq!(
                workspace
                    .document()
                    .effective_channel_pattern(channel.id)
                    .unwrap()
                    .definition_id,
                base.definition_id
            );
        }
        let after = workspace.document().clone();
        workspace.history.undo().unwrap();
        assert_eq!(workspace.document(), &before);
        workspace.history.redo().unwrap();
        assert_eq!(workspace.document(), &after);
    }
}

/// Proves the Advanced command path owns independent source consumers and Wizard recipes preserve them.
/// Writes current project fixtures from both immutable source inputs for private GTK and native export checks.
///
/// # Panics
/// Panics on incorrect routing, private publication, pattern replacement, or persistence boundaries.
#[test]
fn source_consumers_advanced_owns_mapping_and_wizard_preserves_it() {
    for (name, source) in [
        ("raster", "raster-sample.png"),
        ("vector", "vector-sample.svg"),
    ] {
        let mut workspace = load_workspace(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(source),
        )
        .unwrap();
        let channel = ChannelId(3);
        let target = InspectorTarget::Channel(channel);
        let before = workspace.document().clone();
        let mut draft = DocumentHistory::new_draft(&workspace.history);
        for (field, input) in [
            (
                PropertyFieldId::ArtworkWeightMappingComponent,
                InspectorInput::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                    SourceMappingComponent::Green,
                )),
            ),
            (
                PropertyFieldId::ModeledMappingComponent,
                InspectorInput::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                    SourceMappingComponent::Red,
                )),
            ),
            (
                PropertyFieldId::ArtworkWeightMappingGamma,
                InspectorInput::FiniteF64(1.4),
            ),
            (
                PropertyFieldId::ModeledMappingGamma,
                InspectorInput::FiniteF64(0.8),
            ),
            (
                PropertyFieldId::ArtworkWeightStrength,
                InspectorInput::FiniteF64(0.85),
            ),
        ] {
            let current = advanced_settings_values(draft.document(), target)
                .into_iter()
                .find(|value| value.descriptor.field == field)
                .unwrap();
            assert_eq!(current.descriptor.target, PropertyTarget::Channel(channel));
            let command = command_for_inspector_input(
                draft.document(),
                Some(channel),
                DefinitionEditScope::SelectedCopy,
                &current.descriptor,
                input,
            )
            .unwrap();
            draft.apply(&command).unwrap();
        }
        assert_eq!(
            workspace.document(),
            &before,
            "private edits do not publish before Apply"
        );
        workspace.history.squash_draft(&draft).unwrap();
        let weighting = workspace.document().channel_weighting(channel).unwrap();
        let fill = workspace
            .document()
            .modeled_channel(channel)
            .unwrap()
            .mapping;
        assert_eq!(weighting.mapping.component, SourceMappingComponent::Green);
        assert_eq!(fill.component, SourceMappingComponent::Red);
        workspace.history.undo().unwrap();
        assert_eq!(workspace.document(), &before);
        workspace.history.redo().unwrap();
        for pattern in [
            "source-weighted-dispersion-voronoi",
            "even-random-circles",
            "source-weighted-dispersion-voronoi",
        ] {
            PresetRegistry::bundled()
                .apply_to_selected(&mut workspace.history, channel, pattern)
                .unwrap();
            assert_eq!(
                workspace.document().channel_weighting(channel),
                Some(weighting)
            );
            assert_eq!(
                workspace
                    .document()
                    .modeled_channel(channel)
                    .unwrap()
                    .mapping,
                fill
            );
        }
        let (projection, _, _) = wizard_route_for_document(workspace.document(), target).unwrap();
        let wizard = wizard_active_values(workspace.document(), target, &projection);
        assert!(
            wizard
                .iter()
                .all(|value| source_consumer_group(value.descriptor.field).is_none())
        );
        assert!(
            wizard
                .iter()
                .any(|value| value.descriptor.field == PropertyFieldId::RandomDensityModulation)
        );
        let values = advanced_settings_values(workspace.document(), target);
        for field in [
            PropertyFieldId::ArtworkWeightMappingBlackPoint,
            PropertyFieldId::ArtworkWeightMappingWhitePoint,
            PropertyFieldId::ArtworkWeightMappingGamma,
            PropertyFieldId::ArtworkWeightMappingContrast,
            PropertyFieldId::ArtworkWeightMappingCutoff,
        ] {
            assert!(values.iter().any(|value| value.descriptor.field == field));
            assert!(workspace.document().channel_scalar_batch(field).is_ok());
        }
        let output = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/review-source-consumers");
        fs::create_dir_all(&output).unwrap();
        let path = output.join(format!("{name}.toniator"));
        save_container(&path, workspace.document(), &workspace.sources).unwrap();
        let reopened = load_workspace(&path).unwrap();
        assert_eq!(reopened.document(), workspace.document());
    }
}

/// Keeps weighted transforms editable, preserves them on recipe replacement, and scopes guidance.
///
/// Saves a current-format weighted fixture for the private GTK acceptance run; the source
/// bundle remains the immutable raster input. This does not operate GTK or mutate user files.
///
/// # Panics
/// Panics if transform intent, active controls, warning scope, history or persistence diverges.
#[test]
fn gate3_weighted_transform_controls_and_notice() {
    let mut workspace = workspace();
    let channel = ChannelId(1);
    let command = workspace
        .document()
        .set_channel_pattern_rotation_for_effective(channel, 23.0)
        .unwrap();
    workspace.history.apply(&command).unwrap();
    PresetRegistry::bundled()
        .apply_to_selected(
            &mut workspace.history,
            channel,
            "source-weighted-dispersion-voronoi",
        )
        .unwrap();
    assert_eq!(
        workspace
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .pattern_rotation_degrees,
        23.0
    );
    for field in [
        PropertyFieldId::RotationDegrees,
        PropertyFieldId::TranslationX,
        PropertyFieldId::TranslationY,
    ] {
        assert!(
            inline_inspector_values(workspace.document(), InspectorTarget::Channel(channel))
                .iter()
                .any(|value| value.descriptor.field == field)
        );
    }
    assert!(
        artwork_weighted_transform_notice(workspace.document(), InspectorTarget::DocumentAll)
            .is_some()
    );
    assert!(
        artwork_weighted_transform_notice(workspace.document(), InspectorTarget::Channel(channel))
            .is_some()
    );
    assert!(
        artwork_weighted_transform_notice(
            workspace.document(),
            InspectorTarget::Channel(ChannelId(2))
        )
        .is_none()
    );
    let rotated = workspace.document().clone();
    let command = workspace
        .document()
        .set_channel_pattern_rotation_for_effective(channel, 0.0)
        .unwrap();
    workspace.history.apply(&command).unwrap();
    assert!(
        artwork_weighted_transform_notice(workspace.document(), InspectorTarget::DocumentAll)
            .is_none()
    );
    workspace.history.undo().unwrap();
    assert_eq!(workspace.document(), &rotated);
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/review-gate3-placement");
    fs::create_dir_all(&output).unwrap();
    let path = output.join("weighted-transforms.toniator");
    save_container(&path, workspace.document(), &workspace.sources).unwrap();
    let reopened = load_workspace(&path).unwrap();
    assert_eq!(reopened.document(), workspace.document());
    let mut vector = load_workspace(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg"),
    )
    .unwrap();
    PresetRegistry::bundled()
        .apply_to_selected(
            &mut vector.history,
            channel,
            "source-weighted-dispersion-voronoi",
        )
        .unwrap();
    let command = vector
        .document()
        .set_channel_pattern_rotation_for_effective(channel, 23.0)
        .unwrap();
    vector.history.apply(&command).unwrap();
    for (edited_axis, value) in [
        (toniator_domain::TranslationEditedAxis::X, 7.25),
        (toniator_domain::TranslationEditedAxis::Y, -4.5),
    ] {
        vector
            .history
            .apply(&DocumentCommand::SetTranslationAxis {
                channel_id: channel,
                edited_axis,
                value,
            })
            .unwrap();
    }
    save_container(
        &output.join("weighted-vector.toniator"),
        vector.document(),
        &vector.sources,
    )
    .unwrap();
}

/// Builds a current document from immutable source pixels, avoiding persisted historical schemas.
///
/// # Panics
/// Panics when the baseline source cannot initialize the ordinary application workspace.
fn workspace() -> Workspace {
    load_workspace(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
        .expect("baseline artwork opens")
}

/// Keeps one real ALL row per field and retains separate response targets on multi-output channels.
///
/// # Panics
/// Panics if source and geometry homes overlap, fields disappear, or GTK fabricates batch targets.
#[test]
fn normal_controls_retain_channel_output_authority() {
    let mut workspace = workspace();
    let before = workspace.document().clone();
    let configuration = before
        .edit_all_pattern_recipe_configuration(&toniator_domain::PatternRecipeEdit::Append(
            toniator_domain::PatternRecipeConstructionKind::Marks,
        ))
        .unwrap();
    workspace
        .history
        .apply_document_configuration(&before, workspace.history.revision(), &configuration)
        .unwrap();
    let document = workspace.document();
    let all = inline_inspector_values(document, InspectorTarget::DocumentAll);
    for field in [
        PropertyFieldId::TranslationX,
        PropertyFieldId::TranslationY,
        PropertyFieldId::MarkMinimumFill,
        PropertyFieldId::MarkMaximumFill,
    ] {
        let rows = all
            .iter()
            .filter(|row| row.descriptor.field == field)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "one ALL control for {field:?}");
        assert_ne!(rows[0].descriptor.target, PropertyTarget::Document);
        assert!(main_all_scalar(
            InspectorTarget::DocumentAll,
            &rows[0].descriptor
        ));
        assert!(
            document
                .property_descriptors()
                .contains(&rows[0].descriptor)
        );
    }
    for channel in authoritative_channel_ids(document) {
        let named = inline_inspector_values(document, InspectorTarget::Channel(channel));
        let outputs = document
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings;
        assert!(outputs.len() > 1);
        for output in outputs {
            for field in [
                PropertyFieldId::MarkMinimumFill,
                PropertyFieldId::MarkMaximumFill,
            ] {
                let rows = named
                    .iter()
                    .filter(|row| {
                        row.descriptor.field == field
                            && row.descriptor.target
                                == PropertyTarget::ChannelOutput(channel, output.output_layer_id)
                    })
                    .count();
                assert_eq!(rows, 1);
            }
        }
        assert!(
            advanced_settings_values(document, InspectorTarget::Channel(channel))
                .iter()
                .all(|row| !inline_response_field(row.descriptor.field))
        );
    }
    assert_eq!(
        inspector_field_label(PropertyFieldId::Density),
        "Feature size"
    );
    assert_eq!(
        inspector_field_label(PropertyFieldId::MarkMaximumFill),
        "Coverage"
    );
    assert_eq!(
        main_inspector_section(PropertyFieldId::TranslationX),
        main_inspector_section(PropertyFieldId::RotationDegrees)
    );
}

/// Applies representative ALL controls and their easing/reset to every output at End, with exact Undo.
///
/// # Panics
/// Panics if the representative becomes the sole audience or if Start or unrelated values change.
#[test]
fn representative_all_rows_preserve_end_audience() {
    let mut workspace = workspace();
    let initial = workspace.document().clone();
    for field in [
        PropertyFieldId::TranslationX,
        PropertyFieldId::MarkMaximumFill,
    ] {
        let descriptor =
            inline_inspector_values(workspace.document(), InspectorTarget::DocumentAll)
                .into_iter()
                .find(|row| row.descriptor.field == field)
                .unwrap()
                .descriptor;
        let value = if field == PropertyFieldId::TranslationX {
            17.0
        } else {
            0.7
        };
        advanced_batches::apply(
            &mut workspace.history,
            temporal_preview::Endpoint::End,
            field,
            value,
        )
        .unwrap();
        let command = temporal_edit::easing_command(
            workspace.document(),
            &descriptor,
            toniator_domain::Easing::Hold,
            true,
        );
        workspace.history.apply_temporal(&command).unwrap();
        let audience = workspace.document().channel_scalar_batch(field).unwrap();
        assert!(audience.values.len() > 1);
        for target in &audience.values {
            assert!(workspace.document().temporal_end_overrides().iter().any(|entry|
                matches!(entry, toniator_domain::TemporalEndOverride::Scalar(value)
                    if value.target == target.target && value.field == field && value.easing == toniator_domain::Easing::Hold)));
        }
        let before_reset = workspace.document().clone();
        let command = temporal_edit::reset_command(workspace.document(), &descriptor, true);
        workspace.history.apply_temporal(&command).unwrap();
        assert!(!workspace.document().temporal_end_overrides().iter().any(|entry|
            matches!(entry, toniator_domain::TemporalEndOverride::Scalar(value) if value.field == field)));
        workspace.history.undo().unwrap().unwrap();
        assert_eq!(workspace.document(), &before_reset);
    }
    assert_eq!(
        workspace.document().materialize_frame(0).unwrap(),
        initial.materialize_frame(0).unwrap()
    );
}

/// Publishes coupled tonal edits at End as one undoable change and round-trips native source exports.
///
/// # Panics
/// Panics if a source control loses its domain route, an invalid pair publishes,
/// Start changes, persisted tone differs, or either immutable artwork cannot render.
#[test]
fn tonal_controls_publish_selected_frame_and_native_outputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root
        .join("target/validation/review-gate2-source-response")
        .join(format!(
            "run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&directory).unwrap();
    let edits = [
        (PropertyFieldId::ModeledMappingBlackPoint, 0.6),
        (PropertyFieldId::ModeledMappingWhitePoint, 0.9),
        (PropertyFieldId::ModeledMappingGamma, 1.8),
        (PropertyFieldId::ModeledMappingContrast, 1.2),
        (PropertyFieldId::ModeledMappingCutoff, 0.3),
    ];
    for (input, name) in [
        ("raster-sample.png", "raster"),
        ("vector-sample.svg", "vector"),
    ] {
        let mut workspace = load_workspace(&root.join("assets").join(input)).unwrap();
        advanced_batches::apply_fields(
            &mut workspace.history,
            temporal_preview::Endpoint::Start,
            &[(PropertyFieldId::ModeledMappingWhitePoint, 0.4)],
        )
        .unwrap();
        let initial = workspace.snapshot();
        let mut draft = DocumentHistory::new_draft(&workspace.history);
        advanced_batches::apply_fields(&mut draft, temporal_preview::Endpoint::End, &edits)
            .unwrap();
        let candidate = draft.document().clone();
        assert!(
            advanced_batches::apply_fields(
                &mut draft,
                temporal_preview::Endpoint::End,
                &[(PropertyFieldId::ModeledMappingBlackPoint, 0.95),]
            )
            .is_err()
        );
        assert_eq!(draft.document(), &candidate);
        let end = temporal_preview::Endpoint::End.frame(&candidate);
        assert_eq!(
            candidate.materialize_frame(0).unwrap(),
            initial.document.materialize_frame(0).unwrap()
        );
        let rendered_end = candidate.materialize_frame(end).unwrap();
        for (field, expected) in edits {
            let batch = rendered_end.channel_scalar_batch(field).unwrap();
            assert!(batch.values.len() > 1);
            assert!(batch.values.iter().all(|value| value.value == expected));
            for channel in authoritative_channel_ids(&rendered_end) {
                let descriptor =
                    advanced_settings_values(&rendered_end, InspectorTarget::Channel(channel))
                        .into_iter()
                        .find(|value| value.descriptor.field == field)
                        .unwrap()
                        .descriptor;
                let command = command_for_inspector_input(
                    &rendered_end,
                    Some(channel),
                    DefinitionEditScope::SelectedCopy,
                    &descriptor,
                    InspectorInput::FiniteF64(expected),
                )
                .unwrap();
                assert!(
                    matches!(command, DocumentCommand::SetModeledMappingField { channel_id, .. } if channel_id == channel)
                );
            }
        }
        // Every frame must materialize with ordered levels, including interior interpolation.
        for frame in 0..=end {
            candidate.materialize_frame(frame).unwrap();
        }
        workspace.history.squash_draft(&draft).unwrap();
        let after = workspace.snapshot();
        workspace.history.undo().unwrap().unwrap();
        assert_eq!(workspace.snapshot(), initial);
        workspace.history.redo().unwrap().unwrap();
        assert_eq!(workspace.snapshot(), after);
        let persisted = directory.join(format!("{name}.toniator"));
        save_container(&persisted, &after.document, &after.sources).unwrap();
        assert_eq!(load_workspace(&persisted).unwrap().snapshot(), after);
        for (format, suffix) in [(ExportFormat::Png, "png"), (ExportFormat::Svg, "svg")] {
            export_snapshot_frame(
                after.clone(),
                directory.join(format!("{name}-end.{suffix}")),
                ExportSettings {
                    format,
                    background: RasterBackground::Transparent,
                    output_target: None,
                    antialiasing: RasterAntialiasing::On,
                },
                end,
            )
            .unwrap();
        }
    }
    println!("Gate 2 native witnesses: {}", directory.display());
}
