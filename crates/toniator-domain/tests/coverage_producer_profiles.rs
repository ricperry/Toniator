//! Typed producer revisions preserve exact former-profile recognition for supported source-free data.
use toniator_domain::*;

/// Builds a valid mixed output recipe through the same normalized output authority as current files.
fn mixed_recipe() -> PatternDefinitionRecipe {
    let family = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "Mixed topology producers".into(),
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
    };
    let connected = PatternDefinitionRecipe::connected(family.clone())
        .output_settings
        .remove(0);
    let regions = PatternDefinitionRecipe::guide_faces(family.clone(), vec![0, 1])
        .output_settings
        .remove(0);
    PatternDefinitionRecipe {
        structure: PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(family),
            outputs: vec![
                PatternOutputRealizationRecipe::MazeWalls {
                    program: MazeProgram {
                        algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                        seed: 31,
                    },
                    style: Default::default(),
                },
                PatternOutputRealizationRecipe::GuideFaceRegions {
                    dimension_indices: vec![0, 1],
                },
                PatternOutputRealizationRecipe::StructuralPaths {
                    tone_sample_spacing: ToneSampleSpacing::default(),
                    style: Default::default(),
                },
            ],
        },
        output_settings: vec![connected.clone(), regions, connected],
    }
}

/// Recognizes the complete former artistic suffix while rejecting partial and unknown producer strings.
/// # Panics
/// Panics if valid typed mixed outputs cannot bind or obsolete metadata gains ambiguous authority.
#[test]
fn mixed_topology_profiles_recognize_exact_former_complete_suffix_only() {
    let binding = CalibrationBinding::new(mixed_recipe(), 1.0, 0.0).unwrap();
    assert!(binding.uses_maze_walls());
    assert!(binding.uses_guide_faces());
    let former = format!("{CALIBRATION_PROFILE_GUIDE_FACES}+anchored-tone-v1");
    let current = format!("{former}+maze-guard-topology-v2+cell-reach-v1");
    assert_eq!(
        binding.profile_for_reference(CALIBRATION_PROFILE_GUIDE_FACES),
        current
    );
    assert!(binding.is_obsolete_geometry_profile(&former));
    assert!(!binding.is_obsolete_geometry_profile(&current));
    for invalid in [
        format!("{former}+maze-guard-topology-v2"),
        format!("{former}+cell-reach-v1"),
        format!("{former}+unknown-future"),
        format!("{CALIBRATION_PROFILE_SEEDED}+anchored-tone-v1"),
    ] {
        assert!(!binding.is_obsolete_geometry_profile(&invalid), "{invalid}");
    }
}

/// Revises actual authored guide cores while preserving unrelated finite and ordinary mark producers.
/// # Panics
/// Panics if typed authored-guide coverage loses provenance or unknown numeric profiles gain authority.
#[test]
fn authored_guide_ray_profiles_distinguish_changed_core_from_unrelated_artwork() {
    for repetition in [
        GuideRepetition::Single,
        GuideRepetition::TransformStack {
            direction_degrees: 90.0,
            spacing_multiplier: 1.0,
        },
        GuideRepetition::NormalOffset {
            spacing: 8.0,
            cleanup: OffsetCleanup::DissolveCrossings,
        },
    ] {
        let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredResources {
            resources: vec![
                AuthoredStructureDraft::new(
                    AuthoredStructureKind::OpenPath,
                    vec![AuthoredCurveSegment::Line {
                        start: AuthoredPoint2 { x: -64.0, y: 0.0 },
                        end: AuthoredPoint2 { x: 64.0, y: 0.0 },
                    }],
                )
                .unwrap(),
            ],
            definition: Box::new(PatternStructureRecipe::GenericGuides {
                name: "Actual authored core".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
                dimensions: vec![GenericGuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                        resource_index: 0,
                    },
                    repetition,
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.0,
                },
                orientation: MarkOrientationDraft::Fixed,
            }),
        });
        let binding = CalibrationBinding::new(recipe, 1.0, 0.0).unwrap();
        let former = format!("{CALIBRATION_PROFILE}+guide-span-v2");
        assert!(binding.uses_authored_guide_coverage());
        assert_eq!(
            binding.profile_for_reference(CALIBRATION_PROFILE),
            format!("{former}+guide-ray-coverage-v1")
        );
        assert!(binding.is_obsolete_geometry_profile(&former));
        assert!(!binding.is_obsolete_geometry_profile(&format!("{former}+guide-ray-coverage-v2")));
    }
}

/// Assigns Voronoi cell-reach provenance only to actual straight-intersection Voronoi outputs.
/// # Panics
/// Panics if ordinary marks receive a new geometry contract or a superseded Voronoi companion is relabelled.
#[test]
fn straight_voronoi_profiles_are_typed_and_reject_former_numeric_relabeling() {
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = mixed_recipe().structure else {
        unreachable!()
    };
    let structure = *definition;
    let mark = CalibrationBinding::new(PatternDefinitionRecipe::marks(structure.clone()), 1.0, 0.0)
        .unwrap();
    assert!(!mark.uses_straight_intersection_voronoi());
    assert_eq!(
        mark.profile_for_reference(CALIBRATION_PROFILE),
        CALIBRATION_PROFILE
    );
    let binding =
        CalibrationBinding::new(PatternDefinitionRecipe::regions(structure), 1.0, 0.0).unwrap();
    assert!(binding.uses_straight_intersection_voronoi());
    assert_eq!(
        binding.profile_for_reference(CALIBRATION_PROFILE),
        format!("{CALIBRATION_PROFILE}+voronoi-cell-reach-v1")
    );
    assert!(binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE));
    let raw = vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 2.0,
            coverage: 1.0,
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
    assert!(SavedCalibration::new(binding, raw, None).is_ok());
}
