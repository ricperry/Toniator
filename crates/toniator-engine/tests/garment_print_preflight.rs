use std::{
    fs,
    io::Cursor,
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    DocumentCommand, DocumentHistory, DocumentSession, PhysicalPrintSizeMm,
    PrintPreparationSettings, SourceReference,
};
use toniator_engine::{
    EvaluationLimits, MediaTools, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    open_source_media,
    print_preflight::{
        PreflightLimits, PreflightOutcome, PreflightSelection, WidthCheckStatus,
        preflight_current_frame,
    },
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, LoadedDocument, SourceBundle, load};

const PRECHANGE_PROJECT: &[u8] =
    include_bytes!("../../toniator-io/tests/fixtures/garment-schema10-clean-fade.toniator");

/// Loads stable prechange bytes through the production archive reader.
fn fixture() -> LoadedDocument {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "toniator-g1b-fixture-{}-{nonce}.toniator",
        std::process::id()
    ));
    fs::write(&path, PRECHANGE_PROJECT).unwrap();
    let loaded = load(&path).unwrap();
    fs::remove_file(path).unwrap();
    loaded
}

/// Builds one exact current final-target selection from a session's print authority.
fn selection(session: &DocumentSession, positive: f64, negative: f64) -> PreflightSelection {
    let settings = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(101.6, 38.1).unwrap()),
        positive,
        negative,
    )
    .unwrap();
    assert_eq!(session.document().print_preparation(), &settings);
    PreflightSelection {
        frame: 0,
        target: OutputRasterTarget::new(256, 96).unwrap(),
        antialiasing: RasterAntialiasing::On,
        selected_background: RasterBackground::OpaqueBlack,
        settings,
    }
}

/// Opens the fixture with one selected physical intent and optional channel opacity.
fn history(
    positive: f64,
    negative: f64,
    opacity: Option<f64>,
) -> (LoadedDocument, DocumentHistory) {
    let loaded = fixture();
    let settings = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(101.6, 38.1).unwrap()),
        positive,
        negative,
    )
    .unwrap();
    let document = loaded
        .document()
        .clone()
        .with_print_preparation(settings)
        .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    if let Some(opacity) = opacity {
        let channel_id = history.document().channel_topology().unwrap().channels()[0].id;
        history
            .apply(&DocumentCommand::SetOpacity {
                channel_id,
                opacity,
            })
            .unwrap();
    }
    (loaded, history)
}

/// Keeps alpha inventory even with width checks disabled, and binds current output provenance.
#[test]
fn actual_transparent_output_inventory_is_independent_of_width_flags() {
    let (loaded, history) = history(0.0, 0.0, None);
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let selected = selection(history.session(), 0.0, 0.0);
    let cancelled = AtomicBool::new(false);
    let outcome = preflight_current_frame(
        history.session(),
        &mut media,
        &selected,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &cancelled,
    )
    .unwrap();
    let PreflightOutcome::Complete(report) = outcome else {
        panic!("expected complete report")
    };
    assert_eq!(report.positive_status, WidthCheckStatus::Disabled);
    assert_eq!(report.negative_status, WidthCheckStatus::Disabled);
    assert_eq!(report.alpha_histogram.iter().sum::<u64>(), 256 * 96);
    assert!(!report.support_components.is_empty());
    assert_eq!(
        report.identity.selected_background(),
        RasterBackground::OpaqueBlack
    );
    assert!(
        report
            .support_components
            .iter()
            .any(|component| component.core_pixels > 0
                && component.boundary_associated_low_coverage_pixels > 0)
    );
    assert!(
        report
            .identity
            .is_current(history.session(), &mut media, &selected, &cancelled)
            .unwrap()
    );
}

/// Retains actual alpha-1 and alpha-64 output components that a core-only check would lose.
#[test]
fn low_opacity_final_marks_remain_core_free_candidates() {
    for opacity in [1.0 / 255.0, 0.25] {
        let (loaded, history) = history(0.0, 0.0, Some(opacity));
        let mut media =
            open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
        let selected = selection(history.session(), 0.0, 0.0);
        let outcome = preflight_current_frame(
            history.session(),
            &mut media,
            &selected,
            EvaluationLimits::default(),
            PreflightLimits::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let PreflightOutcome::Complete(report) = outcome else {
            panic!("expected complete report")
        };
        assert!(!report.support_components.is_empty());
        assert!(
            report
                .support_components
                .iter()
                .any(|component| component.core_pixels == 0
                    && component.unresolved_low_coverage_pixels > 0
                    && !component.unresolved_low_coverage_runs.is_empty())
        );
        // Overlapping marks can accumulate above one channel's individual opacity.
        assert!(
            report
                .support_components
                .iter()
                .all(|component| component.max_alpha < 128)
        );
    }
}

/// Rejects same-ID/frame changed provider content and revision changes after edit/undo/redo.
#[test]
fn currentness_rechecks_actual_provider_and_document_revision() {
    let (loaded, mut history) = history(1.0, 1.0, None);
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let selected = selection(history.session(), 1.0, 1.0);
    let cancelled = AtomicBool::new(false);
    let outcome = preflight_current_frame(
        history.session(),
        &mut media,
        &selected,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &cancelled,
    )
    .unwrap();
    let PreflightOutcome::Complete(report) = outcome else {
        panic!("expected complete report")
    };
    assert!(matches!(
        report.positive_status,
        WidthCheckStatus::AdvisoryCandidates | WidthCheckStatus::NoWidthCandidatesWithUnresolved
    ));
    assert!(matches!(
        report.negative_status,
        WidthCheckStatus::AdvisoryCandidates | WidthCheckStatus::NoWidthCandidatesWithUnresolved
    ));
    assert!(
        report
            .identity
            .is_current(history.session(), &mut media, &selected, &cancelled)
            .unwrap()
    );
    let SourceReference::Assigned(id) = history.document().source() else {
        panic!("assigned source")
    };
    let original = image::load_from_memory(loaded.sources().get(id).unwrap().bytes()).unwrap();
    let alternate = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        original.width(),
        original.height(),
        image::Rgba([235, 20, 60, 255]),
    ));
    let mut bytes = Vec::new();
    alternate
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    let alternate_bundle = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        EmbeddedSourceFormat::Png,
        bytes,
        None,
    )
    .unwrap()])
    .unwrap();
    let mut changed_media =
        open_source_media(&alternate_bundle, MediaTools::default(), &|| false).unwrap();
    assert!(
        !report
            .identity
            .is_current(history.session(), &mut changed_media, &selected, &cancelled)
            .unwrap()
    );
    let mut changed_selection = selected.clone();
    changed_selection.antialiasing = RasterAntialiasing::Off;
    assert!(
        !report
            .identity
            .is_current(
                history.session(),
                &mut media,
                &changed_selection,
                &cancelled
            )
            .unwrap()
    );
    changed_selection = selected.clone();
    changed_selection.target = OutputRasterTarget::new(257, 96).unwrap();
    assert!(
        !report
            .identity
            .is_current(
                history.session(),
                &mut media,
                &changed_selection,
                &cancelled
            )
            .unwrap()
    );
    changed_selection = selected.clone();
    changed_selection.selected_background = RasterBackground::OpaqueWhite;
    assert!(
        !report
            .identity
            .is_current(
                history.session(),
                &mut media,
                &changed_selection,
                &cancelled
            )
            .unwrap()
    );
    changed_selection = selected.clone();
    changed_selection.frame = 1;
    assert!(
        !report
            .identity
            .is_current(
                history.session(),
                &mut media,
                &changed_selection,
                &cancelled
            )
            .unwrap()
    );
    let different = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(80.0, 40.0).unwrap()),
        1.0,
        1.0,
    )
    .unwrap();
    history
        .apply(&DocumentCommand::SetPrintPreparation {
            base: selected.settings.clone(),
            settings: different,
        })
        .unwrap();
    assert!(
        !report
            .identity
            .is_current(history.session(), &mut media, &selected, &cancelled)
            .unwrap()
    );
    history.undo().unwrap();
    assert!(
        !report
            .identity
            .is_current(history.session(), &mut media, &selected, &cancelled)
            .unwrap()
    );
    history.redo().unwrap();
    assert!(
        !report
            .identity
            .is_current(history.session(), &mut media, &selected, &cancelled)
            .unwrap()
    );
}

/// Distinguishes fully invisible output, unknown placement, and sub-resolution thresholds.
#[test]
fn empty_support_and_unavailable_physical_checks_are_truthful() {
    let loaded = fixture();
    let mut history =
        DocumentHistory::new(DocumentSession::new(loaded.document().clone()).unwrap());
    let channel_id = history.document().channel_topology().unwrap().channels()[0].id;
    history
        .apply(&DocumentCommand::SetVisibility {
            channel_id,
            visible: false,
        })
        .unwrap();
    let target = OutputRasterTarget::new(256, 96).unwrap();
    let unknown = PrintPreparationSettings::new(None, 1.0, 1.0).unwrap();
    let document = history
        .document()
        .clone()
        .with_print_preparation(unknown.clone())
        .unwrap();
    let session = DocumentSession::new(document).unwrap();
    let selected = PreflightSelection {
        frame: 0,
        target,
        antialiasing: RasterAntialiasing::On,
        selected_background: RasterBackground::OpaqueWhite,
        settings: unknown,
    };
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let outcome = preflight_current_frame(
        &session,
        &mut media,
        &selected,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let PreflightOutcome::Complete(report) = outcome else {
        panic!("expected inventory")
    };
    assert!(report.support_components.is_empty());
    assert_eq!(report.alpha_histogram[0], 256 * 96);
    assert_eq!(
        report.positive_status,
        WidthCheckStatus::UnavailablePlacement
    );
    assert_eq!(
        report.negative_status,
        WidthCheckStatus::UnavailablePlacement
    );
    assert!(report.ppi_axes.is_none());
    let tiny = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(101.6, 38.1).unwrap()),
        0.1,
        0.1,
    )
    .unwrap();
    let session = DocumentSession::new(
        loaded
            .document()
            .clone()
            .with_print_preparation(tiny.clone())
            .unwrap(),
    )
    .unwrap();
    let selected = PreflightSelection {
        settings: tiny,
        ..selected
    };
    let outcome = preflight_current_frame(
        &session,
        &mut media,
        &selected,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let PreflightOutcome::Complete(report) = outcome else {
        panic!("expected inventory")
    };
    assert_eq!(report.positive_status, WidthCheckStatus::SamplingLimited);
    assert_eq!(report.negative_status, WidthCheckStatus::SamplingLimited);
    assert!(report.positive_candidates.is_empty() && report.negative_candidates.is_empty());
    let extreme = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(f64::MAX, f64::MAX).unwrap()),
        1.0,
        1.0,
    )
    .unwrap();
    let session = DocumentSession::new(
        loaded
            .document()
            .clone()
            .with_print_preparation(extreme.clone())
            .unwrap(),
    )
    .unwrap();
    let selected = PreflightSelection {
        settings: extreme,
        target: OutputRasterTarget::new(1, 1).unwrap(),
        ..selected
    };
    let outcome = preflight_current_frame(
        &session,
        &mut media,
        &selected,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let PreflightOutcome::Complete(report) = outcome else {
        panic!("expected inventory")
    };
    assert_eq!(report.positive_status, WidthCheckStatus::SamplingLimited);
    assert!(report.sampling_guard_mm_axes.is_none());
    assert!(
        report
            .sampling_note
            .as_deref()
            .unwrap()
            .contains("not representable")
    );
}

/// Returns explicit unavailable or cancelled outcomes without partial findings.
#[test]
fn oversized_or_cancelled_runs_never_publish_findings() {
    let (loaded, history) = history(1.0, 1.0, None);
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false).unwrap();
    let selected = selection(history.session(), 1.0, 1.0);
    let limits = PreflightLimits {
        max_records: u64::MAX,
        ..PreflightLimits::default()
    };
    assert!(matches!(
        preflight_current_frame(
            history.session(),
            &mut media,
            &selected,
            EvaluationLimits::default(),
            limits,
            &AtomicBool::new(false)
        )
        .unwrap(),
        PreflightOutcome::Unavailable { .. }
    ));
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        preflight_current_frame(
            history.session(),
            &mut media,
            &selected,
            EvaluationLimits::default(),
            PreflightLimits::default(),
            &cancelled
        )
        .unwrap(),
        PreflightOutcome::Cancelled
    ));
}
