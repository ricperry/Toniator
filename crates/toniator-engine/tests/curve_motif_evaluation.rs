#[path = "../examples/stage21b_prerequisite_curve_motif_validation.rs"]
#[allow(dead_code)]
mod validation;

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, ChannelId, Document, DocumentCommand, DocumentHistory, DocumentSession,
    PatternOutputRealization, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationProfileCache, EvaluationProgressStage, EvaluationRequest, EvaluationScheduler,
    ResolvedSource, SourceFormatHint, evaluate, evaluate_profiled_cached_with_limits,
};
use toniator_io::{load_preset, save_preset};
use toniator_patterns::PresetRegistry;
use toniator_render::GeometryOutput;

/// Returns the derived-only preset round-trip directory for current catalog witnesses.
fn preset_validation_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage21b-prerequisite-curve-motif/presets");
    fs::create_dir_all(&directory).expect("derived preset directory creates");
    directory
}

/// Evaluates a small ordinary document twice to prove source-driven Curve Motif output is stable.
#[test]
fn curve_motif_uses_the_ordinary_document_evaluator_deterministically() {
    let case = validation::SourceCase {
        label: "curve-motif-test",
        input: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/raster-sample.png"
        ),
        width: 96.0,
        height: 64.0,
        hint: SourceFormatHint::Png,
    };
    let session =
        validation::materialized_session(&case, validation::curve_recipe(true, Some(0.25)));
    let source = Arc::<[u8]>::from(fs::read(case.input).expect("immutable raster source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let first = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source_id.clone(), Arc::clone(&source), case.hint)
            .expect("source resolves"),
    ))
    .expect("first Curve Motif evaluation succeeds");
    let second = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source_id, source, case.hint).expect("source resolves again"),
    ))
    .expect("second Curve Motif evaluation succeeds");
    assert_eq!(first.channels(), second.channels());
    assert_eq!(first.scene(), second.scene());
    assert_eq!(first.raster(), second.raster());
    assert_eq!(first.raster().width(), 96);
    assert_eq!(first.raster().height(), 64);
}

/// Uses the existing high density authority to retain repeated adjacent-site cadence and C0 centerlines.
#[test]
fn curve_motif_high_density_keeps_repeated_cadence_and_connected_centerlines() {
    let case = validation::SourceCase {
        label: "curve-motif-high-density",
        input: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/vector-sample.svg"
        ),
        width: 900.0,
        height: 620.0,
        hint: SourceFormatHint::Svg,
    };
    let session =
        validation::materialized_session(&case, validation::curve_recipe(true, Some(0.25)));
    let source = Arc::<[u8]>::from(fs::read(case.input).expect("immutable vector source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let result = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source_id, source, case.hint).expect("source resolves"),
    ))
    .expect("high-density Curve Motif evaluates");
    let strokes = result
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
        .filter_map(|output| match output.geometry() {
            GeometryOutput::CanonicalStrokes(strokes) => Some(strokes),
            _ => None,
        })
        .flat_map(|strokes| strokes.iter())
        .collect::<Vec<_>>();
    assert!(
        strokes.len() >= 8,
        "density creates legible repeated guide rows"
    );
    let mut copies = 0_usize;
    for stroke in strokes {
        assert!(
            stroke
                .path
                .segments()
                .windows(2)
                .all(|pair| pair[0].end() == pair[1].start()),
            "visible source response never changes the authored chained centerline"
        );
        let chunks = stroke.path.segments().chunks_exact(3);
        assert!(
            chunks.remainder().is_empty(),
            "each copy retains three motif segments"
        );
        let starts = chunks.map(|chunk| chunk[0].start()).collect::<Vec<_>>();
        copies += starts.len();
        if let Some(first_pair) = starts.windows(2).next() {
            let interval = (
                first_pair[1].x - first_pair[0].x,
                first_pair[1].y - first_pair[0].y,
            );
            assert!(starts.windows(2).all(|pair| {
                ((pair[1].x - pair[0].x) - interval.0).abs() < 1.0e-10
                    && ((pair[1].y - pair[0].y) - interval.1).abs() < 1.0e-10
            }));
        }
    }
    assert!(
        copies >= 32,
        "higher density retains many repeated adjacent-site motifs"
    );
}

/// Applies ordinary density-aspect and channel rotation authority without turning Curve Motifs into marks.
#[test]
fn curve_motif_ordinary_aspect_and_channel_rotation_preserve_connected_paths() {
    let case = validation::SourceCase {
        label: "curve-motif-aspect-rotation",
        input: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/raster-sample.png"
        ),
        width: 96.0,
        height: 64.0,
        hint: SourceFormatHint::Png,
    };
    let session =
        validation::materialized_session(&case, validation::curve_recipe(true, Some(0.25)));
    let source = Arc::<[u8]>::from(fs::read(case.input).expect("immutable raster source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let evaluate_current = |history: &DocumentHistory| {
        evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), Arc::clone(&source), case.hint)
                .expect("source resolves"),
        ))
        .expect("ordinary Curve Motif evaluation succeeds")
    };
    let mut history = DocumentHistory::new(session);
    let baseline = evaluate_current(&history);
    let base = history.document().pattern_settings().clone();
    let mut settings = base.clone();
    settings.density.aspect = 1.75;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .expect("ordinary density aspect applies");
    let rotation = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 23.0)
        .expect("connected Curve Motif channel permits rotation");
    history.apply(&rotation).expect("channel rotation applies");
    let transformed = evaluate_current(&history);
    assert_ne!(baseline.scene(), transformed.scene());
    let mut strokes = transformed
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
        .filter_map(|output| match output.geometry() {
            GeometryOutput::CanonicalStrokes(strokes) => Some(strokes),
            _ => None,
        })
        .flat_map(|strokes| strokes.iter());
    assert!(strokes.clone().all(|stroke| {
        stroke
            .path
            .segments()
            .windows(2)
            .all(|pair| pair[0].end() == pair[1].start())
    }));
    assert!(strokes.any(|stroke| stroke.path.segments().len() > 1));
}

/// Proves the seventeenth bundled Curve Motif card materializes and round-trips as preset-v4.
#[test]
fn bundled_curve_motif_card_materializes_and_round_trips_as_preset_v4() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.entries().len(), 17);
    assert!(registry.find("curve-motif-rows").is_some());
    let document = Document::new_default_document(
        CanvasSpec {
            width: 96.0,
            height: 64.0,
        },
        SourceReference::Unassigned,
    )
    .expect("current base document validates");
    for entry in registry.entries() {
        let path = preset_validation_directory().join(format!("{}.preset.json", entry.metadata.id));
        save_preset(&path, entry).expect("current bundled preset saves");
        assert_eq!(
            load_preset(&path).expect("current bundled preset loads"),
            *entry
        );
        let mut history = DocumentHistory::new(
            DocumentSession::new(document.clone()).expect("session validates"),
        );
        registry
            .apply_to_selected(&mut history, ChannelId(1), &entry.metadata.id)
            .expect("current bundled recipe materializes through history");
    }
}

/// Misses the accepted realization cache when an owned Curve Motif resource changes with stable IDs.
#[test]
fn curve_motif_resource_edit_changes_realization_identity_and_cache_key() {
    let case = validation::SourceCase {
        label: "curve-motif-cache",
        input: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/raster-sample.png"
        ),
        width: 96.0,
        height: 64.0,
        hint: SourceFormatHint::Png,
    };
    let session =
        validation::materialized_session(&case, validation::curve_recipe(true, Some(0.25)));
    let mut history = DocumentHistory::new(session);
    let source = Arc::<[u8]>::from(fs::read(case.input).expect("immutable raster source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let mut cache = EvaluationProfileCache::default();
    let first = evaluate_profiled_cached_with_limits(
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), Arc::clone(&source), case.hint)
                .expect("source resolves"),
        ),
        Default::default(),
        &mut cache,
    )
    .expect("baseline Curve Motif evaluates");
    let definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == history.document().pattern_settings().definition_id)
        .expect("selected definition exists");
    let PatternOutputRealization::CurveMotifPaths { structure_id, .. } =
        &definition.definition.output_layers[0].realization
    else {
        panic!("selected output remains Curve Motif");
    };
    let original = history
        .document()
        .authored_structure(*structure_id)
        .expect("owned motif exists")
        .clone();
    let replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.32, y: -0.31 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.32, y: -0.31 },
                end: AuthoredPoint2 { x: 0.7, y: -0.18 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.7, y: -0.18 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("replacement motif validates");
    history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: original,
            replacement,
        })
        .expect("owned motif edit applies");
    let edited = evaluate_profiled_cached_with_limits(
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source, case.hint).expect("source resolves again"),
        ),
        Default::default(),
        &mut cache,
    )
    .expect("edited Curve Motif evaluates");
    assert_ne!(
        first.result.channels()[0].realization_identity(),
        edited.result.channels()[0].realization_identity()
    );
    assert!(
        edited
            .diagnostics
            .channels
            .iter()
            .any(|channel| channel.realization == toniator_engine::CacheDisposition::Miss)
    );
}

/// Coalesces active Curve Motif row progress monotonically and cancels a superseded scheduler ticket.
#[test]
fn curve_motif_scheduler_progress_is_monotonic_and_latest_ticket_cancels_prior_work() {
    let case = validation::SourceCase {
        label: "curve-motif-scheduler",
        input: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/raster-sample.png"
        ),
        width: 96.0,
        height: 64.0,
        hint: SourceFormatHint::Png,
    };
    let session =
        validation::materialized_session(&case, validation::curve_recipe(true, Some(0.25)));
    let source = Arc::<[u8]>::from(fs::read(case.input).expect("immutable raster source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let request = || {
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), Arc::clone(&source), case.hint)
                .expect("source resolves"),
        )
    };
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let first = scheduler.submit(request()).expect("first ticket submits");
    let latest = scheduler.submit(request()).expect("latest ticket submits");
    assert_ne!(first, latest);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut progress = Vec::new();
    let completion = loop {
        while let Some(update) = scheduler
            .try_receive_latest_progress()
            .expect("progress receive succeeds")
        {
            progress.push(update);
        }
        if let Some(completion) = scheduler
            .try_receive_latest()
            .expect("completion receive succeeds")
        {
            break completion;
        }
        assert!(
            Instant::now() < deadline,
            "latest scheduler evaluation timed out"
        );
        std::thread::yield_now();
    };
    assert_eq!(completion.ticket(), latest);
    assert!(completion.result().is_some());
    assert!(
        progress
            .iter()
            .any(|update| { update.stage() == EvaluationProgressStage::RealizingOutputs })
    );
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0].fraction() <= pair[1].fraction())
    );
}
