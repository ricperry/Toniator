//! Project timing controls retain a captured document while shared media authority validates edits.

use super::*;
use toniator_domain::{ProjectTiming, TemporalCommand};
use toniator_engine::{FrameSource, MediaTimingSelection, MediaTools, SourceMediaMetadata};

/// Owns the modal projection and cancellable metadata worker; authored state stays in history.
pub(super) struct Surface {
    window: gtk::Window,
    base: Document,
    fields: [gtk::Entry; 3],
    metadata: Option<SourceMediaMetadata>,
    status: gtk::Label,
    apply: gtk::Button,
    cancelled: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for Surface {
    /// Cancels and reaps metadata decoding before releasing modal ownership.
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Formats reduced exact values without introducing display-rounding edits.
fn exact(numerator: u64, denominator: u64) -> String {
    if denominator == 1 {
        numerator.to_string()
    } else {
        format!("{numerator}/{denominator}")
    }
}

/// Projects frame rate and the half-open source interval without changing timing representation.
///
/// # Errors
/// Reports exact-time overflow from the authoritative project timing.
fn values(timing: &ProjectTiming) -> Result<[String; 3], String> {
    let (start, end) = if let Some(range) = timing.source_time_range() {
        (range.start(), range.end())
    } else {
        let rate = timing.frame_rate();
        (
            rate.time_for_frame(timing.frame_range().start())
                .map_err(|error| error.to_string())?,
            rate.time_for_frame(timing.frame_range().end_exclusive())
                .map_err(|error| error.to_string())?,
        )
    };
    let rate = timing.frame_rate();
    Ok([
        exact(rate.numerator().into(), rate.denominator().into()),
        exact(start.numerator(), start.denominator()),
        exact(end.numerator(), end.denominator()),
    ])
}

/// Builds one source-validated timing command while preserving every authored End override.
///
/// # Errors
/// Rejects malformed rate/time text, invalid intervals and out-of-source bounds atomically.
fn command(
    base: &Document,
    metadata: &SourceMediaMetadata,
    fields: &[String; 3],
) -> Result<TemporalCommand, String> {
    let current = values(base.project_timing())?;
    let rate = toniator_engine::parse_media_rate(fields[0].trim())?;
    let start = toniator_engine::parse_media_time(fields[1].trim())?;
    let end = toniator_engine::parse_media_time(fields[2].trim())?;
    let selection = MediaTimingSelection {
        frame_rate: (rate != base.project_timing().frame_rate()).then_some(rate),
        start_time: (start != toniator_engine::parse_media_time(&current[1])?).then_some(start),
        end_time: (end != toniator_engine::parse_media_time(&current[2])?).then_some(end),
        ..Default::default()
    };
    let timing =
        toniator_engine::select_media_timing(metadata, Some(base.project_timing()), &selection)
            .map_err(|error| error.message().to_owned())?;
    Ok(base.replace_temporal_authority_command(timing, base.temporal_end_overrides().to_vec()))
}

/// Presents a single project timing sheet and probes media off the GTK thread.
pub(super) fn open(state: &Rc<RefCell<AppState>>) {
    if let Some(surface) = state.borrow().temporal_settings.as_ref() {
        surface.window.present();
        return;
    }
    let (snapshot, parent) = {
        let app = state.borrow();
        if lifecycle_is_busy(&app) {
            return;
        }
        let Some(workspace) = app.workspace.as_ref() else {
            return;
        };
        (workspace.snapshot(), app.window.clone())
    };
    let text = match values(snapshot.document.project_timing()) {
        Ok(text) => text,
        Err(error) => {
            set_inspector_status(&mut state.borrow_mut(), error);
            return;
        }
    };
    let window = gtk::Window::builder()
        .title("Animation settings")
        .transient_for(&parent)
        .modal(true)
        .default_width(540)
        .resizable(false)
        .build();
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Animation settings"))));
    window.set_titlebar(Some(&header));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let hint = gtk::Label::new(Some(
        "Choose the source interval and output frame rate. Start and End artwork settings span this interval. Source speed stays unchanged.",
    ));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    content.append(&hint);
    let fields = std::array::from_fn(|index| {
        let title = [
            "Frame rate (fps)",
            "Source start (seconds)",
            "Source end (excluded, seconds)",
        ][index];
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let label = gtk::Label::new(Some(title));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        let entry = gtk::Entry::new();
        entry.set_width_chars(16);
        entry.set_text(&text[index]);
        entry.set_input_purpose(gtk::InputPurpose::Number);
        label.set_mnemonic_widget(Some(&entry));
        relate_descriptor_label(entry.upcast_ref(), &label);
        entry.set_tooltip_text(Some(
            "Enter an integer, decimal, or exact fraction such as 30000/1001.",
        ));
        row.append(&label);
        row.append(&entry);
        content.append(&row);
        entry
    });
    let status = gtk::Label::new(Some("Reading source timing…"));
    status.update_property(&[gtk::accessible::Property::Description(
        "Animation settings status",
    )]);
    status.set_wrap(true);
    status.set_xalign(0.0);
    content.append(&status);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let apply = gtk::Button::with_label("Apply");
    apply.add_css_class("suggested-action");
    apply.set_sensitive(false);
    buttons.append(&cancel);
    buttons.append(&apply);
    content.append(&buttons);
    window.set_child(Some(&content));
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let (sender, receiver) = async_channel::bounded(1);
    let worker = thread::Builder::new()
        .name("toniator-timing-metadata".into())
        .spawn(move || {
            let result = toniator_engine::open_source_media(
                &snapshot.sources,
                MediaTools::default(),
                &|| flag.load(Ordering::Acquire),
            )
            .map(|source| source.metadata().clone())
            .map_err(|error| error.to_string());
            let _ = sender.send_blocking(result);
        });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            set_inspector_status(&mut state.borrow_mut(), error.to_string());
            return;
        }
    };
    state.borrow_mut().temporal_settings = Some(Surface {
        window: window.clone(),
        base: snapshot.document,
        fields,
        metadata: None,
        status,
        apply: apply.clone(),
        cancelled,
        worker: Some(worker),
    });
    sync_ui(&mut state.borrow_mut());
    let handle = state.clone();
    cancel.connect_clicked(move |_| close(&handle));
    let handle = state.clone();
    window.connect_close_request(move |_| {
        close(&handle);
        glib::Propagation::Proceed
    });
    let handle = state.clone();
    apply.connect_clicked(move |_| apply_settings(&handle));
    if let Some(surface) = state.borrow().temporal_settings.as_ref() {
        for field in &surface.fields {
            let handle = state.clone();
            field.connect_changed(move |_| {
                if let Some(surface) = handle.borrow().temporal_settings.as_ref() {
                    refresh_validation(surface);
                }
            });
        }
    }
    let handle = state.clone();
    let expected = window.downgrade();
    glib::MainContext::default().spawn_local(async move {
        let Ok(result) = receiver.recv().await else {
            return;
        };
        let mut app = handle.borrow_mut();
        let Some(surface) = app
            .temporal_settings
            .as_mut()
            .filter(|surface| Some(surface.window.clone()) == expected.upgrade())
        else {
            return;
        };
        match result {
            Ok(metadata) => {
                surface.metadata = Some(metadata);
                refresh_validation(surface);
            }
            Err(error) => surface
                .status
                .set_label(&format!("Couldn’t read source timing: {error}")),
        }
    });
    window.present();
}

/// Projects current shared validation and output-frame count without rendering or changing history.
fn refresh_validation(surface: &Surface) {
    let Some(metadata) = surface.metadata.as_ref() else {
        return;
    };
    let fields = std::array::from_fn(|index| surface.fields[index].text().to_string());
    match command(&surface.base, metadata, &fields) {
        Ok(command) => {
            surface.apply.set_sensitive(true);
            surface.status.set_label(&format!("{} output frames. {} {}",
                command.replacement().project_timing.frame_range().frame_count(),
                if metadata.variable_frame_rate { "Variable-rate source: output uses the selected constant rate; frames may repeat or be skipped." } else { "Output uses the selected constant frame rate." },
                if metadata.has_audio { "Source contains audio; initial exports are silent." } else { "" }));
        }
        Err(error) => {
            surface.apply.set_sensitive(false);
            surface
                .status
                .set_label(&format!("Check animation settings: {error}"));
        }
    }
}

/// Cancels and closes the current sheet before allowing competing document actions.
pub(super) fn close(state: &Rc<RefCell<AppState>>) {
    let surface = state.borrow_mut().temporal_settings.take();
    if let Some(surface) = surface {
        let window = surface.window.clone();
        drop(surface);
        window.close();
        sync_ui(&mut state.borrow_mut());
    }
}

/// Validates the complete sheet and publishes one ordinary history command after releasing the modal.
fn apply_settings(state: &Rc<RefCell<AppState>>) {
    let result = {
        let app = state.borrow();
        let Some(surface) = app.temporal_settings.as_ref() else {
            return;
        };
        let Some(metadata) = surface.metadata.as_ref() else {
            return;
        };
        command(
            &surface.base,
            metadata,
            &std::array::from_fn(|index| surface.fields[index].text().to_string()),
        )
    };
    match result {
        Ok(command) => {
            close(state);
            temporal_edit::apply(state, Ok(command), None, None);
        }
        Err(error) => {
            if let Some(surface) = state.borrow().temporal_settings.as_ref() {
                surface
                    .status
                    .set_label(&format!("Couldn’t apply animation settings: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks exact rate/trim, source bounds, unchanged artwork and one timing Apply/Undo boundary.
    ///
    /// # Panics
    /// Panics if timing text loses precision, invalid input mutates history or endpoints fail persistence/export.
    #[test]
    fn project_timing_sheet_preserves_artwork_and_exact_history() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/stage22-project-timing")
            .join(format!(
                "run-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        for (input, name, format) in [
            ("raster-sample.png", "raster", ExportFormat::Png),
            ("vector-sample.svg", "vector", ExportFormat::Svg),
            ("video-sample0001-0010.mp4", "video", ExportFormat::Png),
        ] {
            let mut workspace = load_workspace(&root.join("assets").join(input)).unwrap();
            workspace
                .history
                .apply_temporal(&workspace.document().initialize_end_command().unwrap())
                .unwrap();
            let before = workspace.snapshot();
            save_container(
                &directory.join(format!("{name}-before.toniator")),
                &before.document,
                &before.sources,
            )
            .unwrap();
            let metadata =
                toniator_engine::open_source_media(&before.sources, MediaTools::default(), &|| {
                    false
                })
                .unwrap()
                .metadata()
                .clone();
            let original = values(before.document.project_timing()).unwrap();
            let mut one_frame = DocumentHistory::new_draft(&workspace.history);
            one_frame
                .apply_temporal(
                    &command(
                        &before.document,
                        &metadata,
                        &["1".into(), "0".into(), "0.5".into()],
                    )
                    .unwrap(),
                )
                .unwrap();
            assert_eq!(
                temporal_preview::Endpoint::End.for_document(one_frame.document()),
                temporal_preview::Endpoint::Start
            );
            assert_eq!(
                one_frame.document().temporal_end_overrides(),
                before.document.temporal_end_overrides()
            );
            assert_eq!(
                command(&before.document, &metadata, &original)
                    .unwrap()
                    .replacement(),
                &before.document.temporal_authority()
            );
            let mut equivalent = original.clone();
            equivalent[0] = format!(
                "{}/{}",
                before.document.project_timing().frame_rate().numerator(),
                before.document.project_timing().frame_rate().denominator()
            );
            equivalent[1] = "0.0000".into();
            assert_eq!(
                command(&before.document, &metadata, &equivalent)
                    .unwrap()
                    .replacement(),
                &before.document.temporal_authority()
            );
            for invalid in [
                ["0", "0", "1"],
                ["12", "1", "1"],
                ["12", "2", "1"],
                ["1/0", "0", "1"],
            ] {
                assert!(command(&before.document, &metadata, &invalid.map(str::to_owned)).is_err());
            }
            if metadata.duration.is_some() {
                assert!(
                    command(
                        &before.document,
                        &metadata,
                        &["12".into(), "0".into(), "999".into()]
                    )
                    .is_err()
                );
            }
            let edit = command(
                &before.document,
                &metadata,
                &["12".into(), "1/6".into(), "1".into()],
            )
            .unwrap();
            workspace.history.apply_temporal(&edit).unwrap();
            let after = workspace.snapshot();
            assert_eq!(
                after.document.temporal_end_overrides(),
                before.document.temporal_end_overrides()
            );
            assert_eq!(after.sources, before.sources);
            assert_eq!(
                after.document.project_timing().frame_range().frame_count(),
                10
            );
            assert_eq!(
                after
                    .document
                    .project_timing()
                    .source_time_for_frame(0)
                    .unwrap(),
                toniator_domain::RationalTime::new(1, 6).unwrap()
            );
            workspace.history.undo().unwrap().unwrap();
            assert_eq!(workspace.snapshot(), before);
            workspace.history.redo().unwrap().unwrap();
            assert_eq!(workspace.snapshot(), after);
            let path = directory.join(format!("{name}-after.toniator"));
            save_container(&path, &after.document, &after.sources).unwrap();
            assert_eq!(load_workspace(&path).unwrap().snapshot(), after);
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
                9,
            )
            .unwrap();
        }
        assert_eq!(
            toniator_engine::parse_media_rate("30000/1001")
                .unwrap()
                .numerator(),
            30000
        );
        assert_eq!(
            toniator_engine::parse_media_rate("29.97")
                .unwrap()
                .denominator(),
            100
        );
        assert!(toniator_engine::parse_media_time("1e2").is_err());
        println!("Project timing native witnesses: {}", directory.display());
    }
}
