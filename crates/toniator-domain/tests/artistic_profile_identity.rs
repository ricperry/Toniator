//! Current numeric measurements distinguish changed artistic producers from supported prior caches.
use toniator_domain::*;
/// Returns a finite grid recipe suitable for stable connection/maze/mark binding checks.
fn grid() -> PatternStructureRecipe {
    PatternStructureRecipe::GeneralizedStraightGuides {
        name: "Profile fixture".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
        dimensions: [0., 90.]
            .into_iter()
            .map(|angle| GuideDimensionDraft {
                baseline_angle_degrees: angle,
                phase: 0.,
                spacing_multiplier: 1.,
            })
            .collect(),
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1],
            merge_epsilon: 1e-9,
        },
        orientation: MarkOrientationDraft::Fixed,
    }
}
/// Checks scoped producer markers and current rejection without weakening unknown-profile validation.
/// # Panics
/// Panics if new producers relabel old profiles or unaffected producers acquire an artistic marker.
#[test]
fn artistic_profiles_are_current_only_and_unaffected_producers_keep_identity() {
    let connection = PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
        selection: Default::default(),
        definition: Box::new(grid()),
        program: ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 20.,
            },
        },
        style: Default::default(),
    });
    let binding = CalibrationBinding::new(connection, 1., 0.).unwrap();
    let current = binding.profile_for_reference(CALIBRATION_PROFILE);
    assert!(current.ends_with("+connection-selection-v2"));
    assert!(binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE));
    assert!(
        !binding.is_obsolete_geometry_profile(&format!("{CALIBRATION_PROFILE}+unknown-future"))
    );
    let raw = vec![
        CalibrationKnot {
            input: 0.,
            coverage: 0.,
        },
        CalibrationKnot {
            input: 2.,
            coverage: 1.,
        },
    ];
    assert!(
        SavedCalibration::with_profile(
            CALIBRATION_PROFILE.into(),
            binding.clone(),
            raw.clone(),
            None
        )
        .is_err()
    );
    assert!(SavedCalibration::with_profile(current, binding, raw, None).is_ok());
    let curve = PatternDefinitionRecipe::connected(PatternStructureRecipe::ParametricCurve {
        name: "Round fixture".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.,
        },
        curve: ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 2.,
            radial_spacing: 10.,
            phase_degrees: 0.,
            winding: CurveWinding::Clockwise,
        }),
        spiral_coverage: SpiralCoveragePolicy::Fixed,
        repetition: GuideRepetition::Single,
        sites: None,
    });
    let curve = CalibrationBinding::new(curve, 1., 0.).unwrap();
    assert!(
        curve
            .profile_for_reference(CALIBRATION_PROFILE)
            .ends_with("+anchored-tone-v1")
    );
    let marks = CalibrationBinding::new(PatternDefinitionRecipe::marks(grid()), 1., 0.).unwrap();
    assert_eq!(
        marks.profile_for_reference(CALIBRATION_PROFILE),
        CALIBRATION_PROFILE
    );
    let maze = CalibrationBinding::new(
        PatternDefinitionRecipe::connected(PatternStructureRecipe::MazeWalls {
            definition: Box::new(grid()),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed: 17,
            },
            style: Default::default(),
        }),
        1.,
        0.,
    )
    .unwrap();
    assert_eq!(
        maze.profile_for_reference(CALIBRATION_PROFILE_SEEDED),
        CALIBRATION_PROFILE_SEEDED
    );
}
