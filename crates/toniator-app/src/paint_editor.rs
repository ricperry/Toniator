//! Advanced paint assignment edits the selected frame through canonical document authority.

use super::*;
use toniator_domain::{
    ColorAnimationEdit, ColorEndMode, ColorEndOverride, Easing, TemporalCommand,
    TemporalEndOverride,
};

/// Represents an explicit artist edit, independent of GTK formatting or selected preview pixels.
#[derive(Clone)]
enum Edit {
    StartColor(ColorValue),
    EndColor(ColorValue),
    StartAlpha(f64),
    EndAlpha(f64),
    Colors,
    Hue(f64),
    Easing(Easing),
    #[cfg(test)]
    Reset,
}

type ParseEdit = fn(&str) -> Result<Edit, String>;

/// Carries a validated configuration or temporal command through the existing history boundaries.
enum Change {
    Start(toniator_domain::DocumentConfiguration),
    End(TemporalCommand),
}

/// Returns this channel's grouped animation, if present, without interpreting its rendered pixels.
fn group(document: &Document, channel: ChannelId) -> Option<&ColorEndOverride> {
    document
        .temporal_end_overrides()
        .iter()
        .find_map(|entry| match entry {
            TemporalEndOverride::Color(value) if value.channel_id == channel => Some(value),
            _ => None,
        })
}

/// Identifies the four canonical paint components, independent of document channel roles.
pub(super) fn component(field: PropertyFieldId) -> Option<ColorComponent> {
    match field {
        PropertyFieldId::ColorRed => Some(ColorComponent::Red),
        PropertyFieldId::ColorGreen => Some(ColorComponent::Green),
        PropertyFieldId::ColorBlue => Some(ColorComponent::Blue),
        PropertyFieldId::ColorAlpha => Some(ColorComponent::Alpha),
        _ => None,
    }
}

/// Returns a shared value only when every projected channel/component agrees exactly.
fn common<T: PartialEq + Clone>(values: &[T]) -> Option<T> {
    values
        .first()
        .filter(|first| values.iter().all(|value| value == *first))
        .cloned()
}

/// Reads grouped easing or a unanimous component easing; mixed paths remain visibly mixed.
fn easing(document: &Document, channel: ChannelId) -> Option<Easing> {
    if let Some(group) = group(document, channel) {
        return Some(group.easing);
    }
    let values = document
        .temporal_end_overrides()
        .iter()
        .filter_map(|entry| match entry {
            TemporalEndOverride::Scalar(value)
                if value.target == PropertyTarget::Channel(channel)
                    && component(value.field).is_some() =>
            {
                Some(value.easing)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        Some(Easing::Linear)
    } else {
        common(&values)
    }
}

/// Prepares one explicit all-or-named-channel edit through domain batch and normalization APIs.
///
/// # Errors
/// Returns missing/sampled-paint, bounds, color-mode or complete-document diagnostics.
fn prepare(document: &Document, channels: &[ChannelId], edit: Edit) -> Result<Change, String> {
    let result = match edit {
        Edit::StartColor(_) | Edit::StartAlpha(_) => {
            let mut colors = Vec::new();
            for channel in channels {
                let mut color = document
                    .solid_paint(*channel)
                    .map_err(|error| error.to_string())?
                    .clone();
                match &edit {
                    Edit::StartColor(value) => color = value.clone(),
                    Edit::StartAlpha(alpha) => color.alpha = *alpha,
                    _ => unreachable!(),
                }
                colors.push((*channel, color));
            }
            return document
                .edit_start_colors_configuration(&colors)
                .map(Change::Start)
                .map_err(|error| error.to_string());
        }
        Edit::EndAlpha(alpha) => {
            document.edit_paint_component_end_command(channels, ColorComponent::Alpha, alpha)
        }
        Edit::Easing(next) => {
            let mut overrides = document.temporal_end_overrides().to_vec();
            for entry in &mut overrides {
                match entry {
                    TemporalEndOverride::Color(value) if channels.contains(&value.channel_id) => {
                        value.easing = next
                    }
                    TemporalEndOverride::Scalar(value) if component(value.field).is_some() => {
                        if let PropertyTarget::Channel(channel) = value.target
                            && channels.contains(&channel)
                        {
                            value.easing = next;
                        }
                    }
                    _ => {}
                }
            }
            Ok(document
                .replace_temporal_authority_command(document.project_timing().clone(), overrides))
        }
        edit => {
            let end = document
                .materialize_frame(temporal_preview::Endpoint::End.frame(document))
                .map_err(|error| error.to_string())?;
            let mut edits = Vec::new();
            for channel in channels {
                let mode = match &edit {
                    Edit::EndColor(color) => Some(ColorEndMode::LinearColor { end: color.clone() }),
                    Edit::Colors => Some(ColorEndMode::LinearColor {
                        end: end
                            .solid_paint(*channel)
                            .map_err(|error| error.to_string())?
                            .clone(),
                    }),
                    Edit::Hue(degrees) => Some(ColorEndMode::HueRotation {
                        end_degrees: *degrees,
                    }),
                    #[cfg(test)]
                    Edit::Reset => None,
                    _ => unreachable!(),
                };
                edits.push(ColorAnimationEdit {
                    channel_id: *channel,
                    mode,
                    easing: easing(document, *channel).unwrap_or(Easing::Linear),
                });
            }
            document.edit_color_animation_command(&edits)
        }
    };
    result.map(Change::End).map_err(|error| error.to_string())
}

/// Applies one color transaction only to the still-live Advanced draft.
/// Cancel discards these changes; Apply publishes the whole draft as one history entry.
fn apply(context: &AdvancedCommitContext, channels: &[ChannelId], edit: Result<Edit, String>) {
    if context.refreshing.get() {
        return;
    }
    let Some((draft, _, _, status)) = context.surface_handles() else {
        return;
    };
    let result = edit
        .and_then(|edit| prepare(draft.borrow().document(), channels, edit))
        .and_then(|change| {
            let mut history = draft.borrow_mut();
            match change {
                Change::Start(configuration) => {
                    let base = history.document().clone();
                    let revision = history.revision();
                    history
                        .apply_document_configuration(&base, revision, &configuration)
                        .map(|result| !result.unchanged)
                        .map_err(|error| error.to_string())
                }
                Change::End(command) => {
                    if command.replacement() == &history.document().temporal_authority() {
                        return Ok(false);
                    }
                    history
                        .apply_temporal(&command)
                        .map(|_| true)
                        .map_err(|error| error.to_string())
                }
            }
        });
    match result {
        Ok(changed) => {
            advanced_temporal::refresh(&context.state, context.epoch);
            if changed {
                submit_advanced_preview(&context.state, context.epoch);
            }
        }
        Err(error) => status.set_label(&error),
    }
}

/// Retains a displayed string so focus traversal never commits quantized HEX back into authority.
struct Entry {
    widget: gtk::Entry,
    displayed: Rc<RefCell<String>>,
}

impl Entry {
    /// Creates one native entry with labeled semantic identity and guarded activate/focus commits.
    fn new(
        context: &AdvancedCommitContext,
        channels: &[ChannelId],
        guard: &Rc<Cell<bool>>,
        name: &str,
        parse: fn(&str) -> Result<Edit, String>,
    ) -> Self {
        let widget = gtk::Entry::new();
        widget.set_width_chars(9);
        widget.set_hexpand(true);
        widget.set_placeholder_text(Some("Mixed"));
        widget.update_property(&[gtk::accessible::Property::Label(name)]);
        let displayed = Rc::new(RefCell::new(String::new()));
        let context = context.clone();
        let channels = channels.to_vec();
        let guard = guard.clone();
        let last = displayed.clone();
        let commit: Rc<dyn Fn(&gtk::Entry)> = Rc::new(move |entry| {
            if guard.get() || entry.text().as_str() == last.borrow().as_str() {
                return;
            }
            apply(&context, &channels, parse(entry.text().as_str()));
        });
        let activate = commit.clone();
        widget.connect_activate(move |entry| activate(entry));
        let focus = gtk::EventControllerFocus::new();
        let entry = widget.clone();
        focus.connect_leave(move |_| commit(&entry));
        widget.add_controller(focus);
        Self { widget, displayed }
    }

    /// Updates display under the owner's guard, recording the exact non-authoritative string.
    fn set(&self, text: Option<String>, enabled: bool) {
        let text = text.unwrap_or_default();
        *self.displayed.borrow_mut() = text.clone();
        if self.widget.text() != text {
            self.widget.set_text(&text);
        }
        self.widget.set_sensitive(enabled);
    }
}

/// Retains one selected-frame paint editor without storing authored colors in widget state.
pub(super) struct Controls {
    channel: ChannelId,
    guard: Rc<Cell<bool>>,
    color: Entry,
    alpha: Entry,
    picker: gtk::ColorDialogButton,
    hue: Entry,
    hue_row: gtk::Box,
    mode: gtk::DropDown,
    easing: gtk::DropDown,
}

/// Parses explicitly entered HEX into canonical straight linear paint.
fn start_hex(text: &str) -> Result<Edit, String> {
    ColorValue::from_srgb_hex(text)
        .map(Edit::StartColor)
        .map_err(|error| error.to_string())
}
/// Parses selected End paint into the existing grouped interpolation authority.
fn end_hex(text: &str) -> Result<Edit, String> {
    ColorValue::from_srgb_hex(text)
        .map(Edit::EndColor)
        .map_err(|error| error.to_string())
}
/// Parses selected Start alpha without quantizing RGB.
fn start_alpha(text: &str) -> Result<Edit, String> {
    text.parse()
        .map(Edit::StartAlpha)
        .map_err(|_| "Enter a number from 0 to 1.".into())
}
/// Parses selected End alpha without replacing its independent easing.
fn end_alpha(text: &str) -> Result<Edit, String> {
    text.parse()
        .map(Edit::EndAlpha)
        .map_err(|_| "Enter a number from 0 to 1.".into())
}
/// Parses an unwrapped hue offset; domain validation rejects nonfinite values.
fn hue(text: &str) -> Result<Edit, String> {
    text.parse()
        .map(Edit::Hue)
        .map_err(|_| "Enter a finite angle in degrees.".into())
}

/// Appends a visible label related to its native interactive widget.
fn labeled(parent: &gtk::Box, name: &str, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_mnemonic_widget(Some(widget));
    row.append(&label);
    row.append(widget);
    parent.append(&row);
}

impl Controls {
    /// Flushes explicitly changed color text into an unpublished Apply draft without refreshing widgets.
    /// Untouched formatted HEX never rewrites color precision; a later error discards the whole draft.
    ///
    /// # Errors
    /// Returns parsing, color ownership, bounds, or history errors without publishing main history.
    pub(super) fn commit_pending(
        &self,
        history: &mut DocumentHistory,
        endpoint: temporal_preview::Endpoint,
    ) -> Result<(), String> {
        let at_start = endpoint == temporal_preview::Endpoint::Start;
        let fields: [(&Entry, ParseEdit); 3] = [
            (&self.color, if at_start { start_hex } else { end_hex }),
            (&self.alpha, if at_start { start_alpha } else { end_alpha }),
            (&self.hue, hue),
        ];
        for (entry, parse) in fields {
            if !entry.widget.is_sensitive()
                || entry.widget.text().as_str() == entry.displayed.borrow().as_str()
            {
                continue;
            }
            match prepare(
                history.document(),
                &[self.channel],
                parse(entry.widget.text().as_str())?,
            )? {
                Change::Start(configuration) => {
                    let base = history.document().clone();
                    history
                        .apply_document_configuration(&base, history.revision(), &configuration)
                        .map_err(|error| error.to_string())?;
                }
                Change::End(command) => {
                    history
                        .apply_temporal(&command)
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        Ok(())
    }

    /// Builds the ordinary Color editor inside the selected channel's Advanced settings.
    /// The surrounding endpoint toggle selects the edit destination; controls never duplicate it.
    pub(super) fn new(
        context: AdvancedCommitContext,
        parent: &gtk::Box,
        channel: ChannelId,
        endpoint: temporal_preview::Endpoint,
    ) -> Self {
        let guard = context.refreshing.clone();
        let at_start = endpoint == temporal_preview::Endpoint::Start;
        let color = Entry::new(
            &context,
            &[channel],
            &guard,
            "HEX",
            if at_start { start_hex } else { end_hex },
        );
        color.widget.set_tooltip_text(Some("Enter #RRGGBB or #RRGGBBAA. Eight digits include alpha. Assigns color to the frame selected below the canvas."));
        let alpha = Entry::new(
            &context,
            &[channel],
            &guard,
            "Alpha",
            if at_start { start_alpha } else { end_alpha },
        );
        alpha.widget.set_input_purpose(gtk::InputPurpose::Number);
        alpha
            .widget
            .set_tooltip_text(Some("Paint strength: 0 is transparent; 1 is opaque."));
        let frame = gtk::Frame::new(Some("Color"));
        frame.update_property(&[gtk::accessible::Property::Label("Color")]);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(8);
        content.set_margin_end(8);
        frame.set_child(Some(&content));
        parent.append(&frame);
        let dialog = gtk::ColorDialog::builder()
            .title("Choose color")
            .with_alpha(true)
            .modal(true)
            .build();
        let picker = gtk::ColorDialogButton::new(Some(dialog));
        picker.update_property(&[gtk::accessible::Property::Label("Choose color")]);
        picker.set_tooltip_text(Some("Choose a color for the selected frame."));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&picker);
        let hex_label = gtk::Label::new(Some("HEX"));
        hex_label.set_mnemonic_widget(Some(&color.widget));
        row.append(&hex_label);
        row.append(&color.widget);
        content.append(&row);
        labeled(&content, "Alpha", &alpha.widget);
        let interpolation = gtk::Expander::new(Some("Color interpolation"));
        interpolation.update_property(&[gtk::accessible::Property::Label("Color interpolation")]);
        let options = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let mode = accessible_string_dropdown(&["Colors", "Hue rotation"]);
        labeled(&options, "Transition", &mode);
        let hue = Entry::new(&context, &[channel], &guard, "Hue degrees", hue);
        hue.widget.set_input_purpose(gtk::InputPurpose::Number);
        let hue_row = gtk::Box::new(gtk::Orientation::Vertical, 0);
        labeled(&hue_row, "Hue degrees", &hue.widget);
        options.append(&hue_row);
        let easing = accessible_string_dropdown(
            &temporal_edit::EASINGS
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
        );
        labeled(&options, "Interpolation", &easing);
        interpolation.set_child(Some(&options));
        interpolation.set_visible(!at_start);
        content.append(&interpolation);
        let callback = context.clone();
        mode.connect_selected_notify(move |control| {
            if callback.refreshing.get() {
                return;
            }
            match control.selected() {
                0 => apply(&callback, &[channel], Ok(Edit::Colors)),
                1 => apply(&callback, &[channel], Ok(Edit::Hue(0.0))),
                _ => {}
            }
        });
        let callback = context.clone();
        easing.connect_selected_notify(move |control| {
            if callback.refreshing.get() {
                return;
            }
            if let Some((_, easing)) = temporal_edit::EASINGS.get(control.selected() as usize) {
                apply(&callback, &[channel], Ok(Edit::Easing(*easing)));
            }
        });
        picker.connect_rgba_notify(move |control| {
            if context.refreshing.get() {
                return;
            }
            let rgba = control.rgba();
            let edit = ColorValue::from_srgb_rgba([
                f64::from(rgba.red()),
                f64::from(rgba.green()),
                f64::from(rgba.blue()),
                f64::from(rgba.alpha()),
            ])
            .map(|color| {
                if at_start {
                    Edit::StartColor(color)
                } else {
                    Edit::EndColor(color)
                }
            })
            .map_err(|error| error.to_string());
            apply(&context, &[channel], edit);
        });
        Self {
            channel,
            guard,
            color,
            alpha,
            picker,
            hue,
            hue_row,
            mode,
            easing,
        }
    }

    /// Refreshes the selected frame's paint without parsing rounded presentation back into storage.
    ///
    /// # Errors
    /// Returns materialization or unsupported-paint diagnostics without changing authority.
    pub(super) fn refresh(
        &self,
        document: &Document,
        endpoint: temporal_preview::Endpoint,
    ) -> Result<(), String> {
        let display = advanced_temporal::display_document(document, endpoint)?;
        let paint = display
            .solid_paint(self.channel)
            .map_err(|error| error.to_string())?;
        let hex = paint.to_srgb_hex().map_err(|error| error.to_string())?;
        let rgba = paint.srgb_rgba().map_err(|error| error.to_string())?;
        let previous = self.guard.replace(true);
        self.color.set(Some(hex), true);
        self.alpha.set(Some(paint.alpha.to_string()), true);
        self.picker.set_rgba(&gtk::gdk::RGBA::new(
            rgba[0] as f32,
            rgba[1] as f32,
            rgba[2] as f32,
            rgba[3] as f32,
        ));
        let hue = group(document, self.channel).and_then(|group| match group.mode {
            ColorEndMode::HueRotation { end_degrees } => Some(end_degrees),
            _ => None,
        });
        self.mode.set_selected(u32::from(hue.is_some()));
        self.hue_row.set_visible(hue.is_some());
        self.hue
            .set(hue.map(|value| value.to_string()), hue.is_some());
        let easing = easing(document, self.channel).unwrap_or(Easing::Linear);
        self.easing.set_selected(
            temporal_edit::EASINGS
                .iter()
                .position(|(_, value)| *value == easing)
                .unwrap_or(0) as u32,
        );
        self.guard.set(previous);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the same prepared transaction boundary without constructing GTK state.
    ///
    /// # Panics
    /// Panics when a valid artist edit cannot be prepared or atomically applied.
    fn execute(history: &mut DocumentHistory, channels: &[ChannelId], edit: Edit) {
        match prepare(history.document(), channels, edit).unwrap() {
            Change::Start(configuration) => {
                let base = history.document().clone();
                history
                    .apply_document_configuration(&base, history.revision(), &configuration)
                    .unwrap();
            }
            Change::End(command) => {
                history.apply_temporal(&command).unwrap();
            }
        }
    }

    /// Keeps precision and independent alpha when artists change modes or edit All paint strength.
    ///
    /// # Panics
    /// Panics if an edit quantizes RGB, changes another channel, loses alpha easing or fails undo.
    #[test]
    fn artist_edits_keep_precision_and_independent_hue_alpha() {
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
                toniator_domain::FrameRate::new(24, 1).unwrap(),
                toniator_domain::FrameRange::new(0, 3).unwrap(),
            ),
            vec![],
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let all = authoritative_channel_ids(history.document());
        let first = [all[0]];
        let precise = ColorValue {
            red: 0.123456789123,
            green: 0.23456789123,
            blue: 0.3456789123,
            alpha: 0.987654321,
        };
        execute(&mut history, &first, Edit::StartColor(precise.clone()));
        execute(&mut history, &all, Edit::StartAlpha(0.6));
        assert_eq!(
            history.document().solid_paint(first[0]).unwrap().red,
            precise.red
        );
        execute(&mut history, &first, end_hex("#ABCDEF80").unwrap());
        execute(&mut history, &first, Edit::Hue(-720.0));
        execute(&mut history, &all, Edit::EndAlpha(0.314159265359));
        execute(&mut history, &first, Edit::Easing(Easing::SmoothStep));
        let end = history.document().materialize_frame(2).unwrap();
        for channel in all {
            assert_eq!(end.solid_paint(channel).unwrap().alpha, 0.314159265359);
        }
        assert!(matches!(
            group(history.document(), first[0]).unwrap().mode,
            ColorEndMode::HueRotation {
                end_degrees: -720.0
            }
        ));
        assert_eq!(
            group(history.document(), first[0]).unwrap().easing,
            Easing::SmoothStep
        );
        assert!(history.document().temporal_end_overrides().iter().any(|entry| matches!(entry,
            TemporalEndOverride::Scalar(value) if value.target == PropertyTarget::Channel(first[0]) && value.field == PropertyFieldId::ColorAlpha && value.easing == Easing::SmoothStep)));
        let before = history.document().clone();
        execute(&mut history, &first, Edit::Reset);
        assert!(group(history.document(), first[0]).is_none());
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
    }

    /// Produces native RGB PNG and CMYK SVG endpoint witnesses through the real desktop exporter.
    ///
    /// # Panics
    /// Panics on immutable source loading, domain edits, current-schema persistence or export failure.
    #[test]
    fn color_endpoints_export_native_rgb_and_cmyk_witnesses() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/stage22-color-authoring")
            .join(format!(
                "run-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        for (source, model, name, format) in [
            (
                "raster-sample.png",
                HalftoneChannelModel::Rgb,
                "rgb",
                ExportFormat::Png,
            ),
            (
                "vector-sample.svg",
                HalftoneChannelModel::Cmyk,
                "cmyk",
                ExportFormat::Svg,
            ),
        ] {
            let mut workspace = load_workspace(&root.join("assets").join(source)).unwrap();
            if model == HalftoneChannelModel::Cmyk {
                let channel = authoritative_channel_ids(workspace.document())[0];
                let template = ChannelTopologyTemplate {
                    pattern_instance: workspace
                        .document()
                        .channel_pattern_instance(channel)
                        .unwrap()
                        .clone(),
                };
                workspace
                    .history
                    .apply(&DocumentCommand::ReplaceChannelTopology {
                        model,
                        topology: toniator_domain::ChannelTopology::canonical(model, template)
                            .unwrap(),
                    })
                    .unwrap();
            }
            let channels = authoritative_channel_ids(workspace.document());
            let colors = if model == HalftoneChannelModel::Rgb {
                vec!["#FF6B6B80", "#4ECDC4AA", "#FFE66DFF"]
            } else {
                vec!["#00B4D880", "#FF006E99", "#FFD166FF", "#3A0CA3FF"]
            };
            let edits = channels
                .iter()
                .zip(colors)
                .map(|(channel_id, hex)| ColorAnimationEdit {
                    channel_id: *channel_id,
                    mode: Some(ColorEndMode::LinearColor {
                        end: ColorValue::from_srgb_hex(hex).unwrap(),
                    }),
                    easing: Easing::SmoothInOut,
                })
                .collect::<Vec<_>>();
            let command = workspace
                .document()
                .edit_color_animation_command(&edits)
                .unwrap();
            workspace.history.apply_temporal(&command).unwrap();
            let snapshot = workspace.snapshot();
            let project = directory.join(format!("{name}-colors.toniator"));
            save_container(&project, &snapshot.document, &snapshot.sources).unwrap();
            assert_eq!(
                load_workspace(&project).unwrap().document(),
                workspace.document()
            );
            for endpoint in [
                temporal_preview::Endpoint::Start,
                temporal_preview::Endpoint::End,
            ] {
                let frame = endpoint.frame(workspace.document());
                let suffix = if endpoint == temporal_preview::Endpoint::Start {
                    "start"
                } else {
                    "end"
                };
                let extension = if format == ExportFormat::Png {
                    "png"
                } else {
                    "svg"
                };
                export_snapshot_frame(
                    snapshot.clone(),
                    directory.join(format!("{name}-{suffix}.{extension}")),
                    ExportSettings {
                        format,
                        background: RasterBackground::Transparent,
                        antialiasing: RasterAntialiasing::On,
                        output_target: None,
                    },
                    frame,
                )
                .unwrap();
            }
        }
        println!("Color witnesses: {}", directory.display());
    }
}
