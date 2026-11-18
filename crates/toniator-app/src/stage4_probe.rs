//! Private Stage4 integration witness; calls the actual Main constructors/signals and native adjustments.
use super::*;
use std::cell::RefCell;
thread_local! { static SCENE: RefCell<Option<Arc<toniator_engine::RenderScene>>> = const { RefCell::new(None) }; static SCENE_ACCEPTS: Cell<u64> = const { Cell::new(0) }; }

/// Observes only scheduler-accepted Main scenes; the witness cannot install an unaccepted canonical Arc.
pub(super) fn accepted_scene(scene: Arc<toniator_engine::RenderScene>) {
    SCENE.with(|s| *s.borrow_mut() = Some(scene));
    SCENE_ACCEPTS.with(|v| v.set(v.get() + 1));
}
/// Pumps the real GTK context in a bounded private test, never inside a product callback.
/// # Panics
/// Panics on the declared finite deadline rather than silently dropping a failed observation.
fn until(seconds: u64, mut ready: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    loop {
        for _ in 0..100 {
            if !glib::MainContext::default().pending() {
                break;
            }
            glib::MainContext::default().iteration(false);
        }
        if ready() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(seconds),
            "Stage4 declared deadline exceeded"
        );
        thread::sleep(Duration::from_millis(1));
    }
}
/// Captures owned process CPU ticks and high-water RSS alongside GLib monotonic time.
/// # Panics
/// Panics if Linux's own process counters are unavailable or malformed.
fn resources() -> serde_json::Value {
    let stat = fs::read_to_string("/proc/self/stat").unwrap();
    let fields = stat[stat.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .collect::<Vec<_>>();
    let status = fs::read_to_string("/proc/self/status").unwrap();
    serde_json::json!({"monotonic_us":glib::monotonic_time(),"cpu_ticks":fields[11].parse::<u64>().unwrap()+fields[12].parse::<u64>().unwrap(),"vm_hwm":status.lines().find(|s|s.starts_with("VmHWM:")),"vm_rss":status.lines().find(|s|s.starts_with("VmRSS:"))})
}
/// Returns only a successful exact real Canvas draw for the current actual target/consumer.
fn canvas_ready(state: &Rc<RefCell<AppState>>) -> bool {
    let s = state.borrow();
    let Some((_, captured, done)) = preview_measurement::captured() else {
        return false;
    };
    done && main_preview_frame_identity(&s) == Some(captured.artwork)
        && main_viewport_target(&s) == Some(captured.raster)
        && s.spatial_preview.provenance() == captured.provenance
        && !preview_measurement::feedback_pending()
}
/// Captures compositor pixels between measurements, using only the verified private output.
/// # Panics
/// Panics if grim cannot capture the owned output; it never opens a desktop portal.
fn screenshot(path: &Path) {
    assert!(
        std::process::Command::new("grim")
            .args(["-o", "HEADLESS-1"])
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
/// Operates the native Switch handler and waits for actual complete Canvas/GDK feedback.
/// # Panics
/// Panics if the real route cannot publish within its finite qualification deadline.
fn mode(state: &Rc<RefCell<AppState>>, fast: bool) {
    let control = state.borrow().fast_preview.clone();
    control.set_active(fast);
    until(150, || canvas_ready(state));
}
/// Reads prospective sampler/grid coverage using current GTK geometry and existing target authority.
/// No adjustment, cache plan, LRU or renderer state changes during this pre-request snapshot.
fn cell_snapshot(state: &Rc<RefCell<AppState>>, x: f64, y: f64) -> Option<serde_json::Value> {
    let s = state.borrow();
    let canvas = s.workspace.as_ref()?.document().canvas();
    let scroll = &s.viewport_scroll;
    let point = s
        .picture
        .compute_point(scroll, &gtk::graphene::Point::new(0., 0.))?;
    let origin = [
        f64::from(point.x()) + scroll.hadjustment().value() - x,
        f64::from(point.y()) + scroll.vadjustment().value() - y,
    ];
    let page = [
        scroll.hadjustment().page_size(),
        scroll.vadjustment().page_size(),
    ];
    let picture = [f64::from(s.picture.width()), f64::from(s.picture.height())];
    let viewport_request::Resolution::Ready(target) = viewport_request::resolve(
        canvas,
        (page[0], page[1]),
        (picture[0], picture[1]),
        (origin[0], origin[1]),
        scroll.scale_factor(),
        false,
    ) else {
        return None;
    };
    let target = target.with_background(main_viewport_target(&s)?.background());
    let (_, _, scale) = target.viewport()?;
    let zoom = s.view_state.zoom();
    let layout = preview_cache::Layout::new(
        [canvas.width, canvas.height],
        [canvas.width * zoom, canvas.height * zoom],
        picture,
        origin,
        page,
        scroll.scale_factor(),
        scale,
    )?
    .with_viewport(scroll)?;
    s.spatial_preview
        .diagnostic_cells(main_preview_frame_identity(&s)?, target, &layout)
}
/// Exercises real Main canonical acceptance, fresh native mode cycles and fixed native scroll origins.
/// The immutable case file is read once; no command API or backend bypass exists. Actual process/source
/// identity must be verified through the owned runtime gate before file loading or measurement.
/// # Panics
/// Panics on isolation, target, canonical reuse, publication, finite resource or artifact failures.
#[test]
#[ignore = "requires verified agent-owned private Stage4 Wayland session"]
fn stage4_main_canvas_qualification() {
    assert_eq!(
        std::env::var("TONIATOR_STAGE4_PRIVATE").as_deref(),
        Ok("owned-private-session")
    );
    assert!(preview_measurement::enabled());
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(std::env::var_os("TONIATOR_STAGE4_CASE").unwrap()).unwrap(),
    )
    .unwrap();
    let out = PathBuf::from(config["output"].as_str().unwrap());
    assert!(!out.exists());
    fs::create_dir_all(&out).unwrap();
    glib::set_prgname(Some("Toniator"));
    glib::set_application_name("Toniator");
    register_resources();
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let state = build_window(&app);
    let gate = PathBuf::from(std::env::var_os("TONIATOR_STAGE4_GATE").unwrap());
    until(60, || gate.exists());
    dismiss_startup(&state);
    if config["initial_settings"].as_bool() == Some(false) {
        let drawer = state.borrow().shell.drawer();
        drawer.set_active(false);
    }
    if config["initial_fast"].as_bool().unwrap_or(false) {
        let control = state.borrow().fast_preview.clone();
        control.set_active(true);
    }
    let started = resources();
    start_load(&state, PathBuf::from(config["project"].as_str().unwrap()));
    until(150, || canvas_ready(&state));
    let cold_initial_end = resources();
    let cold_initial_target = format!("{:?}", main_viewport_target(&state.borrow()));
    // Resize only the owned Sway window to the declared physical sampled viewport, before timings.
    let device = state.borrow().viewport_scroll.scale_factor();
    let requested = [
        config["viewport"][0].as_f64().unwrap_or(640.),
        config["viewport"][1].as_f64().unwrap_or(480.),
    ];
    let zoom = config["zoom"].as_f64().unwrap();
    while state.borrow().view_state.zoom() < zoom {
        let b = state.borrow().zoom_in.clone();
        b.emit_clicked();
    }
    until(150, || {
        canvas_ready(&state)
            && main_viewport_target(&state.borrow())
                .and_then(|t| t.viewport())
                .is_some_and(|(_, _, scale)| scale == zoom * f64::from(device))
    });
    for _ in 0..4 {
        let s = state.borrow();
        let page = [
            s.viewport_scroll.hadjustment().page_size(),
            s.viewport_scroll.vadjustment().page_size(),
        ];
        if page
            == [
                requested[0] / f64::from(device),
                requested[1] / f64::from(device),
            ]
        {
            break;
        }
        let w = f64::from(s.window.width()) + requested[0] / f64::from(device) - page[0];
        let h = f64::from(s.window.height()) + requested[1] / f64::from(device) - page[1];
        drop(s);
        assert!(
            std::process::Command::new("swaymsg")
                .arg(format!(
                    "[pid={}] floating enable, resize set width {} height {}",
                    std::process::id(),
                    w as i32,
                    h as i32
                ))
                .status()
                .unwrap()
                .success()
        );
        let before = glib::monotonic_time();
        until(10, || glib::monotonic_time() - before > 200_000);
        until(150, || canvas_ready(&state));
    }
    let canonical = SCENE.with(|s| s.borrow().clone()).unwrap();
    let page = {
        let s = state.borrow();
        [
            s.viewport_scroll.hadjustment().page_size(),
            s.viewport_scroll.vadjustment().page_size(),
        ]
    };
    assert_eq!(
        page,
        [
            requested[0] / f64::from(device),
            requested[1] / f64::from(device)
        ],
        "declared viewport must match actual allocation"
    );
    mode(&state, true);
    let admitted = state.borrow().spatial_preview.provenance().effective
        == toniator_engine::preview_cache::Quality::Fast;
    let preflight = serde_json::json!({"admitted":admitted,"fallback":format!("{:?}",state.borrow().spatial_preview.fallback_reason()),"zoom":zoom,"device":device,"page":page,"initial_fast":config["initial_fast"],"cold_workflow_start":started,"cold_initial_frame_end":cold_initial_end,"cold_initial_target":cold_initial_target,"preflight_end":resources(),"canonical_scene":canonical.identity().scene_fingerprint(),"canonical_primitives":canonical.circular_mark_count()});
    fs::write(
        out.join("preflight.json"),
        serde_json::to_vec_pretty(&preflight).unwrap(),
    )
    .unwrap();
    screenshot(&out.join("preflight.png"));
    if config["layout_checks"].as_bool().unwrap_or(false) {
        for (name, width, height, settings) in
            [("narrow", 700, 800, false), ("wide", 1440, 1000, true)]
        {
            let drawer = state.borrow().shell.drawer();
            drawer.set_active(settings);
            assert!(
                std::process::Command::new("swaymsg")
                    .arg(format!(
                        "[pid={}] floating enable, resize set width {width} height {height}",
                        std::process::id()
                    ))
                    .status()
                    .unwrap()
                    .success()
            );
            let changed = glib::monotonic_time();
            until(10, || glib::monotonic_time() - changed > 200_000);
            until(150, || canvas_ready(&state));
            screenshot(&out.join(format!("layout-{name}.png")));
        }
    }
    if admitted && !config["preflight_only"].as_bool().unwrap_or(false) {
        let origins = config["origins"].as_array().unwrap();
        let pairs = config["pairs"].as_u64().unwrap_or(5);
        let mut runs = vec![("warmup", false), ("warmup", true)];
        for pair in 0..pairs {
            for fast in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                runs.push(("measured", fast));
            }
        }
        let mut records = Vec::new();
        for (run, (kind, fast)) in runs.into_iter().enumerate() {
            // Actual ordinary mode cycles condition fresh roots; their real work is retained in logs.
            mode(&state, !fast);
            let preparation_start = resources();
            mode(&state, fast);
            let first_canvas_end = resources();
            let mut conditioning = None;
            if config["prepared"].as_bool().unwrap_or(false) {
                // Exact memo display registers only background demand. A real one-pixel pan
                // creates visible demand under diagnostic prefetchOFF, then restores the origin.
                let (h, v, old_target) = {
                    let s = state.borrow();
                    (
                        s.viewport_scroll.hadjustment(),
                        s.viewport_scroll.vadjustment(),
                        main_viewport_target(&s),
                    )
                };
                let (old_x, old_y) = (h.value(), v.value());
                let ready = cell_snapshot(&state, old_x, old_y).is_some_and(|d| {
                    d["root_ready"].as_bool() == Some(true)
                        && d["cells"].as_array().is_some_and(|cells| {
                            cells.iter().all(|c| c["stored"].as_bool() == Some(true))
                        })
                });
                if !ready {
                    h.set_value(old_x + 1.);
                    until(150, || {
                        canvas_ready(&state) && main_viewport_target(&state.borrow()) != old_target
                    });
                    h.set_value(old_x);
                    until(150, || {
                        canvas_ready(&state) && main_viewport_target(&state.borrow()) == old_target
                    });
                    conditioning = Some(
                        serde_json::json!({"logical_from":[old_x,old_y],"logical_nudge":[old_x+1.,old_y],"restored":true,"device_scale":device,"end":resources()}),
                    );
                }
                until(150, || {
                    let (x, y) = {
                        let s = state.borrow();
                        (
                            s.viewport_scroll.hadjustment().value(),
                            s.viewport_scroll.vadjustment().value(),
                        )
                    };
                    cell_snapshot(&state, x, y).is_some_and(|d| {
                        d["root_ready"].as_bool() == Some(true)
                            && d["cells"].as_array().is_some_and(|cells| {
                                cells.iter().all(|c| c["stored"].as_bool() == Some(true))
                            })
                    })
                });
            }
            let preparation_end = resources();
            preview_measurement::emit(
                serde_json::json!({"event":"trace_prepared","run":run,"kind":kind,"fast":fast,"isolated":config["prepared"],"conditioning":conditioning,"start":preparation_start,"first_canvas_end":first_canvas_end,"end":preparation_end,"statistics":format!("{:?}",state.borrow().spatial_preview.statistics())}),
            );
            let before = state.borrow().spatial_preview.statistics();
            let accepts = SCENE_ACCEPTS.with(Cell::get);
            for (step, origin) in origins.iter().enumerate() {
                let x = origin[0].as_f64().unwrap() / f64::from(device);
                let y = origin[1].as_f64().unwrap() / f64::from(device);
                let (h, v) = {
                    let s = state.borrow();
                    (
                        s.viewport_scroll.hadjustment(),
                        s.viewport_scroll.vadjustment(),
                    )
                };
                let pre = resources();
                let stats = format!("{:?}", state.borrow().spatial_preview.statistics());
                let cells_before = cell_snapshot(&state, x, y);
                preview_measurement::emit(
                    serde_json::json!({"event":"pan_action","run":run,"kind":kind,"step":step,"fast":fast,"origin":origin,"before":pre,"cells_before":cells_before}),
                );
                h.set_value(x);
                v.set_value(y);
                until(150, || {
                    canvas_ready(&state)
                        && h.value() == x
                        && v.value() == y
                        && main_viewport_target(&state.borrow())
                            .and_then(|t| t.viewport())
                            .is_some_and(|(ox, oy, scale)| {
                                [ox * scale, oy * scale]
                                    == [origin[0].as_f64().unwrap(), origin[1].as_f64().unwrap()]
                            })
                });
                let s = state.borrow();
                let target = main_viewport_target(&s).unwrap();
                let (ox, oy, scale) = target.viewport().unwrap();
                assert_eq!(
                    [ox * scale, oy * scale],
                    [origin[0].as_f64().unwrap(), origin[1].as_f64().unwrap()]
                );
                assert!(SCENE.with(|v| Arc::ptr_eq(v.borrow().as_ref().unwrap(), &canonical)));
                assert_eq!(
                    SCENE_ACCEPTS.with(Cell::get),
                    accepts,
                    "pan must not reevaluate canonical work"
                );
                records.push(serde_json::json!({"run":run,"kind":kind,"fast":fast,"step":step,"origin":origin,"target":format!("{target:?}"),"before":pre,"after":resources(),"statistics_before":stats,"statistics_after":format!("{:?}",s.spatial_preview.statistics()),"cells_before":cells_before,"capture":preview_measurement::captured().map(|(id,_,_)|id)}));
                drop(s);
                if step == 0 || step == 5 {
                    screenshot(&out.join(format!("run-{run}-step-{step}.png")));
                }
            }
            let after = state.borrow().spatial_preview.statistics();
            if fast {
                assert_eq!(before.fast_preparations, after.fast_preparations);
                assert_eq!(
                    before.fast_device_initializations,
                    after.fast_device_initializations
                );
            }
            fs::write(
                out.join("traces.json"),
                serde_json::to_vec_pretty(&records).unwrap(),
            )
            .unwrap();
        }
    }
    if !admitted {
        let mut fallback = Vec::new();
        for n in 0..config["fallback_cycles"].as_u64().unwrap_or(0) {
            for fast in [false, true] {
                let before = resources();
                mode(&state, fast);
                fallback.push(serde_json::json!({"cycle":n,"fast":fast,"before":before,"after":resources(),"provenance":format!("{:?}",state.borrow().spatial_preview.provenance()),"fallback":format!("{:?}",state.borrow().spatial_preview.fallback_reason())}));
            }
        }
        fs::write(
            out.join("fallback-cycles.json"),
            serde_json::to_vec_pretty(&fallback).unwrap(),
        )
        .unwrap();
    }
    if admitted && config["fractional"].as_bool().unwrap_or(false) {
        let mut fractional = Vec::new();
        for fast in [false, true] {
            mode(&state, fast);
            for (step, (x, y)) in [
                (0.25, 0.5),
                (1024.375, 400.625),
                (2024.5, 800.25),
                (2959.5, 1999.5),
            ]
            .into_iter()
            .enumerate()
            {
                let (h, v) = {
                    let s = state.borrow();
                    (
                        s.viewport_scroll.hadjustment(),
                        s.viewport_scroll.vadjustment(),
                    )
                };
                let before = resources();
                let start = glib::monotonic_time();
                h.set_value(x / f64::from(device));
                v.set_value(y / f64::from(device));
                until(150, || {
                    glib::monotonic_time() - start > 100_000 && canvas_ready(&state)
                });
                fractional.push(serde_json::json!({"fast":fast,"step":step,"requested_physical_origin":[x,y],"adjustment":[h.value(),v.value()],"actual_target":format!("{:?}",main_viewport_target(&state.borrow())),"capture":preview_measurement::captured().map(|(id,_,_)|id),"before":before,"after":resources()}));
            }
        }
        fs::write(
            out.join("fractional.json"),
            serde_json::to_vec_pretty(&fractional).unwrap(),
        )
        .unwrap();
        screenshot(&out.join("fractional-final.png"));
    }
    if config["outputs"].as_bool().unwrap_or(false) {
        output_authority(&state, &out);
    }
    let window = state.borrow().window.clone();
    window.close();
    preview_measurement::emit(
        serde_json::json!({"event":"witness_finished","resources":resources()}),
    );
}

/// Runs independent CPU final consumers from one immutable current capture, never requested UI quality.
/// # Panics
/// Panics on native output, semantic SVG, bounded two-frame sequence or invalid print authority.
/// A bounded print refusal remains an observed CPU outcome rather than a fabricated raster.
fn captured_outputs(capture: print_preparation::ExportCapture, out: PathBuf) {
    fs::create_dir_all(&out).unwrap();
    let settings = ExportSettings {
        format: ExportFormat::Png,
        background: RasterBackground::Transparent,
        antialiasing: RasterAntialiasing::On,
        output_target: None,
        prepare_for_print: false,
        pixels_per_metre: None,
        cleanup: print_cleanup::Settings::default(),
    };
    let cancelled = AtomicBool::new(false);
    export_captured_frame_cancellable(
        capture.clone(),
        out.join("native.png"),
        settings,
        &cancelled,
    )
    .unwrap();
    export_captured_frame_cancellable(
        capture.clone(),
        out.join("semantic.svg"),
        ExportSettings {
            format: ExportFormat::Svg,
            ..settings
        },
        &cancelled,
    )
    .unwrap();
    let prepared = render_captured_png_surface(
        &capture,
        ExportSettings {
            prepare_for_print: true,
            ..settings
        },
        &cancelled,
    )
    .unwrap();
    fs::write(out.join("prepared.png"), encode_png(&prepared).unwrap()).unwrap();
    let first = capture
        .session
        .document()
        .project_timing()
        .frame_range()
        .start();
    let document =
        temporal_export::document_for_export(capture.session.document(), first, first + 1).unwrap();
    let sequence = toniator_engine::export::SequenceExportJob::new(
        document,
        capture.sources.clone(),
        toniator_engine::export::SequenceExportOptions {
            destination: out.join("sequence"),
            format: toniator_io::sequence::SequenceFormat::Png,
            background: Some(RasterBackground::Transparent),
            target: None,
            antialiasing: RasterAntialiasing::On,
            limits: EvaluationLimits::default(),
        },
    )
    .unwrap()
    .run(toniator_engine::MediaTools::default(), &cancelled, &|_| {})
    .unwrap();
    assert_eq!(sequence.frame_count, 2);
    let mut controller = print_preparation::Controller::default();
    let selection = print_preparation::OutputSelection {
        target: StreamingRasterTarget::for_canvas(capture.session.document().canvas()).unwrap(),
        antialiasing: RasterAntialiasing::On,
        background: print_preparation::BackgroundChoice::Explicit(RasterBackground::Transparent),
        prepare_for_print: false,
        pixels_per_metre: None,
        cleanup: print_cleanup::Settings::default(),
    };
    let live = print_preparation::LiveAuthority {
        token: capture.session.document_evaluation_token(),
        frame: capture.frame,
        lifecycle_generation: capture.lifecycle_generation,
        workspace_generation: capture.workspace_generation,
    };
    let job = controller.request(capture, selection).unwrap();
    assert!(controller.complete(job.run(), Some(live)).is_none());
    if let Some(pair) = controller.accepted() {
        fs::write(out.join("print.png"), encode_png(pair.raster()).unwrap()).unwrap();
        fs::write(out.join("print-report.txt"), format!("{:?}", pair.report())).unwrap();
    } else {
        assert_eq!(
            controller.status(),
            print_preparation::CheckStatus::Unavailable
        );
        assert!(controller.unavailable_reason().is_some());
    }
    fs::write(out.join("print-outcome.json"), serde_json::to_vec_pretty(&serde_json::json!({"status":format!("{:?}",controller.status()),"unavailable_reason":controller.unavailable_reason(),"retained_raster":controller.accepted().is_some()})).unwrap()).unwrap();
    fs::write(out.join("authority.json"),serde_json::to_vec_pretty(&serde_json::json!({"sequence_frames":sequence.frame_count,"dimensions":[sequence.width,sequence.height],"print_status":format!("{:?}",controller.status()),"consumer":"independent CPU captured PNG/prepared PNG/semantic SVG/SequenceExportJob/CheckJob"})).unwrap()).unwrap();
}

/// Pairs real Main OFF/ON publications with independent CPU exports and unchanged document authority.
/// # Panics
/// Panics if quality leaks into outputs or output captures mutate current authored/session state.
fn output_authority(state: &Rc<RefCell<AppState>>, out: &Path) {
    let before = {
        let s = state.borrow();
        let w = s.workspace.as_ref().unwrap();
        (
            w.document().clone(),
            w.history.session().document_evaluation_token(),
            w.is_dirty(),
        )
    };
    for fast in [false, true] {
        mode(state, fast);
        screenshot(&out.join(if fast {
            "output-main-fast.png"
        } else {
            "output-main-accurate.png"
        }));
        let capture = {
            let s = state.borrow();
            let w = s.workspace.as_ref().unwrap();
            capture_still_export(
                w,
                w.document().project_timing().frame_range().start(),
                s.generation,
                s.workspace_generation,
            )
        };
        let destination = out.join(if fast {
            "outputs-fast"
        } else {
            "outputs-accurate"
        });
        let worker = thread::spawn(move || captured_outputs(capture, destination));
        until(300, || worker.is_finished());
        worker.join().unwrap();
        let s = state.borrow();
        let w = s.workspace.as_ref().unwrap();
        assert_eq!(
            before,
            (
                w.document().clone(),
                w.history.session().document_evaluation_token(),
                w.is_dirty()
            )
        );
    }
    for file in [
        "native.png",
        "semantic.svg",
        "prepared.png",
        "print-outcome.json",
    ] {
        assert_eq!(
            fs::read(out.join("outputs-accurate").join(file)).unwrap(),
            fs::read(out.join("outputs-fast").join(file)).unwrap(),
            "Fast UI choice changed CPU {file}"
        );
    }
    let print_a = out.join("outputs-accurate/print.png");
    let print_f = out.join("outputs-fast/print.png");
    assert_eq!(print_a.exists(), print_f.exists());
    if print_a.exists() {
        assert_eq!(fs::read(print_a).unwrap(), fs::read(print_f).unwrap());
    }
    let a = out.join("outputs-accurate/sequence");
    let b = out.join("outputs-fast/sequence");
    for file in fs::read_dir(&a).unwrap() {
        let file = file.unwrap();
        if file.path().extension().is_some_and(|e| e == "png") {
            assert_eq!(
                fs::read(file.path()).unwrap(),
                fs::read(b.join(file.file_name())).unwrap()
            );
        }
    }
}
