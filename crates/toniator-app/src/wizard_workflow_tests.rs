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
    let preset = crate::current_test_fixtures::clustered_weighted_lloyd();
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
pub(super) fn wait_until(label: &str, seconds: u64, condition: impl Fn() -> bool) {
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
        thread::sleep(Duration::from_millis(1));
    }
}

/// Captures native private-session pixels when the suite runner requests a visual evidence folder.
///
/// # Panics
/// Panics if requested screenshot evidence cannot be written.
pub(super) fn capture(name: &str) {
    if std::env::var_os("TONIATOR_WIZARD_FOCUSED_CAPTURE").is_some()
        && matches!(name, "drawing-effective-025" | "clustered-family-options")
    {
        return;
    }
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
    let ui = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.agents/skills/gtk-wayland-debug/scripts/ui");
    let scope = if gtk::Window::list_toplevels().iter().any(|window| {
        window.downcast_ref::<gtk::Window>().is_some_and(|window| {
            window.is_visible() && window.title().as_deref() == Some("Advanced Settings")
        })
    }) {
        "Advanced Settings"
    } else if gtk::Window::list_toplevels().iter().any(|window| {
        window.downcast_ref::<gtk::Window>().is_some_and(|window| {
            window.is_visible() && window.title().as_deref() == Some("Pattern Wizard")
        })
    }) {
        "Pattern Wizard"
    } else {
        ""
    };
    if !scope.is_empty() {
        // AT-SPI calls require the GTK event loop; never synchronously wait on it from GTK.
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let output = std::process::Command::new("timeout")
                .arg("25s")
                .arg(ui)
                .args([
                    "controls",
                    "--subtree",
                    scope,
                    "--depth",
                    if scope == "Pattern Wizard" { "5" } else { "8" },
                    "--json",
                    "--application",
                    "",
                ])
                .output()
                .unwrap();
            let _ = sender.send(output);
        });
        let result = RefCell::new(None);
        wait_until("scoped AT-SPI capture", 45, || {
            if let Ok(output) = receiver.try_recv() {
                *result.borrow_mut() = Some(output);
            }
            result.borrow().is_some()
        });
        let output = result.into_inner().unwrap();
        fs::write(
            Path::new(&directory).join(format!("{name}-controls.json")),
            output.stdout,
        )
        .unwrap();
        fs::write(
            Path::new(&directory).join(format!("{name}-controls.stderr")),
            output.stderr,
        )
        .unwrap();
        assert!(output.status.success());
    }
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
pub(super) fn button_named(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button
            .label()
            .is_some_and(|label| label.trim_start_matches('_') == name)
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
pub(super) fn confirm(name: &str) {
    let button = gtk::Window::list_toplevels()
        .iter()
        .find_map(|window| button_named(window, name))
        .unwrap();
    assert!(button.is_sensitive());
    button.emit_clicked();
}

/// Finds a dropdown through its visible mnemonic label in the real GTK hierarchy.
pub(super) fn dropdown(widget: &gtk::Widget, name: &str) -> Option<gtk::DropDown> {
    if let Some(label) = widget.downcast_ref::<gtk::Label>()
        && label.text().trim_start_matches('_') == name
        && let Some(input) = label.mnemonic_widget().and_downcast::<gtk::DropDown>()
    {
        return Some(input);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(input) = dropdown(&widget, name) {
            return Some(input);
        }
        child = widget.next_sibling();
    }
    None
}

/// Locates a native dropdown by its visible label and selects a model item through GTK's signal path.
///
/// # Panics
/// Panics if either the visible control or requested choice is absent.
pub(super) fn select(page: &gtk::Widget, label: &str, choice: &str) {
    if let Some(text) = page.downcast_ref::<gtk::Label>()
        && text.text().trim_start_matches('_') == label
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
pub(super) fn contains_label(widget: &gtk::Widget, label: &str) -> bool {
    if widget
        .downcast_ref::<gtk::Label>()
        .is_some_and(|text| text.text().trim_start_matches('_') == label)
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

/// Resolves a new stage-owned output path without touching earlier validation evidence.
///
/// # Panics
/// Panics when the private runner omits its explicitly scoped evidence directory.
pub(super) fn evidence_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(std::env::var("TONIATOR_WIZARD_EVIDENCE").unwrap());
    fs::create_dir_all(&directory).unwrap();
    directory.join(name)
}

/// Saves and reopens a current workspace with exact document and embedded-source equality.
///
/// # Panics
/// Panics on serialization, current-schema rejection, or changed authority after reopen.
pub(super) fn save_exact(workspace: &Workspace, name: &str) {
    let path = evidence_path(name);
    assert!(
        !path.exists(),
        "evidence already exists: {}",
        path.display()
    );
    save_container(&path, workspace.document(), &workspace.sources).unwrap();
    assert_eq!(
        load_workspace(&path).unwrap().snapshot(),
        workspace.snapshot()
    );
}

/// Waits for a fresh accepted production preview after a source, Apply, or history transition.
///
/// # Panics
/// Panics if a current preview is not accepted within the bounded native-render allowance.
pub(super) fn render_current(state: &Rc<RefCell<AppState>>) {
    let previous = state.borrow().preview_coordinator.last_accepted_ticket();
    set_preview_pending(&mut state.borrow_mut());
    sync_ui(&mut state.borrow_mut());
    schedule_main_preview_submission(state);
    wait_until("fresh main preview", 90, || {
        let app = state.borrow();
        app.preview.is_some()
            && app.preview_coordinator.last_accepted_ticket().is_some()
            && app.preview_coordinator.last_accepted_ticket() != previous
    });
}

/// Rebinds an actually measured Wizard configuration to both immutable source fixtures without fitting.
///
/// # Panics
/// Panics on source/canvas mutation, changed saved binding, a nonempty fit plan, or failed preview.
pub(super) fn reuse_both_sources(state: &Rc<RefCell<AppState>>, donor: &Document, prefix: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = toniator_domain::DocumentConfiguration::capture(donor);
    for (name, source) in [
        ("raster", "raster-sample.png"),
        ("vector", "vector-sample.svg"),
    ] {
        let mut workspace = load_workspace(&root.join("assets").join(source)).unwrap();
        let before = workspace.snapshot();
        let candidate = config.bind(workspace.document()).unwrap();
        assert_eq!(candidate.source(), before.document.source());
        assert_eq!(candidate.canvas(), before.document.canvas());
        assert_eq!(
            candidate.baseline_calibration_binding().unwrap(),
            donor.baseline_calibration_binding().unwrap()
        );
        assert_eq!(
            candidate.baseline_saved_calibration().unwrap(),
            donor.baseline_saved_calibration().unwrap()
        );
        assert!(
            calibration_preparation::plan_to_prepare(
                &candidate,
                &authoritative_channel_ids(&candidate),
                false,
                false
            )
            .unwrap()
            .is_empty()
        );
        workspace.history = fresh_history(candidate).unwrap();
        assert_eq!(workspace.sources, before.sources);
        save_exact(&workspace, &format!("{prefix}-{name}.toniator"));
        install_workspace(state, workspace);
        render_current(state);
        capture(&format!("{prefix}-{name}"));
        eprintln!(
            "{prefix}-{name}: exact saved companion reused; zero new fits; fresh native preview accepted"
        );
    }
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
    // Only this live regression prepares a truthful rendered Undo target. Other fixtures stay cheap.
    if let Some(path) = std::env::var_os("TONIATOR_INITIAL_CALIBRATED") {
        let cached = load_workspace(Path::new(&path)).unwrap();
        assert_eq!(cached.sources, workspace.sources);
        assert_eq!(cached.document().source(), workspace.document().source());
        assert_eq!(
            cached.document().baseline_calibration_binding().unwrap(),
            workspace.document().baseline_calibration_binding().unwrap()
        );
        assert_eq!(
            cached.document().pattern_settings(),
            workspace.document().pattern_settings()
        );
        assert_eq!(
            cached.document().response_settings(),
            workspace.document().response_settings()
        );
        let document = toniator_domain::DocumentConfiguration::capture(cached.document())
            .bind(workspace.document())
            .unwrap();
        assert!(
            calibration_preparation::plan_to_prepare(
                &document,
                &authoritative_channel_ids(&document),
                false,
                false
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            toniator_domain::DocumentConfiguration::capture(&document),
            toniator_domain::DocumentConfiguration::capture(cached.document())
        );
        workspace.history = fresh_history(document).unwrap();
        eprintln!("Initial exact Clustered/Lloyd companion reopened and reused: zero new fits");
    } else {
        let plan = calibration_preparation::plan_to_prepare(
            workspace.document(),
            &authoritative_channel_ids(workspace.document()),
            false,
            false,
        )
        .unwrap();
        assert_eq!(plan.active.len(), 1);
        assert!(
            !plan.baseline,
            "the active representative also satisfies hidden ALL"
        );
        eprintln!(
            "Initial exact Clustered/Lloyd preparation: 1 distinct fit, hidden ALL deduplicated"
        );
        let started = Instant::now();
        let document = workspace.document().clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = cancelled.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let result = calibration_preparation::prepare(
                &document,
                &authoritative_channel_ids(&document),
                false,
                false,
                &token,
            );
            let _ = sender.send(result);
        });
        let prepared = loop {
            match receiver.recv_timeout(Duration::from_secs(30)) {
                Ok(result) => break result.unwrap(),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!(
                        "Initial Clustered/Lloyd fit pending: {:?}",
                        started.elapsed()
                    );
                    if started.elapsed() >= Duration::from_secs(180) {
                        cancelled.store(true, Ordering::Release);
                        panic!(
                            "initial exact Clustered/Lloyd preparation exceeded its measured-cost allowance"
                        );
                    }
                }
                Err(error) => panic!("initial preparation worker failed: {error}"),
            }
        };
        attach_calibrations_to_history(&mut workspace.history, prepared).unwrap();
        eprintln!(
            "Initial exact Clustered/Lloyd preparation accepted: {:?}",
            started.elapsed()
        );
    }
    // The bounded native witness uses the reported .25 size on both sides of Undo/Redo.
    let density =
        authority_numeric_value(workspace.document(), PropertyFieldId::Density, 0.25).unwrap();
    advanced_batches::apply(
        &mut workspace.history,
        temporal_preview::Endpoint::Start,
        PropertyFieldId::Density,
        density,
    )
    .unwrap();
    assert!(
        calibration_preparation::plan_to_prepare(
            workspace.document(),
            &authoritative_channel_ids(workspace.document()),
            false,
            false
        )
        .unwrap()
        .is_empty()
    );
    save_exact(&workspace, "initial-clustered-lloyd-calibrated.toniator");
    let initial = workspace.document().clone();
    let initial_revision = workspace.history.revision();
    install_workspace(&state, workspace);
    state.borrow().window.present();
    render_current(&state);
    capture("initial-clustered-main");
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    press(&state, |surface| surface.edit.clone());
    press(&state, |surface| surface.review.clone());
    press(&state, |surface| surface.review.clone());
    let size = entry(&state, PropertyFieldId::Density);
    assert_eq!(
        size.text().parse::<f64>().unwrap(),
        0.25,
        "current document readback"
    );
    wait_until("initial 0.25 preview", 10, || {
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
    capture("drawing-effective-025");
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
    // Keep the fine preview supersession above, then bound this native construction witness
    // to the reported Feature 0.25 regression. This changes only the test's authored choice.
    let size = entry(&state, PropertyFieldId::Density);
    size.set_text("0.25");
    size.emit_activate();
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
        "Poisson Review at feature size 0.25: {:?}",
        review_started.elapsed()
    );
    capture("poisson-review-025");
    let expected = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .draft
        .borrow()
        .document()
        .clone();
    {
        let app = state.borrow();
        let sources = &app.workspace.as_ref().unwrap().sources;
        let path = evidence_path("reviewed-poisson-025-uncalibrated.toniator");
        assert!(!path.exists());
        save_container(&path, &expected, sources).unwrap();
        assert_eq!(load_workspace(&path).unwrap().document(), &expected);
    }
    let apply_started = Instant::now();
    if let Some(path) = std::env::var_os("TONIATOR_WIZARD_MEASURED_REPLAY") {
        // Replay only an actual measured result retained after a successful real worker publication.
        // This hook certifies remaining history/reuse; it does not claim another measurement.
        let measured = load_workspace(Path::new(&path)).unwrap();
        measured
            .document()
            .require_saved_calibration_authority()
            .unwrap();
        assert_eq!(
            measured.sources,
            state.borrow().workspace.as_ref().unwrap().sources
        );
        assert_eq!(measured.document().source(), expected.source());
        assert_eq!(measured.document().canvas(), expected.canvas());
        assert_eq!(
            measured.document().pattern_definition_bundles(),
            expected.pattern_definition_bundles()
        );
        assert_eq!(
            measured.document().pattern_settings(),
            expected.pattern_settings()
        );
        assert_eq!(
            measured.document().response_settings(),
            expected.response_settings()
        );
        assert_eq!(
            measured.document().channel_topology(),
            expected.channel_topology()
        );
        assert_eq!(
            measured.document().baseline_calibration_binding().unwrap(),
            expected.baseline_calibration_binding().unwrap()
        );
        let plan = calibration_preparation::plan_to_prepare(
            &expected,
            &authoritative_channel_ids(&expected),
            false,
            false,
        )
        .unwrap();
        assert_eq!(plan.active.len(), 1);
        assert!(!plan.baseline);
        let active = plan
            .active
            .into_iter()
            .map(|channel_id| {
                let artifact = measured
                    .document()
                    .saved_calibration_for(channel_id)
                    .unwrap()
                    .unwrap()
                    .clone();
                assert_eq!(
                    artifact.binding(),
                    &toniator_domain::CalibrationBinding::for_channel(&expected, channel_id)
                        .unwrap()
                );
                toniator_domain::AttachedCalibration {
                    channel_id,
                    artifact,
                }
            })
            .collect();
        let (base_document, base_revision, workspace_generation, epoch, candidate_revision) = {
            let app = state.borrow();
            let workspace = app.workspace.as_ref().unwrap();
            let surface = app.pattern_wizard.as_ref().unwrap();
            assert!(
                surface
                    .validation
                    .ready(&expected, surface.draft.borrow().revision().0)
            );
            assert_eq!(workspace.document(), &initial);
            assert_eq!(workspace.history.revision(), initial_revision);
            (
                workspace.document().clone(),
                workspace.history.revision(),
                app.workspace_generation,
                surface.epoch,
                surface.draft.borrow().revision(),
            )
        };
        let id = calibration_preparation::TaskId(20_000);
        {
            let mut app = state.borrow_mut();
            let surface = app.pattern_wizard.as_mut().unwrap();
            assert!(surface.calibration_task.is_none());
            surface.calibration_task = Some(WizardCalibrationTask {
                id,
                epoch,
                workspace_generation,
                base_document,
                base_revision,
                candidate_document: expected.clone(),
                candidate_revision,
                cancelled: Arc::new(AtomicBool::new(false)),
                publication: WizardCalibrationPublication::DraftApply,
            });
            set_wizard_calibration_pending(surface, true);
        }
        handle_wizard_calibration(
            &state,
            id,
            Ok(calibration_preparation::PreparedCalibrations {
                active,
                baseline: None,
            }),
        );
        eprintln!(
            "REPLAY: retained actual measured companion passed exact candidate/source/binding/root guards; zero new fits"
        );
    } else {
        press(&state, |surface| surface.apply.clone());
    }
    // Test-only bound: this exact fit measured 157.78s before and 118.03s after intersection reuse.
    wait_until(
        "calibration finishes preparing the reviewed Pattern",
        180,
        || {
            state
                .borrow()
                .pattern_wizard
                .as_ref()
                .is_none_or(|surface| surface.calibration_task.is_none())
        },
    );
    let diagnostic = state.borrow().pattern_wizard.as_ref().map(|surface| {
        (
            surface.status.text().to_string(),
            surface.draft.borrow().document().clone(),
        )
    });
    if let Some((status, document)) = diagnostic {
        eprintln!(
            "Calibration terminal state without publication after {:?}: {status}",
            apply_started.elapsed()
        );
        {
            let app = state.borrow();
            save_container(
                &evidence_path("private-calibration-terminal-draft.toniator"),
                &document,
                &app.workspace.as_ref().unwrap().sources,
            )
            .unwrap();
        }
        capture("calibration-apply-diagnostic");
        panic!("calibration did not publish: {status}");
    }
    {
        let app = state.borrow();
        let actual = app.workspace.as_ref().unwrap().document();
        assert_eq!(
            actual.pattern_definition_bundles(),
            expected.pattern_definition_bundles()
        );
        assert_eq!(actual.pattern_settings(), expected.pattern_settings());
        assert_eq!(actual.response_settings(), expected.response_settings());
        assert_eq!(actual.channel_topology(), expected.channel_topology());
        assert_eq!(actual.source(), initial.source());
        for channel in authoritative_channel_ids(actual) {
            let density = actual
                .effective_channel_pattern(channel)
                .unwrap()
                .density
                .density;
            assert!(
                (artist_numeric_value(actual, PropertyFieldId::Density, density).unwrap() - 0.25)
                    .abs()
                    < 1e-9
            );

            assert!(actual.saved_calibration_for(channel).unwrap().is_some());
        }
        assert!(actual.baseline_saved_calibration().unwrap().is_some());
    }
    eprintln!(
        "Wizard Apply closed and published (replay={}): {:?}",
        std::env::var_os("TONIATOR_WIZARD_MEASURED_REPLAY").is_some(),
        apply_started.elapsed()
    );
    for field in [
        PropertyFieldId::RandomLloydEnabled,
        PropertyFieldId::RandomLloydDensityWeighted,
        PropertyFieldId::RandomLloydIterations,
    ] {
        let values = |document: &Document| {
            document
                .property_values()
                .into_iter()
                .filter(|value| value.descriptor.field == field)
                .map(|value| value.value)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            values(state.borrow().workspace.as_ref().unwrap().document()),
            values(&initial),
            "authored Lloyd retention {field:?}"
        );
    }
    let applied = state
        .borrow()
        .workspace
        .as_ref()
        .unwrap()
        .document()
        .clone();
    assert_eq!(
        state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .history
            .revision()
            .0,
        initial_revision.0 + 1
    );
    save_exact(
        state.borrow().workspace.as_ref().unwrap(),
        "wizard-poisson-025-calibrated.toniator",
    );
    render_current(&state);
    capture("wizard-poisson-applied");
    open_advanced_settings(&state);
    wait_until("ordinary mark response controls", 30, || {
        state
            .borrow()
            .advanced_settings
            .as_ref()
            .is_some_and(|surface| surface.fields.first_child().is_some())
    });
    let fields = state
        .borrow()
        .advanced_settings
        .as_ref()
        .unwrap()
        .fields
        .clone();
    assert!(
        dropdown(fields.upcast_ref(), "Mark sizing").is_none(),
        "Uniform site density has no artwork-weighted sizing selector"
    );
    for channel in authoritative_channel_ids(&applied) {
        assert!(weighted_mark_density_states(&applied, channel).is_empty());
        assert!(channel_mark_range_visible(&applied, channel));
        assert!(!channel_mark_scale_visible(&applied, channel));
        let output = applied
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        assert!(
            applied
                .effective_mark_density(channel, output)
                .unwrap()
                .uniform_unavailable_reason
                .is_some()
        );
    }
    capture("wizard-poisson-uniform-unavailable");
    let advanced_window = state
        .borrow()
        .advanced_settings
        .as_ref()
        .unwrap()
        .window
        .clone();
    button_named(advanced_window.upcast_ref(), "Cancel")
        .unwrap()
        .emit_clicked();
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &applied
    );

    apply_history_navigation(&state, false);
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &initial
    );
    render_current(&state);
    capture("wizard-clustered-undo");
    apply_history_navigation(&state, true);
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &applied
    );
    render_current(&state);
    capture("wizard-poisson-redo");
    apply_history_navigation(&state, false);
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
    reuse_both_sources(&state, &applied, "wizard-saved-reuse");
    let window = state.borrow().window.clone();
    window.close();
}

/// Keeps Apply calibration feedback intact across late preview completions and a failed fit.
///
/// A real preview completion arrives after an injected private Draft Apply failure. The
/// worker-result handler restores the exact error without publishing the captured candidate.
///
/// # Panics
/// Panics if preview completion replaces Apply feedback, pending controls stay locked, or failure publishes.
#[test]
#[ignore = "requires private GTK Wayland session; run scripts/test-pattern-wizard --gtk"]
fn gtk_draft_calibration_status_survives_late_preview_and_failure() {
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.DraftCalibrationStatus")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg");
    let workspace = load_workspace(&path).unwrap();
    let base_document = workspace.document().clone();
    let base_revision = workspace.history.revision();
    let can_undo = workspace.history.can_undo();
    state.borrow_mut().workspace = Some(workspace);
    state.borrow().window.present();
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    wait_until("wizard preview source", 90, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .is_some_and(|surface| {
                matches!(surface.preview_source, WizardPreviewSource::Ready { .. })
            })
    });

    let candidate_id = "clustered-dispersion-random-links";
    let epoch = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .expect("Pattern Wizard opens")
        .epoch;
    select_wizard_preset_candidate(&state, epoch, Some(candidate_id.to_owned()));
    let (candidate_draft, workspace_generation) = {
        let app_state = state.borrow();
        let surface = app_state.pattern_wizard.as_ref().unwrap();
        assert!(surface.gallery_panel.is_visible());
        assert_eq!(surface.candidate.as_deref(), Some(candidate_id));
        let workspace = app_state.workspace.as_ref().unwrap();
        let draft = build_wizard_gallery_candidate_draft(
            &workspace.history,
            &app_state.catalog,
            surface.target,
            candidate_id,
            &surface.initial_document,
        )
        .unwrap();
        (draft, app_state.workspace_generation)
    };
    let candidate_document = candidate_draft.document().clone();
    let candidate_revision = candidate_draft.revision();
    {
        let mut app_state = state.borrow_mut();
        let surface = app_state.pattern_wizard.as_mut().unwrap();
        surface.draft = Rc::new(RefCell::new(candidate_draft));
        surface.gallery_panel.set_visible(false);
    }
    refresh_wizard_route(&state, epoch).unwrap();
    show_wizard_review_page(&state, epoch);
    wizard_validation::request(&state, epoch);
    wait_until("draft construction check", 90, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .is_some_and(|surface| {
                let draft = surface.draft.borrow();
                surface
                    .validation
                    .ready(draft.document(), draft.revision().0)
            })
    });
    let id = calibration_preparation::TaskId(u64::MAX);
    let error = "fixture fit failure: calibration worker rejected the candidate";
    submit_wizard_preview(&state, epoch);
    let preview_ticket = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .unwrap()
        .preview_submission
        .expect("pending-state preview is submitted");
    {
        let mut app_state = state.borrow_mut();
        let surface = app_state.pattern_wizard.as_mut().unwrap();
        let index = surface
            .route
            .iter()
            .position(|page| *page == WizardRoutePage::Review)
            .unwrap();
        surface.route_index = index;
        surface.calibration_task = Some(WizardCalibrationTask {
            id,
            epoch,
            workspace_generation,
            base_document: base_document.clone(),
            base_revision,
            candidate_document,
            candidate_revision,
            cancelled: Arc::new(AtomicBool::new(false)),
            publication: WizardCalibrationPublication::DraftApply,
        });
        set_wizard_calibration_pending(surface, true);
        assert_eq!(surface.status.text(), "Measuring Pattern calibration…");
        assert!(!surface.apply.is_sensitive());
        assert!(surface.cancel.is_sensitive());
    }
    handle_wizard_calibration(&state, id, Err(error.to_owned()));

    {
        let app_state = state.borrow();
        let surface = app_state.pattern_wizard.as_ref().unwrap();
        assert!(surface.calibration_task.is_none());
        assert!(surface.page_panel.is_sensitive());
        assert!(surface.cancel.is_sensitive());
        assert!(surface.apply.is_sensitive());
        assert_eq!(surface.status.text(), error);
        let workspace = app_state.workspace.as_ref().unwrap();
        assert_eq!(workspace.document(), &base_document);
        assert_eq!(workspace.history.revision(), base_revision);
        assert_eq!(workspace.history.can_undo(), can_undo);
    }

    wait_until("late preview after failed calibration", 90, || {
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .is_some_and(|surface| {
                surface.preview_submission != Some(preview_ticket)
                    && surface.preview_status.text() == "Pattern preview updated."
            })
    });
    assert_eq!(
        state
            .borrow()
            .pattern_wizard
            .as_ref()
            .unwrap()
            .status
            .text(),
        error
    );

    let window = state.borrow().window.clone();
    window.close();
}

/// Renders disposable black Wizard studies over white without changing source-aware viewer policies.
/// Both immutable source bundles and all three models use the production neutral source/evaluator.
///
/// # Panics
/// Panics if black paint/white GTK styling diverge, sampled paint loses alpha, or authority mutates.
#[test]
fn wizard_study_backing_preserves_visible_black_and_sampled_alpha() {
    register_resources();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, source_file) in [
        ("raster", "raster-sample.png"),
        ("vector", "vector-sample.svg"),
    ] {
        for model in PreviewModel::ALL {
            let mut workspace = load_workspace(&root.join("assets").join(source_file)).unwrap();
            replace_model_topology(&mut workspace.history, model).unwrap();
            if model == PreviewModel::SourceColorAlpha {
                // Source-colored paint supports marks, not the solid-only guide paths in Random links.
                let preset = crate::current_test_fixtures::clustered_weighted_lloyd();
                let id = preset.metadata.id.clone();
                PresetRegistry::new(
                    toniator_patterns::BUNDLED_PRESET_REGISTRY_VERSION,
                    vec![preset],
                )
                .unwrap()
                .apply_to_document_base(&mut workspace.history, &id)
                .unwrap();
            } else {
                PresetRegistry::bundled()
                    .apply_to_document_base(
                        &mut workspace.history,
                        "clustered-dispersion-random-links",
                    )
                    .unwrap();
            }
            let before = workspace.snapshot();
            let channels = authoritative_channel_ids(workspace.document());
            let mut targets = vec![("all".to_owned(), InspectorTarget::DocumentAll)];
            if model == PreviewModel::Cmyk {
                targets.extend(channels.iter().enumerate().map(|(index, channel)| {
                    (format!("named-{index}"), InspectorTarget::Channel(*channel))
                }));
            }
            for (scope, target) in targets {
                let session = wizard_preview_session(workspace.document(), target).unwrap();
                let visible = target.channel_id().unwrap_or(channels[0]);
                if model != PreviewModel::SourceColorAlpha {
                    assert_eq!(
                        session
                            .document()
                            .modeled_channel(visible)
                            .unwrap()
                            .mapping
                            .component,
                        toniator_domain::SourceMappingComponent::Luminance
                    );
                    assert_eq!(
                        session
                            .document()
                            .channel_weighting(visible)
                            .unwrap()
                            .mapping
                            .component,
                        toniator_domain::SourceMappingComponent::Luminance
                    );
                }
                let WizardPreviewSource::Ready { source } = prepare_wizard_preview_source(
                    &workspace.sources,
                    workspace.source_presentation.as_ref(),
                ) else {
                    panic!("neutral source exists");
                };
                assert_eq!(
                    wizard_preview_backdrop_class(session.document()),
                    if model == PreviewModel::SourceColorAlpha {
                        PreviewModel::SourceColorAlpha.css_class()
                    } else {
                        PreviewModel::Cmyk.css_class()
                    }
                );
                assert_eq!(
                    preview_backdrop::raster_background_for_document(workspace.document()),
                    match model {
                        PreviewModel::Rgb => toniator_engine::RasterBackground::OpaqueBlack,
                        PreviewModel::Cmyk => toniator_engine::RasterBackground::OpaqueWhite,
                        PreviewModel::SourceColorAlpha =>
                            toniator_engine::RasterBackground::Transparent,
                    }
                );
                let render = |background| {
                    toniator_engine::evaluate_profiled_with_limits(
                        EvaluationRequest::with_preview_target(
                            session.document_evaluation_snapshot(),
                            source.clone(),
                            toniator_engine::PreviewRasterTarget::new(128, 128)
                                .unwrap()
                                .with_background(background),
                        )
                        .for_structural_preview(),
                        toniator_engine::EvaluationLimits::default(),
                    )
                    .unwrap()
                    .result
                };
                let native = render(toniator_engine::RasterBackground::Transparent);
                let mut statistics = Vec::new();
                for (label, background) in [
                    (
                        "old-model-backing",
                        preview_backdrop::raster_background_for_document(session.document()),
                    ),
                    (
                        "white-raster",
                        toniator_engine::RasterBackground::OpaqueWhite,
                    ),
                    (
                        "transparent-native",
                        toniator_engine::RasterBackground::Transparent,
                    ),
                ] {
                    let comparison = render(background);
                    assert_eq!(comparison.scene(), native.scene());
                    let pixels = comparison.raster().pixels();
                    statistics.push(serde_json::json!({"case":label,"marks":comparison.scene().circular_mark_count(),"alpha_positive":pixels.chunks_exact(4).filter(|pixel| pixel[3]>0).count(),"alpha_sum":pixels.chunks_exact(4).map(|pixel|u64::from(pixel[3])).sum::<u64>(),"black_covered":pixels.chunks_exact(4).filter(|pixel|pixel[3]>0 && pixel[..3]==[0,0,0]).count(),"light_opaque":pixels.chunks_exact(4).filter(|pixel|pixel[3]==255 && pixel[..3].iter().all(|v|*v>240)).count()}));
                    if std::env::var_os("TONIATOR_WIZARD_EVIDENCE").is_some() {
                        let path = evidence_path(&format!("{name}-{model:?}-{scope}-{label}.png"));
                        assert!(!path.exists());
                        fs::write(path, encode_png(comparison.raster()).unwrap()).unwrap();
                    }
                    if model == PreviewModel::SourceColorAlpha && label == "old-model-backing" {
                        assert_eq!(comparison.raster(), native.raster());
                    }
                }
                eprintln!("controlled study {name} {model:?} {scope}: {statistics:?}");
                if std::env::var_os("TONIATOR_WIZARD_EVIDENCE").is_some() {
                    fs::write(
                        evidence_path(&format!("{name}-{model:?}-{scope}-statistics.json")),
                        serde_json::to_vec_pretty(&statistics).unwrap(),
                    )
                    .unwrap();
                }
                let pixels = native.raster().pixels();
                assert!(
                    pixels.chunks_exact(4).any(|pixel| pixel[3] > 0),
                    "{name} {model:?} {scope} has visible marks"
                );
                if model == PreviewModel::SourceColorAlpha {
                    assert!(
                        native
                            .scene()
                            .layers()
                            .iter()
                            .flat_map(|layer| layer.outputs())
                            .any(|output| output.primitive_paints().is_some()),
                        "sampled source paint remains sampled"
                    );
                } else {
                    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] < 255));
                    assert!(
                        pixels
                            .chunks_exact(4)
                            .filter(|pixel| pixel[3] > 0)
                            .all(|pixel| pixel[..3] == [0, 0, 0]),
                        "forced-black paint stays black"
                    );
                    let white = render(toniator_engine::RasterBackground::OpaqueWhite);
                    assert!(
                        white
                            .raster()
                            .pixels()
                            .chunks_exact(4)
                            .any(|pixel| pixel[..3].iter().all(|v| *v < 100))
                    );
                    assert!(
                        white
                            .raster()
                            .pixels()
                            .chunks_exact(4)
                            .any(|pixel| pixel[..3].iter().all(|v| *v > 240))
                    );
                }
                assert_eq!(workspace.snapshot(), before);
            }
        }
    }
}

/// Checks guarded Wizard completion publishes valid companions once and rejects cancelled/stale work.
/// The immutable measured catalog artifact avoids fitting in these focused GTK acceptance hooks.
///
/// # Panics
/// Panics on leaked stale/cancelled publication, missing attachments, multiple history steps or lost sources.
#[test]
#[ignore = "requires the private Wayland static-completion harness"]
fn gtk_wizard_calibration_completion_guards() {
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.WizardCompletionGuards")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    state.borrow().window.present();
    let registry = PresetRegistry::bundled();
    let preset = registry
        .find("clustered-dispersion-random-links")
        .unwrap()
        .clone();
    let artifact = preset.saved_calibration.clone().unwrap();
    let mut without_companion = preset;
    without_companion.saved_calibration = None;
    let id = without_companion.metadata.id.clone();
    let registry = PresetRegistry::new(
        toniator_patterns::BUNDLED_PRESET_REGISTRY_VERSION,
        vec![without_companion],
    )
    .unwrap();
    for (index, case) in [
        "success",
        "cancelled",
        "stale-root",
        "stale-draft",
        "stale-epoch",
    ]
    .into_iter()
    .enumerate()
    {
        let workspace = load_workspace(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg"),
        )
        .unwrap();
        install_workspace(&state, workspace);
        let initial = state.borrow().workspace.as_ref().unwrap().snapshot();
        let revision = state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .history
            .revision();
        open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
        let (epoch, candidate, candidate_revision) = {
            let app = state.borrow();
            let surface = app.pattern_wizard.as_ref().unwrap();
            registry
                .apply_to_document_base(&mut surface.draft.borrow_mut(), &id)
                .unwrap();
            let draft = surface.draft.borrow();
            (surface.epoch, draft.document().clone(), draft.revision())
        };
        let channel = authoritative_channel_ids(&candidate)[0];
        assert_eq!(
            artifact.binding(),
            &toniator_domain::CalibrationBinding::for_channel(&candidate, channel).unwrap()
        );
        assert!(candidate.saved_calibration_for(channel).unwrap().is_none());
        let task_id = calibration_preparation::TaskId(10_000 + index as u64);
        let token = Arc::new(AtomicBool::new(case == "cancelled"));
        let generation = state.borrow().workspace_generation;
        state
            .borrow_mut()
            .pattern_wizard
            .as_mut()
            .unwrap()
            .calibration_task = Some(WizardCalibrationTask {
            id: task_id,
            epoch: epoch + u64::from(case == "stale-epoch"),
            workspace_generation: generation,
            base_document: initial.document.clone(),
            base_revision: revision,
            candidate_document: candidate.clone(),
            candidate_revision,
            cancelled: token,
            publication: WizardCalibrationPublication::DraftApply,
        });
        if case == "stale-root" {
            let mut app = state.borrow_mut();
            let history = &mut app.workspace.as_mut().unwrap().history;
            let base = history.document().pattern_settings().clone();
            let mut settings = base.clone();
            settings.density.density *= 0.5;
            history
                .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
                .unwrap();
        }
        if case == "stale-draft" {
            let app = state.borrow();
            let mut draft = app.pattern_wizard.as_ref().unwrap().draft.borrow_mut();
            let base = draft.document().pattern_settings().clone();
            let mut settings = base.clone();
            settings.density.density *= 0.5;
            draft
                .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
                .unwrap();
        }
        let before = state.borrow().workspace.as_ref().unwrap().snapshot();
        let before_revision = state
            .borrow()
            .workspace
            .as_ref()
            .unwrap()
            .history
            .revision();
        handle_wizard_calibration(
            &state,
            task_id,
            Ok(calibration_preparation::PreparedCalibrations {
                active: authoritative_channel_ids(&candidate).into_iter().map(|channel_id|
                    toniator_domain::AttachedCalibration { channel_id, artifact: artifact.clone() }).collect(),
                baseline: Some(artifact.clone()),
            }),
        );
        if case == "success" {
            assert!(state.borrow().pattern_wizard.is_none());
            let app = state.borrow();
            let workspace = app.workspace.as_ref().unwrap();
            assert_eq!(workspace.history.revision().0, revision.0 + 1);
            assert_eq!(workspace.sources, initial.sources);
            assert_eq!(
                workspace.document().saved_calibration_for(channel).unwrap(),
                Some(&artifact)
            );
            workspace
                .document()
                .require_saved_calibration_authority()
                .unwrap();
            drop(app);
            apply_history_navigation(&state, false);
            assert_eq!(
                state.borrow().workspace.as_ref().unwrap().snapshot(),
                initial
            );
            apply_history_navigation(&state, true);
            assert_eq!(
                state
                    .borrow()
                    .workspace
                    .as_ref()
                    .unwrap()
                    .document()
                    .saved_calibration_for(channel)
                    .unwrap(),
                Some(&artifact)
            );
        } else {
            assert!(state.borrow().pattern_wizard.is_some());
            assert_eq!(
                state.borrow().workspace.as_ref().unwrap().snapshot(),
                before
            );
            assert_eq!(
                state
                    .borrow()
                    .workspace
                    .as_ref()
                    .unwrap()
                    .history
                    .revision(),
                before_revision
            );
        }
        eprintln!("Wizard completion guard {case}: pass");
    }
    let workspace = load_workspace(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg"),
    )
    .unwrap();
    install_workspace(&state, workspace);
    let before = state.borrow().workspace.as_ref().unwrap().snapshot();
    let revision = state
        .borrow()
        .workspace
        .as_ref()
        .unwrap()
        .history
        .revision();
    open_pattern_wizard(&state, gtk::Button::with_label("Change…"));
    press(&state, |surface| surface.edit.clone());
    for _ in 0..3 {
        press(&state, |surface| surface.review.clone());
    }
    let (epoch, draft) = {
        let app = state.borrow();
        let surface = app.pattern_wizard.as_ref().unwrap();
        set_wizard_calibration_pending(surface, true);
        assert!(!surface.page_panel.is_sensitive());
        (surface.epoch, surface.draft.clone())
    };
    publish_admitted_wizard_draft(&state, epoch, &draft);
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().snapshot(),
        before
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
    {
        let app = state.borrow();
        let surface = app.pattern_wizard.as_ref().unwrap();
        assert!(surface.page_panel.is_sensitive());
        assert!(surface.gallery_panel.is_sensitive());
        assert!(surface.cancel.is_sensitive());
        assert!(!surface.apply.is_sensitive());
        assert_eq!(
            surface.undo.is_sensitive(),
            surface.draft.borrow().can_undo()
        );
        assert_eq!(
            surface.redo.is_sensitive(),
            surface.draft.borrow().can_redo()
        );
        assert!(
            surface
                .status
                .text()
                .contains("no pattern settings changed")
        );
    }
    capture("wizard-publication-rejection-recovered");
    eprintln!(
        "Wizard unchanged publication recovery: controls restored, main snapshot/revision unchanged"
    );
    let window = state.borrow().window.clone();
    window.destroy();
}
