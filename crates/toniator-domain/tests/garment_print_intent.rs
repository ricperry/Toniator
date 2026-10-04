use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentConfiguration, DocumentHistory, DocumentSession,
    PhysicalPrintSizeMm, PrintPreparationSettings, SourceReference,
};

/// Builds one ordinary project whose print placement remains explicitly unknown.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 320.0,
            height: 180.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    DocumentHistory::new(DocumentSession::new(document).unwrap())
}

/// Checks physical unit conversion, fixed placement at two raster sizes, and invalid bounds.
#[test]
fn physical_units_and_final_target_ppi_are_explicit() {
    let size = PhysicalPrintSizeMm::from_inches(4.0, 2.0).unwrap();
    assert_eq!(size.millimetres(), (101.6, 50.8));
    assert_eq!(size.inches().unwrap(), (4.0, 2.0));
    assert_eq!(
        size.ppi_for_pixels(1200, 400).unwrap().axes(),
        (300.0, 200.0)
    );
    assert_eq!(
        size.ppi_for_pixels(2400, 800).unwrap().axes(),
        (600.0, 400.0)
    );
    assert!(size.ppi_for_pixels(0, 400).is_err());
    assert!(PhysicalPrintSizeMm::new(0.0, 10.0).is_err());
    assert!(PhysicalPrintSizeMm::new(f64::NAN, 10.0).is_err());
    assert!(PhysicalPrintSizeMm::from_inches(f64::MAX, 1.0).is_err());
    assert!(
        PhysicalPrintSizeMm::new(f64::from_bits(1), 10.0)
            .unwrap()
            .inches()
            .is_err()
    );
    assert!(
        PhysicalPrintSizeMm::new(f64::from_bits(1), 10.0)
            .unwrap()
            .ppi_for_pixels(1, 1)
            .is_err()
    );
}

/// Keeps zero disabled and round-trips artist thresholds independently of placement.
#[test]
fn thresholds_allow_unknown_placement_and_normalize_negative_zero() {
    let settings = PrintPreparationSettings::new(None, -0.0, 0.2).unwrap();
    assert_eq!(settings.minimum_positive_feature_width_mm().to_bits(), 0);
    assert_eq!(settings.minimum_negative_gap_width_mm(), 0.2);
    assert_eq!(settings.ppi_for_pixels(100, 100).unwrap(), None);
    assert!(settings.ppi_for_pixels(0, 100).is_err());
    assert_eq!(
        PrintPreparationSettings::threshold_mm_from_inches(0.0).unwrap(),
        0.0
    );
    assert_eq!(
        PrintPreparationSettings::threshold_mm_from_inches(-0.0)
            .unwrap()
            .to_bits(),
        0
    );
    assert_eq!(
        PrintPreparationSettings::threshold_mm_from_inches(1.0).unwrap(),
        25.4
    );
    assert_eq!(
        PrintPreparationSettings::threshold_inches_from_mm(25.4).unwrap(),
        1.0
    );
    assert!(PrintPreparationSettings::threshold_mm_from_inches(f64::INFINITY).is_err());
    assert!(PrintPreparationSettings::threshold_inches_from_mm(f64::from_bits(1)).is_err());
    assert!(PrintPreparationSettings::new(None, -0.1, 0.0).is_err());
    assert!(PrintPreparationSettings::new(None, 0.1, f64::NAN).is_err());
}

/// Makes document-only intent reversible while stale tokens and stale bases stay rejected.
#[test]
fn print_command_has_no_render_invalidation_but_advances_history() {
    let mut history = history();
    let initial = history.document().clone();
    let token = history.session().document_evaluation_snapshot().token();
    let entered = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(100.0, 50.0).unwrap()),
        0.8,
        0.2,
    )
    .unwrap();
    let command = DocumentCommand::SetPrintPreparation {
        base: initial.print_preparation().clone(),
        settings: entered.clone(),
    };
    assert!(
        DocumentSession::new(initial.clone())
            .unwrap()
            .apply(&command)
            .is_err()
    );
    let result = history.apply(&command).unwrap();
    assert_eq!(result.invalidation, None);
    assert!(result.affected_channels.is_empty());
    assert!(!history.session().accepts_document_evaluation(token));
    assert_eq!(history.document().print_preparation(), &entered);
    assert!(history.apply(&command).is_err());
    let revision = history.revision();
    assert!(
        history
            .apply(&DocumentCommand::SetPrintPreparation {
                base: entered.clone(),
                settings: entered.clone(),
            })
            .is_err()
    );
    assert_eq!(history.revision(), revision);
    assert_eq!(history.undo().unwrap().unwrap().invalidation, None);
    assert_eq!(history.document(), &initial);
    assert!(!history.session().accepts_document_evaluation(token));
    assert_eq!(history.redo().unwrap().unwrap().invalidation, None);
    assert_eq!(history.document().print_preparation(), &entered);
    history.undo().unwrap();
    let alternate = PrintPreparationSettings::new(None, 0.3, 0.0).unwrap();
    history
        .apply(&DocumentCommand::SetPrintPreparation {
            base: initial.print_preparation().clone(),
            settings: alternate.clone(),
        })
        .unwrap();
    assert!(!history.can_redo());
    assert_eq!(history.document().print_preparation(), &alternate);
    history
        .apply(&DocumentCommand::SetPrintPreparation {
            base: alternate.clone(),
            settings: PrintPreparationSettings::default(),
        })
        .unwrap();
    assert_eq!(
        history.document().print_preparation(),
        &PrintPreparationSettings::default()
    );
    history.undo().unwrap();
    assert_eq!(history.document().print_preparation(), &alternate);
    history.redo().unwrap();
    assert_eq!(
        history.document().print_preparation(),
        &PrintPreparationSettings::default()
    );
}

/// Keeps destination physical intent when a source-free configuration is rebound.
#[test]
fn configuration_binding_preserves_destination_print_intent() {
    let source = history().document().clone();
    let settings = PrintPreparationSettings::new(
        Some(PhysicalPrintSizeMm::new(80.0, 40.0).unwrap()),
        0.7,
        0.3,
    )
    .unwrap();
    let destination = source
        .clone()
        .with_print_preparation(settings.clone())
        .unwrap();
    let configuration = DocumentConfiguration::capture(&source);
    let bound = configuration.bind(&destination).unwrap();
    assert_eq!(bound.print_preparation(), &settings);
    assert_eq!(bound.canvas(), destination.canvas());
    assert_eq!(bound.source(), destination.source());
}
