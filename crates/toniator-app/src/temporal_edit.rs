//! Endpoint inspector bindings project domain transitions without storing a second Start state.

use super::*;
use toniator_domain::{
    Easing, ScalarEndOverride, TemporalCapability, TemporalCommand, TemporalEndOverride,
    TemporalEndpointEdit,
};

pub(super) const EASINGS: &[(&str, Easing)] = &[
    ("Linear", Easing::Linear),
    ("Hold until End", Easing::Hold),
    ("Ease in", Easing::QuadraticIn),
    ("Ease out", Easing::QuadraticOut),
    ("Smooth step", Easing::SmoothStep),
    ("Smooth in and out", Easing::SmoothInOut),
];

/// Retains the progressively disclosed controls for one descriptor's End override.
pub(super) struct Controls {
    expander: gtk::Expander,
    hint: gtk::Label,
    reset: gtk::Button,
    easing: gtk::DropDown,
    syncing: Rc<Cell<bool>>,
}

/// Reports the exact descriptor's domain-owned scalar animation capability.
pub(super) fn eligible(descriptor: &PropertyDescriptor) -> bool {
    matches!(
        descriptor.temporal,
        TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
    )
}

/// Selects a stored override by its authoritative field and target, never by visible labels.
pub(super) fn scalar<'a>(
    document: &'a Document,
    descriptor: &PropertyDescriptor,
) -> Option<&'a ScalarEndOverride> {
    document
        .temporal_end_overrides()
        .iter()
        .find_map(|entry| match entry {
            TemporalEndOverride::Scalar(value)
                if value.target == descriptor.target && value.field == descriptor.field =>
            {
                Some(value)
            }
            _ => None,
        })
}

/// Builds an effective End edit while retaining the existing easing choice.
///
/// # Errors
/// Rejects static descriptors and invalid values through the canonical domain command builder.
pub(super) fn scalar_command(
    document: &Document,
    descriptor: &PropertyDescriptor,
    value: f64,
) -> Result<TemporalCommand, String> {
    if let (PropertyTarget::Channel(channel), Some(component)) =
        (descriptor.target, paint_editor::component(descriptor.field))
    {
        return document
            .edit_paint_component_end_command(&[channel], component, value)
            .map_err(|error| error.to_string());
    }
    document
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: descriptor.target,
            field: descriptor.field,
            effective_end: value,
            easing: scalar(document, descriptor).map_or(Easing::Linear, |value| value.easing),
        }])
        .map_err(|error| error.to_string())
}

/// Removes only the selected scalar End override, preserving ordinary Start and other transitions.
pub(super) fn reset_command(
    document: &Document,
    descriptor: &PropertyDescriptor,
) -> TemporalCommand {
    let mut overrides = document.temporal_end_overrides().to_vec();
    overrides.retain(|entry| {
        !matches!(entry, TemporalEndOverride::Scalar(value)
        if value.target == descriptor.target && value.field == descriptor.field)
    });
    document.replace_temporal_authority_command(document.project_timing().clone(), overrides)
}

/// Changes easing on an existing scalar override without normalizing or changing its endpoint.
pub(super) fn easing_command(
    document: &Document,
    descriptor: &PropertyDescriptor,
    easing: Easing,
) -> TemporalCommand {
    let mut overrides = document.temporal_end_overrides().to_vec();
    for entry in &mut overrides {
        if let TemporalEndOverride::Scalar(value) = entry
            && value.target == descriptor.target
            && value.field == descriptor.field
        {
            value.easing = easing;
        }
    }
    document.replace_temporal_authority_command(document.project_timing().clone(), overrides)
}

/// Applies one End/timing command through history and schedules the existing cancellable preview.
///
/// Rejected commands preserve history, drafts and accepted pixels. Equal replacements do not
/// dispatch, so incidental focus leave and unchanged easing selections preserve Redo.
pub(super) fn apply(
    state: &Rc<RefCell<AppState>>,
    command: Result<TemporalCommand, String>,
    descriptor: Option<&PropertyDescriptor>,
    focus: Option<InspectorFocusIdentity>,
) {
    let mut app = state.borrow_mut();
    if main_document_edits_blocked(&app) {
        return;
    }
    let result = command.and_then(|command| {
        let workspace = app.workspace.as_mut().ok_or("No document is open")?;
        if command.replacement() == &workspace.document().temporal_authority() {
            return Ok(false);
        }
        workspace
            .history
            .apply_temporal(&command)
            .map(|_| true)
            .map_err(|error| error.to_string())
    });
    match result {
        Ok(true) => {
            if let Some(descriptor) = descriptor {
                app.inspector_runtime
                    .drafts
                    .remove(&inspector_key(descriptor));
            }
            app.inspector_runtime.focus = focus;
            set_preview_pending(&mut app);
            set_inspector_status(&mut app, "Rendering selected frame…");
            sync_ui(&mut app);
            drop(app);
            rebuild_inspector(state);
            schedule_main_preview_submission(state);
        }
        Ok(false) => {}
        Err(error) => set_inspector_status(&mut app, error),
    }
}

/// Builds named reset/easing controls once for one eligible inspector row.
pub(super) fn controls(
    state: &Rc<RefCell<AppState>>,
    descriptor: &PropertyDescriptor,
) -> Option<Controls> {
    if !eligible(descriptor) {
        return None;
    }
    let field = inspector_field_label(descriptor.field);
    let expander = gtk::Expander::new(Some("Animation"));
    expander.update_property(&[gtk::accessible::Property::Label(&format!(
        "{field} animation"
    ))]);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let hint = gtk::Label::new(None);
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    content.append(&hint);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let label = gtk::Label::new(Some("_Easing"));
    label.set_use_underline(true);
    let easing =
        accessible_string_dropdown(&EASINGS.iter().map(|(name, _)| *name).collect::<Vec<_>>());
    easing.set_hexpand(true);
    label.set_mnemonic_widget(Some(&easing));
    easing.update_property(&[gtk::accessible::Property::Label(&format!("{field} easing"))]);
    row.append(&label);
    row.append(&easing);
    content.append(&row);
    let reset = gtk::Button::with_label("Reset End");
    reset.set_halign(gtk::Align::Start);
    reset.set_tooltip_text(Some(
        "Remove this End override and follow the ordinary Start settings and inherited animation.",
    ));
    reset.update_property(&[gtk::accessible::Property::Label(&format!(
        "Reset {field} End"
    ))]);
    content.append(&reset);
    expander.set_child(Some(&content));
    let syncing = Rc::new(Cell::new(false));
    let state_for_easing = Rc::clone(state);
    let descriptor_for_easing = descriptor.clone();
    let guard = syncing.clone();
    easing.connect_selected_notify(move |control| {
        if guard.get() {
            return;
        }
        let Some((_, easing)) = EASINGS.get(control.selected() as usize) else {
            return;
        };
        let command = {
            let app = state_for_easing.borrow();
            if app.endpoint != temporal_preview::Endpoint::End {
                return;
            }
            let Some(workspace) = app.workspace.as_ref() else {
                return;
            };
            easing_command(workspace.document(), &descriptor_for_easing, *easing)
        };
        apply(&state_for_easing, Ok(command), None, None);
    });
    let state_for_reset = Rc::clone(state);
    let descriptor_for_reset = descriptor.clone();
    reset.connect_clicked(move |_| {
        let command = {
            let app = state_for_reset.borrow();
            if app.endpoint != temporal_preview::Endpoint::End {
                return;
            }
            let Some(workspace) = app.workspace.as_ref() else {
                return;
            };
            reset_command(workspace.document(), &descriptor_for_reset)
        };
        apply(
            &state_for_reset,
            Ok(command),
            Some(&descriptor_for_reset),
            None,
        );
    });
    Some(Controls {
        expander,
        hint,
        reset,
        easing,
        syncing,
    })
}

/// Attaches an existing row's animation disclosure to its persistent GTK component.
pub(super) fn append(controls: &Controls, parent: &gtk::Box) {
    parent.append(&controls.expander);
}

/// Projects End applicability, stored easing and inheritance without dispatching a command.
pub(super) fn sync(app: &AppState, component: &DescriptorComponent) {
    let end = app.endpoint == temporal_preview::Endpoint::End;
    component.row.set_sensitive(true);
    if let Some(reset) = component.reset.as_ref() {
        reset.set_visible(!end);
    }
    if let Some(controls) = component.temporal.as_ref() {
        controls.expander.set_visible(end);
        let value = app
            .workspace
            .as_ref()
            .and_then(|workspace| scalar(workspace.document(), &component.value.descriptor));
        controls.hint.set_label(if value.is_some() {
            "This setting has an End override."
        } else {
            "End follows Start and inherited animation."
        });
        controls.reset.set_sensitive(value.is_some());
        controls.easing.set_sensitive(value.is_some());
        controls.syncing.set(true);
        controls.easing.set_selected(
            EASINGS
                .iter()
                .position(|(_, easing)| Some(*easing) == value.map(|value| value.easing))
                .unwrap_or(0) as u32,
        );
        controls.syncing.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::{FrameRange, FrameRate, ProjectTiming};

    /// Exercises inspector End conversion, easing and reset against independently inherited values.
    ///
    /// # Panics
    /// Panics if editing the selected endpoint changes Start, loses All inheritance, or breaks Undo.
    #[test]
    fn inspector_end_commands_preserve_start_inheritance_and_history() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap()
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(30, 1).unwrap(),
                FrameRange::new(0, 3).unwrap(),
            ),
            vec![],
        )
        .unwrap();
        let values = document.property_values();
        let base = &values
            .iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Document
                    && value.descriptor.field == PropertyFieldId::RotationDegrees
            })
            .unwrap()
            .descriptor;
        let channel = &values
            .iter()
            .find(|value| {
                value.descriptor.target == PropertyTarget::Channel(ChannelId(1))
                    && value.descriptor.field == PropertyFieldId::RotationDegrees
            })
            .unwrap()
            .descriptor;
        let mut history = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
        history
            .apply_temporal(&scalar_command(history.document(), base, 90.0).unwrap())
            .unwrap();
        history
            .apply_temporal(&scalar_command(history.document(), channel, 45.0).unwrap())
            .unwrap();
        let end = history.document().materialize_frame(2).unwrap();
        assert_eq!(
            end.effective_channel_pattern(ChannelId(1))
                .unwrap()
                .pattern_rotation_degrees,
            45.0
        );
        assert_eq!(
            end.effective_channel_pattern(ChannelId(2))
                .unwrap()
                .pattern_rotation_degrees,
            90.0
        );
        assert_eq!(
            history.document().pattern_settings(),
            document.pattern_settings()
        );
        history
            .apply_temporal(&easing_command(history.document(), channel, Easing::Hold))
            .unwrap();
        let middle = history.document().materialize_frame(1).unwrap();
        assert_eq!(
            middle
                .effective_channel_pattern(ChannelId(1))
                .unwrap()
                .pattern_rotation_degrees,
            45.0
        );
        history
            .apply_temporal(&scalar_command(history.document(), channel, 60.0).unwrap())
            .unwrap();
        assert_eq!(
            scalar(history.document(), channel).unwrap().easing,
            Easing::Hold
        );
        history
            .apply_temporal(&reset_command(history.document(), channel))
            .unwrap();
        assert_eq!(
            history
                .document()
                .materialize_frame(2)
                .unwrap()
                .effective_channel_pattern(ChannelId(1))
                .unwrap()
                .pattern_rotation_degrees,
            90.0
        );
        history.undo().unwrap();
        assert_eq!(
            history
                .document()
                .materialize_frame(2)
                .unwrap()
                .effective_channel_pattern(ChannelId(1))
                .unwrap()
                .pattern_rotation_degrees,
            60.0
        );
        history.redo().unwrap();
        assert!(scalar(history.document(), channel).is_none());
        assert_eq!(
            history
                .document()
                .materialize_frame(0)
                .unwrap()
                .pattern_settings(),
            document.pattern_settings()
        );
    }
}
