//! Ordered image-sequence authoring projects the shared bounded importer into one cancellable sheet.

use super::*;
use toniator_domain::FrameRate;

/// Owns the reviewed path order and GTK projection until shared import produces a complete workspace.
pub(super) struct Surface {
    window: gtk::Window,
    paths: Vec<PathBuf>,
    model: gtk::StringList,
    selection: gtk::SingleSelection,
    list: gtk::ListView,
    refreshing: Rc<Cell<bool>>,
    rate: gtk::Entry,
    status: gtk::Label,
    add: gtk::Button,
    remove: gtk::Button,
    earlier: gtk::Button,
    later: gtk::Button,
    import: gtk::Button,
    chooser: Option<gio::Cancellable>,
    cancelled: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for Surface {
    /// Cancels the native chooser and importer, joining the worker before releasing its input paths.
    fn drop(&mut self) {
        if let Some(chooser) = self.chooser.take() {
            chooser.cancel();
        }
        self.cancelled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Imports exactly the reviewed order and rate; no frontend sorting or implicit external links occur.
///
/// # Errors
/// Returns bounded importer, unsupported/mismatched image, cancellation and workspace errors.
fn import_workspace(
    paths: &[PathBuf],
    rate: FrameRate,
    cancelled: &dyn Fn() -> bool,
) -> Result<Workspace, String> {
    let imported = toniator_engine::import_source_media(
        paths,
        Some(rate),
        toniator_engine::MediaTools::default(),
        cancelled,
    )
    .map_err(|error| error.message().to_owned())?;
    Workspace::from_imported(imported, "Image sequence".into(), cancelled)
}

/// Rebuilds only the presentation list while suppressing synchronous selection callbacks.
fn rebuild(surface: &Surface, selected: Option<usize>) {
    let labels: Vec<_> = surface
        .paths
        .iter()
        .enumerate()
        .map(|(index, path)| format!("{} · {}", index + 1, path.display()))
        .collect();
    let labels: Vec<_> = labels.iter().map(String::as_str).collect();
    surface.refreshing.set(true);
    surface.model.splice(0, surface.model.n_items(), &labels);
    if let Some(index) = selected.filter(|index| *index < surface.paths.len()) {
        surface.selection.set_selected(index as u32);
    }
    surface.refreshing.set(false);
    refresh(surface);
}

/// Projects native sensitivity from the current selection, input validity and exclusive operation.
fn refresh(surface: &Surface) {
    let busy = surface.worker.is_some() || surface.chooser.is_some();
    let selected = surface.selection.selected() as usize;
    let valid_selection = selected < surface.paths.len();
    surface.add.set_sensitive(!busy);
    surface.list.set_sensitive(!busy);
    surface.rate.set_sensitive(!busy);
    surface.remove.set_sensitive(!busy && valid_selection);
    surface
        .earlier
        .set_sensitive(!busy && valid_selection && selected > 0);
    surface
        .later
        .set_sensitive(!busy && valid_selection && selected + 1 < surface.paths.len());
    let rate = toniator_engine::parse_media_rate(surface.rate.text().trim());
    surface
        .import
        .set_sensitive(!busy && !surface.paths.is_empty() && rate.is_ok());
    if !busy {
        surface.status.set_label(&match rate {
            Ok(_) => format!("{} images. The list order is the source frame order. Each image occupies one source frame.", surface.paths.len()),
            Err(error) => format!("Check frame rate: {error}"),
        });
    }
}

/// Opens the sequence sheet after the existing unsaved-document decision has been resolved.
pub(super) fn open(state: &Rc<RefCell<AppState>>) {
    if let Some(surface) = state.borrow().sequence_import.as_ref() {
        surface.window.present();
        return;
    }
    if lifecycle_is_busy(&state.borrow()) {
        return;
    }
    let window = gtk::Window::builder()
        .title("Import image sequence")
        .transient_for(&super::presentation_parent(&state.borrow()))
        .modal(true)
        .default_width(680)
        .default_height(540)
        .build();
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Import image sequence"))));
    window.set_titlebar(Some(&header));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let hint = gtk::Label::new(Some(
        "Add still images, then review their order. Use Move earlier or Move later to arrange frames. All images must have the same dimensions.",
    ));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    content.append(&hint);
    let model = gtk::StringList::new(&[]);
    let selection = gtk::SingleSelection::new(Some(model.clone()));
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        if let Some(item) = object.downcast_ref::<gtk::ListItem>() {
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            label.set_margin_start(8);
            label.set_margin_end(8);
            item.set_child(Some(&label));
        }
    });
    factory.connect_bind(|_, object| {
        if let Some(item) = object.downcast_ref::<gtk::ListItem>()
            && let Some(value) = item
                .item()
                .and_then(|item| item.downcast::<gtk::StringObject>().ok())
            && let Some(label) = item
                .child()
                .and_then(|child| child.downcast::<gtk::Label>().ok())
        {
            label.set_label(&value.string());
            label.set_tooltip_text(Some(&value.string()));
            item.set_accessible_label(&value.string());
        }
    });
    let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
    list.update_property(&[gtk::accessible::Property::Label("Image sequence")]);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_min_content_height(180);
    scroll.set_child(Some(&list));
    content.append(&scroll);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let add = gtk::Button::with_label("Add images…");
    let remove = gtk::Button::with_label("Remove");
    let earlier = gtk::Button::with_label("Move earlier");
    let later = gtk::Button::with_label("Move later");
    for button in [&add, &remove, &earlier, &later] {
        controls.append(button);
    }
    content.append(&controls);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some("Source frame rate (fps)"));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let rate = gtk::Entry::new();
    rate.set_text("30");
    rate.set_width_chars(16);
    rate.set_input_purpose(gtk::InputPurpose::Number);
    rate.set_tooltip_text(Some(
        "Enter an integer, decimal, or exact fraction such as 30000/1001.",
    ));
    label.set_mnemonic_widget(Some(&rate));
    relate_descriptor_label(rate.upcast_ref(), &label);
    row.append(&label);
    row.append(&rate);
    content.append(&row);
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    status.set_xalign(0.0);
    content.append(&status);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let import = gtk::Button::with_label("Import sequence");
    import.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&import);
    content.append(&buttons);
    window.set_child(Some(&content));
    let refreshing = Rc::new(Cell::new(false));
    state.borrow_mut().sequence_import = Some(Surface {
        window: window.clone(),
        paths: Vec::new(),
        model,
        selection: selection.clone(),
        list,
        refreshing: refreshing.clone(),
        rate: rate.clone(),
        status,
        add: add.clone(),
        remove: remove.clone(),
        earlier: earlier.clone(),
        later: later.clone(),
        import: import.clone(),
        chooser: None,
        cancelled: Arc::new(AtomicBool::new(false)),
        worker: None,
    });
    if let Some(surface) = state.borrow().sequence_import.as_ref() {
        refresh(surface);
    }
    sync_ui(&mut state.borrow_mut());
    let handle = state.clone();
    cancel.connect_clicked(move |_| close(&handle));
    let handle = state.clone();
    window.connect_close_request(move |_| {
        close(&handle);
        glib::Propagation::Proceed
    });
    let handle = state.clone();
    add.connect_clicked(move |_| choose_images(&handle));
    let handle = state.clone();
    remove.connect_clicked(move |_| edit_order(&handle, None));
    let handle = state.clone();
    earlier.connect_clicked(move |_| edit_order(&handle, Some(-1)));
    let handle = state.clone();
    later.connect_clicked(move |_| edit_order(&handle, Some(1)));
    let handle = state.clone();
    import.connect_clicked(move |_| start_import(&handle));
    let handle = state.clone();
    rate.connect_changed(move |_| {
        if let Some(surface) = handle.borrow().sequence_import.as_ref() {
            refresh(surface);
        }
    });
    let handle = state.clone();
    selection.connect_selected_notify(move |_| {
        if !refreshing.get()
            && let Some(surface) = handle.borrow().sequence_import.as_ref()
        {
            refresh(surface);
        }
    });
    window.present();
}

/// Applies one reviewed adjacent move or removal without changing source files or the main document.
fn edit_order(state: &Rc<RefCell<AppState>>, movement: Option<isize>) {
    let mut app = state.borrow_mut();
    let Some(surface) = app.sequence_import.as_mut() else {
        return;
    };
    if surface.worker.is_some() || surface.chooser.is_some() {
        return;
    }
    let index = surface.selection.selected() as usize;
    if index >= surface.paths.len() {
        return;
    }
    let selected = match movement {
        Some(step) => {
            let Some(destination) = index
                .checked_add_signed(step)
                .filter(|destination| *destination < surface.paths.len())
            else {
                return;
            };
            surface.paths.swap(index, destination);
            Some(destination)
        }
        None => {
            surface.paths.remove(index);
            index.checked_sub(usize::from(index == surface.paths.len()))
        }
    };
    rebuild(surface, selected);
}

/// Chooses local images through the native multi-file dialog and retains its cancellation ownership.
fn choose_images(state: &Rc<RefCell<AppState>>) {
    let (window, cancellable) = {
        let mut app = state.borrow_mut();
        let Some(surface) = app.sequence_import.as_mut() else {
            return;
        };
        if surface.worker.is_some() || surface.chooser.is_some() {
            return;
        }
        let cancellable = gio::Cancellable::new();
        surface.chooser = Some(cancellable.clone());
        surface.status.set_label("Choosing images…");
        refresh(surface);
        (surface.window.clone(), cancellable)
    };
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Still images"));
    for pattern in [
        "*.png", "*.svg", "*.jpg", "*.jpeg", "*.webp", "*.bmp", "*.tif", "*.tiff", "*.exr",
        "*.avif", "*.gif",
    ] {
        filter.add_pattern(pattern);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title("Add sequence images")
        .build();
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&filter));
    let handle = state.clone();
    let expected = window.downgrade();
    let token = cancellable.clone();
    dialog.open_multiple(Some(&window), Some(&cancellable), move |result| {
        let mut app = handle.borrow_mut();
        let Some(surface) = app.sequence_import.as_mut().filter(|surface| {
            Some(surface.window.clone()) == expected.upgrade()
                && surface.chooser.as_ref() == Some(&token)
        }) else {
            return;
        };
        surface.chooser = None;
        match result {
            Ok(files) => {
                let paths: Result<Vec<_>, _> = (0..files.n_items())
                    .map(|index| {
                        files
                            .item(index)
                            .and_then(|file| file.downcast::<gio::File>().ok())
                            .and_then(|file| file.path())
                            .ok_or("Choose local image files.")
                    })
                    .collect();
                match paths {
                    Ok(paths) if surface.paths.len().saturating_add(paths.len()) <= 1_000_000 => {
                        let first_new = surface.paths.len();
                        surface.paths.extend(paths);
                        rebuild(surface, Some(first_new));
                    }
                    Ok(_) => {
                        refresh(surface);
                        surface
                            .status
                            .set_label("The sequence exceeds one million frames.");
                    }
                    Err(error) => {
                        refresh(surface);
                        surface.status.set_label(error);
                    }
                }
            }
            Err(_) => refresh(surface),
        }
    });
}

/// Starts one immutable import attempt; a stale or cancelled result cannot replace the current workspace.
fn start_import(state: &Rc<RefCell<AppState>>) {
    let (paths, rate, flag, expected) = {
        let app = state.borrow();
        let Some(surface) = app.sequence_import.as_ref() else {
            return;
        };
        if surface.worker.is_some() || surface.chooser.is_some() || surface.paths.is_empty() {
            return;
        }
        let Ok(rate) = toniator_engine::parse_media_rate(surface.rate.text().trim()) else {
            return;
        };
        (
            surface.paths.clone(),
            rate,
            surface.cancelled.clone(),
            surface.window.downgrade(),
        )
    };
    let (sender, receiver) = async_channel::bounded(1);
    let worker = thread::Builder::new()
        .name("toniator-sequence-import".into())
        .spawn(move || {
            let result = import_workspace(&paths, rate, &|| flag.load(Ordering::Acquire));
            let _ = sender.send_blocking(result);
        });
    {
        let mut app = state.borrow_mut();
        let Some(surface) = app.sequence_import.as_mut() else {
            return;
        };
        match worker {
            Ok(worker) => {
                surface.worker = Some(worker);
                surface.status.set_label("Importing sequence…");
                refresh(surface);
            }
            Err(error) => {
                surface
                    .status
                    .set_label(&format!("Couldn’t start import: {error}"));
                return;
            }
        }
    }
    let handle = state.clone();
    glib::MainContext::default().spawn_local(async move {
        let Ok(result) = receiver.recv().await else {
            return;
        };
        {
            let mut app = handle.borrow_mut();
            let Some(surface) = app
                .sequence_import
                .as_mut()
                .filter(|surface| Some(surface.window.clone()) == expected.upgrade())
            else {
                return;
            };
            if let Some(worker) = surface.worker.take() {
                let _ = worker.join();
            }
            if let Err(error) = &result {
                refresh(surface);
                surface
                    .status
                    .set_label(&format!("Couldn’t import sequence: {error}"));
                return;
            }
        }
        if let Ok(workspace) = result {
            close(&handle);
            install_workspace(&handle, workspace);
        }
    });
}

/// Cancels chooser/import work and discards the sequence projection while retaining the existing workspace.
pub(super) fn close(state: &Rc<RefCell<AppState>>) {
    let surface = state.borrow_mut().sequence_import.take();
    if let Some(surface) = surface {
        let window = surface.window.clone();
        drop(surface);
        window.close();
        sync_ui(&mut state.borrow_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::SourceReferenceId;
    use toniator_engine::FrameSource;
    use toniator_io::SourceMediaManifest;

    /// Checks reviewed order, repeated images, exact rate, portable persistence and native shared exports.
    ///
    /// # Panics
    /// Panics if the desktop import route reorders inputs, loses repeated frames or accepts invalid media.
    #[test]
    fn sequence_import_preserves_order_sources_and_exact_timing() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root
            .join("target/validation/stage22-sequence-import")
            .join(format!(
                "run-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&directory).unwrap();
        let rate = FrameRate::new(30000, 1001).unwrap();
        for (input, name, format) in [
            ("raster-sample.png", "raster", ExportFormat::Png),
            ("vector-sample.svg", "vector", ExportFormat::Svg),
        ] {
            let source = root.join("assets").join(input);
            let alternate = directory.join(format!("alternate-{input}"));
            if matches!(format, ExportFormat::Svg) {
                fs::write(&alternate, r##"<svg xmlns="http://www.w3.org/2000/svg" width="900" height="620"><rect width="900" height="620" fill="#20a050"/><circle cx="450" cy="310" r="180" fill="#4020e0" fill-opacity="0.5"/></svg>"##).unwrap();
            } else {
                fs::copy(&source, &alternate).unwrap();
            }
            let paths = [alternate.clone(), source.clone(), alternate.clone()];
            let workspace = import_workspace(&paths, rate, &|| false).unwrap();
            assert!(workspace.is_dirty());
            assert!(workspace.location.is_none());
            assert_eq!(workspace.document().project_timing().frame_rate(), rate);
            assert_eq!(
                workspace
                    .document()
                    .project_timing()
                    .frame_range()
                    .frame_count(),
                3
            );
            assert_eq!(
                workspace.sources.media(),
                Some(&SourceMediaManifest::ImageSequence {
                    source_ids: vec![
                        SourceReferenceId::new("source-1").unwrap(),
                        SourceReferenceId::new("source-2").unwrap(),
                        SourceReferenceId::new("source-1").unwrap()
                    ],
                    frame_rate: rate,
                })
            );
            let snapshot = workspace.snapshot();
            let path = directory.join(format!("{name}-sequence.toniator"));
            save_container(&path, &snapshot.document, &snapshot.sources).unwrap();
            assert_eq!(load_workspace(&path).unwrap().snapshot(), snapshot);
            let mut media = toniator_engine::open_source_media(
                &snapshot.sources,
                toniator_engine::MediaTools::default(),
                &|| false,
            )
            .unwrap();
            let first = media
                .frame_at(rate.time_for_frame(0).unwrap(), &|| false)
                .unwrap();
            let last = media
                .frame_at(rate.time_for_frame(2).unwrap(), &|| false)
                .unwrap();
            assert_eq!(first.field.identity(), last.field.identity());
            export_snapshot_frame(
                snapshot,
                directory.join(format!(
                    "{name}-middle.{}",
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
                1,
            )
            .unwrap();
            assert!(import_workspace(&paths, rate, &|| true).is_err());
        }
        assert!(import_workspace(&[], rate, &|| false).is_err());
        assert!(
            import_workspace(
                &[
                    root.join("assets/raster-sample.png"),
                    root.join("assets/vector-sample.svg")
                ],
                rate,
                &|| false
            )
            .is_err()
        );
        assert!(
            import_workspace(
                &[root.join("assets/video-sample0001-0010.mp4")],
                rate,
                &|| false
            )
            .is_err()
        );
        println!("Sequence import native witnesses: {}", directory.display());
    }
}
