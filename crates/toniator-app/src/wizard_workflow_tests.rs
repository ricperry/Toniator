//! Repeatable Pattern Wizard regressions using production commands, widgets, and workers.

use super::*;
use std::time::Instant;

/// Loads immutable artwork and the reported current-format recipe without reading personal files.
///
/// # Panics
/// Panics if a checked-in source or recipe cannot initialize a normal workspace.
fn workspace() -> Workspace {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut workspace = load_workspace(&root.join("../../assets/vector-sample.svg")).unwrap();
    let preset =
        toniator_io::load_preset(&root.join("tests/fixtures/clustered-weighted-lloyd.preset.json"))
            .unwrap();
    let id = preset.metadata.id.clone();
    PresetRegistry::new(
        toniator_patterns::BUNDLED_PRESET_REGISTRY_VERSION,
        vec![preset],
    )
    .unwrap()
    .apply_to_document_base(&mut workspace.history, &id)
    .unwrap();
    workspace
}

/// Returns the active shared recipe descriptor for one field through the wizard's own projection.
///
/// # Panics
/// Panics if the requested control is not exposed by this recipe.
fn descriptor(document: &Document, field: PropertyFieldId) -> PropertyDescriptor {
    let (projection, _, _) =
        wizard_route_for_document(document, InspectorTarget::DocumentAll).unwrap();
    wizard_active_values(document, InspectorTarget::DocumentAll, &projection)
        .into_iter()
        .find(|value| value.descriptor.field == field)
        .unwrap()
        .descriptor
}

/// Checks actual scatter selector transitions retain relaxation, weighting and effective fineness.
///
/// # Panics
/// Panics if switching algorithms resets unrelated artist intent or Undo loses the prior recipe.
#[test]
fn scatter_switch_preserves_effective_settings_and_undo() {
    let mut workspace = workspace();
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, 0.1).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    let before = workspace.document().clone();
    let mut draft = DocumentHistory::new_draft(&workspace.history);
    let selector = descriptor(draft.document(), PropertyFieldId::RandomCharacter);
    let transition = draft
        .document()
        .variant_transition_draft(
            &selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    apply_wizard_transition(&mut draft, InspectorTarget::DocumentAll, &transition).unwrap();
    for field in [
        PropertyFieldId::RandomLloydEnabled,
        PropertyFieldId::RandomLloydDensityWeighted,
        PropertyFieldId::RandomLloydIterations,
        PropertyFieldId::RandomDensityModulation,
    ] {
        let values = |document: &Document| {
            document
                .property_values()
                .into_iter()
                .filter(|value| value.descriptor.field == field)
                .map(|value| value.value)
                .collect::<Vec<_>>()
        };
        assert_eq!(values(&before), values(draft.document()), "{field:?}");
    }
    for channel in authoritative_channel_ids(&before) {
        assert_eq!(
            draft
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .density,
            before.effective_channel_pattern(channel).unwrap().density
        );
    }
    draft.undo().unwrap();
    assert_eq!(draft.document(), &before);
}

/// Checks the same effective-value projection used by GTK distinguishes shared and mixed channels.
///
/// # Panics
/// Panics if the wizard reads the base value, averages channels, or changes the source document.
#[test]
fn effective_fineness_readback_preserves_mixed_channels() {
    let mut workspace = workspace();
    let field = descriptor(workspace.document(), PropertyFieldId::Density);
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, 0.1).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    assert_eq!(
        wizard_shared_numeric_text(workspace.document(), InspectorTarget::DocumentAll, &field)
            .flatten()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.1
    );
    let channel = authoritative_channel_ids(workspace.document())[0];
    let mut changed = workspace
        .document()
        .effective_channel_pattern(channel)
        .unwrap()
        .density;
    changed.density *= 2.0;
    let command = workspace
        .document()
        .set_channel_density_for_effective(channel, changed)
        .unwrap();
    workspace.history.apply(&command).unwrap();
    let before = workspace.document().clone();
    assert_eq!(
        wizard_shared_numeric_text(&before, InspectorTarget::DocumentAll, &field),
        Some(None)
    );
    assert_eq!(workspace.document(), &before);
}

/// Profiles native Poisson construction independently of thumbnail size without changing the recipe.
///
/// # Panics
/// Panics if the reported recipe or full evaluator fails; timings are diagnostics, not CI thresholds.
#[test]
#[ignore = "native-size performance diagnostic"]
fn poisson_native_profile() {
    let mut workspace = workspace();
    let size = std::env::var("TONIATOR_TEST_FEATURE_SIZE")
        .unwrap_or_else(|_| "0.1".into())
        .parse()
        .unwrap();
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, size).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    let selector = descriptor(workspace.document(), PropertyFieldId::RandomCharacter);
    let transition = workspace
        .document()
        .variant_transition_draft(
            &selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    apply_wizard_transition(
        &mut workspace.history,
        InspectorTarget::DocumentAll,
        &transition,
    )
    .unwrap();
    let mut media = toniator_engine::open_source_media(
        &workspace.sources,
        toniator_engine::MediaTools::default(),
        &|| false,
    )
    .unwrap();
    let session = DocumentSession::new(workspace.document().clone()).unwrap();
    let request = toniator_engine::frame_evaluation_request(
        &session,
        &mut media,
        temporal_preview::Endpoint::Start.frame(session.document()),
        &|| false,
    )
    .unwrap()
    .for_preview(toniator_engine::PreviewRasterTarget::new(128, 128).unwrap());
    let result = toniator_engine::evaluate_profiled_with_limits(
        request,
        toniator_engine::EvaluationLimits::default(),
    )
    .unwrap();
    eprintln!(
        "Poisson native feature size {size}: {:#?}",
        result.performance
    );
}

/// Pumps the real GTK event loop until an observable workflow condition holds, with a hard deadline.
///
/// # Panics
/// Panics with the named unmet condition instead of leaving a worker or modal waiting indefinitely.
fn wait_until(label: &str, seconds: u64, condition: impl Fn() -> bool) {
    let start = Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        assert!(
            start.elapsed() < Duration::from_secs(seconds),
            "timed out: {label}"
        );
        for _ in 0..32 {
            if !context.pending() {
                break;
            }
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Captures native private-session pixels when the suite runner requests a visual evidence folder.
///
/// # Panics
/// Panics if requested screenshot evidence cannot be written.
fn capture(name: &str) {
    let Ok(directory) = std::env::var("TONIATOR_WIZARD_EVIDENCE") else {
        return;
    };
    let context = glib::MainContext::default();
    for _ in 0..20 {
        for _ in 0..32 {
            if !context.pending() {
                break;
            }
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(10));
    }
    fs::create_dir_all(&directory).unwrap();
    assert!(
        std::process::Command::new("grim")
            .arg(Path::new(&directory).join(format!("{name}.png")))
            .status()
            .unwrap()
            .success()
    );
}

/// Borrows a real registered numeric entry by its authoritative field rather than display coordinates.
///
/// # Panics
/// Panics if the current wizard page does not expose the requested field.
fn entry(state: &Rc<RefCell<AppState>>, field: PropertyFieldId) -> gtk::Entry {
    state.borrow().pattern_wizard.as_ref().unwrap().text_inputs.iter().find_map(|input| {
        matches!(&input.kind, WizardTextInputKind::DescriptorFinite(descriptor) if descriptor.field == field)
            .then(|| input.entry.clone())
    }).unwrap()
}

/// Activates an enabled production wizard button after releasing the state borrow before its signal.
///
/// # Panics
/// Panics if navigation or publication offers a disabled control when the workflow requires it.
fn press(state: &Rc<RefCell<AppState>>, select: impl FnOnce(&PatternWizardSurface) -> gtk::Button) {
    let button = select(state.borrow().pattern_wizard.as_ref().unwrap());
    assert!(
        button.is_sensitive(),
        "disabled wizard button {:?}",
        button.label()
    );
    button.emit_clicked();
}

/// Finds a real confirmation button by its visible label in the GTK hierarchy.
fn button_named(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button.label().as_deref() == Some(name)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = button_named(&widget, name) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

/// Clicks a discard-confirmation action without bypassing the production confirmation callback.
///
/// # Panics
/// Panics if the expected modal action is absent or disabled.
fn confirm(name: &str) {
    let button = gtk::Window::list_toplevels()
        .iter()
        .find_map(|window| button_named(window, name))
        .unwrap();
    assert!(button.is_sensitive());
    button.emit_clicked();
}

/// Locates a native dropdown by its visible label and selects a model item through GTK's signal path.
///
/// # Panics
/// Panics if either the visible control or requested choice is absent.
fn select(page: &gtk::Widget, label: &str, choice: &str) {
    if let Some(text) = page.downcast_ref::<gtk::Label>()
        && text.text() == label
        && let Some(control) = text.mnemonic_widget().and_downcast::<gtk::DropDown>()
    {
        let model = control.model().unwrap();
        let index = (0..model.n_items())
            .find(|index| {
                model
                    .item(*index)
                    .and_downcast::<gtk::StringObject>()
                    .is_some_and(|item| item.string() == choice)
            })
            .unwrap();
        control.set_selected(index);
        return;
    }
    let mut child = page.first_child();
    while let Some(widget) = child {
        if contains_label(&widget, label) {
            select(&widget, label, choice);
            return;
        }
        child = widget.next_sibling();
    }
    panic!("missing dropdown {label}");
}

/// Searches the actual widget hierarchy for a visible product label.
fn contains_label(widget: &gtk::Widget, label: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|text| text.text() == label)
    {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if contains_label(&widget, label) {
            return true;
        }
        child = widget.next_sibling();
    }
    false
}

/// Exercises current-document readback, numeric activation, validation, algorithm switching and Apply.
/// Runs in the private Wayland harness with production GTK signals and asynchronous preview workers.
///
/// # Panics
/// Panics on stale/incorrect readback, blocked navigation, preview timeout, lost settings or wrong publication.
#[test]
#[ignore = "requires private GTK Wayland session; run scripts/test-pattern-wizard --gtk"]
fn gtk_current_pattern_edit_switch_review_apply() {
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.WizardRegression")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    let mut workspace = workspace();
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, 0.1).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    let initial = workspace.document().clone();
    state.borrow_mut().workspace = Some(workspace);
    state.borrow().window.present();
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    press(&state, |surface| surface.edit.clone());
    press(&state, |surface| surface.review.clone());
    press(&state, |surface| surface.review.clone());
    let size = entry(&state, PropertyFieldId::Density);
    assert_eq!(
        size.text().parse::<f64>().unwrap(),
        0.1,
        "current document readback"
    );
    wait_until("initial 0.1 preview", 10, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .preview_status
            .text()
            == "Pattern preview updated."
    });
    size.grab_focus();
    capture("drawing-effective-01");
    size.set_text("0");
    size.emit_activate();
    assert!(
        !state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .review
            .is_sensitive()
    );
    assert_eq!(
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .draft
            .borrow()
            .document(),
        &initial
    );
    size.set_text("0.2");
    size.emit_activate();
    wait_until("edited preview", 10, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .preview_status
            .text()
            == "Pattern preview updated."
    });
    press(&state, |surface| surface.back.clone());
    capture("clustered-family-options");
    let page = state.borrow().pattern_wizard.as_ref().unwrap().page.clone();
    select(page.upcast_ref(), "Scatter style", "Poisson disk");
    wait_until("Poisson selector commits", 10, || {
        let app = state.borrow();
        let surface = app.pattern_wizard.as_ref().unwrap();
        surface
            .draft
            .borrow()
            .document()
            .property_values()
            .iter()
            .any(|value| {
                value.descriptor.field == PropertyFieldId::RandomCharacter
                    && value.value
                        == PropertyCurrentValueKind::EnumChoice(
                            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
                        )
            })
    });
    press(&state, |surface| surface.review.clone());
    assert_eq!(
        entry(&state, PropertyFieldId::Density)
            .text()
            .parse::<f64>()
            .unwrap(),
        0.2
    );
    // Supersede a deliberately expensive preview, then navigate without waiting for it.
    let size = entry(&state, PropertyFieldId::Density);
    size.set_text("0.01");
    size.emit_activate();
    size.set_text("0.1");
    size.emit_activate();
    {
        let app = state.borrow();
        let draft = app.pattern_wizard.as_ref().unwrap().draft.borrow();
        let channel = authoritative_channel_ids(draft.document())[0];
        let value = draft
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .density
            .density;
        assert!(
            (artist_numeric_value(draft.document(), PropertyFieldId::Density, value).unwrap()
                - 0.1)
                .abs()
                < 1e-9
        );
    }
    assert!(
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .review
            .is_sensitive()
    );
    // Review must validate the actual recipe without altering fineness to make it cheap.
    let mut expected_recipe = DocumentHistory::new(DocumentSession::new(initial.clone()).unwrap());
    let selector = descriptor(expected_recipe.document(), PropertyFieldId::RandomCharacter);
    let transition = expected_recipe
        .document()
        .variant_transition_draft(
            &selector,
            PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
        )
        .unwrap();
    apply_wizard_transition(
        &mut expected_recipe,
        InspectorTarget::DocumentAll,
        &transition,
    )
    .unwrap();
    assert_eq!(
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .draft
            .borrow()
            .document()
            .pattern_definition_bundles(),
        expected_recipe.document().pattern_definition_bundles()
    );
    let review_started = Instant::now();
    press(&state, |surface| surface.review.clone());
    wait_until("Poisson Review enables Apply", 90, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .apply
            .is_sensitive()
    });
    {
        let app = state.borrow();
        let surface = app.pattern_wizard.as_ref().unwrap();
        let draft = surface.draft.borrow();
        assert!(
            surface
                .validation
                .ready(draft.document(), draft.revision().0)
        );
    }
    eprintln!(
        "Poisson Review at feature size 0.1: {:?}",
        review_started.elapsed()
    );
    capture("poisson-review-01");
    let expected = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .draft
        .borrow()
        .document()
        .clone();
    press(&state, |surface| surface.apply.clone());
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &expected
    );
    state
        .borrow_mut()
        .workspace
        .as_mut()
        .unwrap()
        .history
        .undo()
        .unwrap();
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &initial
    );
    // Cancel another dirty draft while its fine preview is pending; no edit may leak.
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    press(&state, |surface| surface.edit.clone());
    press(&state, |surface| surface.review.clone());
    press(&state, |surface| surface.review.clone());
    let size = entry(&state, PropertyFieldId::Density);
    size.set_text("0.01");
    size.emit_activate();
    let cancelled = Instant::now();
    press(&state, |surface| surface.cancel.clone());
    confirm("Keep editing");
    assert!(state.borrow().pattern_wizard.is_some());
    press(&state, |surface| surface.cancel.clone());
    confirm("Discard changes");
    assert!(cancelled.elapsed() < Duration::from_secs(1));
    assert!(state.borrow().pattern_wizard.is_none());
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &initial
    );
    let window = state.borrow().window.clone();
    window.close();
}
