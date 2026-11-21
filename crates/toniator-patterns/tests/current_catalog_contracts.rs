//! Current catalog contracts isolated from historical integration-target compilation.
use toniator_domain::{CALIBRATION_PROFILE_MEAN, CalibrationBinding, SavedCalibration};
use toniator_patterns::PresetRegistry;

/// Preserves every established catalog ID and the four artistic patterns in strict sorted order.
/// Reconstruction returns each exact ordinary recipe rather than a label-dispatched variant.
/// # Panics
/// Panics if an ID disappears, duplicates, changes order, or reconstructs a different recipe.
#[test]
fn current_catalog_sorted_ids_reconstruct_every_recipe() {
    let registry = PresetRegistry::bundled();
    let ids = registry
        .entries()
        .iter()
        .map(|entry| entry.metadata.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 20);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        ids,
        vec![
            "adaptive-shape-marks",
            "clustered-dispersion-random-links",
            "concentric-hatching",
            "curve-motif-rows",
            "even-random-circles",
            "grid-voronoi-scale",
            "one-guide-lines",
            "parallel-hatching",
            "ranked-blue-noise-marks",
            "round-spiral-line",
            "round-spiral-marks",
            "source-weighted-dispersion-voronoi",
            "square-spiral-marks",
            "straight-grid-circles",
            "three-guide-cells-scale",
            "three-guide-maze",
            "triagrid-custom-shape-marks",
            "triagrid-spanning-tree",
            "two-guide-cells-uniform-offset",
            "two-guide-maze",
        ]
    );
    for entry in registry.entries() {
        assert_eq!(
            registry.reconstruct(&entry.metadata.id).as_ref(),
            Some(&entry.recipe),
            "{} reconstructs its exact current recipe",
            entry.metadata.id
        );
    }
}

/// Validates every complete companion against its exact recipe and current typed producer profile.
/// Mean profiles retain their variation witness; intrinsic screens retain measured dispersion data.
/// # Panics
/// Panics if a companion is absent, stale, incomplete, or cannot restore its exact validated data.
#[test]
fn current_catalog_companions_have_complete_bindings_and_profiles() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.entries().len(), 20);
    for entry in registry.entries() {
        let saved = entry
            .saved_calibration
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no measured companion", entry.metadata.id));
        toniator_domain::validate_preset_record(entry)
            .expect("the complete companion matches its canonical concrete recipe");
        saved
            .require_complete()
            .expect("every required measurement is present");
        let expected = CalibrationBinding::new(
            entry.recipe.clone(),
            saved.binding().density_aspect(),
            saved.binding().shape_rotation_degrees(),
        )
        .expect("the current recipe has a canonical binding");
        assert_eq!(saved.binding(), &expected);
        assert_eq!(
            saved.variation().is_some(),
            saved.profile() == expected.profile_for_reference(CALIBRATION_PROFILE_MEAN),
            "{} preserves the mean profile's variation witness",
            entry.metadata.id
        );
        if entry.metadata.id == "adaptive-shape-marks" {
            assert!(expected.uses_adaptive_square_marks());
            assert_eq!(
                saved.profile(),
                expected
                    .profile_for_reference(toniator_domain::CALIBRATION_PROFILE_ADAPTIVE_SQUARE)
            );
            assert_eq!(saved.raw().len(), 10);
            assert_eq!(saved.raw()[3].coverage, saved.raw()[4].coverage);
            assert!(saved.uniform().is_none());
        }
        let mut restored = match saved.variation() {
            Some(variation) => SavedCalibration::with_mean_profile(
                expected,
                saved.raw().to_vec(),
                saved.uniform().cloned(),
                variation.clone(),
            ),
            None => SavedCalibration::with_profile(
                saved.profile().to_owned(),
                expected,
                saved.raw().to_vec(),
                saved.uniform().cloned(),
            ),
        }
        .expect("the actual typed profile accepts the current measured payload");
        if let Some(dispersion) = saved.dispersion() {
            restored = restored
                .with_dispersion(dispersion.clone())
                .expect("intrinsic layout observations retain their typed binding");
        }
        restored
            .require_complete()
            .expect("restored calibration retains all required measurements");
        assert_eq!(&restored, saved, "{} restores exactly", entry.metadata.id);
    }
}
