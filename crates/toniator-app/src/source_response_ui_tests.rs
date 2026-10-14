//! Focused source-editor and normal-inspector authority regressions for Gates 2 and 3.

use super::*;

/// Covers Advanced descriptors and All batches across both project inputs, three channel models,
/// and mark, connected-line, region, and source-weighted patterns where each model supports them.
/// It reads current domain values without constructing GTK state and checks the shared absolute
/// coverage contract while retaining Uniform mark scale as its separate 0–2 control.
/// Channel response endpoints and sampling are Advanced controls; structural resize and stroke
/// alignment stay with Pattern construction while Wizard source response fields remain excluded.
///
/// # Panics
/// Panics when current model/pattern capabilities omit a required help description or coverage
/// bounds, move a response field away from Advanced, or move structural Pattern information into it.
#[test]
fn advanced_help_covers_emitted_settings_across_models_and_output_kinds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = PresetRegistry::bundled();
    let catalog = LayeredPresetCatalog::new(&registry, Vec::new()).unwrap();
    let cases = [
        ("round-spiral-marks", PropertyFieldId::MarkMinimumFill),
        (
            "one-guide-lines",
            PropertyFieldId::ConnectedMinimumThickness,
        ),
        ("grid-voronoi-scale", PropertyFieldId::RegionMinimumFill),
        (
            "source-weighted-dispersion-voronoi",
            PropertyFieldId::RegionMinimumFill,
        ),
    ];
    for source in ["raster-sample.png", "vector-sample.svg"] {
        for model in [
            PreviewModel::Rgb,
            PreviewModel::Cmyk,
            PreviewModel::SourceColorAlpha,
        ] {
            let mut workspace = load_workspace(&root.join("assets").join(source)).unwrap();
            if model != PreviewModel::Rgb {
                replace_model_topology(&mut workspace.history, model).unwrap();
            }
            let channel = authoritative_channel_ids(workspace.document())[0];
            for (preset, response_field) in cases {
                if model == PreviewModel::SourceColorAlpha && preset == "one-guide-lines" {
                    continue;
                }
                apply_catalog_pattern_candidate(
                    &mut workspace.history,
                    &catalog,
                    InspectorTarget::Channel(channel),
                    preset,
                )
                .unwrap();
                let document = workspace.document();
                let target = InspectorTarget::Channel(channel);
                let all_values = document.property_values();
                let advanced = advanced_settings_values(document, target);
                let inline = inline_inspector_values(document, target);
                let projection = document
                    .pattern_capabilities(PatternCapabilityScope::Channel(channel))
                    .expect("selected channel capability projection is current");
                let wizard = wizard_active_values(document, target, &projection);
                assert!(
                    !advanced.is_empty(),
                    "{source} {model:?} {preset} exposes Advanced settings"
                );
                assert!(
                    advanced
                        .iter()
                        .all(|value| { !inline_response_field(value.descriptor.field) })
                );
                for value in &advanced {
                    let help = advanced_field_help(value.descriptor.field, Some(&value.descriptor));
                    assert!(
                        help.starts_with(inspector_field_guidance(value.descriptor.field)),
                        "{} {model:?} {preset} has field-specific help",
                        inspector_field_label(value.descriptor.field)
                    );
                    if value.descriptor.bounds.is_some()
                        && value.descriptor.field != PropertyFieldId::Density
                    {
                        assert!(help.contains("Use a value"));
                    }
                    if matches!(
                        value.descriptor.field,
                        PropertyFieldId::MarkMinimumFill
                            | PropertyFieldId::ConnectedMinimumThickness
                            | PropertyFieldId::RegionMinimumFill
                    ) {
                        assert_eq!(
                            inspector_field_label(value.descriptor.field),
                            "Minimum coverage"
                        );
                    }
                    if matches!(
                        value.descriptor.field,
                        PropertyFieldId::MarkMaximumFill
                            | PropertyFieldId::ConnectedMaximumThickness
                            | PropertyFieldId::RegionMaximumFill
                    ) {
                        assert_eq!(
                            inspector_field_label(value.descriptor.field),
                            "Maximum coverage"
                        );
                    }
                    if matches!(
                        value.descriptor.field,
                        PropertyFieldId::MarkMinimumFill
                            | PropertyFieldId::MarkMaximumFill
                            | PropertyFieldId::ConnectedMinimumThickness
                            | PropertyFieldId::ConnectedMaximumThickness
                            | PropertyFieldId::RegionMinimumFill
                            | PropertyFieldId::RegionMaximumFill
                    ) {
                        let bounds = value
                            .descriptor
                            .bounds
                            .expect("coverage endpoints have bounds");
                        assert_eq!(bounds.minimum, Some(0.0));
                        assert!(bounds.minimum_inclusive);
                        assert_eq!(bounds.maximum, Some(1.0));
                        assert!(bounds.maximum_inclusive);
                        assert!(help.contains("requested geometric coverage"));
                        assert!(help.contains("automatically measures and saves"));
                        assert!(help.contains("saturates at the nearest reachable value"));
                        assert!(help.contains("paint Alpha and channel Opacity"));
                    }
                    if value.descriptor.field == PropertyFieldId::MarkSizeScale {
                        assert_eq!(
                            inspector_field_label(value.descriptor.field),
                            "Uniform mark size scale"
                        );
                        let bounds = value.descriptor.bounds.expect("Uniform scale has bounds");
                        assert_eq!(bounds.minimum, Some(0.0));
                        assert!(bounds.minimum_inclusive);
                        assert_eq!(bounds.maximum, Some(2.0));
                        assert!(bounds.maximum_inclusive);
                    }
                }
                let response_descriptor = all_values
                    .iter()
                    .find(|value| value.descriptor.field == response_field)
                    .map(|value| &value.descriptor)
                    .expect("the active output keeps its response descriptor");
                assert!(
                    !inspector_field_detail(response_descriptor)
                        .trim()
                        .is_empty()
                );
                assert!(
                    advanced
                        .iter()
                        .any(|value| value.descriptor.field == response_field)
                );
                assert!(
                    !inline
                        .iter()
                        .any(|value| { advanced_channel_response_field(value.descriptor.field) })
                );
                assert!(
                    !wizard
                        .iter()
                        .any(|value| { advanced_channel_response_field(value.descriptor.field) })
                );
                assert!(
                    !advanced
                        .iter()
                        .any(|value| value.descriptor.field == PropertyFieldId::CurveResponseBias)
                );

                if matches!(
                    preset,
                    "grid-voronoi-scale" | "source-weighted-dispersion-voronoi"
                ) {
                    assert!(advanced.iter().any(|value| {
                        value.descriptor.field == PropertyFieldId::RegionSampling
                    }));
                    assert!(wizard.iter().any(|value| {
                        value.descriptor.field == PropertyFieldId::RegionResizeAlgorithm
                    }));
                    assert!(!advanced.iter().any(|value| {
                        value.descriptor.field == PropertyFieldId::RegionResizeAlgorithm
                    }));
                }

                let weighting = advanced
                    .iter()
                    .filter(|value| {
                        source_consumer_group(value.descriptor.field) == Some("Source weighting")
                    })
                    .collect::<Vec<_>>();
                assert!(!weighting.is_empty());

                for &field in toniator_domain::ANIMATABLE_SCALAR_FIELD_IDS {
                    if !advanced_batches::shown_in_advanced(field)
                        || document.channel_scalar_batch(field).is_err()
                    {
                        continue;
                    }
                    let descriptor = all_values
                        .iter()
                        .find(|value| value.descriptor.field == field)
                        .map(|value| &value.descriptor)
                        .expect("an available scalar batch has a current descriptor");
                    let help = advanced_field_help(field, Some(descriptor));
                    assert!(help.starts_with(inspector_field_guidance(field)));
                }
            }
        }
    }
}

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

/// Shows range or scale controls from the effective Mark density mode and structural eligibility.
///
/// The single-output weighted Pattern exercises the current UI projection; a named Variable edit
/// switches from scale to ranges, while an ordinary Mark Pattern keeps range controls only.
///
/// # Panics
/// Panics if a current single Mark fixture cannot be materialized or its applicable response
/// controls do not follow domain-owned mode and Pattern structure.
#[test]
fn ton018_mark_density_applicability_projects_scale_and_variable_ranges() {
    let mut workspace = workspace();
    let channel = authoritative_channel_ids(workspace.document())[0];
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
        .expect("weighted dispersion supports Mark output");
    apply_wizard_recipe(&mut workspace.history, InspectorTarget::DocumentAll, recipe)
        .expect("weighted Mark structure applies");
    let effective = workspace
        .document()
        .effective_channel_pattern(channel)
        .unwrap();
    assert_eq!(effective.output_settings.len(), 1);
    let output = effective.output_settings[0].output_layer_id;
    assert!(channel_mark_scale_visible(workspace.document(), channel));
    assert!(!channel_mark_range_visible(workspace.document(), channel));
    assert!(channel_fill_mask_inactive(workspace.document(), channel));
    assert!(
        advanced_settings_values(workspace.document(), InspectorTarget::Channel(channel))
            .iter()
            .any(|value| value.descriptor.field == PropertyFieldId::MarkSizeScale)
    );

    let command = workspace
        .document()
        .set_channel_mark_density_mode_for_effective(
            channel,
            output,
            toniator_domain::MarkDensityMode::Variable,
        )
        .unwrap();
    workspace.history.apply(&command).unwrap();
    assert!(!channel_mark_scale_visible(workspace.document(), channel));
    assert!(channel_mark_range_visible(workspace.document(), channel));
    assert!(!channel_fill_mask_inactive(workspace.document(), channel));

    let command = workspace
        .document()
        .set_channel_mark_density_mode_for_effective(
            channel,
            output,
            toniator_domain::MarkDensityMode::Uniform,
        )
        .unwrap();
    workspace.history.apply(&command).unwrap();
    workspace
        .history
        .apply(&DocumentCommand::SetModeledMappingField {
            channel_id: channel,
            edit: toniator_domain::ModeledMappingFieldEdit::Cutoff(0.25),
        })
        .unwrap();
    assert!(!channel_fill_mask_inactive(workspace.document(), channel));

    PresetRegistry::bundled()
        .apply_to_selected(&mut workspace.history, channel, "round-spiral-marks")
        .unwrap();
    assert!(weighted_mark_density_states(workspace.document(), channel).is_empty());
    assert!(!channel_mark_scale_visible(workspace.document(), channel));
    assert!(channel_mark_range_visible(workspace.document(), channel));
}

/// Writes a current-schema vector project with one weighted Mark output and strict visible margin.
///
/// The ignored test is an opt-in private-GTK fixture generator. It uses the canonical recipe and
/// project-container APIs, writes only under `target/validation/`, and leaves the immutable vector
/// source untouched.
///
/// # Panics
/// Panics if the source, current recipe, strict-margin eligibility, or project roundtrip fails.
#[test]
#[ignore = "writes a stage-owned strict-margin vector fixture for private GTK verification"]
fn ton018_write_strict_margin_vector_ui_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let source = root.join("assets/vector-sample.svg");
    let mut workspace = load_workspace(&source).expect("immutable vector source opens");
    let mut recipe = toniator_domain::PatternDefinitionRecipe::starter_for_family(
        toniator_domain::PatternRecipeFamilyKind::Dispersion,
    );
    let toniator_domain::PatternStructureRecipe::RandomSites {
        density_modulation,
        exclusion,
        ..
    } = &mut recipe.structure
    else {
        panic!("the dispersion starter uses random sites");
    };
    *density_modulation = toniator_domain::SiteDensityModulation::ArtworkWeighted;
    *exclusion = toniator_domain::SiteExclusionPolicy::VisibleMarkMargin { margin: 0.0 };
    let recipe = recipe
        .with_output_kind(0, toniator_domain::PatternRecipeOutputKind::Marks)
        .expect("the sole dispersion output is one Mark output");
    apply_wizard_recipe(&mut workspace.history, InspectorTarget::DocumentAll, recipe)
        .expect("strict-margin weighted Marks apply to the private document");

    let channel = authoritative_channel_ids(workspace.document())[0];
    let effective = workspace
        .document()
        .effective_channel_pattern(channel)
        .expect("current document resolves the strict-margin pattern");
    assert_eq!(effective.output_settings.len(), 1);
    let output = effective.output_settings[0].output_layer_id;
    let density = workspace
        .document()
        .effective_mark_density(channel, output)
        .expect("the sole Mark output resolves its effective sizing mode");
    assert_eq!(density.mode, toniator_domain::MarkDensityMode::Variable);
    assert!(density.uniform_unavailable_reason.is_some());

    let destination =
        root.join("target/validation/ton018-final-ui-20261009/strict-margin-vector.toniator");
    fs::create_dir_all(destination.parent().expect("fixture path has a parent"))
        .expect("stage validation directory exists");
    save_container(&destination, workspace.document(), &workspace.sources)
        .expect("strict-margin vector project saves in current schema");
    let reopened = load_workspace(&destination).expect("strict-margin vector fixture reloads");
    assert_eq!(reopened.document(), workspace.document());
}

/// Keeps same-value ALL Start input editable until the stored Appearance baseline records it.
///
/// The test follows the main inspector's equality guard through an unchanged effective density,
/// an explicit ALL batch, and Undo. It also confirms End, named-channel, and non-baseline scalar
/// authority remains outside the Start Appearance baseline rule, and focus leave requires a text
/// change only for baseline-backed ALL Start entries.
///
/// # Panics
/// Panics if default document descriptors, scalar batches, baseline publication, or history
/// boundaries no longer satisfy the main inspector's same-value policy.
#[test]
fn ton016_main_same_value_guard_requires_recorded_start_baseline() {
    let document = Document::new_default_document(
        toniator_domain::CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        toniator_domain::SourceReference::Unassigned,
    )
    .expect("default document validates");
    let all_values = inline_inspector_values(&document, InspectorTarget::DocumentAll);
    let density = all_values
        .iter()
        .find(|value| value.descriptor.field == PropertyFieldId::Density)
        .expect("ALL inspector exposes density");
    assert!(main_all_scalar(
        InspectorTarget::DocumentAll,
        &density.descriptor
    ));
    let batch = document
        .channel_scalar_batch(PropertyFieldId::Density)
        .expect("density has compatible channel values");
    let next = batch.values[0].value;
    assert!(batch.values.iter().all(|value| value.value == next));
    for field in [
        PropertyFieldId::Density,
        PropertyFieldId::DensityAspect,
        PropertyFieldId::RotationDegrees,
        PropertyFieldId::TranslationX,
        PropertyFieldId::TranslationY,
    ] {
        assert!(main_start_appearance_baseline_field(field));
    }
    assert!(!main_all_scalar_same_value_intent_is_recorded(
        temporal_preview::Endpoint::Start,
        &document,
        PropertyFieldId::Density,
        next,
    ));
    let edited_since_construction = Cell::new(false);
    assert!(!main_numeric_focus_leave_should_commit(
        true,
        &edited_since_construction,
    ));
    assert!(main_numeric_focus_leave_should_commit(
        false,
        &edited_since_construction,
    ));
    edited_since_construction.set(true);
    assert!(main_numeric_focus_leave_should_commit(
        true,
        &edited_since_construction,
    ));
    assert!(!edited_since_construction.get());
    assert!(main_all_scalar_same_value_intent_is_recorded(
        temporal_preview::Endpoint::End,
        &document,
        PropertyFieldId::Density,
        next,
    ));

    let nonbaseline = all_values
        .iter()
        .find(|value| {
            main_all_scalar(InspectorTarget::DocumentAll, &value.descriptor)
                && !matches!(
                    value.descriptor.field,
                    PropertyFieldId::Density
                        | PropertyFieldId::DensityAspect
                        | PropertyFieldId::RotationDegrees
                        | PropertyFieldId::TranslationX
                        | PropertyFieldId::TranslationY
                )
        })
        .expect("ALL inspector exposes a non-baseline scalar");
    assert!(!main_start_appearance_baseline_field(
        nonbaseline.descriptor.field
    ));
    let PropertyCurrentValueKind::FiniteF64(nonbaseline_value) = &nonbaseline.value else {
        panic!("non-baseline ALL scalar has a finite value");
    };
    assert!(main_all_scalar_same_value_intent_is_recorded(
        temporal_preview::Endpoint::Start,
        &document,
        nonbaseline.descriptor.field,
        *nonbaseline_value,
    ));
    let channel = document
        .channel_topology()
        .expect("default document has channels")
        .channels()[0]
        .id;
    let named_density = inline_inspector_values(&document, InspectorTarget::Channel(channel))
        .into_iter()
        .find(|value| value.descriptor.field == PropertyFieldId::Density)
        .expect("named inspector exposes density");
    assert!(!main_all_scalar(
        InspectorTarget::Channel(channel),
        &named_density.descriptor
    ));

    let mut history = DocumentHistory::new(
        DocumentSession::new(document.clone()).expect("initial session validates"),
    );
    assert!(
        advanced_batches::apply(
            &mut history,
            temporal_preview::Endpoint::Start,
            PropertyFieldId::Density,
            next,
        )
        .expect("same-value ALL assignment publishes baseline intent")
    );
    assert!(history.can_undo());
    assert!(main_all_scalar_same_value_intent_is_recorded(
        temporal_preview::Endpoint::Start,
        history.document(),
        PropertyFieldId::Density,
        next,
    ));
    assert!(!history.can_redo());
    history.undo().expect("baseline intent remains undoable");
    assert_eq!(history.document(), &document);
    assert!(
        !history.can_undo(),
        "one Undo removes the only baseline edit"
    );
    assert!(history.can_redo());
    assert!(!main_all_scalar_same_value_intent_is_recorded(
        temporal_preview::Endpoint::Start,
        history.document(),
        PropertyFieldId::Density,
        next,
    ));
}

/// Keeps layout controls in Pattern and exposes each compatible response target in Advanced.
/// Uses one current Guides Mark output so the check does not depend on the workspace's default
/// construction topology or combine Marks with another drawn output.
///
/// # Panics
/// Panics if source and geometry homes overlap, fields disappear, or GTK fabricates batch targets.
#[test]
fn pattern_layout_and_advanced_response_keep_channel_output_authority() {
    let mut workspace = workspace();
    let recipe = toniator_domain::PatternDefinitionRecipe::starter_for_family(
        toniator_domain::PatternRecipeFamilyKind::Guides,
    );
    apply_wizard_recipe(&mut workspace.history, InspectorTarget::DocumentAll, recipe).unwrap();
    let document = workspace.document();
    let all = inline_inspector_values(document, InspectorTarget::DocumentAll);
    for field in [PropertyFieldId::TranslationX, PropertyFieldId::TranslationY] {
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
    assert!(!all.iter().any(|row| {
        matches!(
            row.descriptor.field,
            PropertyFieldId::MarkMinimumFill | PropertyFieldId::MarkMaximumFill
        )
    }));
    for channel in authoritative_channel_ids(document) {
        let named = inline_inspector_values(document, InspectorTarget::Channel(channel));
        let advanced = advanced_settings_values(document, InspectorTarget::Channel(channel));
        let outputs = document
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings;
        assert_eq!(outputs.len(), 1, "the fixture uses one drawing output");
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
                assert_eq!(rows, 0, "fill response stays in Advanced");
                let advanced_rows = advanced
                    .iter()
                    .filter(|row| {
                        row.descriptor.field == field
                            && row.descriptor.target
                                == PropertyTarget::ChannelOutput(channel, output.output_layer_id)
                    })
                    .count();
                assert_eq!(advanced_rows, 1, "each active output has one Advanced row");
            }
        }
        assert!(
            advanced
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
        "Maximum coverage"
    );
    assert_eq!(
        main_inspector_section(PropertyFieldId::TranslationX),
        main_inspector_section(PropertyFieldId::RotationDegrees)
    );
}

/// Groups All-channel coverage batches by their current capability-derived output kind.
///
/// Three separate channels may use Marks, Lines, and Regions in the same All scope. The
/// aggregate rows must retain those distinctions without building a mixed-output Pattern.
///
/// # Panics
/// Panics if a current response batch loses its channel-output capability or the All scope
/// stops exposing all three valid output kinds.
#[test]
fn all_response_batches_group_different_output_kinds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut workspace = load_workspace(&root.join("assets/vector-sample.svg")).unwrap();
    let registry = PresetRegistry::bundled();
    let catalog = LayeredPresetCatalog::new(&registry, Vec::new()).unwrap();
    let channels = authoritative_channel_ids(workspace.document());
    let presets = [
        "round-spiral-marks",
        "one-guide-lines",
        "grid-voronoi-scale",
    ];
    for (channel, preset) in channels.iter().copied().zip(presets) {
        apply_catalog_pattern_candidate(
            &mut workspace.history,
            &catalog,
            InspectorTarget::Channel(channel),
            preset,
        )
        .unwrap();
    }

    let document = workspace.document();
    let mut groups = BTreeSet::new();
    for (minimum, maximum, expected) in [
        (
            PropertyFieldId::MarkMinimumFill,
            PropertyFieldId::MarkMaximumFill,
            "Marks",
        ),
        (
            PropertyFieldId::ConnectedMinimumThickness,
            PropertyFieldId::ConnectedMaximumThickness,
            "Lines",
        ),
        (
            PropertyFieldId::RegionMinimumFill,
            PropertyFieldId::RegionMaximumFill,
            "Regions",
        ),
    ] {
        for field in [minimum, maximum] {
            let batch = document.channel_scalar_batch(field).unwrap();
            assert_eq!(
                advanced_batches::response_batch_output_group(document, &batch),
                Some(expected)
            );
        }
        groups.insert(expected);
    }
    assert_eq!(groups, BTreeSet::from(["Lines", "Marks", "Regions"]));
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
        let descriptor = if field == PropertyFieldId::MarkMaximumFill {
            let first_channel = authoritative_channel_ids(workspace.document())[0];
            advanced_settings_values(
                workspace.document(),
                InspectorTarget::Channel(first_channel),
            )
            .into_iter()
            .find(|row| row.descriptor.field == field)
            .expect("Advanced exposes the representative mark response field")
            .descriptor
        } else {
            inline_inspector_values(workspace.document(), InspectorTarget::DocumentAll)
                .into_iter()
                .find(|row| row.descriptor.field == field)
                .expect("ALL exposes the representative structural field")
                .descriptor
        };
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
        let rendered_end_frame = candidate.materialize_frame(end).unwrap();
        let rendered_end = rendered_end_frame.document();
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
                    prepare_for_print: false,
                    pixels_per_metre: None,
                    cleanup: crate::print_cleanup::Settings::default(),
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

/// Exercises static ALL/mixed sizing, private Cancel, atomic Apply/history and model restoration.
/// Uses current saved weighted-even witnesses read-only; no rotation or animation policy is tested.
///
/// # Panics
/// Panics on changed source authority, nonempty fitting plans, incorrect controls or failed previews.
#[test]
#[ignore = "requires the private Wayland static-completion harness"]
fn gtk_static_advanced_and_saved_seed_reuse() {
    use crate::wizard_workflow_tests::{
        button_named, capture, dropdown, render_current, reuse_both_sources, save_exact, select,
        wait_until,
    };
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.StaticRegression")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    state.borrow().window.present();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["raster", "vector"] {
        let path = root.join(format!("target/validation/ton018-config16-final-20261011/weighted-uniform-witnesses/weighted-even-{name}.toniator"));
        let mut workspace = load_workspace(&path).unwrap();
        let channels = authoritative_channel_ids(workspace.document());
        assert!(
            calibration_preparation::plan_to_prepare(workspace.document(), &channels, false, false)
                .unwrap()
                .is_empty()
        );
        let channel = channels[0];
        let output = workspace
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        // A real named override makes ALL mixed before the native modal opens.
        advanced_batches::apply_all_mark_density_mode(
            &mut workspace.history,
            toniator_domain::MarkDensityMode::Uniform,
        )
        .unwrap();
        let command = workspace
            .document()
            .set_channel_mark_density_mode_for_effective(
                channel,
                output,
                toniator_domain::MarkDensityMode::Variable,
            )
            .unwrap();
        workspace.history.apply(&command).unwrap();
        let initial = workspace.snapshot();
        let revision = workspace.history.revision();
        install_workspace(&state, workspace);
        state.borrow_mut().inspector_runtime.target = InspectorTarget::DocumentAll;
        render_current(&state);
        open_advanced_settings(&state);
        wait_until("Advanced ALL controls", 30, || {
            state
                .borrow()
                .advanced_settings
                .as_ref()
                .is_some_and(|surface| {
                    dropdown(surface.fields.upcast_ref(), "Mark sizing").is_some()
                })
        });
        let fields = state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .fields
            .clone();
        let control = dropdown(fields.upcast_ref(), "Mark sizing").unwrap();
        assert!(control.is_sensitive());
        assert_eq!(
            control
                .selected_item()
                .and_downcast::<gtk::StringObject>()
                .unwrap()
                .string(),
            "Mixed"
        );
        control.grab_focus();
        capture(&format!("advanced-{name}-mixed"));
        select(fields.upcast_ref(), "Mark sizing", "Variable");
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().snapshot(),
            initial
        );
        let window = state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .window
            .clone();
        button_named(window.upcast_ref(), "Cancel")
            .unwrap()
            .emit_clicked();
        assert!(state.borrow().advanced_settings.is_none());
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().snapshot(),
            initial
        );
        assert_eq!(
            state
                .borrow()
                .workspace
                .as_ref()
                .unwrap()
                .history
                .revision(),
            revision
        );
        open_advanced_settings(&state);
        wait_until("Advanced ALL controls reopen", 30, || {
            state
                .borrow()
                .advanced_settings
                .as_ref()
                .is_some_and(|surface| {
                    dropdown(surface.fields.upcast_ref(), "Mark sizing").is_some()
                })
        });
        let fields = state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .fields
            .clone();
        select(fields.upcast_ref(), "Mark sizing", "Uniform");
        {
            let app = state.borrow();
            let draft = app.advanced_settings.as_ref().unwrap().draft.borrow();
            for channel in authoritative_channel_ids(draft.document()) {
                assert!(channel_mark_scale_visible(draft.document(), channel));
                assert!(!channel_mark_range_visible(draft.document(), channel));
            }
            assert!(
                draft
                    .document()
                    .channel_response_overrides()
                    .iter()
                    .all(|entry| !matches!(
                        &entry.response,
                        toniator_domain::ChannelOutputResponse::Marks {
                            density_mode: Some(_),
                            ..
                        }
                    ))
            );
        }
        capture(&format!("advanced-{name}-uniform"));
        select(fields.upcast_ref(), "Mark sizing", "Variable");
        let applied = state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .draft
            .borrow()
            .document()
            .clone();
        for channel in authoritative_channel_ids(&applied) {
            assert!(!channel_mark_scale_visible(&applied, channel));
            assert!(channel_mark_range_visible(&applied, channel));
        }
        assert!(
            calibration_preparation::plan_to_prepare(
                &applied,
                &authoritative_channel_ids(&applied),
                false,
                false
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().snapshot(),
            initial
        );
        let apply = state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .apply
            .clone();
        assert!(apply.is_sensitive());
        apply.emit_clicked();
        assert!(state.borrow().advanced_settings.is_none());
        assert_eq!(
            state
                .borrow()
                .workspace
                .as_ref()
                .unwrap()
                .history
                .revision()
                .0,
            revision.0 + 1
        );
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &applied
        );
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().sources,
            initial.sources
        );
        render_current(&state);
        capture(&format!("advanced-{name}-applied"));
        apply_history_navigation(&state, false);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().snapshot(),
            initial
        );
        render_current(&state);
        capture(&format!("advanced-{name}-undo"));
        apply_history_navigation(&state, true);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &applied
        );
        render_current(&state);
        // Native Color model selection uses the production notify/command route.
        let selector = state.borrow().selector.clone();
        let next = if applied.channel_topology().unwrap().channels().len() == 3 {
            PreviewModel::Cmyk
        } else {
            PreviewModel::Rgb
        };
        selector.set_selected(
            PreviewModel::ALL
                .iter()
                .position(|value| *value == next)
                .unwrap() as u32,
        );
        let switched = state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .document()
            .clone();
        assert_ne!(switched.channel_topology(), applied.channel_topology());
        assert_eq!(switched.pattern_settings(), applied.pattern_settings());
        assert_eq!(switched.source(), applied.source());
        render_current(&state);
        capture(&format!("advanced-{name}-model-switched"));
        apply_history_navigation(&state, false);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &applied
        );
        render_current(&state);
        capture(&format!("advanced-{name}-model-restored"));
        apply_history_navigation(&state, true);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &switched
        );
        render_current(&state);
        save_exact(
            state.borrow().workspace.as_ref().unwrap(),
            &format!("advanced-{name}-final.toniator"),
        );
        eprintln!(
            "Advanced {name}: mixed/Uniform/Variable, private Cancel, atomic Apply, Undo/Redo and model restoration pass with zero new fits"
        );
    }
    install_workspace(
        &state,
        load_workspace(&root.join("assets/vector-sample.svg")).unwrap(),
    );
    state.borrow_mut().inspector_runtime.target = InspectorTarget::DocumentAll;
    sync_ui(&mut state.borrow_mut());
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    let epoch = state.borrow().pattern_wizard.as_ref().unwrap().epoch;
    select_wizard_preset_candidate(
        &state,
        epoch,
        Some("clustered-dispersion-random-links".to_owned()),
    );
    wait_until("saved Clustered gallery Apply", 60, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .apply
            .is_sensitive()
    });
    capture("clustered-saved-gallery");
    let apply = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .apply
        .clone();
    apply.emit_clicked();
    wait_until("saved Clustered gallery closes", 30, || {
        state.borrow().pattern_wizard.is_none()
    });
    let before = state
        .borrow()
        .workspace
        .as_ref()
        .unwrap()
        .document()
        .clone();
    assert!(
        calibration_preparation::plan_to_prepare(
            &before,
            &authoritative_channel_ids(&before),
            false,
            false
        )
        .unwrap()
        .is_empty()
    );
    render_current(&state);
    capture("clustered-saved-applied");
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    let edit = state.borrow().pattern_wizard.as_ref().unwrap().edit.clone();
    edit.emit_clicked();
    let next = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .review
        .clone();
    next.emit_clicked();
    let seed = state.borrow().pattern_wizard.as_ref().unwrap().text_inputs.iter().find_map(|input| matches!(&input.kind, WizardTextInputKind::DescriptorU32(descriptor) if descriptor.field == PropertyFieldId::RandomSeed).then(|| input.entry.clone())).unwrap();
    seed.set_text("12345");
    seed.emit_activate();
    let varied = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .draft
        .borrow()
        .document()
        .clone();
    assert_ne!(
        varied.pattern_definition_bundles(),
        before.pattern_definition_bundles()
    );
    assert_eq!(
        varied
            .saved_calibration_for(authoritative_channel_ids(&varied)[0])
            .unwrap(),
        before
            .saved_calibration_for(authoritative_channel_ids(&before)[0])
            .unwrap()
    );
    assert_eq!(
        varied.baseline_saved_calibration().unwrap(),
        before.baseline_saved_calibration().unwrap()
    );
    assert!(
        calibration_preparation::plan_to_prepare(
            &varied,
            &authoritative_channel_ids(&varied),
            false,
            false
        )
        .unwrap()
        .is_empty()
    );
    capture("clustered-seed-varied-private");
    let seed_revision = state
        .borrow()
        .workspace
        .as_ref()
        .unwrap()
        .history
        .revision();
    for _ in 0..2 {
        let next = state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .review
            .clone();
        assert!(next.is_sensitive());
        next.emit_clicked();
    }
    wait_until("saved seed Review", 60, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .apply
            .is_sensitive()
    });
    let apply = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .apply
        .clone();
    apply.emit_clicked();
    assert!(
        state.borrow().pattern_wizard.is_none(),
        "saved seed Apply needs no calibration worker"
    );
    assert_eq!(
        state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .history
            .revision()
            .0,
        seed_revision.0 + 1
    );
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &varied
    );
    render_current(&state);
    capture("clustered-seed-applied");
    save_exact(
        state.borrow().workspace.as_ref().unwrap(),
        "clustered-seed-applied.toniator",
    );
    reuse_both_sources(&state, &varied, "clustered-seed-reuse");
    let window = state.borrow().window.clone();
    window.close();
}
