//! Selected-endpoint preview preparation and private Advanced Settings bindings.

use super::*;
use toniator_engine::{FrameSource, MediaTools};

/// Owns one cancellable source preparation for the lifetime of the private dialog.
pub(super) struct SourceWorker {
    cancelled: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl SourceWorker {
    /// Prepares the selected media endpoint off the GTK thread and returns it under the modal epoch.
    ///
    /// # Errors
    /// Returns a thread-spawn diagnostic; provider or proxy errors arrive as a private status.
    pub(super) fn new(
        document: Document,
        sources: SourceBundle,
        endpoint: temporal_preview::Endpoint,
        sender: async_channel::Sender<AppEvent>,
        epoch: u64,
    ) -> Result<Self, String> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let thread = thread::Builder::new()
            .name("toniator-advanced-media".into())
            .spawn(move || {
                let result = prepare_source(&document, &sources, endpoint, &|| {
                    flag.load(Ordering::Acquire)
                });
                if !flag.load(Ordering::Acquire) {
                    let _ = sender.send_blocking(AppEvent::AdvancedSource { epoch, result });
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            cancelled,
            thread: Some(thread),
        })
    }
}

impl Drop for SourceWorker {
    /// Cancels preparation and joins its owner, ensuring decoder children are reaped on close.
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Opens shared media and creates the existing bounded box-averaged proxy for its exact endpoint.
///
/// # Errors
/// Returns media, timing, cancellation or proxy diagnostics without mutating the document/source.
pub(super) fn prepare_source(
    document: &Document,
    sources: &SourceBundle,
    endpoint: temporal_preview::Endpoint,
    cancelled: &dyn Fn() -> bool,
) -> Result<AdvancedPreviewSource, String> {
    let mut provider =
        toniator_engine::open_source_media(sources, MediaTools::default(), cancelled)
            .map_err(|error| error.to_string())?;
    let time = document
        .project_timing()
        .source_time_for_frame(endpoint.frame(document))
        .map_err(|error| error.to_string())?;
    let frame = provider
        .frame_at(time, cancelled)
        .map_err(|error| error.to_string())?;
    let proxy = toniator_engine::reduced_preview_frame(&frame, 128, cancelled)
        .map_err(|error| error.to_string())?;
    let width = proxy.field.identity().width;
    let height = proxy.field.identity().height;
    let SourceReference::Assigned(id) = document.source() else {
        return Err("No source artwork is assigned".into());
    };
    let source =
        ResolvedSource::from_frame(id.clone(), proxy).map_err(|error| error.to_string())?;
    Ok(AdvancedPreviewSource::Ready {
        source,
        width,
        height,
    })
}

/// Accepts only this live dialog's source preparation and previews its latest private revision.
pub(super) fn source_ready(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    result: Result<AdvancedPreviewSource, String>,
) {
    {
        let mut app = state.borrow_mut();
        let Some(surface) = app
            .advanced_settings
            .as_mut()
            .filter(|surface| surface.epoch == epoch)
        else {
            return;
        };
        surface.preview_source = result.unwrap_or_else(AdvancedPreviewSource::Unavailable);
    }
    submit_advanced_preview(state, epoch);
}

/// Projects the selected endpoint of the private draft without replacing ordinary Start authority.
pub(super) fn display_document(
    document: &Document,
    endpoint: temporal_preview::Endpoint,
) -> Result<Document, String> {
    match endpoint {
        temporal_preview::Endpoint::Start => Ok(document.clone()),
        temporal_preview::Endpoint::End => document
            .materialize_frame(endpoint.frame(document))
            .map_err(|error| error.to_string()),
    }
}

/// Retains a descriptor locator and its native widgets across ordinary private scalar edits.
pub(super) struct Binding {
    target: InspectorTarget,
    locator: AdvancedDescriptorLocator,
    input: gtk::Widget,
    reset_start: Option<gtk::Button>,
    animation: Option<(gtk::Button, gtk::DropDown, gtk::Label)>,
}

/// Applies a current effective Advanced property through the selected endpoint's history authority.
///
/// # Errors
/// Rejects stale locators, invalid values and coupled bounds without changing the draft.
pub(super) fn apply_input(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    target: InspectorTarget,
    locator: AdvancedDescriptorLocator,
    input: InspectorInput,
) -> Result<bool, String> {
    let display = display_document(history.document(), endpoint)?;
    let current = resolve_current_advanced_value(&display, target, locator)?;
    let unchanged = match (&current.value, &input) {
        (PropertyCurrentValueKind::FiniteF64(a), InspectorInput::FiniteF64(b)) => a == b,
        (PropertyCurrentValueKind::U32(a), InspectorInput::U32(b)) => a == b,
        (PropertyCurrentValueKind::Boolean(a), InspectorInput::Boolean(b)) => a == b,
        (PropertyCurrentValueKind::EnumChoice(a), InspectorInput::EnumChoice(b)) => a == b,
        _ => false,
    };
    if unchanged {
        return Ok(false);
    }
    if endpoint == temporal_preview::Endpoint::End && temporal_edit::eligible(&current.descriptor) {
        let InspectorInput::FiniteF64(value) = input else {
            return Err("This setting is shared by all frames; edit it at Start frame.".into());
        };
        let command =
            temporal_edit::scalar_command(history.document(), &current.descriptor, value)?;
        history
            .apply_temporal(&command)
            .map_err(|error| error.to_string())?;
    } else {
        let command = command_for_inspector_input(
            history.document(),
            target.channel_id(),
            DefinitionEditScope::SelectedCopy,
            &current.descriptor,
            input,
        )?;
        history.apply(&command).map_err(|error| error.to_string())?;
    }
    Ok(true)
}

/// Flushes pending numeric text before Apply, including keyboard/default and AT-SPI activation.
/// Captures all pending values before refresh can overwrite a sibling entry. Main history is untouched.
///
/// # Errors
/// Returns an artist-facing input/domain diagnostic and leaves the dialog open for correction.
pub(super) fn commit_pending(state: &Rc<RefCell<AppState>>, epoch: u64) -> Result<(), String> {
    let (draft, endpoint, batches, edits) = {
        let app = state.borrow();
        let surface = app
            .advanced_settings
            .as_ref()
            .filter(|surface| surface.epoch == epoch)
            .ok_or("These Advanced Settings are no longer open.")?;
        let document = display_document(surface.draft.borrow().document(), surface.endpoint)?;
        let batches = surface
            .batch_editors
            .iter()
            .map(advanced_batches::Controls::pending)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut edits = Vec::new();
        for binding in &surface.bindings {
            let Some(entry) = binding.input.downcast_ref::<gtk::Entry>() else {
                continue;
            };
            let current =
                resolve_current_advanced_value(&document, binding.target, binding.locator)?;
            let error = || {
                format!(
                    "{}: enter a valid number.",
                    inspector_field_label(current.descriptor.field)
                )
            };
            let input = match current.value {
                PropertyCurrentValueKind::FiniteF64(current) => {
                    let value = entry
                        .text()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or_else(error)?;
                    if value == current {
                        continue;
                    }
                    InspectorInput::FiniteF64(value)
                }
                PropertyCurrentValueKind::U32(current) => {
                    let value = entry.text().parse::<u32>().map_err(|_| error())?;
                    if value == current {
                        continue;
                    }
                    InspectorInput::U32(value)
                }
                _ => continue,
            };
            edits.push((binding.target, binding.locator, input));
        }
        (surface.draft.clone(), surface.endpoint, batches, edits)
    };
    let mut draft = draft.borrow_mut();
    let mut pending = DocumentHistory::new_draft(&draft);
    advanced_batches::apply_fields(&mut pending, endpoint, &batches)?;
    apply_pending_inputs(&mut pending, endpoint, &edits)?;
    {
        let app = state.borrow();
        let surface = app
            .advanced_settings
            .as_ref()
            .ok_or("Advanced Settings closed during Apply.")?;
        for editor in &surface.paint_editors {
            editor.commit_pending(&mut pending, endpoint)?;
        }
    }
    draft
        .squash_draft(&pending)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Commits pending effective scalar pairs together before remaining discrete inputs.
/// Locators resolve against the current draft; the domain validates all coupled scalar bounds at once.
///
/// # Errors
/// Returns inactive-descriptor, bounds, or history diagnostics; the caller discards the private draft.
pub(super) fn apply_pending_inputs(
    history: &mut DocumentHistory,
    endpoint: temporal_preview::Endpoint,
    edits: &[(InspectorTarget, AdvancedDescriptorLocator, InspectorInput)],
) -> Result<(), String> {
    let display = display_document(history.document(), endpoint)?;
    let mut scalars = Vec::new();
    let mut other = Vec::new();
    for (target, locator, input) in edits {
        let current = resolve_current_advanced_value(&display, *target, *locator)?;
        if let InspectorInput::FiniteF64(value) = input
            && display
                .channel_scalar_batch(current.descriptor.field)
                .is_ok_and(|batch| {
                    batch
                        .values
                        .iter()
                        .any(|entry| entry.target == current.descriptor.target)
                })
        {
            scalars.push((current.descriptor.target, current.descriptor.field, *value));
        } else {
            other.push((*target, *locator, input.clone()));
        }
    }
    if !scalars.is_empty() {
        match endpoint {
            temporal_preview::Endpoint::Start => {
                let base = history.document().clone();
                let configuration = base
                    .edit_channel_start_configuration(&scalars)
                    .map_err(|error| error.to_string())?;
                history
                    .apply_document_configuration(&base, history.revision(), &configuration)
                    .map_err(|error| error.to_string())?;
            }
            temporal_preview::Endpoint::End => {
                let edits = scalars
                    .into_iter()
                    .map(|(target, field, effective_end)| {
                        let easing = history
                            .document()
                            .temporal_end_overrides()
                            .iter()
                            .find_map(|entry| match entry {
                                toniator_domain::TemporalEndOverride::Scalar(value)
                                    if value.target == target && value.field == field =>
                                {
                                    Some(value.easing)
                                }
                                _ => None,
                            })
                            .unwrap_or(toniator_domain::Easing::Linear);
                        toniator_domain::TemporalEndpointEdit {
                            target,
                            field,
                            effective_end,
                            easing,
                        }
                    })
                    .collect::<Vec<_>>();
                let command = history
                    .document()
                    .edit_effective_end_command(&edits)
                    .map_err(|error| error.to_string())?;
                history
                    .apply_temporal(&command)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    for (target, locator, input) in other {
        apply_input(history, endpoint, target, locator, input)?;
    }
    Ok(())
}

/// Rebuilds capability-dependent groups when the dialog opens or a discrete setting changes.
///
/// All exposes shared/mixed values and progressively disclosed named-channel details.
/// Each effective output setting appears once. Apply remains one draft publication.
pub(super) fn rebuild_fields(state: &Rc<RefCell<AppState>>, epoch: u64) {
    let (draft, target, endpoint, fields, guard) = {
        let mut app = state.borrow_mut();
        let Some(surface) = app
            .advanced_settings
            .as_mut()
            .filter(|surface| surface.epoch == epoch)
        else {
            return;
        };
        surface.bindings.clear();
        surface.paint_editors.clear();
        surface.batch_editors.clear();
        (
            surface.draft.clone(),
            surface.target,
            surface.endpoint,
            surface.fields.clone(),
            surface.refreshing.clone(),
        )
    };
    let document = match display_document(draft.borrow().document(), endpoint) {
        Ok(document) => document,
        Err(error) => {
            state
                .borrow()
                .advanced_settings
                .as_ref()
                .unwrap()
                .status
                .set_label(&error);
            return;
        }
    };
    guard.set(true);
    while let Some(child) = fields.first_child() {
        fields.remove(&child);
    }
    let channels = match target {
        InspectorTarget::DocumentAll => authoritative_channel_ids(&document),
        InspectorTarget::Channel(channel) => vec![channel],
    };
    let mut bindings = Vec::new();
    let mut paint_editors = Vec::new();
    let batch_editors = if target == InspectorTarget::DocumentAll {
        advanced_batches::append(state, epoch, &document, &fields, &guard)
    } else {
        Vec::new()
    };
    for channel in channels {
        let target = InspectorTarget::Channel(channel);
        let group_name = format!("{} settings", channel_display_name(&document, channel));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(8);
        content.set_margin_end(8);
        if !batch_editors.is_empty() {
            let group = gtk::Expander::new(Some(&group_name));
            group.update_property(&[gtk::accessible::Property::Label(&group_name)]);
            group.set_child(Some(&content));
            fields.append(&group);
        } else {
            let group = gtk::Frame::new(Some(&group_name));
            group.update_property(&[gtk::accessible::Property::Label(&group_name)]);
            group.set_child(Some(&content));
            fields.append(&group);
        }
        if document.solid_paint(channel).is_ok() {
            paint_editors.push(paint_editor::Controls::new(
                AdvancedCommitContext {
                    state: state.clone(),
                    epoch,
                    target,
                    refreshing: guard.clone(),
                },
                &content,
                channel,
                endpoint,
            ));
        }
        let values = advanced_settings_values(&document, target);
        let fill_content = source_group(&content, "Fill response", None);
        let weighting_used = document
            .effective_channel_pattern(channel)
            .ok()
            .and_then(|effective| {
                document
                    .pattern_definition_bundles()
                    .iter()
                    .find(|bundle| bundle.definition.id == effective.definition_id)
                    .map(|bundle| &bundle.definition)
            })
            .is_some_and(toniator_domain::definition_uses_source_weighting);
        let weighting_content = source_group(
            &content,
            "Source weighting",
            (!weighting_used).then_some(
                "These settings are stored for this channel. The current pattern does not use source weighting.",
            ),
        );
        for value in values.iter().filter(|value| {
            !matches!(value.descriptor.target, PropertyTarget::ChannelOutput(_, _))
                && paint_editor::component(value.descriptor.field).is_none()
        }) {
            if let Some(binding) = append_binding(
                state,
                &draft,
                &document,
                epoch,
                target,
                endpoint,
                match source_consumer_group(value.descriptor.field) {
                    Some("Fill response") => &fill_content,
                    Some("Source weighting") => &weighting_content,
                    _ => &content,
                },
                value.clone(),
                &guard,
            ) {
                bindings.push(binding);
            }
        }
        if let Ok(capabilities) =
            document.pattern_capabilities(PatternCapabilityScope::Channel(channel))
        {
            let multiple_outputs = capabilities.outputs.len() > 1;
            for output in capabilities.outputs {
                let output_values = values
                    .iter()
                    .filter(|value| {
                        value.descriptor.target
                            == PropertyTarget::ChannelOutput(channel, output.output_layer_id)
                    })
                    .collect::<Vec<_>>();
                if output_values.is_empty() {
                    continue;
                }
                let output_content = if multiple_outputs {
                    let output_name = format!("Output {}", output.painter_index + 1);
                    let output_group = gtk::Frame::new(Some(&output_name));
                    output_group.update_property(&[gtk::accessible::Property::Label(&output_name)]);
                    let output_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    output_group.set_child(Some(&output_content));
                    content.append(&output_group);
                    output_content
                } else {
                    content.clone()
                };
                for value in output_values {
                    if let Some(binding) = append_binding(
                        state,
                        &draft,
                        &document,
                        epoch,
                        target,
                        endpoint,
                        &output_content,
                        value.clone(),
                        &guard,
                    ) {
                        bindings.push(binding);
                    }
                }
            }
        }
    }
    if let Some(surface) = state
        .borrow_mut()
        .advanced_settings
        .as_mut()
        .filter(|surface| surface.epoch == epoch)
    {
        surface.bindings = bindings;
        surface.paint_editors = paint_editors;
        surface.batch_editors = batch_editors;
    }
    guard.set(false);
    refresh(state, epoch);
}

/// Groups independently mapped source consumers using visible, accessible product names.
/// Optional guidance explains retained inactive state without disabling valid channel edits.
fn source_group(parent: &gtk::Box, name: &str, explanation: Option<&str>) -> gtk::Box {
    let group = gtk::Frame::new(Some(name));
    group.update_property(&[gtk::accessible::Property::Label(name)]);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    if let Some(text) = explanation {
        let label = gtk::Label::new(Some(text));
        label.set_wrap(true);
        label.set_max_width_chars(48);
        label.set_xalign(0.0);
        label.add_css_class("dim-label");
        content.append(&label);
    }
    group.set_child(Some(&content));
    parent.append(&group);
    content
}

/// Appends one typed row and its optional End reset/easing disclosure.
/// The row factory owns descriptor-derived reset names; endpoint binding controls visibility.
#[allow(clippy::too_many_arguments)] // Captured modal/descriptor context is explicit at this binding boundary.
fn append_binding(
    state: &Rc<RefCell<AppState>>,
    draft: &Rc<RefCell<DocumentHistory>>,
    document: &Document,
    epoch: u64,
    target: InspectorTarget,
    endpoint: temporal_preview::Endpoint,
    parent: &gtk::Box,
    current: PropertyCurrentValue,
    guard: &Rc<Cell<bool>>,
) -> Option<Binding> {
    let descriptor = current.descriptor.clone();
    let locator = advanced_descriptor_locator(document, target, &descriptor)?;
    let row = advanced_descriptor_row(draft.clone(), state.clone(), epoch, target, current);
    // The row factory owns this stable label/input/reset composition; this is not a search of
    // arbitrary runtime widgets or a second descriptor identity system.
    let input = row.first_child()?.next_sibling()?;
    let reset_start = input.next_sibling().and_downcast::<gtk::Button>();
    if let Some(reset) = &reset_start {
        reset.set_visible(endpoint == temporal_preview::Endpoint::Start);
    }
    let hue_controls_rgb = matches!(
        paint_editor::component(descriptor.field),
        Some(ColorComponent::Red | ColorComponent::Green | ColorComponent::Blue)
    ) && draft
        .borrow()
        .document()
        .temporal_end_overrides()
        .iter()
        .any(|entry| {
            matches!(entry,
            toniator_domain::TemporalEndOverride::Color(value)
            if descriptor.target == PropertyTarget::Channel(value.channel_id)
                && matches!(value.mode, toniator_domain::ColorEndMode::HueRotation { .. }))
        });
    let editable = endpoint == temporal_preview::Endpoint::Start || !hue_controls_rgb;
    row.set_sensitive(editable);
    input.set_sensitive(editable);
    parent.append(&row);
    let animation = if endpoint == temporal_preview::Endpoint::End
        && temporal_edit::eligible(&descriptor)
    {
        let disclosure_name = format!("{} animation", inspector_field_label(descriptor.field));
        let disclosure = gtk::Expander::new(Some(&disclosure_name));
        disclosure.update_property(&[gtk::accessible::Property::Label(&disclosure_name)]);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let hint = gtk::Label::new(None);
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        content.append(&hint);
        let label = gtk::Label::new(Some("_Easing"));
        label.set_use_underline(true);
        let easing = accessible_string_dropdown(
            &temporal_edit::EASINGS
                .iter()
                .map(|(label, _)| *label)
                .collect::<Vec<_>>(),
        );
        label.set_mnemonic_widget(Some(&easing));
        content.append(&label);
        content.append(&easing);
        let context = AdvancedCommitContext {
            state: state.clone(),
            epoch,
            target,
            refreshing: guard.clone(),
        };
        let context_for_easing = context.clone();
        easing.connect_selected_notify(move |control| {
            if context_for_easing.refreshing.get() {
                return;
            }
            let Some((_, easing)) = temporal_edit::EASINGS.get(control.selected() as usize) else {
                return;
            };
            context_for_easing.set_easing(locator, *easing);
        });
        let reset = gtk::Button::with_label("Reset End");
        let context_for_reset = context;
        reset.connect_clicked(move |_| context_for_reset.reset(locator));
        content.append(&reset);
        disclosure.set_child(Some(&content));
        parent.append(&disclosure);
        Some((reset, easing, hint))
    } else {
        None
    };
    Some(Binding {
        target,
        locator,
        input,
        reset_start,
        animation,
    })
}

/// Refreshes retained private values after scalar/source edits or resets without replacing widgets.
/// Descriptor choice ordering remains authoritative; the refresh guard suppresses reentrant commits.
/// Keeping native controls preserves channel expansion, focus and scroll position.
pub(super) fn refresh(state: &Rc<RefCell<AppState>>, epoch: u64) {
    refresh_values(state, epoch, false);
}

/// Reprojects the complete reset scope, discarding pending entry text even if native focus remains.
/// The usual refresh guard suppresses signals; main history and preview publication remain separate.
pub(super) fn refresh_after_reset(state: &Rc<RefCell<AppState>>, epoch: u64) {
    refresh_values(state, epoch, true);
}

/// Projects authoritative private values in place; explicit reset may replace focused/error text.
/// Normal edits preserve pending focused numeric input and all refreshes guard recursive callbacks.
fn refresh_values(state: &Rc<RefCell<AppState>>, epoch: u64, discard_pending: bool) {
    let app = state.borrow();
    let Some(surface) = app
        .advanced_settings
        .as_ref()
        .filter(|surface| surface.epoch == epoch)
    else {
        return;
    };
    let draft = surface.draft.borrow();
    let Ok(document) = display_document(draft.document(), surface.endpoint) else {
        return;
    };
    surface.refreshing.set(true);
    for editor in &surface.batch_editors {
        editor.refresh(&document, discard_pending);
    }
    for editor in &surface.paint_editors {
        if let Err(error) = editor.refresh(draft.document(), surface.endpoint) {
            surface.status.set_label(&error);
        }
    }
    for binding in &surface.bindings {
        let Ok(current) =
            resolve_current_advanced_value(&document, binding.target, binding.locator)
        else {
            continue;
        };
        if let Some(entry) = binding.input.downcast_ref::<gtk::Entry>()
            && (discard_pending || !entry.has_focus())
        {
            let text = match current.value {
                PropertyCurrentValueKind::FiniteF64(value) => value.to_string(),
                PropertyCurrentValueKind::U32(value) => value.to_string(),
                _ => continue,
            };
            if entry.text() != text {
                entry.set_text(&text);
            }
        }
        if let Some(dropdown) = binding.input.downcast_ref::<gtk::DropDown>()
            && let PropertyCurrentValueKind::EnumChoice(value) = current.value
            && let Some(selected) = current
                .descriptor
                .choices
                .iter()
                .position(|choice| *choice == value)
            && dropdown.selected() != selected as u32
        {
            dropdown.set_selected(selected as u32);
        }
        if let Some(toggle) = binding.input.downcast_ref::<gtk::Switch>()
            && let PropertyCurrentValueKind::Boolean(value) = current.value
            && toggle.is_active() != value
        {
            toggle.set_active(value);
        }
        if let Some(reset) = &binding.reset_start {
            reset.set_sensitive(current.inheritance == PropertyInheritance::Explicit);
        }
        if let Some((reset, easing, hint)) = &binding.animation {
            let value = temporal_edit::scalar(draft.document(), &current.descriptor);
            reset.set_sensitive(value.is_some());
            easing.set_sensitive(value.is_some());
            let grouped = paint_editor::component(current.descriptor.field).is_some()
                && draft
                    .document()
                    .temporal_end_overrides()
                    .iter()
                    .any(|entry| {
                        matches!(entry,
                    toniator_domain::TemporalEndOverride::Color(value)
                    if current.descriptor.target == PropertyTarget::Channel(value.channel_id))
                    });
            hint.set_label(if value.is_some() {
                "This setting has an End override."
            } else if grouped {
                "This component uses the Color interpolation setting."
            } else {
                "End follows Start and inherited animation."
            });
            easing.set_selected(
                temporal_edit::EASINGS
                    .iter()
                    .position(|(_, easing)| Some(*easing) == value.map(|value| value.easing))
                    .unwrap_or(0) as u32,
            );
        }
    }
    surface.refreshing.set(false);
}

/// Defers capability-changing row replacement until the triggering GTK signal has returned.
pub(super) fn schedule_rebuild(state: &Rc<RefCell<AppState>>, epoch: u64) {
    let state = Rc::downgrade(state);
    glib::idle_add_local_once(move || {
        if let Some(state) = state.upgrade() {
            rebuild_fields(&state, epoch);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Commits weighting levels and ALL gamma at either endpoint without altering fill interpretation.
    ///
    /// # Panics
    /// Panics if channel-owned weighting bypasses endpoint validation or leaks into fill state.
    #[test]
    fn source_consumers_weighting_levels_preserve_fill_at_both_endpoints() {
        for endpoint in [
            temporal_preview::Endpoint::Start,
            temporal_preview::Endpoint::End,
        ] {
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
            history
                .apply(&DocumentCommand::SetSourceWeightingField {
                    channel_id: ChannelId(1),
                    edit: toniator_domain::SourceWeightingFieldEdit::WhitePoint(0.4),
                })
                .unwrap();
            history
                .apply_temporal(&history.document().initialize_end_command().unwrap())
                .unwrap();
            let target = InspectorTarget::Channel(ChannelId(1));
            let display = display_document(history.document(), endpoint).unwrap();
            let values = advanced_settings_values(&display, target);
            let edits = [
                (PropertyFieldId::ArtworkWeightMappingBlackPoint, 0.6),
                (PropertyFieldId::ArtworkWeightMappingWhitePoint, 0.9),
            ]
            .into_iter()
            .map(|(field, value)| {
                let current = values
                    .iter()
                    .find(|value| value.descriptor.field == field)
                    .unwrap();
                (
                    target,
                    advanced_descriptor_locator(&display, target, &current.descriptor).unwrap(),
                    InspectorInput::FiniteF64(value),
                )
            })
            .collect::<Vec<_>>();
            apply_pending_inputs(&mut history, endpoint, &edits).unwrap();
            let result = display_document(history.document(), endpoint).unwrap();
            let tone = result.channel_weighting(ChannelId(1)).unwrap().mapping.tone;
            assert_eq!((tone.black_point, tone.white_point), (0.6, 0.9));
            assert_eq!(
                result.modeled_channel(ChannelId(1)).unwrap().mapping.tone,
                toniator_domain::SourceTone::identity()
            );
            if endpoint == temporal_preview::Endpoint::End {
                assert_eq!(
                    history
                        .document()
                        .channel_weighting(ChannelId(1))
                        .unwrap()
                        .mapping
                        .tone
                        .white_point,
                    0.4
                );
            }
            assert!(
                advanced_batches::apply(
                    &mut history,
                    endpoint,
                    PropertyFieldId::ArtworkWeightMappingGamma,
                    1.7
                )
                .unwrap()
            );
            let all = display_document(history.document(), endpoint).unwrap();
            for channel in authoritative_channel_ids(&all) {
                assert_eq!(
                    all.channel_weighting(channel).unwrap().mapping.tone.gamma,
                    1.7
                );
                assert_eq!(
                    all.modeled_channel(channel).unwrap().mapping.tone,
                    toniator_domain::SourceTone::identity()
                );
            }
        }
    }

    /// Applies a valid named-channel black/white pair independent of its old sibling bounds.
    ///
    /// # Panics
    /// Panics if pending Apply publishes order-dependent scalar edits or changes another channel.
    #[test]
    fn pending_named_bounds_validate_as_one_pair_at_either_endpoint() {
        for endpoint in [
            temporal_preview::Endpoint::Start,
            temporal_preview::Endpoint::End,
        ] {
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
            history
                .apply(&DocumentCommand::SetModeledMappingField {
                    channel_id: ChannelId(1),
                    edit: ModeledMappingFieldEdit::WhitePoint(0.4),
                })
                .unwrap();
            let command = history.document().initialize_end_command().unwrap();
            history.apply_temporal(&command).unwrap();
            let target = InspectorTarget::Channel(ChannelId(1));
            let display = display_document(history.document(), endpoint).unwrap();
            let values = advanced_settings_values(&display, target);
            let edits = [
                (PropertyFieldId::ModeledMappingBlackPoint, 0.6),
                (PropertyFieldId::ModeledMappingWhitePoint, 0.9),
            ]
            .into_iter()
            .map(|(field, value)| {
                let descriptor = &values
                    .iter()
                    .find(|value| value.descriptor.field == field)
                    .unwrap()
                    .descriptor;
                (
                    target,
                    advanced_descriptor_locator(&display, target, descriptor).unwrap(),
                    InspectorInput::FiniteF64(value),
                )
            })
            .collect::<Vec<_>>();
            apply_pending_inputs(&mut history, endpoint, &edits).unwrap();
            let display = display_document(history.document(), endpoint).unwrap();
            for (field, expected) in [
                (PropertyFieldId::ModeledMappingBlackPoint, 0.6),
                (PropertyFieldId::ModeledMappingWhitePoint, 0.9),
            ] {
                let value = advanced_settings_values(&display, target)
                    .into_iter()
                    .find(|value| value.descriptor.field == field)
                    .unwrap();
                assert_eq!(value.value, PropertyCurrentValueKind::FiniteF64(expected));
            }
        }
    }

    /// Proves still and moving private previews retain their endpoint and bounded decoded source.
    ///
    /// # Panics
    /// Panics on immutable-fixture loading, proxy bounds, cancellation or canonical evaluation failure.
    #[test]
    fn advanced_endpoint_sources_use_bounded_shared_media() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for name in [
            "raster-sample.png",
            "vector-sample.svg",
            "video-sample0001-0010.mp4",
        ] {
            let workspace = load_workspace(&root.join(name)).unwrap();
            let document = workspace.document();
            let mut identities = Vec::new();
            for endpoint in [
                temporal_preview::Endpoint::Start,
                temporal_preview::Endpoint::End,
            ] {
                let AdvancedPreviewSource::Ready {
                    source,
                    width,
                    height,
                } = prepare_source(document, &workspace.sources, endpoint, &|| false).unwrap()
                else {
                    panic!("ready source");
                };
                assert_eq!(width.max(height), 128);
                assert!(source.bytes().is_none(), "no encoded proxy round trip");
                let request = EvaluationRequest::with_preview_target(
                    workspace
                        .history
                        .session()
                        .document_evaluation_snapshot_at_frame(endpoint.frame(document))
                        .unwrap(),
                    source,
                    toniator_engine::PreviewRasterTarget::new(32, 32).unwrap(),
                );
                let result = evaluate_with_limits(request, EvaluationLimits::default()).unwrap();
                identities.push(result.source_identity().decoded_pixel_hash.clone());
            }
            if name.ends_with("mp4") {
                assert_ne!(identities[0], identities[1]);
            } else {
                assert_eq!(identities[0], identities[1]);
            }
            assert!(
                prepare_source(
                    document,
                    &workspace.sources,
                    temporal_preview::Endpoint::End,
                    &|| true
                )
                .is_err()
            );
        }
    }

    /// Proves Advanced End response edits stay private and publish as one undoable transaction.
    ///
    /// # Panics
    /// Panics if private tonal edits alter Start, another channel, or the main draft boundary.
    #[test]
    fn private_end_response_edits_squash_without_changing_start() {
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
        let mut main = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
        let mut draft = DocumentHistory::new_draft(&main);
        for (channel, end) in [(ChannelId(1), 0.3), (ChannelId(2), 0.6)] {
            let display =
                display_document(draft.document(), temporal_preview::Endpoint::End).unwrap();
            let current = advanced_settings_values(&display, InspectorTarget::Channel(channel))
                .into_iter()
                .find(|value| value.descriptor.field == PropertyFieldId::ModeledMappingGamma)
                .unwrap();
            let command =
                temporal_edit::scalar_command(draft.document(), &current.descriptor, end).unwrap();
            draft.apply_temporal(&command).unwrap();
        }
        assert_eq!(main.document(), &document);
        main.squash_draft(&draft).unwrap();
        assert_eq!(main.document().temporal_end_overrides().len(), 2);
        assert_eq!(
            main.document()
                .materialize_frame(0)
                .unwrap()
                .pattern_definition_bundles(),
            document.pattern_definition_bundles()
        );
        main.undo().unwrap();
        assert_eq!(main.document(), &document);
        main.redo().unwrap();
        assert_eq!(main.document(), draft.document());
    }
}
