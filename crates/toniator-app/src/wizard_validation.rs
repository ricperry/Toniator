//! Cancellable full-document construction admission, separate from the neutral wizard picture.

use super::*;

/// Owns the latest immutable draft validation and its cancellable worker.
#[derive(Default)]
pub(super) struct Validation {
    revision: Option<u64>,
    document: Option<Document>,
    generation: u64,
    result: Option<Result<(), String>>,
    worker: Option<Worker>,
    fix: Option<gtk::Button>,
}

/// Reaps one private evaluator before its wizard is discarded or its draft is superseded.
struct Worker {
    cancelled: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Worker {
    /// Cancels geometry/media work and joins the worker; no child or publication survives the modal.
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Validation {
    /// Binds the current Review correction control to the latest validation result.
    pub(super) fn attach_fix(&mut self, button: &gtk::Button) {
        button.set_sensitive(matches!(self.result, Some(Err(_))));
        button.set_visible(matches!(self.result, Some(Err(_))));
        if matches!(self.result, Some(Err(_))) {
            let button = button.clone();
            glib::idle_add_local_once(move || {
                if button.root().is_some() && button.is_mapped() {
                    button.grab_focus();
                }
            });
        }
        self.fix = Some(button.clone());
    }

    /// Admits only success for the exact current revision, never a previous picture or stale result.
    pub(super) fn ready(&self, document: &Document, revision: u64) -> bool {
        self.revision == Some(revision)
            && self.document.as_ref() == Some(document)
            && matches!(self.result, Some(Ok(())))
    }

    /// Combines exact construction validation with the same publication rules used by Apply.
    ///
    /// # Errors
    /// Returns the current construction, unchanged-draft, pending-choice or recipe diagnostic.
    pub(super) fn admission(
        &self,
        document: &Document,
        revision: u64,
        initial_document: &Document,
        target: InspectorTarget,
        current: WizardRoutePage,
        has_pending_proposal: bool,
    ) -> Result<(), String> {
        if !self.ready(document, revision) {
            return Err(self.message(revision));
        }
        wizard_apply_admission(
            document,
            initial_document,
            target,
            current,
            has_pending_proposal,
        )
    }

    /// Describes current construction readiness without exposing a fabricated missing setting.
    pub(super) fn message(&self, revision: u64) -> String {
        if self.revision != Some(revision) {
            return "Checking pattern construction…".into();
        }
        match &self.result {
            Some(Ok(())) => "Pattern construction checked.".into(),
            Some(Err(error)) => diagnostic(error),
            None => "Checking pattern construction at document size…".into(),
        }
    }

    /// Cancels a check when editing invalidates its captured authority.
    pub(super) fn invalidate(&mut self, document: &Document, revision: u64) {
        if self.revision != Some(revision) || self.document.as_ref() != Some(document) {
            self.worker = None;
            self.revision = None;
            self.document = None;
            self.result = None;
        }
    }
}

/// Explains a construction failure in existing artist-facing vocabulary, retaining technical detail.
fn diagnostic(error: &str) -> String {
    if error.contains("curve.parametric.segment_limit")
        || error.contains("curve.parametric.residual_limit")
    {
        "This spiral is too detailed to construct. Reduce Revolutions, then check the pattern again. Apply is unavailable until it constructs successfully.".into()
    } else {
        format!(
            "These settings could not construct a pattern: {error}. Correct the settings before applying."
        )
    }
}

/// Validates native geometry and source response at both authored endpoints with a bounded raster.
/// The engine retains native canvas/density; only the discarded raster is reduced. No files publish.
///
/// # Errors
/// Returns canonical media, construction, realization or cancellation errors without mutating input.
fn validate(
    document: Document,
    sources: SourceBundle,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let mut media = toniator_engine::open_source_media(
        &sources,
        toniator_engine::MediaTools::default(),
        &|| cancelled.load(Ordering::Acquire),
    )
    .map_err(|error| error.to_string())?;
    let session = DocumentSession::new(document).map_err(|error| error.to_string())?;
    let mut frames = vec![temporal_preview::Endpoint::Start.frame(session.document())];
    let end = temporal_preview::Endpoint::End.frame(session.document());
    if frames[0] != end {
        frames.push(end);
    }
    for frame in frames {
        let request =
            toniator_engine::frame_evaluation_request(&session, &mut media, frame, &|| {
                cancelled.load(Ordering::Acquire)
            })
            .map_err(|error| error.to_string())?
            .for_preview(
                toniator_engine::PreviewRasterTarget::new(128, 128)
                    .expect("fixed validation raster"),
            );
        toniator_engine::evaluate_cancellable_with_limits(
            request,
            toniator_engine::EvaluationLimits::default(),
            cancelled,
        )
        .map_err(|error| match error {
            toniator_engine::EvaluationRunError::Evaluation(error) => error.to_string(),
            toniator_engine::EvaluationRunError::Cancelled => "Pattern check cancelled.".into(),
        })?;
    }
    Ok(())
}

/// Starts a latest-revision check when Review is entered, reusing only an exact-revision result.
pub(super) fn request(state: &Rc<RefCell<AppState>>, epoch: u64) {
    let mut app = state.borrow_mut();
    let Some(sources) = app
        .workspace
        .as_ref()
        .map(|workspace| workspace.sources.clone())
    else {
        return;
    };
    let sender = app.event_sender.clone();
    let Some(surface) = app
        .pattern_wizard
        .as_mut()
        .filter(|surface| surface.epoch == epoch)
    else {
        return;
    };
    let revision = surface.draft.borrow().revision().0;
    let document = surface.draft.borrow().document().clone();
    if surface.validation.revision == Some(revision)
        && surface.validation.document.as_ref() == Some(&document)
    {
        return;
    }
    surface.validation.invalidate(&document, revision);
    surface.validation.revision = Some(revision);
    surface.validation.document = Some(document.clone());
    surface.validation.generation += 1;
    let generation = surface.validation.generation;
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    match thread::Builder::new()
        .name("toniator-wizard-validation".into())
        .spawn(move || {
            let result = validate(document, sources, &flag);
            if !flag.load(Ordering::Acquire) {
                let _ = sender.send_blocking(AppEvent::WizardValidation {
                    epoch,
                    revision,
                    generation,
                    result,
                });
            }
        }) {
        Ok(thread) => {
            surface.validation.worker = Some(Worker {
                cancelled,
                thread: Some(thread),
            })
        }
        Err(error) => surface.validation.result = Some(Err(error.to_string())),
    }
    surface.apply.set_sensitive(false);
    surface
        .status
        .set_label(&surface.validation.message(revision));
}

/// Accepts only the live modal/revision and refreshes admission without publishing document history.
pub(super) fn complete(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    revision: u64,
    generation: u64,
    result: Result<(), String>,
) {
    let mut app = state.borrow_mut();
    let Some(surface) = app.pattern_wizard.as_mut().filter(|surface| {
        surface.epoch == epoch
            && surface.draft.borrow().revision().0 == revision
            && surface.validation.revision == Some(revision)
            && surface.validation.generation == generation
            && surface.validation.document.as_ref() == Some(surface.draft.borrow().document())
    }) else {
        return;
    };
    surface.validation.worker = None;
    surface.validation.result = Some(result);
    if let Some(button) = &surface.validation.fix {
        button.set_sensitive(matches!(surface.validation.result, Some(Err(_))));
        button.set_visible(matches!(surface.validation.result, Some(Err(_))));
        if matches!(surface.validation.result, Some(Err(_)))
            && surface.route.get(surface.route_index) == Some(&WizardRoutePage::Review)
        {
            let button = button.clone();
            glib::idle_add_local_once(move || {
                if button.root().is_some() && button.is_mapped() {
                    button.grab_focus();
                }
            });
        }
    }
    refresh_wizard_action_controls(surface);
}

/// Moves a failed construction back to its existing settings control instead of inventing a limit UI.
pub(super) fn focus_error(state: &Rc<RefCell<AppState>>, epoch: u64) {
    let message = {
        let mut app = state.borrow_mut();
        let Some(surface) = app
            .pattern_wizard
            .as_mut()
            .filter(|surface| surface.epoch == epoch)
        else {
            return;
        };
        let document = surface.draft.borrow().document().clone();
        let Ok((projection, _, _)) = wizard_route_for_document(&document, surface.target) else {
            return;
        };
        let error = surface
            .validation
            .result
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .cloned()
            .unwrap_or_default();
        let field = if error.contains("curve.parametric") {
            PropertyFieldId::ParametricTurns
        } else if error.contains("random") {
            PropertyFieldId::RandomEvenMinimumCenterDistance
        } else {
            PropertyFieldId::Density
        };
        if let Some(value) = wizard_active_values(&document, surface.target, &projection)
            .into_iter()
            .find(|value| value.descriptor.field == field)
        {
            let page = fixed_wizard_page_for_descriptor(&value.descriptor, &projection.family);
            if let Some(index) = surface
                .route
                .iter()
                .position(|candidate| *candidate == page)
            {
                surface.route_index = index;
                surface.recipe_control_focus = Some(WizardRecipeControlFocus::Descriptor(
                    value.descriptor.target,
                    field,
                ));
            }
        } else if let Some(index) = surface
            .route
            .iter()
            .position(|page| *page == WizardRoutePage::FamilySettings)
        {
            surface.route_index = index;
        }
        surface
            .validation
            .message(surface.draft.borrow().revision().0)
    };
    show_wizard_current_route_page(state, epoch);
    if let Some(surface) = state.borrow().pattern_wizard.as_ref() {
        surface.status.set_label(&message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps successful construction distinct from unchanged, incomplete and publishable drafts.
    ///
    /// # Panics
    /// Panics if successful rendering hides a publication blocker or rejects a changed valid draft.
    #[test]
    fn successful_construction_reports_actual_publication_readiness() {
        let mut history = DocumentHistory::new(
            DocumentSession::new(
                Document::new_default_document(
                    CanvasSpec {
                        width: 100.0,
                        height: 100.0,
                    },
                    SourceReference::Unassigned,
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let initial = history.document().clone();
        let mut validation = Validation {
            revision: Some(history.revision().0),
            document: Some(initial.clone()),
            result: Some(Ok(())),
            ..Validation::default()
        };
        assert!(
            validation
                .admission(
                    &initial,
                    history.revision().0,
                    &initial,
                    InspectorTarget::DocumentAll,
                    WizardRoutePage::Review,
                    false
                )
                .unwrap_err()
                .contains("no pattern settings changed")
        );
        assert!(!validation.message(history.revision().0).contains("Ready"));
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, "even-random-circles")
            .unwrap();
        validation.revision = Some(history.revision().0);
        validation.document = Some(history.document().clone());
        assert!(
            validation
                .admission(
                    history.document(),
                    history.revision().0,
                    &initial,
                    InspectorTarget::DocumentAll,
                    WizardRoutePage::Review,
                    true
                )
                .unwrap_err()
                .contains("pending choice")
        );
        assert!(
            validation
                .admission(
                    history.document(),
                    history.revision().0,
                    &initial,
                    InspectorTarget::DocumentAll,
                    WizardRoutePage::Review,
                    false
                )
                .is_ok()
        );
    }

    /// Rejects pending/failed/stale checks and admits only the exact successful draft revision.
    #[test]
    fn construction_admission_requires_current_success() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let mut validation = Validation {
            revision: Some(7),
            document: Some(document.clone()),
            ..Validation::default()
        };
        assert!(!validation.ready(&document, 7));
        validation.result = Some(Err("curve.parametric.segment_limit".into()));
        assert!(!validation.ready(&document, 7));
        assert!(validation.message(7).contains("Revolutions"));
        validation.result = Some(Ok(()));
        assert!(validation.ready(&document, 7));
        assert!(!validation.ready(&document, 8));
        let other = Document::new_default_document(
            CanvasSpec {
                width: 200.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        assert!(!validation.ready(&other, 7));
        validation.invalidate(&document, 8);
        assert!(!validation.ready(&document, 7));
    }

    /// Constructs bundled Spiral recipes on both immutable native canvases without changing their sources.
    ///
    /// # Panics
    /// Panics if a bundled recipe, native-scale construction or source input is invalid.
    #[test]
    fn bundled_spirals_construct_at_native_canvas_sizes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for input in ["raster-sample.png", "vector-sample.svg"] {
            for id in [
                "round-spiral-line",
                "round-spiral-marks",
                "square-spiral-marks",
            ] {
                let mut workspace = load_workspace(&root.join("assets").join(input)).unwrap();
                PresetRegistry::bundled()
                    .apply_to_document_base(&mut workspace.history, id)
                    .unwrap();
                validate(
                    workspace.document().clone(),
                    workspace.sources.clone(),
                    &AtomicBool::new(false),
                )
                .unwrap_or_else(|error| panic!("{input} / {id}: {error}"));
            }
        }
    }

    /// Rejects a schema-valid but over-detailed Spiral before it can enter main document history.
    ///
    /// # Panics
    /// Panics if the geometry failure is missed, misreported as a missing field, or mutates input.
    #[test]
    fn valid_spiral_parameters_can_fail_construction_before_apply() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut workspace = load_workspace(&root.join("assets/raster-sample.png")).unwrap();
        PresetRegistry::bundled()
            .apply_to_document_base(&mut workspace.history, "round-spiral-line")
            .unwrap();
        let descriptor = workspace
            .document()
            .property_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.field == PropertyFieldId::ParametricTurns)
            .unwrap();
        let command = command_for_inspector_input(
            workspace.document(),
            None,
            DefinitionEditScope::DocumentBase,
            &descriptor,
            InspectorInput::FiniteF64(1024.0),
        )
        .unwrap();
        workspace.history.apply(&command).unwrap();
        let before = workspace.snapshot();
        let error = validate(
            before.document.clone(),
            before.sources.clone(),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(error.contains("curve.parametric.segment_limit"), "{error}");
        assert!(diagnostic(&error).contains("Reduce Revolutions"));
        assert_eq!(workspace.snapshot(), before);
        assert!(validate(before.document, before.sources, &AtomicBool::new(true)).is_err());
    }
}
