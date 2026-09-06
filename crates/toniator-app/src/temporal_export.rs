//! Desktop export projects immutable shared jobs and retains their cancellation/recovery ownership.

use super::*;
use toniator_domain::{FrameRange, ProjectTiming, TimeRange};
use toniator_engine::export::video::{
    VideoCodec, VideoExportJob, VideoExportOptions, VideoPhase, VideoProgress, VideoRecovery,
};
use toniator_engine::export::{
    ExportPhase, ExportProgress, SequenceExportJob, SequenceExportOptions,
};
use toniator_engine::{FrameSource, MediaTools};
use toniator_io::{export_defaults::ExportDefaults, sequence::SequenceFormat};

/// Identifies the three artist-facing temporal output choices without duplicating codec policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Lossless,
    Pngs,
    Sharing,
}

impl Format {
    /// Resolves one native selector position; unexpected positions never imply a codec.
    ///
    /// # Errors
    /// Rejects positions outside the three displayed output choices.
    fn selected(position: u32) -> Result<Self, String> {
        match position {
            0 => Ok(Self::Lossless),
            1 => Ok(Self::Pngs),
            2 => Ok(Self::Sharing),
            _ => Err("Choose an output format.".into()),
        }
    }
    /// Returns the destination suffix; PNG jobs own a child directory.
    fn suffix(self) -> &'static str {
        match self {
            Self::Lossless => ".mkv",
            Self::Pngs => "",
            Self::Sharing => ".webm",
        }
    }
}

/// Captures validated consumer choices before transferring a snapshot to the worker.
#[derive(Clone)]
struct Options {
    format: Format,
    destination: PathBuf,
    temporary: Option<PathBuf>,
    background: RasterBackground,
    target: Option<OutputRasterTarget>,
    antialiasing: RasterAntialiasing,
    first: u64,
    last: u64,
}

/// Carries phase-local work to GTK; no percentage claims an ETA.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Progress {
    phase: &'static str,
    fraction: f64,
    detail: String,
}

/// Returns either metadata, a finalized output, or owned recovery after one worker operation.
pub(super) enum Outcome {
    Metadata(Result<String, String>),
    Complete {
        path: Option<PathBuf>,
        message: String,
    },
    Failed {
        error: String,
        recovery: Option<Box<VideoRecovery>>,
    },
}

/// Tags asynchronous progress and terminal ownership by the still-live export sheet epoch.
pub(super) enum Event {
    Progress(u64, Progress),
    Finished(u64, Outcome),
}

/// Owns exactly one export operation and its worker; dropping cancels and reaps that worker.
struct Worker {
    cancelled: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for Worker {
    /// Cancels subprocess work and joins the worker before releasing its ownership.
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Holds a modal projection and immutable snapshot, never a second editable document.
pub(super) struct Surface {
    epoch: u64,
    window: gtk::Window,
    snapshot: SavedContent,
    format: gtk::DropDown,
    directory: gtk::Entry,
    name: gtk::Entry,
    first: gtk::Entry,
    last: gtk::Entry,
    dimensions: gtk::Entry,
    background: gtk::DropDown,
    antialiasing: gtk::DropDown,
    temporary: gtk::Entry,
    configuration: gtk::Box,
    destination_controls: gtk::Box,
    configuration_widgets: Vec<gtk::Widget>,
    destination_widgets: Vec<gtk::Widget>,
    status: gtk::Label,
    audio: gtk::Label,
    progress: gtk::ProgressBar,
    export: gtk::Button,
    cancel: gtk::Button,
    retry: gtk::Button,
    save_png: gtk::Button,
    discard: gtk::Button,
    close: gtk::Button,
    still: gtk::Button,
    chooser: Option<gio::Cancellable>,
    worker: Option<Worker>,
    recovery: Option<Box<VideoRecovery>>,
    close_when_done: bool,
    quit_when_done: bool,
}

/// Appends a native labeled field with its real label relationship and keyboard path.
fn row(parent: &gtk::Box, title: &str, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_mnemonic_widget(Some(widget));
    row.append(&label);
    row.append(widget);
    parent.append(&row);
}

/// Creates one ordinary text field without interpreting document or filesystem authority.
fn entry(value: &str) -> gtk::Entry {
    let field = gtk::Entry::new();
    field.set_text(value);
    field.set_width_chars(22);
    field
}

/// Returns the personal settings location; no path is persisted into artwork or a Preset.
fn defaults_path() -> PathBuf {
    glib::user_config_dir().join("Toniator/export-defaults.json")
}

/// Presents one reviewable export sheet and asynchronously measures source/audio metadata.
pub(super) fn open(state: &Rc<RefCell<AppState>>) {
    if let Some(surface) = state.borrow().temporal_export.as_ref() {
        surface.window.present();
        return;
    }
    let (snapshot, name, parent) = {
        let app = state.borrow();
        if lifecycle_is_busy(&app) {
            return;
        }
        let Some(workspace) = app
            .workspace
            .as_ref()
            .filter(|workspace| workspace.can_save())
        else {
            return;
        };
        (
            workspace.snapshot(),
            Path::new(&workspace.display_name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            app.window.clone(),
        )
    };
    let (defaults, notice) = match ExportDefaults::load(&defaults_path()) {
        Ok(defaults) => (defaults, "Preparing source information…".to_owned()),
        Err(error) => (
            ExportDefaults::default(),
            format!("Couldn’t read personal export defaults: {error}"),
        ),
    };
    let window = gtk::Window::builder()
        .title("Export animation")
        .modal(true)
        .transient_for(&parent)
        .default_width(610)
        .default_height(850)
        .build();
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Export animation"))));
    window.set_titlebar(Some(&header));
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let fields = gtk::Box::new(gtk::Orientation::Vertical, 10);
    scroll.set_child(Some(&fields));
    root.append(&scroll);
    let configuration = gtk::Box::new(gtk::Orientation::Vertical, 10);
    fields.append(&configuration);
    let format = accessible_string_dropdown(&[
        "Lossless video — FFV1 / Matroska",
        "PNG sequence",
        "Sharing video — AV1 / WebM (lossy)",
    ]);
    row(&configuration, "Format", &format);
    let timing = snapshot.document.project_timing();
    let fps = timing.frame_rate();
    let info = gtk::Label::new(Some(&format!(
        "{} / {} frames per second · {} frames available",
        fps.numerator(),
        fps.denominator(),
        timing.frame_range().frame_count()
    )));
    info.set_xalign(0.0);
    configuration.append(&info);
    let first = entry(&timing.frame_range().start().to_string());
    let last = entry(&(timing.frame_range().end_exclusive() - 1).to_string());
    row(&configuration, "First frame", &first);
    row(&configuration, "Last frame (included)", &last);
    let dimensions = entry("");
    dimensions.set_placeholder_text(Some("Original size"));
    dimensions.set_tooltip_text(Some(
        "Optional WIDTHxHEIGHT. Empty uses the document’s original size.",
    ));
    row(&configuration, "Size", &dimensions);
    let canvas = snapshot.document.canvas();
    let size = gtk::Label::new(Some(&format!(
        "Original size: {} × {}",
        canvas.width, canvas.height
    )));
    size.set_xalign(0.0);
    configuration.append(&size);
    let background = accessible_string_dropdown(&["Transparent", "Black", "White"]);
    background.set_selected(png_background_dropdown_position(
        snapshot.document.channel_model(),
    ));
    row(&configuration, "Background", &background);
    let matte = gtk::Label::new(Some(
        "Choose Transparent to retain alpha in lossless video or PNG. Sharing video requires a black or white background.",
    ));
    matte.set_xalign(0.0);
    matte.set_wrap(true);
    configuration.append(&matte);
    let antialiasing = accessible_string_dropdown(&["On", "Off"]);
    row(&configuration, "Antialiasing", &antialiasing);
    let audio = gtk::Label::new(Some("Audio: checking source…"));
    audio.set_xalign(0.0);
    audio.set_wrap(true);
    configuration.append(&audio);
    let destination_controls = gtk::Box::new(gtk::Orientation::Vertical, 10);
    fields.append(&destination_controls);
    let directory = entry(
        &defaults
            .destination
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    directory.set_placeholder_text(Some("Choose an output folder"));
    row(&destination_controls, "Destination folder", &directory);
    let browse = gtk::Button::with_label("Browse folders…");
    destination_controls.append(&browse);
    let name = entry(if name.is_empty() { "animation" } else { &name });
    row(&destination_controls, "Job name", &name);
    let explanation = gtk::Label::new(Some(
        "Creates a new video file or a new folder of numbered PNGs. Existing output is never overwritten.",
    ));
    explanation.set_xalign(0.0);
    explanation.set_wrap(true);
    destination_controls.append(&explanation);
    let personal = gtk::Expander::new(Some("Export defaults"));
    personal.update_property(&[gtk::accessible::Property::Label("Export defaults")]);
    let personal_fields = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let temporary = entry(
        &defaults
            .temporary_directory
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    temporary.set_placeholder_text(Some("/tmp"));
    row(&personal_fields, "Temporary folder", &temporary);
    let save_defaults = gtk::Button::with_label("Save these folder defaults");
    personal_fields.append(&save_defaults);
    let clear_defaults = gtk::Button::with_label("Prompt for folders next time");
    personal_fields.append(&clear_defaults);
    personal.set_child(Some(&personal_fields));
    configuration.append(&personal);
    let status = gtk::Label::new(Some(&notice));
    status.set_xalign(0.0);
    status.set_wrap(true);
    root.append(&status);
    let progress = gtk::ProgressBar::new();
    progress.update_property(&[gtk::accessible::Property::Label("Export progress")]);
    progress.set_show_text(true);
    progress.set_text(Some("Preparing"));
    root.append(&progress);
    let recovery_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let retry = gtk::Button::with_label("Retry encoding");
    let save_png = gtk::Button::with_label("Save PNG sequence");
    let discard = gtk::Button::with_label("Discard rendered PNGs");
    for button in [&retry, &save_png, &discard] {
        recovery_row.append(button);
        button.set_visible(false);
    }
    root.append(&recovery_row);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let still = gtk::Button::with_label("Export current frame…");
    buttons.append(&still);
    let close = gtk::Button::with_label("Close export");
    buttons.append(&close);
    let cancel = gtk::Button::with_label("Cancel export");
    buttons.append(&cancel);
    let export = gtk::Button::with_label("Export animation");
    export.add_css_class("suggested-action");
    buttons.append(&export);
    root.append(&buttons);
    window.set_child(Some(&root));
    let epoch = {
        let mut app = state.borrow_mut();
        app.temporal_export_epoch = app.temporal_export_epoch.wrapping_add(1);
        app.temporal_export_epoch
    };
    state.borrow_mut().temporal_export = Some(Surface {
        epoch,
        window: window.clone(),
        snapshot: snapshot.clone(),
        configuration_widgets: vec![
            format.clone().upcast(),
            first.clone().upcast(),
            last.clone().upcast(),
            dimensions.clone().upcast(),
            background.clone().upcast(),
            antialiasing.clone().upcast(),
            temporary.clone().upcast(),
            personal.clone().upcast(),
            save_defaults.clone().upcast(),
            clear_defaults.clone().upcast(),
        ],
        destination_widgets: vec![
            directory.clone().upcast(),
            name.clone().upcast(),
            browse.clone().upcast(),
        ],
        format,
        directory,
        name,
        first,
        last,
        dimensions,
        background,
        antialiasing,
        temporary,
        configuration,
        destination_controls,
        status,
        audio,
        progress,
        export: export.clone(),
        cancel: cancel.clone(),
        retry: retry.clone(),
        save_png: save_png.clone(),
        discard: discard.clone(),
        close: close.clone(),
        still: still.clone(),
        chooser: None,
        worker: None,
        recovery: None,
        close_when_done: false,
        quit_when_done: false,
    });
    let app = state.clone();
    export.connect_clicked(move |_| submit(&app, epoch, Action::Render));
    let app = state.clone();
    retry.connect_clicked(move |_| submit(&app, epoch, Action::Retry));
    let app = state.clone();
    save_png.connect_clicked(move |_| submit(&app, epoch, Action::SavePng));
    let app = state.clone();
    discard.connect_clicked(move |_| submit(&app, epoch, Action::Discard));
    let app = state.clone();
    cancel.connect_clicked(move |_| cancel_work(&app, epoch));
    let app = state.clone();
    close.connect_clicked(move |_| {
        request_close(&app, false);
    });
    let app = state.clone();
    window.connect_close_request(move |_| {
        if request_close(&app, false) {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    let app = state.clone();
    browse.connect_clicked(move |_| browse_directory(&app, epoch));
    let app = state.clone();
    save_defaults.connect_clicked(move |_| save_personal_defaults(&app, epoch, false));
    let app = state.clone();
    clear_defaults.connect_clicked(move |_| save_personal_defaults(&app, epoch, true));
    let app = state.clone();
    still.connect_clicked(move |_| {
        let ready = app
            .borrow()
            .temporal_export
            .as_ref()
            .is_some_and(|surface| {
                surface.epoch == epoch && surface.worker.is_none() && surface.recovery.is_none()
            });
        if ready {
            dismiss(&app);
            choose_still_export(&app);
        }
    });
    sync_ui(&mut state.borrow_mut());
    window.present();
    launch(state, epoch, move |cancelled, _| {
        let result =
            toniator_engine::open_source_media(&snapshot.sources, MediaTools::default(), &|| {
                cancelled.load(Ordering::Acquire)
            })
            .map(|media| {
                if media.metadata().has_audio {
                    "Source contains audio. This export is silent.".to_owned()
                } else {
                    "Source has no audio. Output is silent.".to_owned()
                }
            })
            .map_err(|error| error.to_string());
        Outcome::Metadata(result)
    });
}

/// Validates an absolute existing directory while retaining Unicode path input.
///
/// # Errors
/// Rejects missing, relative or unavailable required folders before a worker starts.
fn directory(text: &str, optional: bool) -> Result<Option<PathBuf>, String> {
    if text.trim().is_empty() && optional {
        return Ok(None);
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() || !path.is_dir() {
        return Err("Choose an accessible absolute folder path. If a saved folder is unavailable, choose it again.".into());
    }
    Ok(Some(path))
}

/// Resolves a single child name without allowing traversal or implicit destination replacement.
///
/// # Errors
/// Rejects unavailable destination folders, empty names and child-path traversal.
fn destination(folder: &str, name: &str, suffix: &str) -> Result<PathBuf, String> {
    let folder = directory(folder, false)?.ok_or("Choose a destination folder.")?;
    let name = name.trim();
    if name.is_empty() || matches!(name, "." | "..") || name.contains('/') || name.contains('\0') {
        return Err("Use a job name without folder separators.".into());
    }
    Ok(folder.join(format!("{name}{suffix}")))
}

/// Converts an inclusive requested interval once while preserving the source's original speed.
///
/// # Errors
/// Rejects out-of-range frames, empty intervals and exact-time overflow without editing the project.
fn interval(document: &Document, first: u64, last: u64) -> Result<Document, String> {
    let timing = document.project_timing();
    let range = FrameRange::new(
        first,
        last.checked_add(1).ok_or("Last frame is too large.")?,
    )
    .map_err(|error| error.to_string())?;
    timing
        .frame_range()
        .local_offset(first)
        .map_err(|error| error.to_string())?;
    timing
        .frame_range()
        .local_offset(last)
        .map_err(|error| error.to_string())?;
    let start = timing
        .source_time_for_frame(first)
        .map_err(|error| error.to_string())?;
    let mut end = timing
        .source_time_for_frame(last)
        .and_then(|last| last.checked_add(timing.frame_rate().time_for_frame(1)?))
        .map_err(|error| error.to_string())?;
    if let Some(source) = timing.source_time_range()
        && end.checked_cmp(source.end()).is_gt()
    {
        end = source.end();
    }
    let timing = ProjectTiming::new(timing.frame_rate(), range)
        .with_source_time_range(TimeRange::new(start, end).map_err(|error| error.to_string())?);
    document
        .clone()
        .with_temporal_authority(timing, document.temporal_end_overrides().to_vec())
        .map_err(|error| error.to_string())
}

/// Projects real sequence frame work into one phase-local progress value.
fn sequence_progress(value: ExportProgress) -> Progress {
    Progress {
        phase: match value.phase {
            ExportPhase::Preflight => "Preflight",
            ExportPhase::Rendering => "Rendering",
            ExportPhase::Finalizing => "Finalizing",
            ExportPhase::Complete => "Complete",
        },
        fraction: (value.completed_frames as f64 + value.frame_fraction)
            .min(value.total_frames as f64)
            / value.total_frames.max(1) as f64,
        detail: format!("{} / {} frames", value.completed_frames, value.total_frames),
    }
}

/// Keeps encoding separate from rendering and includes encoder-reported time.
fn video_progress(value: VideoProgress) -> Progress {
    if let Some(render) = value.render {
        return sequence_progress(render);
    }
    Progress {
        phase: match value.phase {
            VideoPhase::Preflight => "Preflight",
            VideoPhase::Rendering => "Rendering",
            VideoPhase::Encoding => "Encoding",
            VideoPhase::Validating => "Validating video",
            VideoPhase::SavingPngs => "Saving PNGs",
            VideoPhase::Complete => "Complete",
        },
        fraction: value.completed_frames.min(value.total_frames) as f64
            / value.total_frames.max(1) as f64,
        detail: format!(
            "{} / {} frames · {:.2} seconds encoded",
            value.completed_frames,
            value.total_frames,
            value.encoded_time_micros as f64 / 1_000_000.0
        ),
    }
}

/// Runs a validated immutable snapshot through the shared headless exporter.
fn render(
    snapshot: SavedContent,
    options: Options,
    cancelled: &AtomicBool,
    report: &(dyn Fn(Progress) + Sync),
) -> Outcome {
    let result = interval(&snapshot.document, options.first, options.last).and_then(|document| {
        if options.format == Format::Pngs {
            let job = SequenceExportJob::new(
                document,
                snapshot.sources,
                SequenceExportOptions {
                    destination: options.destination,
                    format: SequenceFormat::Png,
                    background: Some(options.background),
                    target: options.target,
                    antialiasing: options.antialiasing,
                    limits: EvaluationLimits::default(),
                },
            )
            .map_err(|error| error.to_string())?;
            return job
                .run(MediaTools::default(), cancelled, &|progress| {
                    report(sequence_progress(progress))
                })
                .map(|result| Outcome::Complete {
                    path: Some(result.directory),
                    message: format!("Exported {} PNG frames.", result.frame_count),
                })
                .map_err(|error| error.to_string());
        }
        let job = VideoExportJob::new(
            document,
            snapshot.sources,
            VideoExportOptions {
                destination: options.destination,
                codec: if options.format == Format::Lossless {
                    VideoCodec::Ffv1Matroska
                } else {
                    VideoCodec::Av1Webm
                },
                temporary_directory: options.temporary,
                background: Some(options.background),
                target: options.target,
                antialiasing: options.antialiasing,
                limits: EvaluationLimits::default(),
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(
            match job.run(MediaTools::default(), cancelled, &|progress| {
                report(video_progress(progress))
            }) {
                Ok(result) => Outcome::Complete {
                    path: Some(result.file),
                    message: result.cleanup_warning.unwrap_or_else(|| {
                        format!("Exported {} video frames.", result.frame_count)
                    }),
                },
                Err(error) => Outcome::Failed {
                    error: error.to_string(),
                    recovery: error.recovery,
                },
            },
        )
    });
    result.unwrap_or_else(|error| Outcome::Failed {
        error,
        recovery: None,
    })
}

/// Names explicit recovery decisions; none silently destroys retained frames.
#[derive(Clone, Copy)]
enum Action {
    Render,
    Retry,
    SavePng,
    Discard,
}

/// Prepares one operation without moving recovery ownership until input validates.
/// Invalid input reports that the attempted operation has not started and clears
/// any preceding operation's completed progress; retained frames remain owned.
fn submit(state: &Rc<RefCell<AppState>>, epoch: u64, action: Action) {
    let prepared = {
        let mut app = state.borrow_mut();
        let Some(surface) = app
            .temporal_export
            .as_mut()
            .filter(|surface| surface.epoch == epoch && surface.worker.is_none())
        else {
            return;
        };
        if matches!(action, Action::Render) && surface.recovery.is_some() {
            return;
        }
        let options = (|| {
            if matches!(action, Action::Discard) {
                return Ok(Options {
                    format: Format::Pngs,
                    destination: PathBuf::new(),
                    temporary: None,
                    background: RasterBackground::Transparent,
                    target: None,
                    antialiasing: RasterAntialiasing::On,
                    first: 0,
                    last: 0,
                });
            }
            let format = Format::selected(surface.format.selected())?;
            Ok::<_, String>(Options {
                format,
                destination: destination(
                    surface.directory.text().as_str(),
                    surface.name.text().as_str(),
                    if matches!(action, Action::SavePng) {
                        ""
                    } else {
                        format.suffix()
                    },
                )?,
                temporary: directory(surface.temporary.text().as_str(), true)?,
                background: png_background_for_dropdown_position(surface.background.selected()),
                target: parse_output_target(surface.dimensions.text().as_str())?,
                antialiasing: if surface.antialiasing.selected() == 1 {
                    RasterAntialiasing::Off
                } else {
                    RasterAntialiasing::On
                },
                first: surface
                    .first
                    .text()
                    .parse()
                    .map_err(|_| "First frame must be an unsigned integer.")?,
                last: surface
                    .last
                    .text()
                    .parse()
                    .map_err(|_| "Last frame must be an unsigned integer.")?,
            })
        })();
        match options {
            Ok(options) => Some((surface.snapshot.clone(), options, surface.recovery.take())),
            Err(error) => {
                surface.status.set_label(&error);
                surface.progress.set_fraction(0.0);
                surface.progress.set_text(Some("Not started"));
                None
            }
        }
    };
    let Some((snapshot, options, recovery)) = prepared else {
        return;
    };
    let recovery = Arc::new(std::sync::Mutex::new(recovery));
    let worker_recovery = recovery.clone();
    let started = launch(state, epoch, move |cancelled, report| {
        if matches!(action, Action::Render) {
            return render(snapshot, options, cancelled, report);
        }
        let Some(mut recovery) = worker_recovery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            return Outcome::Failed {
                error: "No rendered PNGs are available for recovery.".into(),
                recovery: None,
            };
        };
        let result = match action {
            Action::Retry => recovery
                .retry(
                    MediaTools::default(),
                    &options.destination,
                    cancelled,
                    &|value| report(video_progress(value)),
                )
                .map(|result| {
                    (
                        Some(result.file),
                        result
                            .cleanup_warning
                            .unwrap_or_else(|| "Video encoding complete.".into()),
                    )
                }),
            Action::SavePng => recovery
                .save_png_sequence(&options.destination, cancelled, &|value| {
                    report(video_progress(value))
                })
                .map(|path| (Some(path), "Saved rendered PNG sequence.".into())),
            Action::Discard => recovery
                .discard()
                .map(|()| (None, "Discarded this job’s rendered PNGs.".into())),
            Action::Render => unreachable!(),
        };
        match result {
            Ok((path, message)) => Outcome::Complete { path, message },
            Err(error) => Outcome::Failed {
                error: error.to_string(),
                recovery: Some(recovery),
            },
        }
    });
    if !started
        && let Some(surface) = state
            .borrow_mut()
            .temporal_export
            .as_mut()
            .filter(|surface| surface.epoch == epoch)
    {
        surface.recovery = recovery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        refresh(surface);
    }
}

/// Spawns one cancellable worker and reports its terminal ownership even after a Rust panic.
fn launch(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    operation: impl FnOnce(&AtomicBool, &(dyn Fn(Progress) + Sync)) -> Outcome + Send + 'static,
) -> bool {
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let sender = state.borrow().event_sender.clone();
    let result = thread::Builder::new()
        .name("toniator-export".into())
        .spawn(move || {
            let report = |progress| {
                let _ = sender
                    .send_blocking(AppEvent::TemporalExport(Event::Progress(epoch, progress)));
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                operation(&flag, &report)
            }))
            .unwrap_or_else(|_| Outcome::Failed {
                error: "Export worker stopped unexpectedly.".into(),
                recovery: None,
            });
            let _ = sender.send_blocking(AppEvent::TemporalExport(Event::Finished(epoch, result)));
        });
    let started = result.is_ok();
    let mut app = state.borrow_mut();
    if let Some(surface) = app
        .temporal_export
        .as_mut()
        .filter(|surface| surface.epoch == epoch)
    {
        match result {
            Ok(handle) => {
                surface.worker = Some(Worker {
                    cancelled,
                    handle: Some(handle),
                })
            }
            Err(error) => surface
                .status
                .set_label(&format!("Couldn’t start export: {error}")),
        }
        surface.progress.set_fraction(0.0);
        surface.progress.set_text(Some("Preparing"));
        refresh(surface);
        app.pending_export = app
            .temporal_export
            .as_ref()
            .is_some_and(|surface| surface.worker.is_some());
    }
    sync_ui(&mut app);
    started
}

/// Projects worker/recovery ownership into truthful enabled actions.
fn refresh(surface: &Surface) {
    let running = surface.worker.is_some();
    let recovery = surface.recovery.is_some();
    surface.configuration.set_sensitive(!running && !recovery);
    surface.destination_controls.set_sensitive(!running);
    for control in &surface.configuration_widgets {
        control.set_sensitive(!running && !recovery);
    }
    for control in &surface.destination_widgets {
        control.set_sensitive(!running);
    }
    surface.export.set_sensitive(!running && !recovery);
    surface.cancel.set_visible(running);
    surface.cancel.set_sensitive(
        surface
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.cancelled.load(Ordering::Acquire)),
    );
    for button in [&surface.retry, &surface.save_png, &surface.discard] {
        button.set_visible(recovery);
        button.set_sensitive(!running);
    }
    surface.close.set_sensitive(!recovery);
    surface.still.set_sensitive(!running && !recovery);
}

/// Accepts only the current sheet's progress and terminal worker result, then releases the worker.
pub(super) fn event(state: &Rc<RefCell<AppState>>, event: Event) {
    let mut app = state.borrow_mut();
    let epoch = match &event {
        Event::Progress(epoch, _) | Event::Finished(epoch, _) => *epoch,
    };
    let Some(surface) = app
        .temporal_export
        .as_mut()
        .filter(|surface| surface.epoch == epoch)
    else {
        return;
    };
    if let Event::Progress(_, progress) = event {
        surface
            .progress
            .set_fraction(progress.fraction.clamp(0.0, 1.0));
        surface.progress.set_text(Some(&format!(
            "{} · {:.1}%",
            progress.phase,
            progress.fraction * 100.0
        )));
        surface
            .status
            .set_label(&format!("{} — {}", progress.phase, progress.detail));
        return;
    }
    let Event::Finished(_, outcome) = event else {
        return;
    };
    surface.worker.take();
    match outcome {
        Outcome::Metadata(result) => {
            surface.progress.set_text(Some("Ready"));
            match result {
                Ok(text) => {
                    surface.audio.set_label(&text);
                    surface
                        .status
                        .set_label("Choose the output and destination, then export.");
                }
                Err(error) => {
                    surface
                        .audio
                        .set_label("Audio information unavailable. Output remains silent.");
                    surface.status.set_label(&error);
                }
            }
        }
        Outcome::Complete { path, message } => {
            surface.status.set_label(&match path {
                Some(path) => format!("{message}\n{}", path.display()),
                None => message,
            });
            surface.progress.set_fraction(1.0);
            surface.progress.set_text(Some("Complete"));
        }
        Outcome::Failed { error, recovery } => {
            surface.status.set_label(&format!(
                "{error}{}",
                if recovery.is_some() {
                    "\nChoose Retry encoding, Save PNG sequence, or Discard rendered PNGs."
                } else {
                    ""
                }
            ));
            surface.recovery = recovery;
            surface.progress.set_text(Some("Stopped"));
        }
    }
    refresh(surface);
    let close = surface.close_when_done && surface.recovery.is_none();
    app.pending_export = false;
    sync_ui(&mut app);
    drop(app);
    if close {
        dismiss(state);
    }
}

/// Requests cancellation without blocking GTK while the shared jobs reap their subprocesses.
fn cancel_work(state: &Rc<RefCell<AppState>>, epoch: u64) {
    if let Some(surface) = state
        .borrow()
        .temporal_export
        .as_ref()
        .filter(|surface| surface.epoch == epoch)
        && let Some(worker) = &surface.worker
    {
        worker.cancelled.store(true, Ordering::Release);
        surface.cancel.set_sensitive(false);
        surface
            .status
            .set_label("Cancelling export and finishing cleanup…");
    }
}

/// Defers sheet/application closing until cancellation is reaped; retained PNGs require a decision.
pub(super) fn request_close(state: &Rc<RefCell<AppState>>, quit: bool) -> bool {
    let mut app = state.borrow_mut();
    let Some(surface) = app.temporal_export.as_mut() else {
        return false;
    };
    surface.quit_when_done |= quit;
    surface.close_when_done = true;
    if let Some(chooser) = &surface.chooser {
        chooser.cancel();
        return true;
    }
    if surface.recovery.is_some() {
        surface.status.set_label("Rendered PNGs are retained. Choose Retry encoding, Save PNG sequence, or Discard rendered PNGs before closing.");
        surface.window.present();
        return true;
    }
    if surface.worker.is_some() {
        surface.close_when_done = true;
        let epoch = surface.epoch;
        drop(app);
        cancel_work(state, epoch);
        return true;
    }
    drop(app);
    dismiss(state);
    true
}

/// Releases a terminal sheet before destroying GTK and resumes a deferred main-window close.
fn dismiss(state: &Rc<RefCell<AppState>>) {
    let Some(surface) = state.borrow_mut().temporal_export.take() else {
        return;
    };
    let quit = surface.quit_when_done;
    let message = surface.status.text().to_string();
    surface.window.destroy();
    drop(surface);
    let mut app = state.borrow_mut();
    app.pending_export = false;
    set_inspector_status(&mut app, message);
    sync_ui(&mut app);
    let window = app.window.clone();
    drop(app);
    if quit {
        glib::idle_add_local_once(move || window.close());
    }
}

/// Keeps portal folder selection scoped to its live sheet and clears chooser ownership on return.
fn browse_directory(state: &Rc<RefCell<AppState>>, epoch: u64) {
    let parent = {
        let app = state.borrow();
        let Some(surface) = app
            .temporal_export
            .as_ref()
            .filter(|surface| surface.epoch == epoch && surface.worker.is_none())
        else {
            return;
        };
        surface.window.clone()
    };
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Choose export folder");
    let cancellable = gio::Cancellable::new();
    {
        let mut app = state.borrow_mut();
        if app.pending_file_chooser {
            return;
        }
        app.pending_file_chooser = true;
        if let Some(surface) = app.temporal_export.as_mut() {
            surface.chooser = Some(cancellable.clone());
        }
        sync_ui(&mut app);
    }
    let app = state.clone();
    dialog.select_folder(Some(&parent), Some(&cancellable), move |result| {
        let mut state = app.borrow_mut();
        state.pending_file_chooser = false;
        let mut close = false;
        if let Some(surface) = state
            .temporal_export
            .as_mut()
            .filter(|surface| surface.epoch == epoch)
        {
            surface.chooser = None;
            close = surface.close_when_done && surface.recovery.is_none();
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                surface.directory.set_text(&path.to_string_lossy());
            }
        }
        sync_ui(&mut state);
        drop(state);
        if close {
            dismiss(&app);
        }
    });
}

/// Saves or clears only personal folder defaults after explicit user action in Export defaults.
fn save_personal_defaults(state: &Rc<RefCell<AppState>>, epoch: u64, clear: bool) {
    let app = state.borrow();
    let Some(surface) = app.temporal_export.as_ref().filter(|surface| {
        surface.epoch == epoch && surface.worker.is_none() && surface.recovery.is_none()
    }) else {
        return;
    };
    let defaults = if clear {
        Ok(ExportDefaults::default())
    } else {
        directory(surface.directory.text().as_str(), true).and_then(|destination| {
            directory(surface.temporary.text().as_str(), true).map(|temporary_directory| {
                ExportDefaults {
                    destination,
                    temporary_directory,
                }
            })
        })
    };
    let result = defaults.and_then(|defaults| {
        defaults
            .save(&defaults_path())
            .map_err(|error| error.to_string())
    });
    surface.status.set_label(&match result {
        Ok(()) => {
            if clear {
                "Exports will prompt for folders next time.".into()
            } else {
                "Saved personal export folder defaults.".into()
            }
        }
        Err(error) => error,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::{Easing, FrameRate, RationalTime, TemporalEndpointEdit};

    /// Builds a current timed document with a nonzero source trim and distinct endpoints.
    ///
    /// # Panics
    /// Panics if the bounded test fixture violates domain validation.
    fn timed_document() -> Document {
        let base =
            Document::new_default_document(DEFAULT_CANVAS, SourceReference::Unassigned).unwrap();
        let timing = ProjectTiming::new(
            FrameRate::new(30_000, 1_001).unwrap(),
            FrameRange::new(0, 10).unwrap(),
        )
        .with_source_time_range(
            TimeRange::new(
                RationalTime::new(3, 1).unwrap(),
                RationalTime::new(10_001, 3_000).unwrap(),
            )
            .unwrap(),
        );
        let base = base.with_temporal_authority(timing, vec![]).unwrap();
        let command = base
            .edit_effective_end_command(&[TemporalEndpointEdit {
                target: PropertyTarget::Document,
                field: PropertyFieldId::RotationDegrees,
                effective_end: 90.0,
                easing: Easing::Linear,
            }])
            .unwrap();
        base.clone()
            .with_temporal_authority(
                base.project_timing().clone(),
                command.replacement().end_overrides.clone(),
            )
            .unwrap()
    }

    /// Proves inclusive trimming preserves exact source speed while stretching the authored transition.
    ///
    /// # Panics
    /// Panics if source time, endpoint values, one-frame behavior or bounds diverge from domain authority.
    #[test]
    fn export_interval_preserves_source_time_and_endpoint_authority() {
        let source = timed_document();
        let trimmed = interval(&source, 2, 4).unwrap();
        for frame in 2..=4 {
            assert_eq!(
                trimmed
                    .project_timing()
                    .source_time_for_frame(frame)
                    .unwrap(),
                source
                    .project_timing()
                    .source_time_for_frame(frame)
                    .unwrap()
            );
        }
        assert_eq!(
            trimmed
                .materialize_frame(2)
                .unwrap()
                .pattern_settings()
                .pattern_rotation_degrees,
            0.0
        );
        assert_eq!(
            trimmed
                .materialize_frame(4)
                .unwrap()
                .pattern_settings()
                .pattern_rotation_degrees,
            90.0
        );
        assert_eq!(
            interval(&source, 3, 3)
                .unwrap()
                .materialize_frame(3)
                .unwrap()
                .pattern_settings()
                .pattern_rotation_degrees,
            0.0
        );
        assert!(interval(&source, 4, 2).is_err());
        assert!(interval(&source, 0, 10).is_err());
        assert_eq!(source.project_timing().frame_range().frame_count(), 10);
    }

    /// Proves rendering and encoding expose separate truthful percentages with fractional frame work.
    ///
    /// # Panics
    /// Panics if the projection changes phase names, fractional work or encoded-time reporting.
    #[test]
    fn rendering_and_encoding_progress_are_separate_phases() {
        let progress = sequence_progress(ExportProgress {
            phase: ExportPhase::Rendering,
            completed_frames: 4,
            total_frames: 10,
            frame: Some(4),
            frame_fraction: 0.5,
        });
        assert_eq!(progress.fraction, 0.45);
        assert_eq!(progress.phase, "Rendering");
        let progress = video_progress(VideoProgress {
            phase: VideoPhase::Encoding,
            completed_frames: 2,
            total_frames: 10,
            encoded_time_micros: 1_500_000,
            render: None,
        });
        assert_eq!(progress.fraction, 0.2);
        assert_eq!(progress.phase, "Encoding");
        assert!(progress.detail.contains("1.50 seconds"));
    }

    /// Extracts an actual published artifact and includes shared-job diagnostics in a test failure.
    ///
    /// # Panics
    /// Panics if the worker did not complete a file-producing operation.
    fn completed(outcome: Outcome) -> PathBuf {
        match outcome {
            Outcome::Complete {
                path: Some(path), ..
            } => path,
            Outcome::Failed { error, .. } => panic!("{error}"),
            _ => panic!("expected a published output"),
        }
    }

    /// Runs the exact desktop bridge against both immutable stills and the supplied moving source.
    ///
    /// # Panics
    /// Panics on timing, native PNG size, software video, cancellation or exclusive-output regressions.
    #[test]
    fn desktop_jobs_export_native_stills_and_video_without_changing_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/stage22-desktop-export")
            .join(format!(
                "run-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        for (input, name, size) in [
            ("raster-sample.png", "raster", (1024, 1024)),
            ("vector-sample.svg", "vector", (900, 620)),
        ] {
            let workspace = load_workspace(&root.join("assets").join(input)).unwrap();
            let before = workspace.snapshot();
            let options = Options {
                format: Format::Pngs,
                destination: directory.join(name),
                temporary: None,
                background: RasterBackground::Transparent,
                target: None,
                antialiasing: RasterAntialiasing::On,
                first: 1,
                last: 2,
            };
            let path = completed(render(
                before.clone(),
                options.clone(),
                &AtomicBool::new(false),
                &|_| {},
            ));
            for index in 0..2 {
                let bytes = fs::read(path.join(format!("frame-{index:06}.png"))).unwrap();
                assert_eq!(
                    u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                    size.0
                );
                assert_eq!(
                    u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
                    size.1
                );
            }
            assert_eq!(workspace.snapshot(), before);
            assert!(matches!(
                render(before, options, &AtomicBool::new(false), &|_| {}),
                Outcome::Failed { recovery: None, .. }
            ));
        }
        let workspace = load_workspace(&root.join("assets/video-sample0001-0010.mp4")).unwrap();
        let before = workspace.snapshot();
        let options = Options {
            format: Format::Lossless,
            destination: directory.join("video.mkv"),
            temporary: Some(directory.clone()),
            background: RasterBackground::Transparent,
            target: Some(OutputRasterTarget::new(64, 64).unwrap()),
            antialiasing: RasterAntialiasing::On,
            first: 2,
            last: 4,
        };
        completed(render(
            before.clone(),
            options.clone(),
            &AtomicBool::new(false),
            &|_| {},
        ));
        let reopened = load_workspace(&options.destination).unwrap();
        assert_eq!(
            reopened
                .document()
                .project_timing()
                .frame_range()
                .frame_count(),
            3
        );
        assert_eq!(workspace.snapshot(), before);
        let cancelled = Options {
            destination: directory.join("cancelled.mkv"),
            ..options
        };
        assert!(matches!(
            render(before, cancelled.clone(), &AtomicBool::new(true), &|_| {}),
            Outcome::Failed { recovery: None, .. }
        ));
        assert!(!cancelled.destination.exists());
        println!("Desktop export artifacts: {}", directory.display());
    }
}
