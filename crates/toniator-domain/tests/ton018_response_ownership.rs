use toniator_domain::{
    CanvasSpec, ChannelId, ChannelOutputResponse, ChannelTopologyTemplate, Document,
    DocumentCommand, DocumentHistory, DocumentResponseFieldEdit, DocumentResponseSettings,
    DocumentSession, Easing, FrameRange, FrameRate, HalftoneChannelModel, MarkGeometryResponse,
    PatternCapabilityScope, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternRecipeConstructionKind, ProjectTiming, PropertyFieldId, PropertyInheritance,
    PropertyTarget, RegionSamplingStrategy, ScalarEndOverride, SourceReference,
    TemporalEndOverride,
};

/// Starts a current single-output RGB document with no authored response override.
///
/// # Panics
/// Panics if the canonical document cannot validate.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("canonical document validates")
}

/// Returns the selected channel's sole effective response and its structural output identity.
///
/// # Panics
/// Panics if the current single-output contract is broken.
fn output(
    document: &Document,
    channel: ChannelId,
) -> (
    toniator_domain::PatternOutputLayerId,
    PatternGeometryResponse,
) {
    let effective = document
        .effective_channel_pattern(channel)
        .expect("channel resolves");
    let only = effective.output_settings.first().expect("one output");
    (only.output_layer_id, only.response.clone())
}

/// Returns one output field's authoritative inheritance state.
///
/// # Panics
/// Panics if the selected output does not expose the requested response field.
fn response_inheritance(
    document: &Document,
    channel: ChannelId,
    output_layer_id: toniator_domain::PatternOutputLayerId,
    field: PropertyFieldId,
) -> PropertyInheritance {
    document
        .property_values()
        .into_iter()
        .find(|value| {
            value.descriptor.target == PropertyTarget::ChannelOutput(channel, output_layer_id)
                && value.descriptor.field == field
        })
        .expect("selected output exposes response field")
        .inheritance
}

/// Replaces the ALL Pattern using one current recipe while retaining the history boundary.
///
/// # Panics
/// Panics if recipe reconstruction or the current replacement command fails.
fn replace_all(history: &mut DocumentHistory, recipe: PatternDefinitionRecipe) {
    let document = history.document();
    let base = document.pattern_settings().clone();
    let definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base exists")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition: definition,
            recipe,
        })
        .expect("replacement applies");
}

/// Proves ALL intent and named response stay outside a compatible Pattern replacement.
///
/// # Panics
/// Panics if a valid response command or replacement cannot publish.
#[test]
fn all_and_named_ranges_survive_compatible_replacement_and_undo() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let mut settings = history.document().response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 0.2,
        maximum_fill: 0.8,
    };
    history
        .apply(&DocumentCommand::SetAllResponseSettings {
            base: history.document().response_settings().clone(),
            settings,
        })
        .expect("ALL response applies");
    let (old_id, _) = output(history.document(), ChannelId(1));
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id: old_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.35),
                maximum_fill: Some(0.7),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("named response applies");
    let before = history.document().clone();
    let recipe = before
        .reconstruct_pattern_definition_recipe(before.pattern_settings().definition_id)
        .expect("current recipe reconstructs");
    replace_all(&mut history, recipe);
    let (new_id, r) = output(history.document(), ChannelId(1));
    assert_ne!(new_id, old_id);
    assert_eq!(
        r,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.35,
            maximum_fill: 0.7
        })
    );
    assert_eq!(
        output(history.document(), ChannelId(2)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.8
        })
    );
    assert_eq!(
        history.document().channel_response_overrides()[0].output_layer_id,
        new_id
    );
    assert_eq!(
        history.document().response_settings().marks.minimum_fill,
        0.2
    );
    history.undo().expect("replacement undoes");
    assert_eq!(history.document(), &before);
    history.redo().expect("replacement redoes");
    assert_eq!(output(history.document(), ChannelId(1)).0, new_id);
}

/// Proves changing output kind defaults the new range and forgets dormant old intent.
///
/// # Panics
/// Panics if the current recipe cannot change between marks and connections.
#[test]
fn kind_change_defaults_then_return_defaults() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let mut settings: DocumentResponseSettings = history.document().response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 0.2,
        maximum_fill: 0.8,
    };
    history
        .apply(&DocumentCommand::SetAllResponseSettings {
            base: history.document().response_settings().clone(),
            settings,
        })
        .expect("ALL marks apply");
    let marks = history
        .document()
        .reconstruct_pattern_definition_recipe(history.document().pattern_settings().definition_id)
        .expect("marks recipe");
    let lines = marks
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("compatible line construction");
    replace_all(&mut history, lines);
    assert!(matches!(output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Connected(value) if value.minimum_thickness == 0.0 && value.maximum_thickness == 1.0));
    assert_eq!(
        history.document().response_settings(),
        &DocumentResponseSettings::default()
    );
    replace_all(&mut history, marks);
    assert_eq!(
        output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 1.0
        })
    );
}

/// Proves fixed authored mark size and region sampling remain response authority.
///
/// # Panics
/// Panics if valid fixed endpoints or a region recipe cannot publish.
#[test]
fn fixed_mark_range_and_canonical_region_sampling_are_independent_of_pattern() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let mut settings = history.document().response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 1.0,
        maximum_fill: 1.0,
    };
    history
        .apply(&DocumentCommand::SetAllResponseSettings {
            base: history.document().response_settings().clone(),
            settings,
        })
        .expect("fixed range applies");
    assert_eq!(
        output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 1.0,
            maximum_fill: 1.0
        })
    );
    let recipe = history
        .document()
        .reconstruct_pattern_definition_recipe(history.document().pattern_settings().definition_id)
        .expect("recipe");
    let regions = recipe
        .with_construction_kind(0, PatternRecipeConstructionKind::Regions)
        .expect("region construction");
    replace_all(&mut history, regions);
    assert!(matches!(output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Regions(value) if value.minimum_fill == 0.0
            && value.maximum_fill == 1.0 && value.sampling == RegionSamplingStrategy::AreaAverage));
    assert_eq!(
        history.document().response_settings().region_sampling,
        RegionSamplingStrategy::AreaAverage
    );
    let pattern_controls = history
        .document()
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("Pattern capabilities resolve")
        .active_controls;
    assert!(!pattern_controls.iter().any(|descriptor| matches!(
        descriptor.field,
        PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::RegionMinimumFill
            | PropertyFieldId::RegionMaximumFill
            | PropertyFieldId::RegionSampling
    )));
}

/// Proves named kind changes default only that channel while ALL intent remains available elsewhere.
///
/// # Panics
/// Panics if valid selected replacement or response edits cannot apply.
#[test]
fn selected_kind_change_defaults_only_selected_channel() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let mut settings = history.document().response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 0.2,
        maximum_fill: 0.8,
    };
    history
        .apply(&DocumentCommand::SetAllResponseSettings {
            base: history.document().response_settings().clone(),
            settings,
        })
        .expect("ALL marks apply");
    let (mark_id, _) = output(history.document(), ChannelId(1));
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id: mark_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.3),
                maximum_fill: Some(0.7),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("named marks apply");
    let base = history.document().pattern_settings().clone();
    let definition = history.document().pattern_definition_bundles()[0]
        .definition
        .clone();
    let marks = history
        .document()
        .reconstruct_pattern_definition_recipe(base.definition_id)
        .expect("marks recipe");
    let lines = marks
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("line recipe");
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: base.clone(),
                channel_id: ChannelId(1),
                base_definition: definition.clone(),
                recipe: lines,
            },
        )
        .expect("selected lines apply");
    assert!(matches!(output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Connected(value) if value.minimum_thickness == 0.0 && value.maximum_thickness == 1.0));
    assert_eq!(
        output(history.document(), ChannelId(2)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.8
        })
    );
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                channel_id: ChannelId(1),
                base_definition: definition,
                recipe: marks,
            },
        )
        .expect("selected marks return");
    assert_eq!(
        output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 1.0
        })
    );
    assert_eq!(
        history.document().response_settings().marks.minimum_fill,
        0.2
    );
}

/// Proves a compatible replacement rebinds End to the new output and retains its easing.
///
/// # Panics
/// Panics if valid project timing, End, or Pattern replacement cannot apply.
#[test]
fn compatible_replacement_preserves_end_value_easing_and_frame_output() {
    let mut doc = document();
    let mut settings = doc.response_settings().clone();
    settings.marks = MarkGeometryResponse {
        minimum_fill: 0.2,
        maximum_fill: 0.8,
    };
    doc = doc
        .apply_command(&DocumentCommand::SetAllResponseSettings {
            base: doc.response_settings().clone(),
            settings,
        })
        .expect("ALL response applies")
        .0;
    let (old_id, _) = output(&doc, ChannelId(1));
    let end = TemporalEndOverride::Scalar(ScalarEndOverride {
        target: PropertyTarget::ChannelOutput(ChannelId(1), old_id),
        field: PropertyFieldId::MarkMinimumFill,
        end: 0.4,
        easing: Easing::SmoothStep,
    });
    doc = doc
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(1, 1).expect("rate"),
                FrameRange::new(0, 3).expect("range"),
            ),
            vec![end],
        )
        .expect("End validates");
    let before_end = output(&doc.materialize_frame(2).expect("End frame"), ChannelId(1)).1;
    let mut history = DocumentHistory::new(DocumentSession::new(doc).expect("session validates"));
    let recipe = history
        .document()
        .reconstruct_pattern_definition_recipe(history.document().pattern_settings().definition_id)
        .expect("recipe");
    replace_all(&mut history, recipe);
    let (new_id, _) = output(history.document(), ChannelId(1));
    assert_ne!(new_id, old_id);
    assert!(history.document().temporal_end_overrides().iter().any(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::ChannelOutput(ChannelId(1), new_id)
            && value.field == PropertyFieldId::MarkMinimumFill && value.end == 0.4
            && value.easing == Easing::SmoothStep)));
    assert_eq!(
        output(
            &history
                .document()
                .materialize_frame(2)
                .expect("rebound End frame"),
            ChannelId(1)
        )
        .1,
        before_end
    );
}

/// Proves an explicit ALL field edit equalizes that field even when its base value is unchanged.
/// Other named response fields remain authored and history restores the exact prior state.
///
/// # Panics
/// Panics if valid response commands or reversible history fail.
#[test]
fn all_field_equalizes_only_the_selected_named_field() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let (output_layer_id, _) = output(history.document(), ChannelId(1));
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.2),
                maximum_fill: Some(0.6),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("named response applies");
    let before = history.document().clone();
    history
        .apply(&DocumentCommand::SetAllResponseField {
            base: history.document().response_settings().clone(),
            edit: DocumentResponseFieldEdit::MarkMinimumFill(0.0),
        })
        .expect("unchanged ALL base still equalizes");
    assert_eq!(
        output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 0.6
        })
    );
    assert_eq!(history.document().channel_response_overrides().len(), 1);
    assert_eq!(
        history.document().channel_response_overrides()[0].response,
        ChannelOutputResponse::Marks {
            minimum_fill: None,
            maximum_fill: Some(0.6),
            density_mode: None,
            size_scale: None,
        }
    );
    history.undo().expect("ALL edit undoes");
    assert_eq!(history.document(), &before);
}

/// Proves ALL replacement reconciles each channel independently in a mixed-kind document.
/// Compatible named marks retain their range while an incompatible named line defaults.
///
/// # Panics
/// Panics if valid selected or ALL Pattern replacement cannot publish.
#[test]
fn mixed_kind_all_replacement_preserves_compatible_named_response() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let (old_mark_id, _) = output(history.document(), ChannelId(1));
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id: old_mark_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.2),
                maximum_fill: Some(0.8),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("named mark applies");
    let base = history.document().pattern_settings().clone();
    let definition = history.document().pattern_definition_bundles()[0]
        .definition
        .clone();
    let marks = history
        .document()
        .reconstruct_pattern_definition_recipe(base.definition_id)
        .expect("marks recipe");
    let lines = marks
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("line recipe");
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                channel_id: ChannelId(2),
                base_definition: definition,
                recipe: lines,
            },
        )
        .expect("named lines apply");
    replace_all(&mut history, marks);
    assert_eq!(
        output(history.document(), ChannelId(1)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.2,
            maximum_fill: 0.8
        })
    );
    assert_eq!(
        output(history.document(), ChannelId(2)).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.0,
            maximum_fill: 1.0
        })
    );
}

/// Proves compatible line replacement preserves thickness End but resets Pattern-owned alignment End.
///
/// # Panics
/// Panics if a current connected recipe, temporal target, or replacement cannot validate.
#[test]
fn connected_replacement_keeps_response_end_and_resets_alignment_end() {
    let mut doc = document();
    let marks = doc
        .reconstruct_pattern_definition_recipe(doc.pattern_settings().definition_id)
        .expect("marks recipe");
    let lines = marks
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("line recipe");
    let mut history = DocumentHistory::new(DocumentSession::new(doc).expect("session validates"));
    replace_all(&mut history, lines);
    doc = history.document().clone();
    let (old_id, _) = output(&doc, ChannelId(1));
    let timing = ProjectTiming::new(
        FrameRate::new(1, 1).expect("rate"),
        FrameRange::new(0, 3).expect("range"),
    );
    doc = doc
        .with_temporal_authority(
            timing,
            vec![
                TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: PropertyTarget::ChannelOutput(ChannelId(1), old_id),
                    field: PropertyFieldId::ConnectedMinimumThickness,
                    end: 0.3,
                    easing: Easing::Linear,
                }),
                TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: PropertyTarget::ChannelOutput(ChannelId(1), old_id),
                    field: PropertyFieldId::CurveResponseBias,
                    end: 0.2,
                    easing: Easing::SmoothStep,
                }),
            ],
        )
        .expect("End validates");
    let mut history = DocumentHistory::new(DocumentSession::new(doc).expect("session validates"));
    let lines = history
        .document()
        .reconstruct_pattern_definition_recipe(history.document().pattern_settings().definition_id)
        .expect("current lines recipe");
    replace_all(&mut history, lines);
    let (new_id, _) = output(history.document(), ChannelId(1));
    assert!(history.document().temporal_end_overrides().iter().any(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::ChannelOutput(ChannelId(1), new_id)
            && value.field == PropertyFieldId::ConnectedMinimumThickness && value.end == 0.3)));
    assert!(
        !history
            .document()
            .temporal_end_overrides()
            .iter()
            .any(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.field == PropertyFieldId::CurveResponseBias))
    );
}

/// Proves the ALL scalar batch publishes paired bounds atomically and model switches reuse its baseline.
/// A named partial field is equalized without deleting its unrelated bound.
///
/// # Panics
/// Panics if the current batch, configuration history, or canonical color model fails.
#[test]
fn all_batch_publishes_paired_response_and_model_switch_restores_it() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let (output_layer_id, _) = output(history.document(), ChannelId(1));
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: ChannelId(1),
            output_layer_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.2),
                maximum_fill: None,
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("named min applies");
    let before = history.document().clone();
    let configuration = before
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::MarkMinimumFill, 0.8),
            (PropertyFieldId::MarkMaximumFill, 0.9),
        ])
        .expect("paired response validates atomically");
    history
        .apply_document_configuration(&before, history.revision(), &configuration)
        .expect("ALL configuration publishes");
    assert_eq!(
        history.document().response_settings().marks,
        MarkGeometryResponse {
            minimum_fill: 0.8,
            maximum_fill: 0.9
        }
    );
    assert!(history.document().channel_response_overrides().is_empty());
    history.undo().expect("ALL batch undoes");
    assert_eq!(history.document(), &before);
    history.redo().expect("ALL batch redoes");

    let template = ChannelTopologyTemplate {
        pattern_instance: history
            .document()
            .channel_topology()
            .expect("RGB topology")
            .channels()[0]
            .pattern_instance
            .clone(),
    };
    let topology = history
        .document()
        .canonical_channel_topology_with_appearance_baseline(HalftoneChannelModel::Cmyk, template)
        .expect("CMYK topology validates");
    history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology,
        })
        .expect("model switch applies");
    let first_channel = history
        .document()
        .channel_topology()
        .expect("CMYK topology")
        .channels()[0]
        .id;
    assert_eq!(
        output(history.document(), first_channel).1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.8,
            maximum_fill: 0.9
        })
    );
}

/// Proves an ALL response batch rebases stored End deltas while retaining End geometry and easing.
///
/// # Panics
/// Panics if timing, paired Start publication, or final-frame materialization fails.
#[test]
fn all_response_batch_preserves_initialized_end_geometry() {
    let mut document = document();
    let (output_layer_id, _) = output(&document, ChannelId(1));
    document = document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(1, 1).expect("rate"),
                FrameRange::new(0, 3).expect("range"),
            ),
            vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                target: PropertyTarget::ChannelOutput(ChannelId(1), output_layer_id),
                field: PropertyFieldId::MarkMinimumFill,
                end: 0.4,
                easing: Easing::SmoothStep,
            })],
        )
        .expect("End validates");
    let before_end = output(
        &document.materialize_frame(2).expect("old End frame"),
        ChannelId(1),
    )
    .1;
    let configuration = document
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::MarkMinimumFill, 0.2),
            (PropertyFieldId::MarkMaximumFill, 0.8),
        ])
        .expect("paired Start validates");
    let changed = configuration.bind(&document).expect("configuration binds");
    let after_end = output(
        &changed.materialize_frame(2).expect("new End frame"),
        ChannelId(1),
    )
    .1;
    assert!(matches!((before_end, after_end),
        (PatternGeometryResponse::Marks(before), PatternGeometryResponse::Marks(after))
            if before.minimum_fill == after.minimum_fill && after.maximum_fill == 0.8));
    assert!(
        changed
            .temporal_end_overrides()
            .iter()
            .any(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.easing == Easing::SmoothStep
            && value.field == PropertyFieldId::MarkMinimumFill))
    );
}

/// Proves equal-to-baseline named members remain explicit while a partial reset preserves siblings and End intent.
/// The domain owns override membership, typed reset commands, and Start-to-End reconciliation.
///
/// # Panics
/// Panics if response fields lose their inherited/explicit state or a partial reset changes siblings.
#[test]
fn named_response_inheritance_and_field_reset_preserve_siblings() {
    let channel = ChannelId(1);
    let mut doc = document();
    let (output_layer_id, _) = output(&doc, channel);
    doc = doc
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(1, 1).expect("rate"),
                FrameRange::new(0, 3).expect("range"),
            ),
            vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                target: PropertyTarget::ChannelOutput(channel, output_layer_id),
                field: PropertyFieldId::MarkMinimumFill,
                end: 0.4,
                easing: Easing::SmoothStep,
            })],
        )
        .expect("initialized End validates");
    let mut history = DocumentHistory::new(DocumentSession::new(doc).expect("session validates"));
    let (output_layer_id, _) = output(history.document(), channel);
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::MarkMinimumFill,
        ),
        PropertyInheritance::Inherited
    );

    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: channel,
            output_layer_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.0),
                maximum_fill: Some(1.0),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("equal-to-baseline explicit response applies");
    for field in [
        PropertyFieldId::MarkMinimumFill,
        PropertyFieldId::MarkMaximumFill,
    ] {
        assert_eq!(
            response_inheritance(history.document(), channel, output_layer_id, field),
            PropertyInheritance::Explicit
        );
    }

    let reset_minimum = history
        .document()
        .reset_channel_response_field(channel, output_layer_id, PropertyFieldId::MarkMinimumFill)
        .expect("minimum reset command is typed");
    history
        .apply(&reset_minimum)
        .expect("minimum reset applies");
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::MarkMinimumFill,
        ),
        PropertyInheritance::Inherited
    );
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::MarkMaximumFill,
        ),
        PropertyInheritance::Explicit
    );
    assert_eq!(
        history.document().channel_response_overrides()[0].response,
        ChannelOutputResponse::Marks {
            minimum_fill: None,
            maximum_fill: Some(1.0),
            density_mode: None,
            size_scale: None,
        }
    );
    assert!(history.document().temporal_end_overrides().iter().any(|entry| matches!(entry,
        TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::ChannelOutput(channel, output_layer_id)
            && value.field == PropertyFieldId::MarkMinimumFill && value.end == 0.4
            && value.easing == Easing::SmoothStep)));
    assert!(matches!(
        output(
            &history
                .document()
                .materialize_frame(2)
                .expect("End frame materializes"),
            channel
        )
        .1,
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.4,
            ..
        })
    ));

    let reset_maximum = history
        .document()
        .reset_channel_response_field(channel, output_layer_id, PropertyFieldId::MarkMaximumFill)
        .expect("final response reset command is typed");
    history
        .apply(&reset_maximum)
        .expect("final response reset applies");
    assert!(history.document().channel_response_overrides().is_empty());
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::MarkMaximumFill,
        ),
        PropertyInheritance::Inherited
    );
}

/// Rejects clearing a named endpoint when its ALL endpoint would invert the effective paired range.
/// The candidate must fail validation without changing the document or history.
///
/// # Panics
/// Panics if valid setup cannot publish or an invalid partial reset changes the document.
#[test]
fn named_response_field_reset_rejects_invalid_paired_bounds_atomically() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let channel = ChannelId(1);
    let (output_layer_id, _) = output(history.document(), channel);
    let base = history.document().clone();
    let configuration = base
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::MarkMinimumFill, 0.5),
            (PropertyFieldId::MarkMaximumFill, 1.0),
        ])
        .expect("valid ALL pair prepares");
    history
        .apply_document_configuration(&base, history.revision(), &configuration)
        .expect("valid ALL pair applies");
    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: channel,
            output_layer_id,
            response: ChannelOutputResponse::Marks {
                minimum_fill: Some(0.2),
                maximum_fill: Some(0.4),
                density_mode: None,
                size_scale: None,
            },
        })
        .expect("valid named pair applies");
    let before = history.document().clone();
    let reset_minimum = before
        .reset_channel_response_field(channel, output_layer_id, PropertyFieldId::MarkMinimumFill)
        .expect("partial minimum reset command is typed");

    assert!(history.apply(&reset_minimum).is_err());
    assert_eq!(history.document(), &before);
}

/// Checks document-owned size scale inheritance, named edits, and eased Start/End materialization.
///
/// # Panics
/// Fails if a mark scale escapes its 0..=2 range, changes the variable bounds, or loses End intent.
#[test]
fn uniform_mark_scale_is_document_owned_and_temporal() {
    let mut doc = document();
    let channel = ChannelId(1);
    let (output_layer_id, original_response) = output(&doc, channel);
    doc = doc
        .apply_command(&DocumentCommand::SetAllResponseField {
            base: doc.response_settings().clone(),
            edit: DocumentResponseFieldEdit::MarkSizeScale(1.2),
        })
        .expect("ALL size scale applies")
        .0;
    assert_eq!(
        doc.effective_mark_density(channel, output_layer_id)
            .unwrap()
            .size_scale,
        1.2
    );
    let named = doc
        .set_channel_mark_size_scale_for_effective(channel, output_layer_id, 1.6)
        .unwrap();
    doc = doc
        .apply_command(&named)
        .expect("named size scale applies")
        .0;
    assert_eq!(
        response_inheritance(
            &doc,
            channel,
            output_layer_id,
            PropertyFieldId::MarkSizeScale
        ),
        PropertyInheritance::Explicit
    );
    assert_eq!(
        doc.effective_mark_density(channel, output_layer_id)
            .unwrap()
            .size_scale,
        1.6
    );
    assert_eq!(output(&doc, channel).1, original_response);
    doc = doc
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(1, 1).unwrap(),
                FrameRange::new(0, 3).unwrap(),
            ),
            vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                target: PropertyTarget::ChannelOutput(channel, output_layer_id),
                field: PropertyFieldId::MarkSizeScale,
                end: 0.6,
                easing: Easing::SmoothStep,
            })],
        )
        .expect("size scale End validates");
    assert!(
        (doc.materialize_frame(1)
            .unwrap()
            .effective_mark_density(channel, output_layer_id)
            .unwrap()
            .size_scale
            - 1.7)
            .abs()
            < 1e-12
    );
    assert!(
        (doc.materialize_frame(2)
            .unwrap()
            .effective_mark_density(channel, output_layer_id)
            .unwrap()
            .size_scale
            - 1.8)
            .abs()
            < 1e-12
    );
    assert!(
        doc.set_channel_mark_size_scale_for_effective(channel, output_layer_id, 2.01)
            .is_err()
    );
}

/// Keeps structural Stroke alignment on its legacy Pattern delta while response fields use named intent.
/// Resetting line thickness must leave the alignment inheritance state explicit.
///
/// # Panics
/// Panics if a one-output line recipe or either independent authority cannot publish.
#[test]
fn response_field_reset_preserves_structural_bias_inheritance() {
    let mut history =
        DocumentHistory::new(DocumentSession::new(document()).expect("session validates"));
    let marks = history
        .document()
        .reconstruct_pattern_definition_recipe(history.document().pattern_settings().definition_id)
        .expect("Mark recipe reconstructs");
    let lines = marks
        .with_construction_kind(0, PatternRecipeConstructionKind::Connections)
        .expect("one-output line recipe reconstructs");
    replace_all(&mut history, lines);
    let channel = ChannelId(1);
    let (output_layer_id, response) = output(history.document(), channel);
    let PatternGeometryResponse::Connected(mut connected) = response else {
        panic!("the current line output has a connected response");
    };
    connected.bias = 0.2;
    let bias_command = history
        .document()
        .set_channel_output_response_for_effective(
            channel,
            output_layer_id,
            PatternGeometryResponse::Connected(connected),
        )
        .expect("structural alignment command builds");
    history
        .apply(&bias_command)
        .expect("structural alignment applies");
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::CurveResponseBias,
        ),
        PropertyInheritance::Explicit
    );

    history
        .apply(&DocumentCommand::SetChannelResponse {
            channel_id: channel,
            output_layer_id,
            response: ChannelOutputResponse::Connected {
                minimum_thickness: Some(0.25),
                maximum_thickness: None,
            },
        })
        .expect("named thickness applies");
    let reset_thickness = history
        .document()
        .reset_channel_response_field(
            channel,
            output_layer_id,
            PropertyFieldId::ConnectedMinimumThickness,
        )
        .expect("thickness reset keeps structural authority separate");
    history
        .apply(&reset_thickness)
        .expect("named thickness reset applies");
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::ConnectedMinimumThickness,
        ),
        PropertyInheritance::Inherited
    );
    assert_eq!(
        response_inheritance(
            history.document(),
            channel,
            output_layer_id,
            PropertyFieldId::CurveResponseBias,
        ),
        PropertyInheritance::Explicit
    );
}

/// Resolves density mode from the validated sole random-site chain and strict exclusion.
///
/// # Panics
/// Fails if unrelated mechanisms become valid or non-overlap silently enables Uniform.
#[test]
fn uniform_density_eligibility_respects_actual_random_chain_and_nonoverlap() {
    use toniator_domain::{
        CoveragePolicy, MarkDensityMode, PatternDefinition, PatternDefinitionBundle,
        PatternDefinitionId, PatternMechanism, PatternMechanismId, PatternOutputLayerId,
        PatternOutputSettings, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
        validate_pattern_definition,
    };

    let base = document();
    let output_id = PatternOutputLayerId(1);
    for (exclusion, expected) in [
        (SiteExclusionPolicy::None, MarkDensityMode::Uniform),
        (
            SiteExclusionPolicy::VisibleMarkMargin { margin: 0.0 },
            MarkDensityMode::Variable,
        ),
    ] {
        let definition = PatternDefinition::random_sites(
            PatternDefinitionId(1),
            "weighted mark eligibility",
            PatternMechanismId(11),
            PatternMechanismId(12),
            PatternMechanismId(13),
            PatternMechanismId(14),
            output_id,
            RandomSiteCharacter::RawUniform,
            907,
            SiteDensityModulation::ArtworkWeighted,
            exclusion,
            10_000,
            10_000,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        let mut with_unrelated = definition.clone();
        with_unrelated
            .mechanisms
            .push(PatternMechanism::SiteDensityModulation {
                id: PatternMechanismId(99),
                base_site_process_id: PatternMechanismId(11),
                modulation: SiteDensityModulation::ArtworkWeighted,
            });
        assert!(validate_pattern_definition(&with_unrelated).is_err());
        let document = Document::with_source_and_topology(
            base.id(),
            base.canvas().clone(),
            base.source().clone(),
            vec![PatternDefinitionBundle {
                definition,
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: output_id,
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                }],
            }],
            base.pattern_settings().clone(),
            base.channel_model().unwrap(),
            base.channel_topology().unwrap().clone(),
        )
        .expect("validated single-chain document builds");
        let density = document
            .effective_mark_density(ChannelId(1), output_id)
            .unwrap();
        assert_eq!(density.mode, expected);
        assert_eq!(
            density.uniform_unavailable_reason.is_some(),
            expected == MarkDensityMode::Variable
        );
        if expected == MarkDensityMode::Uniform {
            let scale = document
                .set_channel_mark_size_scale_for_effective(ChannelId(1), output_id, 1.4)
                .unwrap();
            let (scaled, change) = document.apply_command(&scale).unwrap();
            assert_eq!(
                change.invalidation,
                Some(toniator_domain::InvalidationLevel::Realization)
            );
            let explicit_same = scaled
                .set_channel_mark_density_mode_for_effective(
                    ChannelId(1),
                    output_id,
                    MarkDensityMode::Contextual,
                )
                .unwrap();
            let (explicit_same, change) = scaled.apply_command(&explicit_same).unwrap();
            assert_eq!(change.invalidation, None);
            assert!(matches!(
                &explicit_same.channel_response_overrides()[0].response,
                ChannelOutputResponse::Marks {
                    density_mode: Some(MarkDensityMode::Contextual),
                    size_scale: Some(1.4),
                    ..
                }
            ));
            let reset = explicit_same
                .reset_channel_mark_density_mode(ChannelId(1), output_id)
                .unwrap();
            let (inherited, change) = explicit_same.apply_command(&reset).unwrap();
            assert_eq!(change.invalidation, None);
            assert!(matches!(
                &inherited.channel_response_overrides()[0].response,
                ChannelOutputResponse::Marks {
                    density_mode: None,
                    size_scale: Some(1.4),
                    ..
                }
            ));
            assert_eq!(
                inherited
                    .effective_mark_density(ChannelId(1), output_id)
                    .unwrap()
                    .mode,
                MarkDensityMode::Uniform
            );
            let variable = inherited
                .set_channel_mark_density_mode_for_effective(
                    ChannelId(1),
                    output_id,
                    MarkDensityMode::Variable,
                )
                .unwrap();
            let (variable, change) = inherited.apply_command(&variable).unwrap();
            assert_eq!(
                change.invalidation,
                Some(toniator_domain::InvalidationLevel::Family)
            );
            let inactive = variable
                .set_channel_mark_size_scale_for_effective(ChannelId(1), output_id, 1.6)
                .unwrap();
            let (_, change) = variable.apply_command(&inactive).unwrap();
            assert_eq!(change.invalidation, None);
        }
    }
}
