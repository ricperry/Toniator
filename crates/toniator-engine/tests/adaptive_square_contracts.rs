use std::{fs, path::PathBuf};
use toniator_domain::*;
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{calibrate_saved_pattern, reference_document},
};
use toniator_patterns::{PresetRegistry, adaptive_square_marks_recipe};

/// Verifies the bundled companion against new canonical measurements and all current storage routes.
/// # Panics
/// Panics if nearest-area plateaus, producer identity, persistence or source-free binding drift.
#[test]
fn adaptive_measured_companion_and_storage_roundtrips() {
    let (document, _, channel) = reference_document(&adaptive_square_marks_recipe(), 32).unwrap();
    let measured = calibrate_saved_pattern(
        &document,
        channel,
        EvaluationLimits::new(2_000_000).unwrap(),
        &|| false,
    )
    .unwrap();
    let registry = PresetRegistry::bundled();
    let record = registry.find("adaptive-shape-marks").unwrap();
    assert_eq!(record.saved_calibration.as_ref(), Some(&measured));
    assert_eq!(measured.raw().len(), 10);
    assert_eq!(measured.raw()[0].coverage, 0.0);
    assert!((measured.raw()[1].coverage - (std::f64::consts::FRAC_PI_4 - 0.5)).abs() < 0.001);
    assert_eq!(measured.raw()[2].coverage, 0.5);
    assert_eq!(measured.raw()[3].coverage, measured.raw()[4].coverage);
    assert_eq!(measured.raw()[5].coverage, 1.0);
    assert!(measured.uniform().is_none());
    assert!(
        SavedCalibration::with_profile(
            measured
                .binding()
                .profile_for_reference(CALIBRATION_PROFILE),
            measured.binding().clone(),
            measured.raw().to_vec(),
            None
        )
        .is_err()
    );
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/adaptive-shape-marks/implementation/storage");
    fs::create_dir_all(&directory).unwrap();
    let pattern = directory.join("adaptive.toniator-pattern");
    toniator_io::save_preset(&pattern, record).unwrap();
    assert_eq!(toniator_io::load_preset(&pattern).unwrap(), *record);
    let document = document.with_saved_calibration(channel, measured).unwrap();
    let project = directory.join("adaptive.toniator");
    let SourceReference::Assigned(source_id) = document.source() else {
        panic!("reference source")
    };
    let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
        source_id.clone(),
        toniator_io::EmbeddedSourceFormat::Png,
        fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .unwrap(),
        None,
    )
    .unwrap()])
    .unwrap();
    toniator_io::save(&project, &document, &sources).unwrap();
    assert_eq!(toniator_io::load(&project).unwrap().document(), &document);
    let configuration = DocumentConfiguration::capture(&document);
    let preset = directory.join("adaptive.toniator-preset");
    let guard = toniator_io::capture_document_preset_destination(&preset).unwrap();
    toniator_io::save_document_preset(&preset, &configuration, &guard).unwrap();
    let restored = toniator_io::load_document_preset(&preset, &document).unwrap();
    assert_eq!(restored, configuration);
}
