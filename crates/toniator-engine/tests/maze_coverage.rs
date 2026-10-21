//! Production pre-generation reserves complete topology reach without changing placement calibration.
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::ConstructionMode;

/// Builds one skew intersection family with either complete Maze or Guide Faces output authority.
fn recipe(faces: bool) -> PatternDefinitionRecipe {
    let family = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "Production complete skew cells".into(),
        coverage: CoveragePolicy {
            guard_steps: 0,
            additional_margin: 3.0,
        },
        dimensions: [0.0, 5.0]
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
    if faces {
        PatternDefinitionRecipe::guide_faces(family, vec![0, 1])
    } else {
        PatternDefinitionRecipe::connected(PatternStructureRecipe::MazeWalls {
            definition: Box::new(family),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed: 31,
            },
            style: Default::default(),
        })
    }
}

/// Installs explicit synthetic metadata for cache/authority tests, without asserting measurement accuracy.
/// # Panics
/// Panics if valid fixture commands or strict metadata installation fail.
fn fixture(faces: bool, source: SourceReferenceId) -> Document {
    fixture_recipe(recipe(faces), source)
}

/// Installs one complete current recipe with bounded synthetic transfer metadata.
/// # Panics
/// Panics if current recipe installation, effective settings or strict artifact attachment fail.
fn fixture_recipe(recipe: PatternDefinitionRecipe, source: SourceReferenceId) -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 80.0,
            height: 60.0,
        },
        SourceReference::Assigned(source),
    )
    .unwrap();
    let original = base
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base.pattern_settings().clone(),
            base_definition: base.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let mut document = original
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::Density, 20.0),
            (PropertyFieldId::DensityAspect, 1.5),
        ])
        .unwrap()
        .bind(&original)
        .unwrap();
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
    let channels = document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for id in channels {
        document = document
            .with_saved_calibration(
                id,
                SavedCalibration::new(
                    CalibrationBinding::for_channel(&document, id).unwrap(),
                    raw.clone(),
                    None,
                )
                .unwrap(),
            )
            .unwrap();
    }
    let baseline =
        SavedCalibration::new(document.baseline_calibration_binding().unwrap(), raw, None).unwrap();
    document.with_saved_baseline_calibration(baseline).unwrap()
}

/// Creates the current scene-only request so no raster tolerance or fitting participates in this proof.
/// # Panics
/// Panics if the declared immutable PNG cannot form a valid current request.
fn request(document: &Document, source: &SourceReferenceId) -> EvaluationRequest {
    EvaluationRequest::new(
        DocumentSession::new(document.clone())
            .unwrap()
            .document_evaluation_snapshot(),
        ResolvedSource::new(
            source.clone(),
            include_bytes!("../../../assets/raster-sample.png").as_slice(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    )
    .for_scene()
    .unwrap()
}

/// Reserves selected skew-cell reach before both production output kinds, including guard zero.
/// # Panics
/// Panics if pre-generation uses nominal mark size or movement changes strict artifacts/responses.
#[test]
fn production_maze_and_guide_faces_reserve_complete_cell_reach_and_reuse_placement_authority() {
    for faces in [false, true] {
        let source = SourceReferenceId::new(format!("topology-coverage-{faces}")).unwrap();
        let original = fixture(faces, source.clone());
        let saved = original.saved_calibrations().to_vec();
        let baseline = original.baseline_saved_calibration().unwrap().cloned();
        let response = original.response_settings().clone();
        let mut moved = original.clone();
        for command in [
            DocumentCommand::SetTranslationAxis {
                channel_id: ChannelId(1),
                edited_axis: TranslationEditedAxis::X,
                value: -240.0,
            },
            DocumentCommand::SetTranslationAxis {
                channel_id: ChannelId(1),
                edited_axis: TranslationEditedAxis::Y,
                value: 180.0,
            },
        ] {
            moved = moved.apply_command(&command).unwrap().0;
        }
        moved = moved
            .apply_command(&DocumentCommand::SetChannelPatternRotationDelta {
                base: moved.pattern_settings().clone(),
                channel_id: ChannelId(1),
                rotation_degrees: 37.0,
            })
            .unwrap()
            .0;
        moved.require_saved_calibration_authority().unwrap();
        assert_eq!(saved, moved.saved_calibrations());
        assert_eq!(
            baseline.as_ref(),
            moved.baseline_saved_calibration().unwrap()
        );
        assert_eq!(&response, moved.response_settings());
        let limits = EvaluationLimits::new(1_000_000)
            .unwrap()
            .with_construction_mode(ConstructionMode::Complete);
        let mut cache = EvaluationProfileCache::default();
        let _initial =
            evaluate_profiled_cached_with_limits(request(&original, &source), limits, &mut cache)
                .unwrap();
        let cold =
            evaluate_profiled_cached_with_limits(request(&moved, &source), limits, &mut cache)
                .unwrap();
        let warm =
            evaluate_profiled_cached_with_limits(request(&moved, &source), limits, &mut cache)
                .unwrap();
        assert!(
            warm.diagnostics
                .channels
                .iter()
                .all(|channel| channel.family == CacheDisposition::Hit
                    && channel.realization == CacheDisposition::Hit)
        );
        let effective = moved.effective_channel_pattern(ChannelId(1)).unwrap();
        let family = cold.result.family_output(ChannelId(1)).unwrap();
        let reach = toniator_patterns::straight_topology_cell_reach(
            family.family(),
            moved.canvas(),
            &effective.resolved_density,
            effective.pattern_rotation_degrees,
            &family.family().site_selection,
        )
        .unwrap();
        assert!(family.planned_support_radius() >= reach + 3.0);
        if faces {
            let definition = moved
                .pattern_definition_bundles()
                .iter()
                .find(|bundle| bundle.id == effective.definition_id)
                .unwrap();
            let PatternOutputRealization::Regions {
                source:
                    RegionSourceIntent::GuideFaces {
                        guide_mechanism_id,
                        dimensions,
                    },
            } = &definition.definition.output_layers[0].realization
            else {
                unreachable!()
            };
            let complete = toniator_patterns::build_guide_faces_cancellable(
                toniator_patterns::GuideFaceRequest {
                    output_layer_id: definition.definition.output_layers[0].id,
                    guide_mechanism_id: *guide_mechanism_id,
                    dimensions: dimensions.clone(),
                    paths: family.structural_path_set().unwrap().clone(),
                    canvas: toniator_patterns::Bounds::new(
                        toniator_patterns::Point2::new(0.0, 0.0),
                        toniator_patterns::Point2::new(80.0, 60.0),
                    )
                    .unwrap(),
                },
                toniator_patterns::GuideFaceLimits::default(),
                || false,
            )
            .unwrap();
            for y in [0.0, 30.0, 60.0] {
                for x in [0.0, 40.0, 80.0] {
                    assert!(
                        complete.regions.regions().iter().any(|region| {
                            let crosses = region
                                .ring
                                .segments()
                                .iter()
                                .map(|segment| {
                                    let a = segment.start();
                                    let b = segment.end();
                                    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
                                })
                                .collect::<Vec<_>>();
                            crosses.iter().all(|cross| *cross >= -1e-7)
                                || crosses.iter().all(|cross| *cross <= 1e-7)
                        }),
                        "straight Guide Faces miss ({x},{y}) after translation and rotation"
                    );
                }
            }
        }
        assert_eq!(family, warm.result.family_output(ChannelId(1)).unwrap());
        assert_eq!(original.saved_calibrations(), moved.saved_calibrations());
    }
}

/// Keeps positive-epsilon Maze and Voronoi populations exact while retaining exact-key warm reuse.
/// # Panics
/// Panics if a narrower request consumes a broader greedy merge population or changes cold output.
#[test]
fn positive_merge_topology_rejects_broader_family_cache_and_reuses_exact_population() {
    for maze in [false, true] {
        let family = PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Exact finite Voronoi population".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: [0.0, 5.0]
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
        let source = SourceReferenceId::new(format!("positive-merge-cache-{maze}")).unwrap();
        let recipe = if maze {
            PatternDefinitionRecipe::connected(PatternStructureRecipe::MazeWalls {
                definition: Box::new(family),
                program: MazeProgram {
                    algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                    seed: 31,
                },
                style: Default::default(),
            })
        } else {
            PatternDefinitionRecipe::regions(family)
        };
        let narrow = fixture_recipe(recipe, source.clone());
        // Seed the shared structural cache from a valid single-output connection document.
        // Its family IDs/mechanisms remain exact; output intent and numeric metadata are separate.
        let mut bundles = narrow.pattern_definition_bundles().to_vec();
        let selected = bundles
            .iter_mut()
            .find(|bundle| bundle.id == narrow.pattern_settings().definition_id)
            .unwrap();
        let site_mechanism_id = selected
            .definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::SelectedGuideIntersections { id, .. } => Some(*id),
                _ => None,
            })
            .unwrap();
        let effective = narrow.effective_channel_pattern(ChannelId(1)).unwrap();
        let plan =
            toniator_patterns::resolve_document_pattern_pipeline(&narrow, &selected.definition)
                .unwrap();
        let diameter = toniator_patterns::straight_topology_cell_reach(
            &plan.family,
            narrow.canvas(),
            &effective.resolved_density,
            effective.pattern_rotation_degrees,
            &plan.family.site_selection,
        )
        .unwrap();
        let extent = toniator_patterns::maximum_nominal_cell_diameter(
            &plan.family,
            narrow.canvas(),
            &effective.resolved_density,
        )
        .unwrap();
        let scale = DensityMetric2D::default_for_canvas(narrow.canvas())
            .unwrap()
            .density
            / effective.density.density;
        let maximum_distance = (1.5 * diameter + extent + 20.0) / (2.0 * scale);
        selected.definition.output_layers[0].realization =
            PatternOutputRealization::ConnectionPaths {
                selection: Default::default(),
                site_mechanism_id,
                program: ConnectionProgram::NearestLinks {
                    adjacency: ConnectionAdjacencyIntent {
                        maximum_degree: 2,
                        maximum_distance,
                    },
                },
                style: Default::default(),
            };
        selected.output_settings[0].response =
            PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            });
        let mut broad = Document::with_source_topology_and_authored_structures(
            narrow.id(),
            narrow.canvas().clone(),
            narrow.source().clone(),
            bundles,
            narrow.pattern_settings().clone(),
            narrow.channel_model().unwrap(),
            narrow.channel_topology().unwrap().clone(),
            narrow.authored_structures().to_vec(),
        )
        .unwrap();
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
        let channels = broad
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        for id in channels {
            let binding = CalibrationBinding::for_channel(&broad, id).unwrap();
            broad = broad
                .with_saved_calibration(
                    id,
                    SavedCalibration::new(binding, raw.clone(), None).unwrap(),
                )
                .unwrap();
        }
        let baseline =
            SavedCalibration::new(broad.baseline_calibration_binding().unwrap(), raw, None)
                .unwrap();
        broad = broad.with_saved_baseline_calibration(baseline).unwrap();
        broad.require_saved_calibration_authority().unwrap();
        narrow.require_saved_calibration_authority().unwrap();
        let limits = EvaluationLimits::new(1_000_000)
            .unwrap()
            .with_construction_mode(ConstructionMode::Complete);
        let mut cache = EvaluationProfileCache::default();
        let broad_result =
            evaluate_profiled_cached_with_limits(request(&broad, &source), limits, &mut cache)
                .unwrap();
        let narrowed =
            evaluate_profiled_cached_with_limits(request(&narrow, &source), limits, &mut cache)
                .unwrap();
        assert!(
            narrowed
                .diagnostics
                .channels
                .iter()
                .all(|channel| channel.family == CacheDisposition::Miss)
        );
        assert!(
            broad_result
                .result
                .family_output(ChannelId(1))
                .unwrap()
                .planned_support_radius()
                > narrowed
                    .result
                    .family_output(ChannelId(1))
                    .unwrap()
                    .planned_support_radius()
        );
        let mut cold_cache = EvaluationProfileCache::default();
        let cold = evaluate_profiled_cached_with_limits(
            request(&narrow, &source),
            limits,
            &mut cold_cache,
        )
        .unwrap();
        assert_eq!(
            narrowed.result.family_output(ChannelId(1)),
            cold.result.family_output(ChannelId(1))
        );
        assert_eq!(narrowed.result.scene(), cold.result.scene());
        let warm =
            evaluate_profiled_cached_with_limits(request(&narrow, &source), limits, &mut cache)
                .unwrap();
        assert!(
            warm.diagnostics
                .channels
                .iter()
                .all(|channel| channel.family == CacheDisposition::Hit
                    && channel.realization == CacheDisposition::Hit)
        );
        assert_eq!(warm.result.scene(), cold.result.scene());
    }
}
