use toniator_domain::{
    CALIBRATION_PROFILE_SEEDED, CALIBRATION_REFERENCE_SEED, CalibrationBinding, CalibrationKnot,
    ConnectionAdjacencyIntent, ConnectionProgram, CoveragePolicy, GeneralizedSiteProductDraft,
    GridMazeAlgorithm, GridSpanningTreeAlgorithm, GuideDimensionDraft, MarkOrientationDraft,
    MazeProgram, PathStrokeStyle, PatternDefinitionRecipe, PatternOutputRealizationRecipe,
    PatternStructureRecipe, RandomSiteCharacter, RandomSiteRefinement, SavedCalibration,
    SiteDensityModulation, SiteExclusionPolicy, StrokeCap, StrokeJoin,
};

/// Builds a complete random-site identity with only its authored variation seed changed.
fn random_sites(seed: u32) -> PatternStructureRecipe {
    PatternStructureRecipe::RandomSites {
        name: "Seed-neutral sites".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
        character: RandomSiteCharacter::RawUniform,
        nominal_spacing: Some(10.24),
        seed,
        density_modulation: SiteDensityModulation::Uniform,
        exclusion: SiteExclusionPolicy::None,
        maximum_attempts: 16_000_000,
        maximum_neighbor_checks: 16_000_000,
        refinement: RandomSiteRefinement::default(),
    }
}

/// Builds a two-guide site family for deterministic connection and maze seed checks.
fn guide_sites() -> PatternStructureRecipe {
    PatternStructureRecipe::GeneralizedStraightGuides {
        name: "Seed-neutral grid".into(),
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
    }
}

/// Checks one pair of recipes differs only in authored seed and shares a v5 saved binding.
///
/// # Panics
/// Panics if seed normalization changes structural identity or admits a former v4 profile.
fn assert_seed_equivalent(first: PatternDefinitionRecipe, second: PatternDefinitionRecipe) {
    assert_ne!(first, second);
    let one = CalibrationBinding::new(first, 1.0, 0.0).unwrap();
    let two = CalibrationBinding::new(second, 1.0, 0.0).unwrap();
    assert_eq!(one, two);
    assert!(one.has_variation_seed());
    let artifact = SavedCalibration::new(
        one.clone(),
        vec![
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 0.9,
            },
        ],
        None,
    )
    .unwrap();
    assert_eq!(artifact.profile(), CALIBRATION_PROFILE_SEEDED);
    assert!(
        SavedCalibration::with_profile(
            "ton018-reference-fixed-canvas1024-spacings32-v4".into(),
            two,
            artifact.raw().to_vec(),
            None,
        )
        .is_err()
    );
}

/// Ignores numerical variation seeds in each current family/output representation.
///
/// # Panics
/// Panics if site, random-link, tree, maze, or ordered-output seeds still split a saved binding.
#[test]
fn seed_values_are_not_saved_calibration_identity() {
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 3,
        maximum_distance: 48.0,
    };
    let style = PathStrokeStyle {
        join: StrokeJoin::Round,
        cap: StrokeCap::Round,
    };
    assert_seed_equivalent(
        PatternDefinitionRecipe::marks(random_sites(CALIBRATION_REFERENCE_SEED)),
        PatternDefinitionRecipe::marks(random_sites(0)),
    );
    let links = |site_seed, line_seed| {
        PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
            definition: Box::new(random_sites(site_seed)),
            program: ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree: 0,
                seed: line_seed,
            },
            style,
        })
    };
    assert_seed_equivalent(links(43, 43), links(0, u32::MAX));
    let tree = |seed| {
        PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
            definition: Box::new(guide_sites()),
            program: ConnectionProgram::GridSpanningTree {
                adjacency,
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed,
            },
            style,
        })
    };
    assert_seed_equivalent(tree(43), tree(0));
    let maze = |seed| {
        PatternDefinitionRecipe::connected(PatternStructureRecipe::MazeWalls {
            definition: Box::new(guide_sites()),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed,
            },
            style,
        })
    };
    assert_seed_equivalent(maze(43), maze(u32::MAX));
    let ordered = |seed| {
        PatternDefinitionRecipe::connected(PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(guide_sites()),
            outputs: vec![PatternOutputRealizationRecipe::ConnectionPaths {
                program: ConnectionProgram::GridSpanningTree {
                    adjacency,
                    algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                    seed,
                },
                style,
            }],
        })
    };
    assert_seed_equivalent(ordered(43), ordered(1));
}
