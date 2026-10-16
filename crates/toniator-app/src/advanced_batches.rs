//! All-channel Advanced controls project domain-owned scalar batches at the selected endpoint.

use super::*;
use toniator_domain::{ANIMATABLE_SCALAR_FIELD_IDS, ChannelScalarBatch};

type AllEntryCommitAction = Rc<dyn Fn(&gtk::Entry, bool)>;
/// Retains all ALL-scope controls that refresh against private history.
pub(super) type AdvancedBatchControls = (
    Vec<Controls>,
    Option<RegionSamplingControl>,
    Vec<MarkDensityControl>,
    Option<FillMaskDisclosure>,
    Vec<FineAdjustmentDisclosure>,
);

#[derive(Clone, Copy)]
struct MarkDensityBuildContext<'a> {
    state: &'a Rc<RefCell<AppState>>,
    epoch: u64,
    document: &'a Document,
    guard: &'a Rc<Cell<bool>>,
}

/// Retains one shared/mixed input and its displayed-text guard across private draft refreshes.
pub(super) struct Controls {
    field: PropertyFieldId,
    row: gtk::Box,
    input: gtk::Entry,
    detail: gtk::Label,
    displayed: Rc<RefCell<String>>,
}

/// Retains the live ALL region-sampling selector for endpoint refreshes.
pub(super) struct RegionSamplingControl {
    input: gtk::DropDown,
    choices: Vec<PropertyEnumChoice>,
    model: gtk::StringList,
    mixed: Rc<Cell<bool>>,
}

/// Retains one static density-mode selector and its scope-specific reset/equalization action.
pub(super) struct MarkDensityControl {
    input: gtk::DropDown,
    model: gtk::StringList,
    label: gtk::Label,
    reset_inherit: Option<gtk::Button>,
    apply_to_all: Option<gtk::Button>,
    mixed: Rc<Cell<bool>>,
    channel: Option<ChannelId>,
    output: Option<PatternOutputLayerId>,
}

/// Retains the optional Fill tone mask disclosure and its applicability transition state.
pub(super) struct FillMaskDisclosure {
    expander: gtk::Expander,
    content: gtk::Box,
    channels: Option<Vec<ChannelId>>,
    was_inactive: Cell<bool>,
}

/// Retains one progressive disclosure and its field scope for live default-state refresh.
pub(super) struct FineAdjustmentDisclosure {
    expander: gtk::Expander,
    content: gtk::Box,
    fields: Vec<PropertyFieldId>,
    channel: Option<ChannelId>,
    group: &'static str,
}

impl RegionSamplingControl {
    /// Reprojects common or mixed effective sampling under the caller's refresh guard.
    pub(super) fn refresh(&self, document: &Document) {
        let values = document
            .property_values()
            .into_iter()
            .filter(|value| {
                value.descriptor.field == PropertyFieldId::RegionSampling
                    && matches!(value.descriptor.target, PropertyTarget::ChannelOutput(_, _))
            })
            .filter_map(|value| match value.value {
                PropertyCurrentValueKind::EnumChoice(choice) => Some(choice),
                _ => None,
            })
            .collect::<Vec<_>>();
        let selected = values.first().copied();
        let mixed = selected.is_some_and(|first| values.iter().any(|value| *value != first));
        let mut labels = self
            .choices
            .iter()
            .copied()
            .map(enum_choice_label)
            .collect::<Vec<_>>();
        if mixed {
            labels.insert(0, "Mixed");
        }
        self.model.splice(0, self.model.n_items(), &labels);
        self.mixed.set(mixed);
        if mixed {
            self.input.set_selected(0);
        } else if let Some(selected) = selected
            && let Some(index) = self.choices.iter().position(|choice| *choice == selected)
        {
            self.input.set_selected(index as u32);
        }
    }
}

impl MarkDensityControl {
    /// Reprojects effective Uniform/Variable applicability from the domain-owned draft state.
    pub(super) fn refresh(&self, document: &Document) {
        let channels = self.channel.map_or_else(
            || authoritative_channel_ids(document),
            |channel| vec![channel],
        );
        let states = channels
            .into_iter()
            .flat_map(|channel| {
                weighted_mark_density_states(document, channel)
                    .into_iter()
                    .filter(|(output, _)| self.output.is_none_or(|selected| selected == *output))
            })
            .collect::<Vec<_>>();
        let eligible = states
            .iter()
            .filter(|(_, density)| density.uniform_unavailable_reason.is_none())
            .map(|(_, density)| density.mode)
            .collect::<Vec<_>>();
        let strict_reason = states
            .iter()
            .find_map(|(_, density)| density.uniform_unavailable_reason);
        let shown_modes = if eligible.is_empty() && strict_reason.is_some() {
            vec![MarkDensityMode::Variable]
        } else {
            eligible
        };
        let selected = shown_modes.first().copied();
        let mixed = selected.is_some_and(|first| shown_modes.iter().any(|mode| *mode != first));
        let mut labels = vec!["Uniform", "Variable"];
        if mixed {
            labels.insert(0, "Mixed");
        }
        self.model.splice(0, self.model.n_items(), &labels);
        self.mixed.set(mixed);
        if mixed {
            self.input.set_selected(0);
        } else if let Some(mode) = selected {
            let index = match mode {
                MarkDensityMode::Uniform | MarkDensityMode::Contextual => 0,
                MarkDensityMode::Variable => 1,
            };
            self.input.set_selected(index);
        }
        let can_choose = states
            .iter()
            .any(|(_, density)| density.uniform_unavailable_reason.is_none());
        self.input.set_sensitive(can_choose);
        let help = if let Some(reason) = strict_reason {
            if self.channel.is_none() && can_choose {
                format!(
                    "This choice applies to compatible weighted Mark outputs; strict non-overlap channels stay Variable. {reason}. Variable maps artwork tones to minimum and maximum occupied-area targets with an automatically measured and saved response; unreachable targets saturate within the measured size range. Artwork weighting still controls point density. Mark sizing is shared by Start and End."
                )
            } else {
                format!(
                    "{reason}. Variable maps artwork tones to minimum and maximum occupied-area targets with an automatically measured and saved response; unreachable targets saturate within the measured size range. Artwork weighting still controls point density. Mark sizing is shared by Start and End."
                )
            }
        } else {
            "Uniform uses one automatically measured mark size; artwork weighting controls point density. Variable maps artwork tones to minimum and maximum occupied-area targets with an automatically measured and saved response; unreachable targets saturate within the measured size range. Artwork weighting still controls point density. Mark sizing is shared by Start and End.".to_owned()
        };
        self.input.set_tooltip_text(Some(&help));
        self.input.update_property(&[
            gtk::accessible::Property::Label("Mark sizing"),
            gtk::accessible::Property::Description(&help),
        ]);
        self.label.set_tooltip_text(Some(&help));
        self.label
            .update_property(&[gtk::accessible::Property::Description(&help)]);
        if let Some(reset) = &self.reset_inherit {
            let explicit = self
                .channel
                .zip(self.output)
                .is_some_and(|(channel, output)| {
                    document.channel_response_overrides().iter().any(|entry| {
                        entry.channel_id == channel
                            && entry.output_layer_id == output
                            && matches!(
                                &entry.response,
                                toniator_domain::ChannelOutputResponse::Marks {
                                    density_mode: Some(_),
                                    ..
                                }
                            )
                    })
                });
            reset.set_sensitive(explicit);
        }
        if let Some(apply) = &self.apply_to_all {
            let chosen = if mixed {
                None
            } else {
                labels
                    .get(self.input.selected() as usize)
                    .copied()
                    .and_then(mode_from_label)
            };
            let has_named_modes = document.channel_response_overrides().iter().any(|entry| {
                matches!(
                    &entry.response,
                    toniator_domain::ChannelOutputResponse::Marks {
                        density_mode: Some(_),
                        ..
                    }
                )
            });
            apply.set_sensitive(can_choose && chosen.is_some() && has_named_modes);
        }
    }
}

impl FillMaskDisclosure {
    /// Returns the content box for Fill cutoff and tone controls.
    pub(super) fn content(&self) -> &gtk::Box {
        &self.content
    }

    /// Returns the disclosure widget to place after mode-dependent response rows.
    pub(super) fn expander(&self) -> &gtk::Expander {
        &self.expander
    }

    /// Follows Fill response meaning and mask applicability without overriding manual disclosure state.
    pub(super) fn refresh(&self, document: &Document) {
        update_fill_tone_disclosure_copy(&self.expander, document, self.channels.as_deref());
        let inactive = fill_mask_inactive(document, self.channels.as_deref());
        if inactive != self.was_inactive.get() {
            self.expander.set_expanded(!inactive);
            self.was_inactive.set(inactive);
        }
    }
}

/// Adds a progressive Fill tone response group with live mode-specific help on its heading.
/// Domain response mode owns expansion defaults; collapsed children cannot receive native focus.
pub(super) fn append_fill_mask_disclosure(
    document: &Document,
    channels: Option<Vec<ChannelId>>,
) -> FillMaskDisclosure {
    let expander = gtk::Expander::new(None);
    update_fill_tone_disclosure_copy(&expander, document, channels.as_deref());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    expander.set_child(Some(&content));
    expander
        .bind_property("expanded", &content, "visible")
        .sync_create()
        .build();
    let was_inactive = fill_mask_inactive(document, channels.as_deref());
    expander.set_expanded(!was_inactive);
    FillMaskDisclosure {
        expander,
        content,
        channels,
        was_inactive: Cell::new(was_inactive),
    }
}

/// Updates the existing disclosure name and help to match the current response mode and scope.
fn update_fill_tone_disclosure_copy(
    expander: &gtk::Expander,
    document: &Document,
    scoped: Option<&[ChannelId]>,
) {
    let channels = scoped.map_or_else(
        || authoritative_channel_ids(document),
        <[ChannelId]>::to_vec,
    );
    let mask_only = fill_tone_scope_is_uniform_mark_mask(document, &channels);
    let includes_uniform_marks = channels.iter().any(|channel| {
        weighted_mark_density_states(document, *channel)
            .iter()
            .any(|(_, density)| density.mode == MarkDensityMode::Uniform)
    });
    let (name, help) = if mask_only {
        (
            "Fill tone mask",
            "For Uniform weighted Marks, Fill tone mapping is an optional independent mask. At cutoff 0 the mask is Off; Source weighting still controls point density.",
        )
    } else if includes_uniform_marks {
        (
            "Fill tone response",
            "Artwork tones map to requested occupied area for Variable Marks, lines, and regions. Toniator measures and saves the response automatically; unreachable targets saturate at the nearest coverage in the measured size range. Uniform weighted Marks can use Fill tone as a separate optional mask; cutoff 0 disables it. Source weighting separately controls point density.",
        )
    } else {
        (
            "Fill tone response",
            "Artwork tones map to requested occupied area for Variable Marks, lines, and regions. Toniator measures and saves the response automatically; unreachable targets saturate at the nearest coverage in the measured size range. Source weighting separately controls point density where the current Pattern uses it.",
        )
    };
    expander.set_label(Some(name));
    expander.set_tooltip_text(Some(help));
    expander.update_property(&[
        gtk::accessible::Property::Label(name),
        gtk::accessible::Property::Description(help),
    ]);
    if let Some(label) = expander.label_widget().and_downcast::<gtk::Label>() {
        label.set_tooltip_text(Some(help));
        label.update_property(&[gtk::accessible::Property::Description(help)]);
    }
}

/// Reports whether the complete scope consists only of Uniform artwork-weighted Mark outputs.
fn fill_tone_scope_is_uniform_mark_mask(document: &Document, channels: &[ChannelId]) -> bool {
    !channels.is_empty()
        && channels.iter().all(|channel| {
            let Ok(pattern) = document.effective_channel_pattern(*channel) else {
                return false;
            };
            if pattern.output_settings.is_empty()
                || !pattern
                    .output_settings
                    .iter()
                    .all(|output| matches!(output.response, PatternGeometryResponse::Marks(_)))
            {
                return false;
            }
            let states = weighted_mark_density_states(document, *channel);
            states.len() == pattern.output_settings.len()
                && states.iter().all(|(output, density)| {
                    pattern
                        .output_settings
                        .iter()
                        .any(|setting| setting.output_layer_id == *output)
                        && density.mode == MarkDensityMode::Uniform
                        && density.uniform_unavailable_reason.is_none()
                })
        })
}

/// Collapses and disables Fill mappings only when every scoped output uses a zero-cutoff mask.
pub(super) fn fill_mask_inactive(document: &Document, scoped: Option<&[ChannelId]>) -> bool {
    let channels = scoped.map_or_else(
        || authoritative_channel_ids(document),
        <[ChannelId]>::to_vec,
    );
    fill_tone_scope_is_uniform_mark_mask(document, &channels)
        && channels
            .iter()
            .all(|channel| channel_fill_mask_inactive(document, *channel))
}

impl FineAdjustmentDisclosure {
    /// Returns the interior used to place only this group's gain and bias rows.
    pub(super) fn content(&self) -> &gtk::Box {
        &self.content
    }

    /// Returns the disclosure that controls visibility of the scoped fine adjustments.
    pub(super) fn expander(&self) -> &gtk::Expander {
        &self.expander
    }

    /// Opens the disclosure when a nondefault appears without closing a manually opened section.
    pub(super) fn refresh(&self, document: &Document) {
        let nondefault = self.fields.iter().any(|field| {
            document.channel_scalar_batch(*field).is_ok_and(|batch| {
                batch.values.iter().any(|value| {
                    let Some(channel) = property_target_channel_id(value.target) else {
                        return false;
                    };
                    if self.channel.is_some_and(|selected| selected != channel) {
                        return false;
                    }
                    document.channel_property_default(channel, *field).as_ref()
                        != Some(&PropertyCurrentValueKind::FiniteF64(value.value))
                })
            })
        });
        if nondefault && !self.expander.is_expanded() {
            self.expander.set_expanded(true);
        }
    }
}

/// Projects only Advanced batch fields; layout-base controls and artist color pickers keep their homes.
/// Eligibility, target enumeration and bounds remain domain responsibilities.
pub(super) fn shown_in_advanced(field: PropertyFieldId) -> bool {
    !inline_response_field(field)
        && !matches!(
            field,
            PropertyFieldId::TranslationX
                | PropertyFieldId::TranslationY
                | PropertyFieldId::Density
                | PropertyFieldId::DensityAspect
                | PropertyFieldId::RotationDegrees
                | PropertyFieldId::ShapeRotationDegrees
                | PropertyFieldId::ColorRed
                | PropertyFieldId::ColorGreen
                | PropertyFieldId::ColorBlue
        )
}

/// Reports response fields whose only current compatible target is already covered by the ALL row.
///
/// The ALL authority stays visible so its baseline remains editable; the named duplicate is
/// omitted from the same ALL dialog when it would address that single target a second time.
pub(super) fn single_target_response_fields(document: &Document) -> BTreeSet<PropertyFieldId> {
    let mut single = BTreeSet::new();
    for field in [
        PropertyFieldId::MarkMinimumFill,
        PropertyFieldId::MarkMaximumFill,
        PropertyFieldId::MarkSizeScale,
        PropertyFieldId::ConnectedMinimumThickness,
        PropertyFieldId::ConnectedMaximumThickness,
        PropertyFieldId::RegionMinimumFill,
        PropertyFieldId::RegionMaximumFill,
    ] {
        if document
            .channel_scalar_batch(field)
            .is_ok_and(|batch| batch.values.len() == 1)
        {
            single.insert(field);
        }
    }
    let region_targets = document
        .property_values()
        .into_iter()
        .filter(|value| {
            value.descriptor.field == PropertyFieldId::RegionSampling
                && matches!(value.descriptor.target, PropertyTarget::ChannelOutput(_, _))
        })
        .count();
    if region_targets == 1 {
        single.insert(PropertyFieldId::RegionSampling);
    }
    single
}

/// Returns the one capability-derived output kind represented by a response batch.
///
/// A mixed or channel-level batch has no single output group and stays in the existing Fill
/// response section. This never infers output kind from a preset name or field label.
pub(super) fn response_batch_output_group(
    document: &Document,
    batch: &ChannelScalarBatch,
) -> Option<&'static str> {
    let mut group = None;
    for value in &batch.values {
        let current = advanced_response_output_group_name(document, value.target)?;
        if group.is_some_and(|existing| existing != current) {
            return None;
        }
        group = Some(current);
    }
    group
}

/// Builds the ALL group with typed Fill response and Source weighting sections.
///
/// The domain owns available targets, scalar batches, enum choices, bounds, and response edits.
/// This UI projects mixed values truthfully, separates mixed output-kind coverage groups, keeps
/// the ALL baseline visible for one target, and returns controls for in-place refresh after edits.
pub(super) fn append(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    document: &Document,
    parent: &gtk::Box,
    guard: &Rc<Cell<bool>>,
) -> AdvancedBatchControls {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    group.update_property(&[gtk::accessible::Property::Label("All channels")]);
    let heading = gtk::Label::new(Some("All channels"));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.add_css_class("heading");
    group.append(&heading);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    group.append(&content);

    let mut batches = ANIMATABLE_SCALAR_FIELD_IDS
        .iter()
        .copied()
        .filter(|field| shown_in_advanced(*field))
        .filter_map(|field| document.channel_scalar_batch(field).ok())
        .collect::<Vec<_>>();
    batches.sort_by_key(|batch| {
        if batch.field == PropertyFieldId::ModeledMappingCutoff {
            0
        } else {
            1
        }
    });
    let fill_fields = batches
        .iter()
        .any(|batch| source_consumer_group(batch.field) == Some("Fill response"))
        || has_region_sampling_target(document);
    let weighting_fields = batches
        .iter()
        .any(|batch| source_consumer_group(batch.field) == Some("Source weighting"));
    let fill_group = fill_fields.then(|| {
        all_section(
            &content,
            "Fill response",
            "These settings map artwork tones to requested occupied area for marks, lines, or regions. Toniator measures and saves the response automatically; unreachable targets saturate in the measured size range. Region sampling controls how artwork is read across each region.",
            false,
        )
    });
    let fill_mask_fields = batches.iter().any(|batch| {
        source_consumer_group(batch.field) == Some("Fill response")
            && (is_fill_tone_mapping_field(batch.field)
                || batch.field == PropertyFieldId::ModeledMappingCutoff)
    });
    let fill_mask_disclosure =
        fill_mask_fields.then(|| append_fill_mask_disclosure(document, None));
    let weighting_group = weighting_fields.then(|| {
        let used = authoritative_channel_ids(document)
            .into_iter()
            .any(|channel| channel_uses_source_weighting(document, channel));
        let help = if used {
            "These settings change point placement or density. They remain separate from Fill response."
        } else {
            "These settings change point placement or density. They remain separate from Fill response and stay stored while the current channels use other patterns."
        };
        all_section(
            &content,
            "Source weighting",
            help,
            !used,
        )
    });
    let fill_fine_fields = batches
        .iter()
        .filter(|batch| advanced_fine_adjustment_field(batch.field))
        .filter(|batch| source_consumer_group(batch.field) == Some("Fill response"))
        .map(|batch| batch.field)
        .collect::<Vec<_>>();
    let weighting_fine_fields = batches
        .iter()
        .filter(|batch| advanced_fine_adjustment_field(batch.field))
        .filter(|batch| source_consumer_group(batch.field) == Some("Source weighting"))
        .map(|batch| batch.field)
        .collect::<Vec<_>>();
    let fill_fine = if !fill_fine_fields.is_empty() {
        fill_group.as_ref().map(|_| {
            fine_adjustments(
                fill_fine_fields.clone(),
                None,
                "Fill response",
                batches
                    .iter()
                    .filter(|batch| fill_fine_fields.contains(&batch.field)),
                document,
            )
        })
    } else {
        None
    };
    let weighting_fine = if !weighting_fine_fields.is_empty() {
        weighting_group.as_ref().map(|_| {
            fine_adjustments(
                weighting_fine_fields.clone(),
                None,
                "Source weighting",
                batches
                    .iter()
                    .filter(|batch| weighting_fine_fields.contains(&batch.field)),
                document,
            )
        })
    } else {
        None
    };
    let mut fine_disclosures = Vec::new();
    fine_disclosures.extend(fill_fine);
    fine_disclosures.extend(weighting_fine);
    let density_controls = fill_group
        .as_ref()
        .and_then(|(_, section)| {
            append_all_mark_density_control(state, epoch, document, section, guard)
        })
        .into_iter()
        .collect::<Vec<_>>();
    let mut controls = Vec::new();
    let fill_output_groups = batches
        .iter()
        .filter(|batch| source_consumer_group(batch.field) == Some("Fill response"))
        .filter_map(|batch| response_batch_output_group(document, batch))
        .collect::<BTreeSet<_>>();
    let separate_fill_output_groups = fill_output_groups.len() > 1;
    let mut grouped_fill_sections = BTreeMap::<&'static str, gtk::Box>::new();
    let property_values = document.property_values();
    for batch in &batches {
        let field = batch.field;
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let label = gtk::Label::new(Some(&inspector_field_label(field)));
        label.set_xalign(0.0);
        label.set_wrap(true);
        labels.append(&label);
        let detail = gtk::Label::new(None);
        detail.set_xalign(0.0);
        detail.set_wrap(true);
        detail.add_css_class("dim-label");
        labels.append(&detail);
        row.append(&labels);
        let input = gtk::Entry::new();
        input.set_width_chars(12);
        input.set_input_purpose(gtk::InputPurpose::Number);
        label.set_mnemonic_widget(Some(&input));
        let descriptor = property_values
            .iter()
            .find(|value| value.descriptor.field == field)
            .map(|value| &value.descriptor);
        let help = advanced_field_help(field, descriptor);
        label.set_tooltip_text(Some(&help));
        label.update_property(&[gtk::accessible::Property::Description(&help)]);
        let displayed = Rc::new(RefCell::new(String::new()));
        let context = AdvancedCommitContext {
            state: state.clone(),
            epoch,
            target: InspectorTarget::DocumentAll,
            refreshing: guard.clone(),
        };
        let previous = displayed.clone();
        let action: AllEntryCommitAction = Rc::new(move |input, explicit_activation| {
            if context.refreshing.get() {
                return;
            }
            if !explicit_activation && input.text().as_str() == previous.borrow().as_str() {
                input.remove_css_class("error");
                input.update_state(&[gtk::accessible::State::Invalid(
                    gtk::AccessibleInvalidState::False,
                )]);
                return;
            }
            let Some((draft, _, endpoint, status)) = context.surface_handles() else {
                return;
            };
            let result = input
                .text()
                .parse::<f64>()
                .map_err(|_| "Enter a finite number.".to_owned())
                .and_then(|value| apply(&mut draft.borrow_mut(), endpoint, field, value));
            match result {
                Ok(changed) => {
                    *previous.borrow_mut() = input.text().to_string();
                    input.remove_css_class("error");
                    input.update_state(&[gtk::accessible::State::Invalid(
                        gtk::AccessibleInvalidState::False,
                    )]);
                    if changed {
                        context.refresh_preview(false);
                    }
                }
                Err(error) => {
                    input.add_css_class("error");
                    input.update_state(&[gtk::accessible::State::Invalid(
                        gtk::AccessibleInvalidState::True,
                    )]);
                    status.set_label(&format!("Couldn’t apply this All-channel setting: {error}"));
                }
            }
        });
        let activate = action.clone();
        input.connect_activate(move |entry| activate(entry, true));
        let focus = gtk::EventControllerFocus::new();
        let leave = input.clone();
        focus.connect_leave(move |_| action(&leave, false));
        input.add_controller(focus);
        row.append(&input);
        let control = Controls {
            field,
            row: row.clone(),
            input,
            detail,
            displayed,
        };
        control.project(batch, false);
        set_advanced_control_help(control.input.upcast_ref(), &label, field, &help);
        controls.push(control);
        let fine = fine_disclosures
            .iter()
            .find(|disclosure| Some(disclosure.group) == source_consumer_group(field));
        let output_group_section = (separate_fill_output_groups
            && source_consumer_group(field) == Some("Fill response"))
        .then(|| response_batch_output_group(document, batch))
        .flatten()
        .and_then(|group_name| {
            fill_group.as_ref().map(|(_, section)| {
                grouped_fill_sections
                    .entry(group_name)
                    .or_insert_with(|| {
                        all_section(
                            section,
                            group_name,
                            "Minimum and maximum coverage target occupied area for these outputs.",
                            false,
                        )
                        .1
                    })
                    .clone()
            })
        });
        let section = match source_consumer_group(field) {
            Some("Fill response") => fine
                .filter(|_| advanced_fine_adjustment_field(field))
                .map(FineAdjustmentDisclosure::content)
                .or_else(|| {
                    (is_fill_tone_mapping_field(field)
                        || field == PropertyFieldId::ModeledMappingCutoff)
                        .then(|| {
                            fill_mask_disclosure
                                .as_ref()
                                .map(FillMaskDisclosure::content)
                        })
                        .flatten()
                })
                .or(output_group_section.as_ref())
                .or_else(|| fill_group.as_ref().map(|(_, section)| section)),
            Some("Source weighting") => fine
                .filter(|_| advanced_fine_adjustment_field(field))
                .map(FineAdjustmentDisclosure::content)
                .or_else(|| weighting_group.as_ref().map(|(_, section)| section)),
            _ => None,
        };
        if let Some(section) = section {
            section.append(&row);
        } else {
            content.append(&row);
        }
    }

    let sampling_control = fill_group.as_ref().and_then(|(_, section)| {
        append_all_region_sampling(state, epoch, document, section, guard)
    });
    if let Some(disclosure) = &fill_mask_disclosure {
        if let Some(fine) = fine_disclosures
            .iter()
            .find(|fine| fine.group == "Fill response")
        {
            disclosure.content().append(&fine.expander);
        }
        if let Some((_, section)) = &fill_group {
            section.append(&disclosure.expander);
        }
    } else if let Some(fine) = fine_disclosures
        .iter()
        .find(|fine| fine.group == "Fill response")
        && let Some((_, section)) = &fill_group
    {
        section.append(&fine.expander);
    }
    if let (Some((_, section)), Some(disclosure)) = (
        weighting_group.as_ref(),
        fine_disclosures
            .iter()
            .find(|disclosure| disclosure.group == "Source weighting"),
    ) {
        section.append(&disclosure.expander);
    }
    for disclosure in &fine_disclosures {
        disclosure.refresh(document);
    }
    for (frame, section) in [fill_group.as_ref(), weighting_group.as_ref()]
        .into_iter()
        .flatten()
    {
        frame.set_visible(section.first_child().is_some());
    }

    if !controls.is_empty() || sampling_control.is_some() {
        parent.append(&group);
    }
    (
        controls,
        sampling_control,
        density_controls,
        fill_mask_disclosure,
        fine_disclosures,
    )
}

/// Adds the ALL static Mark density choice when the current scope contains weighted Mark outputs.
fn append_all_mark_density_control(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    document: &Document,
    parent: &gtk::Box,
    guard: &Rc<Cell<bool>>,
) -> Option<MarkDensityControl> {
    if !authoritative_channel_ids(document)
        .iter()
        .any(|channel| !weighted_mark_density_states(document, *channel).is_empty())
    {
        return None;
    }
    let context = MarkDensityBuildContext {
        state,
        epoch,
        document,
        guard,
    };
    Some(append_mark_density_control(
        context,
        parent,
        None,
        None,
        "Mark sizing",
    ))
}

/// Adds named per-output Mark density choices while keeping the ALL baseline as domain authority.
pub(super) fn append_named_mark_density_controls(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    document: &Document,
    parent: &gtk::Box,
    guard: &Rc<Cell<bool>>,
    channel: ChannelId,
    skip: bool,
) -> Vec<MarkDensityControl> {
    if skip {
        return Vec::new();
    }
    let outputs = weighted_mark_density_states(document, channel);
    let count = outputs.len();
    let context = MarkDensityBuildContext {
        state,
        epoch,
        document,
        guard,
    };
    outputs
        .into_iter()
        .enumerate()
        .map(|(index, (output, _))| {
            let name = if count == 1 {
                "Mark sizing".to_owned()
            } else {
                format!("Mark sizing — Output {}", index + 1)
            };
            append_mark_density_control(context, parent, Some(channel), Some(output), &name)
        })
        .collect()
}

/// Builds one typed Mark density selector with the domain's ALL or named response route.
fn append_mark_density_control(
    context: MarkDensityBuildContext<'_>,
    parent: &gtk::Box,
    channel: Option<ChannelId>,
    output: Option<PatternOutputLayerId>,
    name: &str,
) -> MarkDensityControl {
    let MarkDensityBuildContext {
        state,
        epoch,
        document,
        guard,
    } = context;
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(&format!("_{name}")));
    label.set_use_underline(true);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let input = accessible_string_dropdown(&["Uniform", "Variable"]);
    label.set_mnemonic_widget(Some(&input));
    let model = input
        .model()
        .and_then(|model| model.downcast::<gtk::StringList>().ok())
        .expect("Mark density selector retains its string-list model");
    let reset_inherit = channel.map(|_| gtk::Button::with_label("Reset to inherit"));
    let apply_to_all = channel
        .is_none()
        .then(|| gtk::Button::with_label("Use for all"));
    let help = "Uniform uses one automatically measured mark size; artwork weighting controls point density. Variable maps artwork tones to minimum and maximum occupied-area targets with an automatically measured and saved response; unreachable targets saturate within the measured size range. Artwork weighting still controls point density. Mark sizing is shared by Start and End.";
    for control in [
        input.upcast_ref::<gtk::Widget>(),
        label.upcast_ref::<gtk::Widget>(),
    ] {
        control.set_tooltip_text(Some(help));
    }
    input.update_property(&[
        gtk::accessible::Property::Label(name),
        gtk::accessible::Property::Description(help),
    ]);
    label.update_property(&[gtk::accessible::Property::Description(help)]);
    relate_descriptor_label(input.upcast_ref(), &label);
    row.append(&label);
    row.append(&input);
    if let Some(reset) = &reset_inherit {
        let description = "Clear this Mark sizing choice so it follows the ALL response setting. Variable ranges and scale remain independent.";
        reset.set_tooltip_text(Some(description));
        reset.update_property(&[
            gtk::accessible::Property::Label("Reset Mark sizing to inherit"),
            gtk::accessible::Property::Description(description),
        ]);
        row.append(reset);
    }
    if let Some(apply) = &apply_to_all {
        let description = "Set the selected Mark sizing for all compatible weighted Mark outputs and clear named mode overrides.";
        apply.set_tooltip_text(Some(description));
        apply.update_property(&[
            gtk::accessible::Property::Label("Use Mark sizing for all"),
            gtk::accessible::Property::Description(description),
        ]);
        row.append(apply);
    }
    parent.append(&row);
    let mixed = Rc::new(Cell::new(false));
    let control = MarkDensityControl {
        input: input.clone(),
        model,
        label,
        reset_inherit,
        apply_to_all,
        mixed: mixed.clone(),
        channel,
        output,
    };
    let state_for_change = state.clone();
    let guard_for_change = guard.clone();
    let mixed_for_change = mixed.clone();
    input.connect_selected_notify(move |input| {
        if guard_for_change.get() {
            return;
        }
        let mode = selected_mode(input.selected(), mixed_for_change.get());
        if let Some(mode) = mode {
            apply_density_mode_change(&state_for_change, epoch, channel, output, mode, false);
        }
    });
    if let Some(reset) = &control.reset_inherit {
        let state_for_reset = state.clone();
        reset.connect_clicked(move |_| {
            apply_density_mode_change(
                &state_for_reset,
                epoch,
                channel,
                output,
                MarkDensityMode::Contextual,
                true,
            );
        });
    }
    if let Some(apply) = &control.apply_to_all {
        let state_for_apply = state.clone();
        let input_for_apply = input.clone();
        let mixed_for_apply = mixed.clone();
        apply.connect_clicked(move |_| {
            if let Some(mode) = selected_mode(input_for_apply.selected(), mixed_for_apply.get()) {
                apply_density_mode_change(&state_for_apply, epoch, None, None, mode, false);
            }
        });
    }
    control.refresh(document);
    control
}

/// Converts the current visible selector row into its static domain mode.
fn selected_mode(selected: u32, mixed: bool) -> Option<MarkDensityMode> {
    if mixed && selected == 0 {
        return None;
    }
    match selected.saturating_sub(u32::from(mixed)) {
        0 => Some(MarkDensityMode::Uniform),
        1 => Some(MarkDensityMode::Variable),
        _ => None,
    }
}

/// Converts a visible static mode label back to its domain value for ALL equalization state.
fn mode_from_label(label: &str) -> Option<MarkDensityMode> {
    match label {
        "Uniform" => Some(MarkDensityMode::Uniform),
        "Variable" => Some(MarkDensityMode::Variable),
        _ => None,
    }
}

/// Applies a named mode, named field reset, or ALL field equalization in one private history edit.
fn apply_density_mode_change(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    channel: Option<ChannelId>,
    output: Option<PatternOutputLayerId>,
    mode: MarkDensityMode,
    reset: bool,
) {
    let Some((draft, status)) = state
        .borrow()
        .advanced_settings
        .as_ref()
        .filter(|surface| surface.epoch == epoch)
        .map(|surface| (surface.draft.clone(), surface.status.clone()))
    else {
        return;
    };
    let result = {
        let mut history = draft.borrow_mut();
        match (channel, output) {
            (Some(channel), Some(output)) if reset => (|| {
                let command = history
                    .document()
                    .reset_channel_mark_density_mode(channel, output)
                    .map_err(|error| error.to_string())?;
                history
                    .apply(&command)
                    .map(|_| true)
                    .map_err(|error| error.to_string())
            })(),
            (Some(channel), Some(output)) => (|| {
                let command = history
                    .document()
                    .set_channel_mark_density_mode_for_effective(channel, output, mode)
                    .map_err(|error| error.to_string())?;
                history
                    .apply(&command)
                    .map(|_| true)
                    .map_err(|error| error.to_string())
            })(),
            (None, None) => apply_all_mark_density_mode(&mut history, mode),
            _ => Err("Mark sizing target is no longer available.".to_owned()),
        }
    };
    match result {
        Ok(true) => {
            advanced_temporal::refresh(state, epoch);
            submit_advanced_preview(state, epoch);
        }
        Ok(false) => advanced_temporal::refresh(state, epoch),
        Err(error) => status.set_label(&format!("Couldn’t update Mark sizing: {error}")),
    }
}

/// Equalizes ALL Mark density intent and clears only named mode overrides through typed authority.
pub(super) fn apply_all_mark_density_mode(
    history: &mut DocumentHistory,
    mode: MarkDensityMode,
) -> Result<bool, String> {
    let document = history.document();
    let has_named_modes = document.channel_response_overrides().iter().any(|entry| {
        matches!(
            &entry.response,
            toniator_domain::ChannelOutputResponse::Marks {
                density_mode: Some(_),
                ..
            }
        )
    });
    if document.response_settings().mark_density_mode == mode && !has_named_modes {
        return Ok(false);
    }
    let before = document.clone();
    let command = DocumentCommand::SetAllResponseField {
        base: document.response_settings().clone(),
        edit: DocumentResponseFieldEdit::MarkDensityMode(mode),
    };
    history
        .apply(&command)
        .map(|_| history.document() != &before)
        .map_err(|error| error.to_string())
}

/// Creates one accessible Advanced section without adding a persistent help paragraph.
/// Collapsed native disclosures hide their children from GTK keyboard traversal.
fn all_section(
    parent: &gtk::Box,
    name: &str,
    help: &str,
    collapsed: bool,
) -> (gtk::Widget, gtk::Box) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let (group, label) = if collapsed {
        let expander = gtk::Expander::new(Some(name));
        expander.set_expanded(false);
        expander.set_child(Some(&section));
        expander
            .bind_property("expanded", &section, "visible")
            .sync_create()
            .build();
        let label = expander.label_widget().and_downcast::<gtk::Label>();
        if let Some(label) = &label {
            label.set_wrap(true);
        }
        (expander.upcast::<gtk::Widget>(), label)
    } else {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let label = gtk::Label::new(Some(name));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.add_css_class("heading");
        group.append(&label);
        group.append(&section);
        (group.upcast::<gtk::Widget>(), Some(label))
    };
    group.update_property(&[
        gtk::accessible::Property::Label(name),
        gtk::accessible::Property::Description(help),
    ]);
    group.set_tooltip_text(Some(help));
    if let Some(label) = label {
        label.set_tooltip_text(Some(help));
        label.update_property(&[gtk::accessible::Property::Description(help)]);
    }
    parent.append(&group);
    (group, section)
}

/// Prepares a detached gain/bias disclosure and opens it for nondefault projected values.
/// Domain defaults determine initial expansion; child visibility excludes collapsed controls from focus.
fn fine_adjustments<'a>(
    fields: Vec<PropertyFieldId>,
    channel: Option<ChannelId>,
    group: &'static str,
    batches: impl Iterator<Item = &'a toniator_domain::ChannelScalarBatch>,
    document: &Document,
) -> FineAdjustmentDisclosure {
    let expander = gtk::Expander::new(Some("Fine adjustments"));
    let help = "Gain multiplies the mapped source response. Tone offset adds to it before Fill or site weighting is applied.";
    expander.update_property(&[
        gtk::accessible::Property::Label("Fine adjustments"),
        gtk::accessible::Property::Description(help),
    ]);
    expander.set_tooltip_text(Some(help));
    if let Some(label) = expander.label_widget().and_downcast::<gtk::Label>() {
        label.set_tooltip_text(Some(help));
        label.update_property(&[gtk::accessible::Property::Description(help)]);
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    expander.set_child(Some(&content));
    expander
        .bind_property("expanded", &content, "visible")
        .sync_create()
        .build();
    let expanded = batches.into_iter().any(|batch| {
        fields.contains(&batch.field)
            && batch.values.iter().any(|value| {
                let Some(value_channel) = property_target_channel_id(value.target) else {
                    return false;
                };
                if channel.is_some_and(|selected| selected != value_channel) {
                    return false;
                }
                document
                    .channel_property_default(value_channel, batch.field)
                    .as_ref()
                    != Some(&PropertyCurrentValueKind::FiniteF64(value.value))
            })
    });
    expander.set_expanded(expanded);
    FineAdjustmentDisclosure {
        expander,
        content,
        fields,
        channel,
        group,
    }
}

/// Builds a detached named-channel fine-adjustment disclosure from current channel batches.
pub(super) fn fine_adjustment_disclosure(
    document: &Document,
    channel: Option<ChannelId>,
    group: &'static str,
    fields: Vec<PropertyFieldId>,
) -> FineAdjustmentDisclosure {
    let batches = fields
        .iter()
        .filter_map(|field| document.channel_scalar_batch(*field).ok())
        .collect::<Vec<_>>();
    fine_adjustments(fields, channel, group, batches.iter(), document)
}

/// Reports whether an active region output supplies the descriptor choices for ALL sampling.
fn has_region_sampling_target(document: &Document) -> bool {
    document.property_values().iter().any(|value| {
        value.descriptor.field == PropertyFieldId::RegionSampling
            && matches!(value.descriptor.target, PropertyTarget::ChannelOutput(_, _))
    })
}

/// Reports whether one channel's effective Pattern uses the separate site-weighting response.
fn channel_uses_source_weighting(document: &Document, channel: ChannelId) -> bool {
    document
        .effective_channel_pattern(channel)
        .ok()
        .and_then(|effective| {
            document
                .pattern_definition_bundles()
                .iter()
                .find(|bundle| bundle.definition.id == effective.definition_id)
                .map(|bundle| &bundle.definition)
        })
        .is_some_and(toniator_domain::definition_uses_source_weighting)
}

/// Appends the document-owned ALL region sampling choice to the Fill response section.
fn append_all_region_sampling(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    document: &Document,
    parent: &gtk::Box,
    guard: &Rc<Cell<bool>>,
) -> Option<RegionSamplingControl> {
    let descriptor = document.property_values().into_iter().find(|value| {
        value.descriptor.field == PropertyFieldId::RegionSampling
            && matches!(value.descriptor.target, PropertyTarget::ChannelOutput(_, _))
    })?;
    let choices = descriptor.descriptor.choices.to_vec();
    let labels = choices
        .iter()
        .copied()
        .map(enum_choice_label)
        .collect::<Vec<_>>();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(&inspector_field_label(
        PropertyFieldId::RegionSampling,
    )));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let input = accessible_string_dropdown(&labels);
    let model = input
        .model()
        .and_then(|model| model.downcast::<gtk::StringList>().ok())
        .expect("accessible dropdown retains its string-list model");
    let mixed = Rc::new(Cell::new(false));
    label.set_mnemonic_widget(Some(&input));
    let help = advanced_field_help(
        PropertyFieldId::RegionSampling,
        Some(&descriptor.descriptor),
    );
    set_advanced_control_help(
        input.upcast_ref(),
        &label,
        PropertyFieldId::RegionSampling,
        &help,
    );
    let state_for_change = state.clone();
    let guard_for_change = guard.clone();
    let choices_for_change = choices.clone();
    let mixed_for_change = mixed.clone();
    input.connect_selected_notify(move |input| {
        if guard_for_change.get() {
            return;
        }
        let offset = usize::from(mixed_for_change.get());
        if offset == 1 && input.selected() == 0 {
            return;
        }
        let Some(PropertyEnumChoice::RegionSampling(sampling)) = choices_for_change
            .get((input.selected() as usize).saturating_sub(offset))
            .copied()
        else {
            return;
        };
        let Some((draft, status)) = state_for_change
            .borrow()
            .advanced_settings
            .as_ref()
            .filter(|surface| surface.epoch == epoch)
            .map(|surface| (surface.draft.clone(), surface.status.clone()))
        else {
            return;
        };
        let result = {
            let mut draft = draft.borrow_mut();
            apply_region_sampling(&mut draft, sampling)
        };
        match result {
            Ok(changed) => {
                if changed {
                    advanced_temporal::refresh(&state_for_change, epoch);
                    submit_advanced_preview(&state_for_change, epoch);
                }
            }
            Err(error) => {
                status.set_label(&format!("Couldn’t apply this All-channel setting: {error}"));
                advanced_temporal::refresh(&state_for_change, epoch);
            }
        }
    });
    row.append(&label);
    row.append(&input);
    parent.append(&row);
    let control = RegionSamplingControl {
        input,
        choices,
        model,
        mixed,
    };
    control.refresh(document);
    Some(control)
}

/// Applies one ALL region-sampling edit through domain response authority and one history entry.
/// An already-default baseline skips only when no named Region sampling override needs equalizing.
///
/// # Errors
/// Returns stale response, validation, or history errors without publishing a partial edit.
pub(super) fn apply_region_sampling(
    history: &mut DocumentHistory,
    sampling: toniator_domain::RegionSamplingStrategy,
) -> Result<bool, String> {
    let document = history.document();
    let has_named_sampling = document.channel_response_overrides().iter().any(|entry| {
        matches!(
            &entry.response,
            toniator_domain::ChannelOutputResponse::Regions {
                sampling: Some(_),
                ..
            }
        )
    });
    if document.response_settings().region_sampling == sampling && !has_named_sampling {
        return Ok(false);
    }
    let before = history.document().clone();
    let base = document.response_settings().clone();
    let command = DocumentCommand::SetAllResponseField {
        base,
        edit: DocumentResponseFieldEdit::RegionSampling(sampling),
    };
    history
        .apply(&command)
        .map(|_| history.document() != &before)
        .map_err(|error| error.to_string())
}

impl Controls {
    /// Discards an invalid pending edit when the dialog explicitly resets its complete target scope.
    pub(super) fn clear_pending_error(&self) {
        self.input.remove_css_class("error");
        self.input.update_state(&[gtk::accessible::State::Invalid(
            gtk::AccessibleInvalidState::False,
        )]);
    }

    /// Parses only unpublished text; an untouched Mixed placeholder is not an edit.
    ///
    /// # Errors
    /// Rejects nonnumeric/nonfinite pending text before the Advanced draft is published.
    pub(super) fn pending(&self) -> Result<Option<(PropertyFieldId, f64)>, String> {
        if self.input.text().as_str() == self.displayed.borrow().as_str() {
            return Ok(None);
        }
        let value = self
            .input
            .text()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                format!(
                    "{}: enter a finite number.",
                    inspector_field_label(self.field)
                )
            })?;
        Ok(Some((self.field, value)))
    }

    /// Projects shared values without redundant captions and retains visible Mixed ranges.
    /// Display rounding never authors values; explicit reset replaces pending text.
    fn project(&self, batch: &ChannelScalarBatch, discard_pending: bool) {
        self.input.set_sensitive(true);
        let mixed = batch.minimum != batch.maximum;
        self.detail.set_visible(mixed);
        if mixed {
            self.detail.set_label(&format!(
                "Mixed · {} values · Range {}–{}",
                batch.values.len(),
                batch.minimum,
                batch.maximum
            ));
        }
        if discard_pending || (!self.input.has_focus() && !self.input.has_css_class("error")) {
            let text = if batch.minimum == batch.maximum {
                batch.minimum.to_string()
            } else {
                String::new()
            };
            self.input.set_placeholder_text(Some("Mixed"));
            *self.displayed.borrow_mut() = text.clone();
            self.input.set_text(&text);
        }
    }

    /// Refreshes capability-bound values; explicit reset clears errors and overwrites pending input.
    pub(super) fn refresh(&self, document: &Document, discard_pending: bool) {
        if discard_pending {
            self.clear_pending_error();
        }
        match document.channel_scalar_batch(self.field) {
            Ok(batch) => {
                self.row
                    .set_visible(all_response_field_visible(document, self.field));
                let mask_disabled = is_fill_tone_mapping_field(self.field)
                    && batch.values.iter().all(|value| {
                        property_target_channel_id(value.target)
                            .is_some_and(|channel| channel_fill_mask_inactive(document, channel))
                    });
                self.project(&batch, discard_pending);
                self.row.set_sensitive(!mask_disabled);
                self.input.set_sensitive(!mask_disabled);
            }
            Err(_) => {
                self.row.set_visible(false);
                self.input.set_sensitive(false);
                self.detail.set_label("No compatible values in this frame.");
            }
        }
    }
}

/// Publishes one domain batch to private history while preserving no-op/Undo/Redo semantics.
///
/// # Errors
/// Reports invalid values, incompatible targets, coupled bounds or stale history without partial edits.
pub(super) fn apply(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    field: PropertyFieldId,
    value: f64,
) -> Result<bool, String> {
    apply_fields(history, endpoint, &[(field, value)])
}

/// Publishes a complete pending ALL scalar batch, validating coupled bounds atomically.
///
/// # Errors
/// Returns the domain/history diagnostic without publishing a partial batch.
pub(super) fn apply_fields(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    edits: &[(PropertyFieldId, f64)],
) -> Result<bool, String> {
    if edits.is_empty() {
        return Ok(false);
    }
    let base = history.document().clone();
    match endpoint {
        temporal_preview::Endpoint::Start => {
            let configuration = base
                .edit_all_channel_start_configuration(edits)
                .map_err(|error| error.to_string())?;
            history
                .apply_document_configuration(&base, history.revision(), &configuration)
                .map(|result| !result.unchanged)
                .map_err(|error| error.to_string())
        }
        temporal_preview::Endpoint::End => {
            let command = base
                .edit_all_channel_end_command(edits)
                .map_err(|error| error.to_string())?;
            if command.replacement() == &base.temporal_authority() {
                return Ok(false);
            }
            history
                .apply_temporal(&command)
                .map(|_| true)
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equalizes ALL Mark density mode while retaining named range and scale siblings.
    ///
    /// The document baseline and named response override stay domain-owned; equalization clears
    /// only the mode member and keeps the user's independently authored values intact.
    ///
    /// # Panics
    /// Panics when the single Mark fixture cannot be prepared or field-scoped equalization changes
    /// a sibling response field.
    #[test]
    fn ton018_all_mark_density_equalization_preserves_named_range_and_scale() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 80.0,
                height: 60.0,
            },
            SourceReference::Unassigned,
        )
        .expect("default document validates");
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, "round-spiral-marks")
            .expect("one Mark output is available");
        let channel = ChannelId(1);
        let output = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        history
            .apply(&DocumentCommand::SetChannelResponse {
                channel_id: channel,
                output_layer_id: output,
                response: toniator_domain::ChannelOutputResponse::Marks {
                    minimum_fill: Some(0.2),
                    maximum_fill: Some(1.6),
                    density_mode: Some(MarkDensityMode::Variable),
                    size_scale: Some(1.7),
                },
            })
            .unwrap();

        assert!(apply_all_mark_density_mode(&mut history, MarkDensityMode::Uniform).unwrap());

        assert_eq!(
            history.document().response_settings().mark_density_mode,
            MarkDensityMode::Uniform
        );
        assert_eq!(
            history.document().channel_response_overrides()[0].response,
            toniator_domain::ChannelOutputResponse::Marks {
                minimum_fill: Some(0.2),
                maximum_fill: Some(1.6),
                density_mode: None,
                size_scale: Some(1.7),
            }
        );
    }

    /// Applies unchanged visible ALL Appearance values through the Advanced batch publication path.
    ///
    /// # Panics
    /// Panics if an explicit same-value batch fails to record baseline intent as one undoable,
    /// render-inert configuration update.
    #[test]
    fn ton016_same_value_all_appearance_batch_records_baseline() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 80.0,
                height: 60.0,
            },
            SourceReference::Unassigned,
        )
        .expect("default document validates");
        let first_channel = document
            .channel_topology()
            .expect("default document has modeled channels")
            .channels()[0]
            .id;
        let effective = document
            .effective_channel_pattern(first_channel)
            .expect("first channel resolves its Pattern");
        let mut history = DocumentHistory::new(
            DocumentSession::new(document.clone()).expect("initial session validates"),
        );

        assert!(
            apply_fields(
                &mut history,
                temporal_preview::Endpoint::Start,
                &[
                    (PropertyFieldId::Density, effective.density.density),
                    (PropertyFieldId::DensityAspect, effective.density.aspect),
                    (
                        PropertyFieldId::RotationDegrees,
                        effective.pattern_rotation_degrees,
                    ),
                    (PropertyFieldId::TranslationX, effective.translation_x),
                    (PropertyFieldId::TranslationY, effective.translation_y),
                ],
            )
            .expect("same-value batch publishes")
        );
        assert_ne!(history.document(), &document);
        assert_eq!(
            history.document().appearance_baseline().density,
            Some(effective.density.density)
        );
        assert_eq!(
            history.document().appearance_baseline().density_aspect,
            Some(effective.density.aspect)
        );
        assert_eq!(
            history
                .document()
                .appearance_baseline()
                .pattern_rotation_degrees,
            Some(effective.pattern_rotation_degrees)
        );
        assert_eq!(
            history.document().appearance_baseline().translation_x,
            Some(effective.translation_x)
        );
        assert_eq!(
            history.document().appearance_baseline().translation_y,
            Some(effective.translation_y)
        );
        assert!(history.can_undo());
        history
            .undo()
            .unwrap()
            .expect("baseline update is undoable");
        assert_eq!(history.document(), &document);
    }

    /// Exercises explicit ALL Appearance intent through the app model selector with both sample sources.
    ///
    /// # Panics
    /// Panics if either immutable source fails to load, model replacement loses effective shared
    /// values, or the resulting Start frame cannot produce a native PNG witness.
    #[test]
    fn ton016_model_replacement_retains_all_baseline_for_both_sample_sources() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_directory =
            root.join("target/validation/issues-20261008/ton016-model-witnesses");
        fs::create_dir_all(&output_directory).expect("sample witness directory creates");
        for (source_name, output_name) in [
            ("raster-sample.png", "raster"),
            ("vector-sample.svg", "vector"),
        ] {
            let mut workspace = load_workspace(&root.join("assets").join(source_name))
                .expect("immutable sample source imports");
            let edits = [
                (PropertyFieldId::Density, 37.0),
                (PropertyFieldId::DensityAspect, 1.6),
                (PropertyFieldId::RotationDegrees, 23.0),
                (PropertyFieldId::TranslationX, 0.17),
                (PropertyFieldId::TranslationY, -0.21),
            ];
            assert!(
                apply_fields(
                    &mut workspace.history,
                    temporal_preview::Endpoint::Start,
                    &edits,
                )
                .expect("ALL Appearance batch publishes")
            );
            let expected = *workspace.document().appearance_baseline();
            for model in [
                PreviewModel::Cmyk,
                PreviewModel::SourceColorAlpha,
                PreviewModel::Rgb,
            ] {
                replace_model_topology(&mut workspace.history, model)
                    .expect("model selector replacement publishes");
                assert_eq!(workspace.document().appearance_baseline(), &expected);
                for channel in workspace
                    .document()
                    .channel_topology()
                    .expect("selected model has channel topology")
                    .channels()
                {
                    let effective = workspace
                        .document()
                        .effective_channel_pattern(channel.id)
                        .expect("replacement channel resolves");
                    assert_eq!(effective.density.density, 37.0);
                    assert_eq!(effective.density.aspect, 1.6);
                    assert_eq!(effective.pattern_rotation_degrees, 23.0);
                    assert_eq!(effective.translation_x, 0.17);
                    assert_eq!(effective.translation_y, -0.21);
                }
            }
            let output = output_directory.join(format!("{output_name}-start.png"));
            export_snapshot_frame(
                workspace.snapshot(),
                output.clone(),
                ExportSettings {
                    format: ExportFormat::Png,
                    prepare_for_print: false,
                    pixels_per_metre: None,
                    cleanup: crate::print_cleanup::Settings::default(),
                    background: RasterBackground::Transparent,
                    output_target: None,
                    antialiasing: RasterAntialiasing::On,
                },
                0,
            )
            .expect("model-seeded sample evaluates and exports");
            assert!(fs::metadata(output).expect("sample output exists").len() > 0);
        }
    }

    /// Exercises private All edits, no-op/invalid inputs, one Apply Undo and native shared exports.
    ///
    /// # Panics
    /// Panics if channel differences, selected-frame authority, persistence or native output is lost.
    #[test]
    fn private_all_batches_preserve_frames_and_publish_one_change() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/review-gate1-channel-batches")
            .join(format!(
                "run-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            ));
        fs::create_dir_all(&directory).unwrap();
        for (input, name, format) in [
            ("raster-sample.png", "raster", ExportFormat::Png),
            ("vector-sample.svg", "vector", ExportFormat::Svg),
            ("video-sample0001-0010.mp4", "video", ExportFormat::Png),
        ] {
            let mut workspace = load_workspace(&root.join("assets").join(input)).unwrap();
            for (channel_id, gain) in [(ChannelId(1), 0.75), (ChannelId(3), 1.25)] {
                workspace
                    .history
                    .apply(&DocumentCommand::SetModeledMappingField {
                        channel_id,
                        edit: ModeledMappingFieldEdit::Gain(gain),
                    })
                    .unwrap();
            }
            let command = workspace.document().initialize_end_command().unwrap();
            workspace.history.apply_temporal(&command).unwrap();
            let before = workspace.snapshot();
            save_container(
                &directory.join(format!("{name}-before.toniator")),
                &before.document,
                &before.sources,
            )
            .unwrap();
            let mut draft = DocumentHistory::new_draft(&workspace.history);
            assert!(
                apply(
                    &mut draft,
                    temporal_preview::Endpoint::End,
                    PropertyFieldId::ModeledMappingGain,
                    1.5
                )
                .unwrap()
            );
            assert!(
                apply(
                    &mut draft,
                    temporal_preview::Endpoint::End,
                    PropertyFieldId::ColorAlpha,
                    0.7
                )
                .unwrap()
            );
            let candidate = draft.document().clone();
            assert!(
                !apply(
                    &mut draft,
                    temporal_preview::Endpoint::End,
                    PropertyFieldId::ModeledMappingGain,
                    1.5
                )
                .unwrap()
            );
            assert!(
                apply(
                    &mut draft,
                    temporal_preview::Endpoint::End,
                    PropertyFieldId::ModeledMappingGain,
                    -1.0
                )
                .is_err()
            );
            assert_eq!(draft.document(), &candidate);
            let end_frame = temporal_preview::Endpoint::End.frame(&candidate);
            let values = candidate
                .materialize_frame(end_frame)
                .unwrap().document()
                .channel_scalar_batch(PropertyFieldId::ModeledMappingGain)
                .unwrap();
            assert_eq!(
                values
                    .values
                    .iter()
                    .map(|value| value.value)
                    .collect::<Vec<_>>(),
                [1.5, 1.5, 1.5]
            );
            assert_eq!(workspace.snapshot(), before);
            assert_eq!(
                candidate.materialize_frame(0).unwrap(),
                before.document.materialize_frame(0).unwrap()
            );
            workspace.history.squash_draft(&draft).unwrap();
            let after = workspace.snapshot();
            workspace.history.undo().unwrap().unwrap();
            assert_eq!(workspace.snapshot(), before);
            workspace.history.redo().unwrap().unwrap();
            assert_eq!(workspace.snapshot(), after);
            let persisted = directory.join(format!("{name}-after.toniator"));
            save_container(&persisted, &after.document, &after.sources).unwrap();
            assert_eq!(load_workspace(&persisted).unwrap().snapshot(), after);
            export_snapshot_frame(
                after,
                directory.join(format!(
                    "{name}-end.{}",
                    if matches!(format, ExportFormat::Svg) {
                        "svg"
                    } else {
                        "png"
                    }
                )),
                ExportSettings {
                    format,
                    prepare_for_print: false,
                    pixels_per_metre: None,
                    cleanup: crate::print_cleanup::Settings::default(),
                    background: RasterBackground::Transparent,
                    output_target: None,
                    antialiasing: RasterAntialiasing::On,
                },
                end_frame,
            )
            .unwrap();
        }
        println!("All-channel native witnesses: {}", directory.display());
    }
}
