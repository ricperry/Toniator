use toniator_domain::{
    CalibrationBinding, CanvasSpec, ConnectionAdjacencyIntent, ConnectionCyclePolicy,
    ConnectionProgram, ConnectionSelectionIntent, CoveragePolicy, DensityMetric2D,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft, PathStrokeStyle,
    PatternDefinitionRecipe, PatternStructureRecipe, StrokeCap, StrokeJoin, ToneSampleSpacing,
};

/// Builds current ID-free connection intent without deriving topology or source samples.
fn connection_recipe(selection: ConnectionSelectionIntent, seed: u32) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
        selection,
        definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Artistic graph authority".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: [0.0, 90.0]
                .into_iter()
                .map(|angle| GuideDimensionDraft {
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                })
                .collect(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-9,
            },
            orientation: MarkOrientationDraft::Fixed,
        }),
        program: ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 4,
                maximum_distance: 40.0,
            },
            minimum_degree: 1,
            seed,
        },
        style: PathStrokeStyle {
            join: StrokeJoin::Round,
            cap: StrokeCap::Round,
        },
    })
}

/// Proves requested cadence scales with Feature size at both canvas aspects, without pixel floors.
/// # Panics
/// Panics when current positive domain intent fails validation or proportionality is lost.
#[test]
fn tone_spacing_tracks_feature_size_without_a_pixel_floor() {
    for canvas in [
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
    ] {
        for aspect in [1.0, 4.0] {
            let spacing = ToneSampleSpacing { multiplier: 2.0 };
            let one = spacing
                .resolve(
                    &canvas,
                    &DensityMetric2D {
                        density: 100.0,
                        aspect,
                    },
                )
                .unwrap();
            let two = spacing
                .resolve(
                    &canvas,
                    &DensityMetric2D {
                        density: 50.0,
                        aspect,
                    },
                )
                .unwrap();
            assert_eq!(two, 2.0 * one);
            assert_eq!(ToneSampleSpacing::default().multiplier, 1.0);
            assert!(
                ToneSampleSpacing { multiplier: 1e-12 }
                    .resolve(
                        &canvas,
                        &DensityMetric2D {
                            density: 100.0,
                            aspect
                        }
                    )
                    .unwrap()
                    < 1e-8
            );
        }
    }
}

/// Rejects invalid artistic bounds while retaining unrestricted graph selection defaults.
/// # Panics
/// Panics if invalid values enter current authority or the approved defaults change.
#[test]
fn artistic_intent_bounds_and_unrestricted_defaults_are_explicit() {
    for multiplier in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(ToneSampleSpacing { multiplier }.validate().is_err());
    }
    let default = ConnectionSelectionIntent::default();
    assert_eq!(default.maximum_final_degree, None);
    assert_eq!(default.cycles, ConnectionCyclePolicy::Allow);
    assert_eq!(default.minimum_component_points, 1);
    assert!(
        ConnectionSelectionIntent {
            maximum_final_degree: Some(0),
            ..default
        }
        .validate()
        .is_err()
    );
    assert!(
        ConnectionSelectionIntent {
            minimum_component_points: 0,
            ..default
        }
        .validate()
        .is_err()
    );
    assert!(
        ConnectionSelectionIntent {
            maximum_final_degree: Some(u32::MAX),
            minimum_component_points: u32::MAX,
            ..default
        }
        .validate()
        .is_ok()
    );
}

/// Binds final graph policy while preserving the established seed-neutral calibration contract.
/// # Panics
/// Panics if valid recipes fail binding or policy/seed identity has the wrong scope.
#[test]
fn connection_policy_is_calibration_geometry_but_variation_seed_is_not() {
    let default = ConnectionSelectionIntent::default();
    let first = CalibrationBinding::new(connection_recipe(default, 7), 1.0, 0.0).unwrap();
    let seeded = CalibrationBinding::new(connection_recipe(default, 9), 1.0, 0.0).unwrap();
    assert!(first.is_compatible_with(&seeded));
    for selection in [
        ConnectionSelectionIntent {
            maximum_final_degree: Some(2),
            ..default
        },
        ConnectionSelectionIntent {
            cycles: ConnectionCyclePolicy::Forbid,
            ..default
        },
        ConnectionSelectionIntent {
            minimum_component_points: 3,
            ..default
        },
    ] {
        let changed = CalibrationBinding::new(connection_recipe(selection, 7), 1.0, 0.0).unwrap();
        assert!(!first.is_compatible_with(&changed));
    }
}
