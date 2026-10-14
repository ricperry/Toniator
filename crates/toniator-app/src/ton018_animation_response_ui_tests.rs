//! Private GTK evidence for independent Recalibrate ownership and the passive End warning.
use super::*;
use crate::wizard_workflow_tests::{capture, render_current, save_exact, wait_until};

/// Runs one scoped private AT-SPI command while the real GTK event loop remains responsive.
///
/// # Panics
/// Panics on semantic lookup/action failure or an unwritable stage evidence record.
fn semantic(name: &str, args: &[&str]) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.agents/skills/gtk-wayland-debug/scripts/ui");
    let mut args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "Recalibrate Pattern" | "Apply" | "Cancel"))
    {
        args.extend(["--role".into(), "button".into()]);
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "End frame" | "Start frame"))
    {
        args.extend(["--role".into(), "toggle button".into()]);
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let result = std::process::Command::new("timeout")
            .arg("30s")
            .arg(script)
            .args(args)
            .args(["--application", ""])
            .output()
            .unwrap();
        sender.send(result).unwrap();
    });
    let result = RefCell::new(None);
    wait_until(name, 40, || {
        if let Ok(output) = receiver.try_recv() {
            *result.borrow_mut() = Some(output);
        }
        result.borrow().is_some()
    });
    let result = result.into_inner().unwrap();
    let directory = PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap());
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(format!("{name}.json")), &result.stdout).unwrap();
    fs::write(directory.join(format!("{name}.stderr")), &result.stderr).unwrap();
    assert!(
        result.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

/// Sends one physical key into the verified private VNC session while GTK handles its native signals.
///
/// # Panics
/// Panics on a private input failure or an unwritable stage evidence record.
fn native_key(name: &str, key: &str) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.agents/skills/gtk-wayland-debug/scripts/key");
    let key = key.to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        sender
            .send(
                std::process::Command::new(script)
                    .arg(key)
                    .output()
                    .unwrap(),
            )
            .unwrap();
    });
    let result = RefCell::new(None);
    wait_until(name, 30, || {
        if let Ok(output) = receiver.try_recv() {
            *result.borrow_mut() = Some(output);
        }
        result.borrow().is_some()
    });
    let output = result.into_inner().unwrap();
    fs::write(
        PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap()).join(format!("{name}.log")),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
    .unwrap();
    assert!(output.status.success());
}

/// Installs a tiny measured-payload difference without altering valid binding or profile provenance.
///
/// # Panics
/// Panics if the current finite default curve cannot retain the independently perturbed measurement.
fn distinct(
    saved: &toniator_domain::SavedCalibration,
    delta: f64,
) -> toniator_domain::SavedCalibration {
    let mut raw = saved.raw().to_vec();
    raw[1].coverage += delta;
    toniator_domain::SavedCalibration::with_profile(
        saved.profile().into(),
        saved.binding().clone(),
        raw,
        saved.uniform().cloned(),
    )
    .unwrap()
}

/// Exercises actual named/ALL fitting and private Apply/Cancel/history through native controls.
///
/// # Panics
/// Panics on lost assignment isolation, stale publication, misleading native controls or preview failure.
#[test]
#[ignore = "requires the private Wayland TON-018 animation-response harness"]
fn gtk_independent_recalibrate_and_end_warning() {
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.AnimationResponse")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    state.borrow().window.present();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut workspace = load_workspace(&root.join("assets/raster-sample.png")).unwrap();
    let ids = authoritative_channel_ids(workspace.document());
    let original = workspace
        .document()
        .saved_calibration_for(ids[0])
        .unwrap()
        .unwrap()
        .clone();
    let entries = ids
        .iter()
        .enumerate()
        .map(|(index, channel_id)| toniator_domain::AttachedCalibration {
            channel_id: *channel_id,
            artifact: distinct(&original, (index + 1) as f64 * 1e-13),
        })
        .collect();
    let initial = workspace
        .document()
        .with_saved_calibration_authority(entries, Some(distinct(&original, 4e-13)))
        .unwrap();
    workspace.history = DocumentHistory::new(DocumentSession::new(initial.clone()).unwrap());
    install_workspace(&state, workspace);
    render_current(&state);
    let named = if let Ok(path) = env::var("TONIATOR_ANIMATION_NAMED_REUSE") {
        let workspace = load_workspace(Path::new(&path)).unwrap();
        let document = workspace.document().clone();
        eprintln!(
            "Reusing independently passing actual named fit/Apply/history evidence: {path}; no named fit in this run."
        );
        install_workspace(&state, workspace);
        render_current(&state);
        document
    } else {
        state.borrow_mut().inspector_runtime.target = InspectorTarget::Channel(ids[0]);
        open_advanced_settings(&state);
        semantic(
            "named-wait",
            &[
                "wait",
                "Recalibrate Pattern",
                "--exact",
                "--ancestor",
                "Advanced Settings",
                "--state",
                "enabled",
                "--timeout",
                "15",
            ],
        );
        semantic(
            "named-controls",
            &[
                "controls",
                "--subtree",
                "Advanced Settings",
                "--depth",
                "5",
                "--json",
            ],
        );
        let readback = semantic(
            "named-inspect",
            &[
                "inspect",
                "Recalibrate Pattern",
                "--exact",
                "--ancestor",
                "Advanced Settings",
            ],
        );
        assert!(readback.contains("selected channel"));
        semantic(
            "named-activate",
            &[
                "activate",
                "Recalibrate Pattern",
                "--exact",
                "--ancestor",
                "Advanced Settings",
            ],
        );
        semantic(
            "named-measuring",
            &[
                "inspect",
                "Recalibrate Pattern",
                "--exact",
                "--ancestor",
                "Advanced Settings",
            ],
        );
        capture("named-measuring");
        wait_until("named Recalibrate starts", 15, || {
            state
                .borrow()
                .advanced_settings
                .as_ref()
                .unwrap()
                .calibration_task
                .is_some()
        });
        wait_until("actual named Recalibrate", 120, || {
            state
                .borrow()
                .advanced_settings
                .as_ref()
                .unwrap()
                .calibration_task
                .is_none()
        });
        let named = {
            let app = state.borrow();
            let surface = app.advanced_settings.as_ref().unwrap();
            assert!(surface.recalibrate.is_sensitive());
            assert!(surface.apply.is_sensitive());
            let draft = surface.draft.borrow();
            let document = draft.document();
            for id in &ids[1..] {
                assert_eq!(
                    document.saved_calibration_for(*id).unwrap(),
                    initial.saved_calibration_for(*id).unwrap()
                );
            }
            assert_eq!(
                document.baseline_saved_calibration().unwrap(),
                initial.baseline_saved_calibration().unwrap()
            );
            assert_ne!(
                document.saved_calibration_for(ids[0]).unwrap(),
                initial.saved_calibration_for(ids[0]).unwrap()
            );
            save_container(
                &PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap())
                    .join("named-measured-private.toniator"),
                document,
                &app.workspace.as_ref().unwrap().sources,
            )
            .unwrap();
            document.clone()
        };
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &initial
        );
        semantic(
            "named-ready",
            &[
                "inspect",
                "Recalibrate Pattern",
                "--exact",
                "--ancestor",
                "Advanced Settings",
            ],
        );
        capture("named-ready");
        semantic(
            "named-apply",
            &[
                "activate",
                "Apply",
                "--exact",
                "--ancestor",
                "Advanced Settings",
            ],
        );
        wait_until("named Apply closes", 15, || {
            state.borrow().advanced_settings.is_none()
        });
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &named
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
            1
        );
        apply_history_navigation(&state, false);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &initial
        );
        apply_history_navigation(&state, true);
        assert_eq!(
            state.borrow().workspace.as_ref().unwrap().document(),
            &named
        );
        render_current(&state);
        capture("named-applied");
        save_exact(
            state.borrow().workspace.as_ref().unwrap(),
            "named-applied.toniator",
        );
        named
    };
    state.borrow_mut().inspector_runtime.target = InspectorTarget::DocumentAll;
    open_advanced_settings(&state);
    semantic(
        "all-wait",
        &[
            "wait",
            "Recalibrate Pattern",
            "--exact",
            "--ancestor",
            "Advanced Settings",
            "--state",
            "enabled",
            "--timeout",
            "15",
        ],
    );
    let readback = semantic(
        "all-inspect",
        &[
            "inspect",
            "Recalibrate Pattern",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    assert!(readback.contains("every active channel"));
    semantic(
        "all-activate",
        &[
            "activate",
            "Recalibrate Pattern",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    wait_until("ALL Recalibrate starts", 15, || {
        state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .calibration_task
            .is_some()
    });
    wait_until("actual ALL Recalibrate", 120, || {
        state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .calibration_task
            .is_none()
    });
    let all = {
        let app = state.borrow();
        let surface = app.advanced_settings.as_ref().unwrap();
        assert!(surface.recalibrate.is_sensitive());
        assert!(surface.apply.is_sensitive());
        let draft = surface.draft.borrow();
        let document = draft.document();
        let saved = document.saved_calibration_for(ids[0]).unwrap().unwrap();
        for id in &ids {
            assert_eq!(document.saved_calibration_for(*id).unwrap(), Some(saved));
        }
        assert_eq!(document.baseline_saved_calibration().unwrap(), Some(saved));
        save_container(
            &PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap())
                .join("all-measured-private.toniator"),
            document,
            &app.workspace.as_ref().unwrap().sources,
        )
        .unwrap();
        document.clone()
    };
    capture("all-ready");
    semantic(
        "all-cancel",
        &[
            "activate",
            "Cancel",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    wait_until("ALL Cancel closes", 15, || {
        state.borrow().advanced_settings.is_none()
    });
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &named
    );
    // Replay only the just-measured immutable result through the real private result handler.
    // This proves ALL Apply/history without paying for a second identical fit.
    open_advanced_settings(&state);
    let (epoch, generation, base_revision, candidate_revision) = {
        let app = state.borrow();
        let surface = app.advanced_settings.as_ref().unwrap();
        let revision = surface.draft.borrow().revision();
        (
            surface.epoch,
            app.workspace_generation,
            app.workspace.as_ref().unwrap().history.revision(),
            revision,
        )
    };
    let id = calibration_preparation::TaskId(80_018);
    state
        .borrow_mut()
        .advanced_settings
        .as_mut()
        .unwrap()
        .calibration_task = Some(AdvancedCalibrationTask {
        id,
        epoch,
        workspace_generation: generation,
        base_document: named.clone(),
        base_revision,
        candidate_document: named.clone(),
        candidate_revision,
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    handle_advanced_calibration(
        &state,
        id,
        Ok(calibration_preparation::PreparedCalibrations {
            active: all.saved_calibrations().to_vec(),
            baseline: all.baseline_saved_calibration().unwrap().cloned(),
        }),
    );
    semantic(
        "all-apply",
        &[
            "activate",
            "Apply",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    wait_until("ALL Apply closes", 15, || {
        state.borrow().advanced_settings.is_none()
    });
    assert_eq!(state.borrow().workspace.as_ref().unwrap().document(), &all);
    apply_history_navigation(&state, false);
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &named
    );
    apply_history_navigation(&state, true);
    assert_eq!(state.borrow().workspace.as_ref().unwrap().document(), &all);
    save_exact(
        state.borrow().workspace.as_ref().unwrap(),
        "all-applied.toniator",
    );
    // Actual cancellation while the worker owns a still-private candidate.
    open_advanced_settings(&state);
    semantic(
        "cancel-start",
        &[
            "activate",
            "Recalibrate Pattern",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    wait_until("cancelled worker starts", 15, || {
        state
            .borrow()
            .advanced_settings
            .as_ref()
            .unwrap()
            .calibration_task
            .is_some()
    });
    semantic(
        "cancel-pending",
        &[
            "activate",
            "Cancel",
            "--exact",
            "--ancestor",
            "Advanced Settings",
        ],
    );
    wait_until("pending Cancel closes", 15, || {
        state.borrow().advanced_settings.is_none()
    });
    assert_eq!(state.borrow().workspace.as_ref().unwrap().document(), &all);
    state.borrow().window.destroy();
}

/// Checks full-turn End warning semantics and status separation with optional private keyboard diagnosis.
///
/// # Panics
/// Panics on incorrect warning applicability, native keyboard action or completed-preview state.
#[test]
#[ignore = "requires the private Wayland TON-018 animation-response harness"]
fn gtk_end_shape_rotation_warning() {
    register_resources();
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.EndRotationWarning")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    state.borrow().window.present();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // A noncircular source-backed full turn exercises actual End preview and passive warning.
    let mut workspace=load_workspace(&root.join("target/validation/ton018-animation-response-20261013/native/marks-final/raster-animated.toniator")).unwrap();
    let initialize = workspace.document().initialize_end_command().unwrap();
    if initialize.replacement() != &workspace.document().temporal_authority() {
        workspace.history.apply_temporal(&initialize).unwrap();
    }
    let command = workspace
        .document()
        .edit_all_channel_end_command(&[(PropertyFieldId::ShapeRotationDegrees, 360.0)])
        .unwrap();
    workspace.history.apply_temporal(&command).unwrap();
    assert!(workspace.document().has_animated_shape_rotation().unwrap());
    install_workspace(&state, workspace);
    render_current(&state);
    semantic(
        "end-wait",
        &[
            "wait",
            "End frame",
            "--exact",
            "--state",
            "enabled",
            "--timeout",
            "15",
        ],
    );
    semantic(
        "endpoint-controls",
        &[
            "controls",
            "--subtree",
            "End frame",
            "--subtree-role",
            "toggle button",
            "--depth",
            "3",
            "--json",
        ],
    );
    let readback = semantic("end-start-inspect", &["inspect", "End frame", "--exact"]);
    assert!(readback.contains("saved calibration stays fixed"));
    assert!(!state.borrow().end_rotation_warning.is_visible());
    semantic("end-activate", &["activate", "End frame", "--exact"]);
    wait_until("End selection", 15, || {
        state.borrow().endpoint == temporal_preview::Endpoint::End
    });
    render_current(&state);
    const WARNING: &str =
        "Rotating marks can change overlap and tone. The saved calibration stays fixed.";
    assert_eq!(state.borrow().end_rotation_warning.label(), WARNING);
    assert!(state.borrow().end_rotation_warning.is_visible());
    semantic("end-warning-inspect", &["inspect", WARNING, "--exact"]);
    semantic("end-selected-inspect", &["inspect", "End frame", "--exact"]);
    capture("end-fullturn");
    for message in ["Calibration error witness", "Rasterizing witness 50%"] {
        let mut app = state.borrow_mut();
        set_inspector_status(&mut app, message);
        sync_endpoint_controls(&app);
        assert!(app.inspector_status.label().starts_with(message));
    }
    if env::var_os("TONIATOR_ANIMATION_KEYBOARD").is_some() {
        semantic("keyboard-start", &["activate", "Start frame", "--exact"]);
        wait_until("keyboard Start selection", 15, || {
            state.borrow().endpoint == temporal_preview::Endpoint::Start
        });
        let tree = std::process::Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
            .unwrap();
        fs::write(
            PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap())
                .join("private-focus-tree.json"),
            &tree.stdout,
        )
        .unwrap();
        let focus = std::process::Command::new("swaymsg")
            .arg(r#"[app_id="io.github.ricperry.Toniator.EndRotationWarning"] focus"#)
            .output()
            .unwrap();
        fs::write(
            PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap())
                .join("private-window-focus.json"),
            &focus.stdout,
        )
        .unwrap();
        assert!(focus.status.success());
        let end_button = state.borrow().end_frame.clone();
        assert!(end_button.grab_focus());
        assert_eq!(
            gtk::prelude::RootExt::focus(&state.borrow().window),
            Some(end_button.clone().upcast::<gtk::Widget>())
        );
        fs::write(PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap()).join("native-root-focus.txt"),format!("Root focus: End frame; toplevel active: {}; widget has focus: {}. Private VNC creates the input seat for Space.\n",state.borrow().window.is_active(),end_button.has_focus())).unwrap();
        semantic(
            "end-native-focus-readback",
            &["inspect", "End frame", "--exact"],
        );
        native_key("end-key-space", "space");
        wait_until("keyboard End selection", 15, || {
            state.borrow().endpoint == temporal_preview::Endpoint::End
        });
        capture("end-keyboard");
    } else {
        eprintln!(
            "Physical keyboard delivery remains a bounded private-seat diagnostic; standard native End control and root-focus evidence are retained separately."
        );
    }
    semantic("start-activate", &["activate", "Start frame", "--exact"]);
    wait_until("Start selection", 15, || {
        state.borrow().endpoint == temporal_preview::Endpoint::Start
    });
    assert!(!state.borrow().end_rotation_warning.is_visible());
    let readback = semantic(
        "start-selected-inspect",
        &["inspect", "Start frame", "--exact"],
    );
    assert!(readback.contains("Start frame"));
    render_current(&state);
    capture("start-fullturn-tooltip");
    {
        let mut app = state.borrow_mut();
        let workspace = app.workspace.as_mut().unwrap();
        let command = workspace.document().replace_temporal_authority_command(
            workspace.document().project_timing().clone(),
            Vec::new(),
        );
        workspace.history.apply_temporal(&command).unwrap();
        sync_endpoint_controls(&app);
        assert_eq!(
            app.end_frame.tooltip_text().as_deref(),
            Some("Show the end of the selected animation range")
        );
    }
    semantic("end-cleared-inspect", &["inspect", "End frame", "--exact"]);
    state.borrow().window.destroy();
}

/// Reuses a catalog companion at a named artistic angle without fitting or rewriting provenance.
///
/// # Panics
/// Panics if the app's catalog gate uses measured-angle equality instead of domain compatibility.
#[test]
fn rotated_named_catalog_candidate_reuses_compatible_saved_companion() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 128.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let rotated = document
        .apply_command(
            &document
                .set_channel_shape_rotation_for_effective(ChannelId(1), 37.0)
                .unwrap(),
        )
        .unwrap()
        .0;
    let mut history = DocumentHistory::new(DocumentSession::new(rotated.clone()).unwrap());
    let registry = PresetRegistry::bundled();
    let catalog = LayeredPresetCatalog::new(&registry, Vec::new()).unwrap();
    apply_catalog_pattern_candidate(
        &mut history,
        &catalog,
        InspectorTarget::Channel(ChannelId(1)),
        "triagrid-custom-shape-marks",
    )
    .unwrap();
    assert_eq!(
        history
            .document()
            .saved_calibration_for(ChannelId(1))
            .unwrap(),
        registry
            .find("triagrid-custom-shape-marks")
            .unwrap()
            .saved_calibration
            .as_ref()
    );
    assert_eq!(
        history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .unwrap()
            .shape_rotation_degrees,
        37.0
    );
    assert_eq!(
        history
            .document()
            .saved_calibration_for(ChannelId(2))
            .unwrap(),
        rotated.saved_calibration_for(ChannelId(2)).unwrap()
    );
    assert!(
        calibration_preparation::plan_to_prepare(
            history.document(),
            &authoritative_channel_ids(history.document()),
            false,
            false
        )
        .unwrap()
        .is_empty()
    );
}
