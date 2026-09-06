//! All-channel Advanced controls project domain-owned scalar batches at the selected endpoint.

use super::*;
use toniator_domain::{ANIMATABLE_SCALAR_FIELD_IDS, ChannelScalarBatch};

/// Retains one average input and its exact displayed-text guard across private draft refreshes.
pub(super) struct Controls {
    field: PropertyFieldId,
    input: gtk::Entry,
    detail: gtk::Label,
    displayed: Rc<RefCell<String>>,
}

/// Projects only Advanced batch fields; layout-base controls and artist color pickers keep their homes.
/// Eligibility, target enumeration, averages and bounds remain domain responsibilities.
fn shown_in_advanced(field: PropertyFieldId) -> bool {
    !matches!(
        field,
        PropertyFieldId::Density
            | PropertyFieldId::DensityAspect
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
    )
}

/// Builds the All group from active scalar capabilities, retaining separate named-channel details.
pub(super) fn append(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    document: &Document,
    parent: &gtk::Box,
    guard: &Rc<Cell<bool>>,
) -> Vec<Controls> {
    let group = gtk::Frame::new(Some("All channels"));
    group.update_property(&[gtk::accessible::Property::Label("All channels")]);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    let explanation = gtk::Label::new(Some(
        "Values are averages. Edits move all compatible values equally, preserving their differences and interpolation. Individual settings are below.",
    ));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    content.append(&explanation);
    group.set_child(Some(&content));
    let mut controls = Vec::new();
    for &field in ANIMATABLE_SCALAR_FIELD_IDS {
        if !shown_in_advanced(field) {
            continue;
        }
        let Ok(batch) = document.channel_scalar_batch(field) else {
            continue;
        };
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        let label = gtk::Label::new(Some(&inspector_field_label(field)));
        label.set_xalign(0.0);
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
        relate_descriptor_label(input.upcast_ref(), &label);
        input.update_relation(&[gtk::accessible::Relation::DescribedBy(&[
            detail.upcast_ref()
        ])]);
        if let Some(description) = advanced_descriptor_description(field) {
            row.set_tooltip_text(Some(description));
        }
        let displayed = Rc::new(RefCell::new(String::new()));
        let context = AdvancedCommitContext {
            state: state.clone(),
            epoch,
            target: InspectorTarget::DocumentAll,
            refreshing: guard.clone(),
        };
        let previous = displayed.clone();
        let action: Rc<dyn Fn(&gtk::Entry)> = Rc::new(move |input| {
            if context.refreshing.get() {
                return;
            }
            if input.text().as_str() == previous.borrow().as_str() {
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
        input.connect_activate(move |entry| activate(entry));
        let focus = gtk::EventControllerFocus::new();
        let leave = input.clone();
        focus.connect_leave(move |_| action(&leave));
        input.add_controller(focus);
        row.append(&input);
        content.append(&row);
        let control = Controls {
            field,
            input,
            detail,
            displayed,
        };
        control.project(&batch);
        controls.push(control);
    }
    if !controls.is_empty() {
        parent.append(&group);
    }
    controls
}

impl Controls {
    /// Updates derived average/range text without turning display rounding into an authored edit.
    fn project(&self, batch: &ChannelScalarBatch) {
        self.input.set_sensitive(true);
        self.detail.set_label(&if batch.minimum == batch.maximum {
            format!("{} matching values", batch.values.len())
        } else {
            format!(
                "Average of {} values · Range {:.4}–{:.4}",
                batch.values.len(),
                batch.minimum,
                batch.maximum
            )
        });
        if !self.input.has_focus() && !self.input.has_css_class("error") {
            let text = current_display(&PropertyCurrentValueKind::FiniteF64(batch.average));
            *self.displayed.borrow_mut() = text.clone();
            self.input.set_text(&text);
        }
    }

    /// Refreshes this capability-bound batch from the displayed document without a second effective model.
    pub(super) fn refresh(&self, document: &Document) {
        match document.channel_scalar_batch(self.field) {
            Ok(batch) => self.project(&batch),
            Err(_) => {
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
fn apply(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    field: PropertyFieldId,
    value: f64,
) -> Result<bool, String> {
    let base = history.document().clone();
    match endpoint {
        temporal_preview::Endpoint::Start => {
            let configuration = base
                .edit_all_channel_start_configuration(&[(field, value)])
                .map_err(|error| error.to_string())?;
            history
                .apply_document_configuration(&base, history.revision(), &configuration)
                .map(|result| !result.unchanged)
                .map_err(|error| error.to_string())
        }
        temporal_preview::Endpoint::End => {
            let command = base
                .edit_all_channel_end_command(&[(field, value)])
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

    /// Exercises private All edits, no-op/invalid inputs, one Apply Undo and native shared exports.
    ///
    /// # Panics
    /// Panics if channel differences, selected-frame authority, persistence or native output is lost.
    #[test]
    fn private_all_batches_preserve_frames_and_publish_one_change() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/stage22-channel-batches")
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
                .unwrap()
                .channel_scalar_batch(PropertyFieldId::ModeledMappingGain)
                .unwrap();
            assert_eq!(
                values
                    .values
                    .iter()
                    .map(|value| value.value)
                    .collect::<Vec<_>>(),
                [1.25, 1.5, 1.75]
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
