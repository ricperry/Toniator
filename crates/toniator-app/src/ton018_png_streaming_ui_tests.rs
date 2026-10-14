//! Current still-PNG integration admission, captured export, and private native GTK evidence.
use super::*;
use crate::wizard_workflow_tests::{capture, render_current, wait_until};

/// Returns only this stage's output directory, optionally selected by the private harness.
/// # Panics
/// Panics on a stage-owned filesystem failure.
fn stage() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-png-export-integration-20261013");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Creates an ordinary captured PNG selection with no dormant cleanup authority.
/// # Panics
/// Panics on invalid fixed test dimensions.
fn selection(width: u32, height: u32) -> print_preparation::OutputSelection {
    print_preparation::OutputSelection {
        target: StreamingRasterTarget::new(width, height).unwrap(),
        antialiasing: RasterAntialiasing::Off,
        background: print_preparation::BackgroundChoice::Explicit(RasterBackground::Transparent),
        prepare_for_print: false,
        pixels_per_metre: None,
        cleanup: print_cleanup::Settings::default(),
    }
}

/// Proves export, preview, advisory Review, video, physical sizing, and cleanup admit independently.
/// # Panics
/// Panics if large intent is reduced or a nonzero global correction is silently omitted.
#[test]
fn exact_large_png_intent_and_independent_admission() {
    let canvas = CanvasSpec {
        width: 2048.0,
        height: 1164.0,
    };
    let dimensions = png_export_size::requested_png_dimensions(&canvas, 3, "").unwrap();
    assert_eq!(
        dimensions.streaming_target().unwrap(),
        selection(16_384, 9_312).target
    );
    let fit = png_export_size::fit_print_box_dimensions(
        &canvas,
        16_384.0 * 25.4 / 300.0,
        9_312.0 * 25.4 / 300.0,
        300.0,
    )
    .unwrap();
    assert_eq!(
        fit.streaming_target().unwrap(),
        dimensions.streaming_target().unwrap()
    );
    assert_eq!(fit.target_dpi(), 300.0);
    assert_eq!(fit.pixels_per_metre(), 11_811);
    let mut selected = selection(16_384, 9_312);
    assert!(selected.admit_export().is_ok());
    assert!(png_preview_target(selected).is_none());
    assert!(png_preview_target(selection(2048, 1164)).is_some());
    assert!(
        png_export_size::video_output_size(&canvas, 4, "").is_err(),
        "video keeps its existing full-raster cap"
    );
    for (remove, feature, gap) in [(1, 1, 0), (0, 1, 0), (0, 0, 1)] {
        selected.cleanup.remove_below_px = remove;
        selected.cleanup.minimum_feature_px = feature;
        selected.cleanup.minimum_gap_px = gap;
        assert!(
            selected.admit_export().is_ok(),
            "ordinary output ignores dormant correction widths"
        );
        selected.prepare_for_print = true;
        let error = selected.admit_export().unwrap_err();
        assert!(error.contains("32,000,000") && error.contains("to 0"));
        selected.prepare_for_print = false;
    }
    selected.prepare_for_print = true;
    selected.cleanup = print_cleanup::Settings::default();
    assert!(selected.admit_export().is_ok());
    assert!(png_export_size::requested_png_dimensions(&canvas, 4, "invalid").is_err());
}

/// Proves app captured exports preserve exact native source-backed PNG pixels and prior files.
/// # Panics
/// Panics on source/capture mutation, streaming/full RGBA differences, or cancellation publication.
#[test]
fn captured_both_sources_stream_exact_pixels_and_preserve_cancelled_destination() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source in ["raster-sample.png", "vector-sample.svg"] {
        let mut workspace = load_workspace(&root.join("assets").join(source)).unwrap();
        let capture = capture_still_export(
            &workspace,
            workspace.document().project_timing().frame_range().start(),
            3,
            9,
        );
        let original = capture.session.document().clone();
        let captured_token = capture.session.document_evaluation_token();
        let channel = authoritative_channel_ids(workspace.document())[0];
        workspace
            .history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: channel,
                visible: false,
            })
            .unwrap();
        assert_ne!(
            workspace.history.session().document_evaluation_token(),
            captured_token
        );
        assert_eq!(capture.session.document_evaluation_token(), captured_token);
        assert!(capture.workspace_is_current(3, 9));
        assert!(!capture.workspace_is_current(3, 10));
        for prepared in [false, true] {
            let settings = ExportSettings {
                format: ExportFormat::Png,
                background: RasterBackground::Transparent,
                antialiasing: RasterAntialiasing::On,
                output_target: Some(StreamingRasterTarget::new(128, 96).unwrap()),
                prepare_for_print: prepared,
                pixels_per_metre: prepared.then_some(11_811),
                cleanup: print_cleanup::Settings::default(),
            };
            let expected =
                render_captured_png_surface(&capture, settings, &AtomicBool::new(false)).unwrap();
            let path = stage().join(format!("{source}-prepared{prepared}-captured.png"));
            export_captured_frame_cancellable(
                capture.clone(),
                path.clone(),
                settings,
                &AtomicBool::new(false),
            )
            .unwrap();
            let mut reader =
                png::Decoder::new(std::io::BufReader::new(fs::File::open(&path).unwrap()))
                    .read_info()
                    .unwrap();
            let mut actual = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut actual).unwrap();
            assert_eq!(frame.color_type, png::ColorType::Rgba);
            actual.truncate(frame.buffer_size());
            assert!(
                actual == expected.pixels(),
                "exact captured parity: {source}/{prepared}"
            );
            let approved = fs::read(&path).unwrap();
            assert!(
                export_captured_frame_cancellable(
                    capture.clone(),
                    path.clone(),
                    settings,
                    &AtomicBool::new(true)
                )
                .is_err()
            );
            assert_eq!(fs::read(path).unwrap(), approved);
        }
        assert_eq!(capture.session.document(), &original);
    }
}

/// Proves a published PNG warning cannot be hidden by cancellation or prepared-output wording.
/// # Panics
/// Panics if late cancellation or prepared mode takes precedence over already-published output.
#[test]
fn published_warning_precedes_late_cancel_and_prepared_failure() {
    let published = StillExportError {
        message: "injected postpublication directory sync failure".into(),
        published: true,
    };
    for cancelled in [false, true] {
        for prepared in [false, true] {
            assert_eq!(
                still_export_failure_status(&published, cancelled, prepared),
                Some("PNG exported; final folder synchronization failed. See error log.")
            );
        }
    }
    let unpublished: StillExportError = "prepublication failure".into();
    assert_eq!(
        still_export_failure_status(&unpublished, true, true),
        Some("Export cancelled.")
    );
    assert_eq!(
        still_export_failure_status(&unpublished, false, true),
        Some("Print PNG export failed; see error log.")
    );
}

/// Performs a scoped real AT-SPI command while pumping GTK and retains raw semantic readback.
/// # Panics
/// Panics on unavailable/ambiguous controls or a failed native semantic operation.
fn semantic(label: &str, args: &[&str]) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.agents/skills/gtk-wayland-debug/scripts/ui");
    let arguments = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        sender
            .send(
                std::process::Command::new("timeout")
                    .arg("30s")
                    .arg(script)
                    .args(arguments)
                    .args(["--application", ""])
                    .output()
                    .unwrap(),
            )
            .unwrap();
    });
    let result = RefCell::new(None);
    wait_until(label, 40, || {
        if let Ok(output) = receiver.try_recv() {
            *result.borrow_mut() = Some(output);
        }
        result.borrow().is_some()
    });
    let output = result.into_inner().unwrap();
    let directory = PathBuf::from(env::var("TONIATOR_WIZARD_EVIDENCE").unwrap());
    fs::write(directory.join(format!("{label}.json")), &output.stdout).unwrap();
    fs::write(directory.join(format!("{label}.stderr")), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Exercises actual PNG widgets, exact large output, unavailable previews, corrections, and Cancel.
/// # Panics
/// Panics on stale selection export, reduced DPI/size, lost publication guards, or native GTK errors.
#[test]
#[ignore = "requires the private Wayland PNG integration harness"]
fn gtk_exact_large_png_export_and_admission() {
    register_resources();
    glib::set_prgname(Some("Toniator"));
    glib::set_application_name("Toniator");
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.ricperry.Toniator.PngIntegration")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    state.borrow().window.present();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace = load_workspace(&root.join("target/validation/ton018-builtin-document-presets-20261013/photo-r30-g60-b0-feature025-current.toniator")).unwrap();
    let original = workspace.document().clone();
    install_workspace(&state, workspace);
    render_current(&state);
    let reuse_actual = env::var_os("TONIATOR_PNG_REUSE_ACTUAL").is_some();
    let ordinary = stage().join("gtk-photo-ordinary-16384x9312.png");
    let prior_ordinary = fs::read(&ordinary).ok();
    open_print_preparation_surface(&state, Some(ordinary.clone()), false);
    let controls = print_preparation_controls_handles(
        &state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .controls,
    );
    controls.options.background().set_selected(1);
    controls.options.antialiasing().set_selected(1);
    controls.options.scale().set_selected(3);
    assert_eq!(
        state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .controls
            .selection
            .get()
            .target,
        selection(16_384, 9_312).target
    );
    assert!(controls.export.is_sensitive());
    assert!(!controls.options.size_error().is_visible());
    assert!(
        controls
            .status
            .text()
            .contains("Full-size preview unavailable")
    );
    semantic(
        "ordinary-wait",
        &["wait", "Output size", "--exact", "--role", "combo box"],
    );
    semantic(
        "ordinary-controls",
        &[
            "controls",
            "--subtree",
            "PNG export options",
            "--depth",
            "12",
            "--json",
        ],
    );
    semantic(
        "ordinary-export-enabled",
        &["inspect", "Export PNG…", "--exact", "--role", "button"],
    );
    capture("ordinary-8x-valid-preview-unavailable");

    controls.options.scale().set_selected(4);
    controls.options.dimensions().set_text("invalid");
    assert!(!controls.export.is_sensitive());
    controls.export.emit_clicked(); // Native signal bypass proves revalidation, not sensitivity, owns safety.
    assert!(!state.borrow().pending_export);
    assert_eq!(
        fs::read(&ordinary).ok(),
        prior_ordinary,
        "invalid activation never publishes any target"
    );
    capture("invalid-current-input-cannot-export-retained-target");
    controls.options.scale().set_selected(3);
    if !reuse_actual {
        controls.export.emit_clicked();
        assert!(
            state.borrow().pending_export,
            "actual worker installed before completion wait"
        );
        wait_until("actual ordinary large PNG", 150, || {
            !state.borrow().pending_export
        });
    }
    assert!(ordinary.is_file());
    capture("ordinary-large-export-complete");

    // Small → large → small cancels old jobs and clears textures without collapsing large intent.
    controls.options.scale().set_selected(4);
    controls.options.dimensions().set_text("256x145");
    wait_until("small exact preview", 90, || {
        state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .preview_status
            == print_preparation_view::PreviewStatus::Current
    });
    controls.options.scale().set_selected(3);
    assert!(
        state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .raster_texture
            .is_none()
    );
    controls.options.scale().set_selected(4);
    wait_until("recovered small preview", 90, || {
        state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .preview_status
            == print_preparation_view::PreviewStatus::Current
    });
    capture("small-large-small-preview-recovers");

    controls.review_expander.set_expanded(true);
    controls.prepare_toggle.set_active(true);
    controls.box_unit.set_selected(0);
    controls.box_width.set_text("1387.1786666666667");
    controls.box_height.set_text("788.416");
    controls.target_dpi.set_text("300");
    controls.remove_below.set_text("0");
    controls.min_feature.set_text("0");
    controls.min_gap.set_text("0");
    assert_eq!(
        state
            .borrow()
            .print_preparation_surface
            .as_ref()
            .unwrap()
            .controls
            .selection
            .get()
            .target,
        selection(16_384, 9_312).target
    );
    assert!(controls.export.is_sensitive());
    assert!(controls.options.size_summary().text().contains("300 DPI"));
    controls.min_feature.set_text("1");
    assert!(!controls.export.is_sensitive());
    assert!(controls.options.size_error().text().contains("32,000,000"));
    semantic(
        "cleanup-disabled-controls",
        &[
            "controls",
            "--subtree",
            "PNG export options",
            "--depth",
            "12",
            "--json",
        ],
    );
    capture("large-nonzero-cleanup-disabled-specific-reason");
    controls.prepare_toggle.set_active(false);
    assert!(controls.export.is_sensitive(), "dormant cleanup is ignored");
    controls.prepare_toggle.set_active(true);
    assert!(!controls.export.is_sensitive());
    controls.min_feature.set_text("0");
    assert!(controls.export.is_sensitive());
    semantic(
        "prepared-controls",
        &[
            "controls",
            "--subtree",
            "PNG export options",
            "--depth",
            "12",
            "--json",
        ],
    );
    capture("prepared-exact-DPI-zero-cleanup-export-enabled");
    let prepared = stage().join("gtk-photo-prepared-16384x9312-300dpi.png");
    open_print_preparation_surface(&state, Some(prepared.clone()), false);
    if !reuse_actual {
        controls.export.emit_clicked();
        assert!(state.borrow().pending_export);
        wait_until("actual prepared large PNG", 150, || {
            !state.borrow().pending_export
        });
    }
    assert!(prepared.is_file());
    capture("prepared-large-export-complete");

    let sentinel = stage().join("gtk-cancel-approved-sentinel.png");
    fs::write(&sentinel, b"approved existing bytes").unwrap();
    controls.prepare_toggle.set_active(false);
    controls.options.scale().set_selected(3);
    open_print_preparation_surface(&state, Some(sentinel.clone()), false);
    controls.export.emit_clicked();
    assert!(state.borrow().pending_export);
    semantic(
        "cancel-wait",
        &["wait", "Cancel export", "--exact", "--role", "button"],
    );
    semantic(
        "cancel-activate",
        &["activate", "Cancel export", "--exact", "--role", "button"],
    );
    wait_until("cancelled PNG worker reaped", 90, || {
        !state.borrow().pending_export
    });
    assert_eq!(fs::read(&sentinel).unwrap(), b"approved existing bytes");
    assert_eq!(
        state.borrow().workspace.as_ref().unwrap().document(),
        &original
    );
    capture("cancel-preserves-approved-file");
    fs::write(stage().join(if reuse_actual { "gtk-native-replay-result.txt" } else { "gtk-native-result.txt" }), "ordinary/prepared exact large output; zero-cleanup admission; invalid live input guard; small-large-small; native Cancel sentinel preservation; unchanged captured document\n").unwrap();
    let epoch = state
        .borrow()
        .print_preparation_surface
        .as_ref()
        .unwrap()
        .epoch;
    detach_print_preparation_surface(&state, epoch, true);
    let window = state.borrow().window.clone();
    window.close();
}
