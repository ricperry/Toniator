use std::{fs, path::PathBuf};
use toniator_domain::*;
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{calibrate_saved_pattern, reference_document},
};
use toniator_patterns::{
    PresetRegistry, adaptive_shape_marks_calibration, adaptive_square_marks_recipe,
};

/// Proves nearest and blue-noise companions retain distinct genuine measurements through storage.
/// # Panics
/// Panics if current producer observations, project/Pattern/Preset readback or catalog identity drift.
#[test]
fn adaptive_blue_noise_measured_companion_and_current_storage() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/adaptive-blue-noise-control/storage");
    fs::create_dir_all(&root).unwrap();
    let registry = PresetRegistry::bundled();
    assert_eq!(
        registry
            .entries()
            .iter()
            .filter(|record| record.metadata.id == "adaptive-shape-marks")
            .count(),
        1
    );
    let mut observations = Vec::new();
    for enabled in [false, true] {
        let mut recipe = adaptive_square_marks_recipe();
        if enabled {
            let PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut recipe.structure
            else {
                panic!("ordered adaptive recipe")
            };
            outputs[0] = PatternOutputRealizationRecipe::AdaptiveSquareBlueNoiseMarks;
        }
        let (document, _, channel) = reference_document(&recipe, 32).unwrap();
        let binding = CalibrationBinding::for_channel(&document, channel).unwrap();
        assert_eq!(binding.uses_adaptive_blue_noise(), enabled);
        let measured = calibrate_saved_pattern(
            &document,
            channel,
            EvaluationLimits::new(2_000_000).unwrap(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            adaptive_shape_marks_calibration(&binding).unwrap(),
            Some(measured.clone())
        );
        observations.push(measured.raw().to_vec());
        let document = document
            .with_saved_calibration(channel, measured.clone())
            .unwrap();
        let suffix = if enabled { "on" } else { "off" };
        let SourceReference::Assigned(source_id) = document.source() else {
            panic!("reference source")
        };
        let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
            source_id.clone(),
            toniator_io::EmbeddedSourceFormat::Png,
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"),
            )
            .unwrap(),
            None,
        )
        .unwrap()])
        .unwrap();
        let project = root.join(format!("{suffix}.toniator"));
        toniator_io::save(&project, &document, &sources).unwrap();
        assert_eq!(toniator_io::load(&project).unwrap().document(), &document);
        let configuration = DocumentConfiguration::capture(&document);
        let preset = root.join(format!("{suffix}.toniator-preset"));
        let guard = toniator_io::capture_document_preset_destination(&preset).unwrap();
        toniator_io::save_document_preset(&preset, &configuration, &guard).unwrap();
        assert_eq!(
            toniator_io::load_document_preset(&preset, &document).unwrap(),
            configuration
        );
        let mut pattern = registry.find("adaptive-shape-marks").unwrap().clone();
        pattern.recipe = recipe;
        pattern.saved_calibration = Some(measured);
        let path = root.join(format!("{suffix}.toniator-pattern"));
        toniator_io::save_preset(&path, &pattern).unwrap();
        assert_eq!(toniator_io::load_preset(&path).unwrap(), pattern);
    }
    assert_ne!(observations[0], observations[1]);
    assert!((observations[1][1].coverage - 2.0 / 9.0).abs() < 0.002);
    assert_eq!(observations[0][3].coverage, observations[0][4].coverage);
}
