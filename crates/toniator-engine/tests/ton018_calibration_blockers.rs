//! Focused calibration-blocker regressions without personal files or historical artifacts.
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
use toniator_domain::*;
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{FrozenCoverageProbe, calibrate_saved_binding, reference_document},
};
use toniator_io::{self as io, StagedDocumentPreset};
use toniator_patterns::PresetRegistry;
use toniator_render::VectorOccupancyPrecision;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Supplies a current-stage-only output directory for hermetic persistence witnesses.
///
/// # Panics
/// Panics if the validation directory cannot be created.
fn stage() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-calibration-blockers-20261013/tests");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Reconstructs the observed 35/90-degree circle lattice without a personal Preset dependency.
fn hex_recipe() -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::marks(PatternStructureRecipe::OrderedOutputs {
        definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Hex regression".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: [35.0, 90.0]
                .into_iter()
                .map(|angle| GuideDimensionDraft {
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                })
                .collect(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 0.0,
            },
            orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
        }),
        outputs: vec![PatternOutputRealizationRecipe::Marks],
    })
}

/// Reconstructs the authored cubic and perpendicular stack with exact observed control points.
///
/// # Panics
/// Panics if the finite authored regression resource cannot pass current domain validation.
fn skinny_recipe() -> PatternDefinitionRecipe {
    let point = |x, y| AuthoredPoint2 { x, y };
    let resource = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: point(-450.0, 0.0),
            control_1: point(-155.33362563775512, -278.8863141196115),
            control_2: point(156.9770408163265, 261.4760527318838),
            end: point(450.0, 0.0),
        }],
    )
    .unwrap();
    PatternDefinitionRecipe::connected(PatternStructureRecipe::AuthoredResources {
        resources: vec![resource],
        definition: Box::new(PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(PatternStructureRecipe::GenericGuides {
                name: "Skinny regression".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
                dimensions: vec![GenericGuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                        resource_index: 0,
                    },
                    repetition: GuideRepetition::TransformStack {
                        direction_degrees: 90.0,
                        spacing_multiplier: 1.0,
                    },
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.0,
                },
                orientation: MarkOrientationDraft::Fixed,
            }),
            outputs: vec![PatternOutputRealizationRecipe::StructuralPaths {
                style: Default::default(),
            }],
        }),
    })
}

/// Attaches one exact shared binding to active channels and the hidden ALL companion.
///
/// # Panics
/// Panics if this single-recipe regression document has inconsistent channel or ALL bindings.
fn attach(document: Document, artifact: &SavedCalibration) -> Document {
    let channels = document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    let mut document = document;
    for channel in channels {
        document = document
            .with_saved_calibration(channel, artifact.clone())
            .unwrap();
    }
    document
        .with_saved_baseline_calibration(artifact.clone())
        .unwrap()
}

/// Validates the real lattice through strict population/low-tone checks and current persistence.
///
/// # Panics
/// Panics if steep saturation cannot fit truthfully, or its greater-than-128-knot artifact changes on reload.
#[test]
fn hex_steep_saturation_fits_and_roundtrips_without_relaxed_accuracy() {
    let recipe = hex_recipe();
    let (document, _, channel) = reference_document(&recipe, 32).unwrap();
    let binding = CalibrationBinding::for_channel(&document, channel).unwrap();
    let artifact =
        calibrate_saved_binding(&binding, EvaluationLimits::default(), &|| false).unwrap();
    assert!(artifact.raw().len() > 128);
    assert_eq!(artifact.profile(), CALIBRATION_PROFILE);
    let document = attach(document, &artifact);
    let path = stage().join("hex-131-knots.toniator-preset");
    io::save_document_preset(
        &path,
        &DocumentConfiguration::capture(&document),
        &io::capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
    assert_eq!(
        io::load_document_preset(&path, &document)
            .unwrap()
            .bind(&document)
            .unwrap(),
        document
    );
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "hex-regression".into(),
            name: "Hex regression".into(),
            category: "Regression".into(),
            description: "Measured steep saturation".into(),
            thumbnail: None,
        },
        recipe,
        saved_calibration: Some(artifact.clone()),
    };
    let pattern = stage().join("hex-131-knots.toniator-pattern");
    io::save_preset(&pattern, &record).unwrap();
    assert_eq!(
        io::load_preset(&pattern).unwrap().saved_calibration,
        Some(artifact)
    );
}

/// Checks the corrected authoritative producer at default and quarter-Feature-equivalent populations.
///
/// # Panics
/// Panics if Feature-dependent padding changes coverage beyond the unchanged strict transfer gate.
#[test]
fn authored_curve_preserves_default_fine_transfer_and_saved_provenance() {
    let recipe = skinny_recipe();
    let (document, source, channel) = reference_document(&recipe, 32).unwrap();
    let binding = CalibrationBinding::for_channel(&document, channel).unwrap();
    let artifact =
        calibrate_saved_binding(&binding, EvaluationLimits::default(), &|| false).unwrap();
    assert!(artifact.profile().ends_with("+guide-span-v2"));
    let first = FrozenCoverageProbe::new(
        &document,
        source,
        channel,
        EvaluationLimits::default(),
        &|| false,
    )
    .unwrap();
    let (fine, fine_source, fine_channel) = reference_document(&recipe, 128).unwrap();
    let fine = FrozenCoverageProbe::new(
        &fine,
        fine_source,
        fine_channel,
        EvaluationLimits::default(),
        &|| false,
    )
    .unwrap();
    for size in [0.01, 0.1, 0.5, 1.0] {
        let default = first
            .measure_at(size, VectorOccupancyPrecision::Tight, &|| false)
            .unwrap()
            .coverage;
        let smaller = fine
            .measure_at(size, VectorOccupancyPrecision::Tight, &|| false)
            .unwrap()
            .coverage;
        assert!(
            (default - smaller).abs() < 0.0025,
            "raw={size} default={default} fine={smaller}"
        );
    }
    assert!(
        SavedCalibration::with_profile(
            CALIBRATION_PROFILE.into(),
            binding,
            artifact.raw().to_vec(),
            None
        )
        .is_err()
    );
}

/// Rewrites only one JSON archive entry for a current-stage malformed/obsolete companion witness.
///
/// # Panics
/// Panics if the hermetic ZIP/JSON fixture cannot be read or written.
fn rewrite_preset(
    source: &std::path::Path,
    destination: &std::path::Path,
    change: impl FnOnce(&mut serde_json::Value),
) {
    let mut bytes = Vec::new();
    ZipArchive::new(fs::File::open(source).unwrap())
        .unwrap()
        .by_name("preset.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut value = serde_json::from_slice(&bytes).unwrap();
    change(&mut value);
    let mut archive = ZipWriter::new(fs::File::create(destination).unwrap());
    archive
        .start_file("preset.json", SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    archive.finish().unwrap();
}

/// Drops only formerly valid affected optional companions while preserving authored and temporal intent.
///
/// # Panics
/// Panics if a source-free conversion touches bytes/intent, removes unrelated authority, or accepts a future profile.
#[test]
fn revised_geometry_companions_recover_selectively_and_projects_stay_current_only() {
    let neutral = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("blocker-provenance").unwrap()),
    )
    .unwrap();
    let mut document = neutral
        .apply_command(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: neutral.pattern_settings().clone(),
                channel_id: ChannelId(1),
                base_definition: neutral.pattern_definition_bundles()[0].definition.clone(),
                recipe: skinny_recipe(),
            },
        )
        .unwrap()
        .0;
    let binding = CalibrationBinding::for_channel(&document, ChannelId(1)).unwrap();
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
    let artifact = SavedCalibration::new(binding.clone(), raw, None).unwrap();
    document = document
        .with_saved_calibration(ChannelId(1), artifact.clone())
        .unwrap();
    let base = document.response_settings().clone();
    let mut response = base.clone();
    response.connected_minimum_thickness = 0.19;
    response.connected_maximum_thickness = 0.71;
    document = document
        .apply_command(&DocumentCommand::SetAllResponseSettings {
            base,
            settings: response,
        })
        .unwrap()
        .0;
    let initialized = document.initialize_end_command().unwrap();
    document = document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(24, 1).unwrap(),
                FrameRange::new(0, 2).unwrap(),
            ),
            initialized.replacement().end_overrides.clone(),
        )
        .unwrap();
    let output = document
        .effective_channel_pattern(ChannelId(1))
        .unwrap()
        .output_settings[0]
        .output_layer_id;
    let end = document
        .edit_effective_end_command(&[TemporalEndpointEdit {
            target: PropertyTarget::ChannelOutput(ChannelId(1), output),
            field: PropertyFieldId::ConnectedMaximumThickness,
            effective_end: 0.83,
            easing: Easing::SmoothStep,
        }])
        .unwrap();
    document = document
        .with_temporal_authority(
            end.replacement().project_timing.clone(),
            end.replacement().end_overrides.clone(),
        )
        .unwrap();
    document.require_saved_calibration_authority().unwrap();
    let current = stage().join("mixed-current.toniator-preset");
    io::save_document_preset(
        &current,
        &DocumentConfiguration::capture(&document),
        &io::capture_document_preset_destination(&current).unwrap(),
    )
    .unwrap();
    let former = stage().join("mixed-former.toniator-preset");
    rewrite_preset(&current, &former, |value| {
        for companion in value["configuration"]["saved_calibrations"]
            .as_array_mut()
            .unwrap()
        {
            if companion["profile"]
                .as_str()
                .unwrap()
                .ends_with("+guide-span-v2")
            {
                companion["profile"] = CALIBRATION_PROFILE.into();
            }
        }
    });
    let original = fs::read(&former).unwrap();
    let StagedDocumentPreset::Current(configuration) =
        io::load_document_preset_staged(&former, &neutral).unwrap()
    else {
        panic!("desired coverage stays current");
    };
    let restored = configuration.bind(&neutral).unwrap();
    assert!(
        restored
            .saved_calibration_for(ChannelId(1))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        restored.saved_calibration_for(ChannelId(2)).unwrap(),
        document.saved_calibration_for(ChannelId(2)).unwrap()
    );
    assert_eq!(
        restored.baseline_saved_calibration().unwrap(),
        document.baseline_saved_calibration().unwrap()
    );
    assert_eq!(
        restored.pattern_definition_bundles(),
        document.pattern_definition_bundles()
    );
    assert_eq!(restored.response_settings(), document.response_settings());
    assert_eq!(
        restored.temporal_end_overrides(),
        document.temporal_end_overrides()
    );
    assert_eq!(fs::read(&former).unwrap(), original);
    let future = stage().join("mixed-future.toniator-preset");
    rewrite_preset(&current, &future, |value| {
        value["configuration"]["saved_calibrations"][0]["profile"] = "future-producer-v99".into();
    });
    assert!(io::load_document_preset_staged(&future, &neutral).is_err());
    let project = stage().join("mixed-current.toniator");
    io::save(
        &project,
        &document,
        &io::SourceBundle::new([io::EmbeddedSource::new(
            SourceReferenceId::new("blocker-provenance").unwrap(),
            io::EmbeddedSourceFormat::Svg,
            b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1024\" height=\"1024\"/>".to_vec(),
            None,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let bytes = fs::read(&project).unwrap();
    let mut source = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let obsolete = stage().join("mixed-obsolete.toniator");
    let mut target = ZipWriter::new(fs::File::create(&obsolete).unwrap());
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "document.json" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace("+guide-span-v2", "")
                .into_bytes();
        }
        target
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        target.write_all(&bytes).unwrap();
    }
    target.finish().unwrap();
    assert!(
        io::load(&obsolete)
            .unwrap_err()
            .to_string()
            .contains("calibration.profile")
    );
    let record = PresetRecord {
        metadata: PresetMetadata {
            id: "skinny-migration".into(),
            name: "Skinny migration".into(),
            category: "Regression".into(),
            description: "Geometry provenance".into(),
            thumbnail: None,
        },
        recipe: skinny_recipe(),
        saved_calibration: Some(artifact),
    };
    let path = stage().join("skinny-optional.toniator-pattern");
    io::save_preset(&path, &record).unwrap();
    let former_pattern = stage().join("skinny-former.toniator-pattern");
    fs::write(
        &former_pattern,
        String::from_utf8(fs::read(&path).unwrap())
            .unwrap()
            .replace("+guide-span-v2", ""),
    )
    .unwrap();
    let bytes = fs::read(&former_pattern).unwrap();
    let restored = io::load_preset(&former_pattern).unwrap();
    assert_eq!(restored.recipe, record.recipe);
    assert!(restored.saved_calibration.is_none());
    assert_eq!(fs::read(&former_pattern).unwrap(), bytes);
    assert!(!binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE_GUIDE_FACES));
}

/// Round-trips marked stochastic metadata and rejects mismatched producer provenance.
///
/// # Panics
/// Panics if mean metadata reaches deterministic dispatch or a formerly illegal profile is discarded.
#[test]
fn marked_mean_companion_roundtrips_with_typed_reference_eligibility() {
    let mut record = PresetRegistry::bundled()
        .entries()
        .iter()
        .find(|record| record.metadata.id == "source-weighted-dispersion-voronoi")
        .unwrap()
        .clone();
    let PatternGeometryResponse::Regions(response) = &mut record.recipe.output_settings[0].response
    else {
        panic!("region recipe");
    };
    response.algorithm = RegionResizeAlgorithm::UniformOffset;
    let binding = CalibrationBinding::new(record.recipe.clone(), 1.0, 0.0).unwrap();
    assert!(binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE_MEAN));
    assert!(!binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE_GUIDE_FACES));
    let variation = CalibrationVariation::new(0.01, 0.01, 0.001, 0.05, None, None).unwrap();
    record.saved_calibration = Some(
        SavedCalibration::with_mean_profile(
            binding.clone(),
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 1.0,
                },
            ],
            None,
            variation,
        )
        .unwrap(),
    );
    let path = stage().join("marked-mean.toniator-pattern");
    io::save_preset(&path, &record).unwrap();
    assert_eq!(io::load_preset(&path).unwrap(), record);
    let bad = stage().join("mismatched-mean.toniator-pattern");
    fs::write(
        &bad,
        String::from_utf8(fs::read(&path).unwrap())
            .unwrap()
            .replace(CALIBRATION_PROFILE_MEAN, CALIBRATION_PROFILE_GUIDE_FACES),
    )
    .unwrap();
    assert!(io::load_preset(&bad).is_err());
}

/// Requires canonical combined producer provenance and rejects partial or foreign markers.
///
/// # Panics
/// Panics if either corrected producer can reuse an obsolete or incompletely marked artifact.
#[test]
fn combined_producer_markers_require_exact_typed_provenance() {
    let PatternStructureRecipe::AuthoredResources {
        resources,
        definition,
    } = skinny_recipe().structure
    else {
        panic!("authored root");
    };
    let PatternStructureRecipe::OrderedOutputs { mut definition, .. } = *definition else {
        panic!("ordered guide root");
    };
    let PatternStructureRecipe::GenericGuides { dimensions, .. } = definition.as_mut() else {
        panic!("generic guides");
    };
    let mut second = dimensions[0].clone();
    second.baseline_angle_degrees = 90.0;
    dimensions.push(second);
    let mut recipe = PatternDefinitionRecipe::guide_faces(*definition, vec![0, 1]);
    recipe.structure = PatternStructureRecipe::AuthoredResources {
        resources,
        definition: Box::new(recipe.structure),
    };
    let PatternGeometryResponse::Regions(response) = &mut recipe.output_settings[0].response else {
        panic!("region response");
    };
    response.algorithm = RegionResizeAlgorithm::UniformOffset;
    let binding = CalibrationBinding::new(recipe, 1.0, 0.0).unwrap();
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
    let artifact = SavedCalibration::new(binding.clone(), raw.clone(), None).unwrap();
    assert_eq!(
        artifact.profile(),
        format!("{CALIBRATION_PROFILE_GUIDE_FACES}+guide-span-v2+convex-inset-v3")
    );
    assert!(binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE_GUIDE_FACES));
    assert!(!binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE));
    assert!(!binding.is_obsolete_geometry_profile(CALIBRATION_PROFILE_MEAN));
    for suffix in [
        "+guide-span-v2",
        "+convex-inset-v3",
        "+convex-inset-v3+guide-span-v2",
        "+future-v99",
    ] {
        assert!(
            SavedCalibration::with_profile(
                format!("{CALIBRATION_PROFILE_GUIDE_FACES}{suffix}"),
                binding.clone(),
                raw.clone(),
                None
            )
            .is_err()
        );
    }
}

/// Produces the canonical zero-rank authored guide through the public typed family path.
///
/// # Errors
/// Returns current recipe, family, coverage, or missing structural-guide diagnostics.
fn derived_authored_guide(
    recipe: &PatternDefinitionRecipe,
    side: u32,
    support: f64,
) -> Result<toniator_patterns::CurvePath, Box<dyn std::error::Error>> {
    let (document, _, channel) = reference_document(recipe, side)?;
    let effective = document.effective_channel_pattern(channel)?;
    let definition = &document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == effective.definition_id)
        .ok_or("definition missing")?
        .definition;
    let channel = document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .find(|value| value.id == channel)
        .unwrap();
    let plan = toniator_patterns::resolve_document_pattern_pipeline_for_channel(
        &document,
        definition,
        channel.weighting,
    )?;
    let request = toniator_patterns::GridInspectRequest {
        canvas: document.canvas().clone(),
        density: effective.resolved_density,
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: definition.coverage.guard_steps,
        support_radius: support,
        max_family_candidates: EvaluationLimits::default().max_family_candidates(),
    };
    let family = toniator_patterns::evaluate_typed_family_product_cancellable(
        &plan.family,
        &request,
        &|| false,
    )?;
    Ok(family
        .structural_path_set()
        .ok_or("guide paths missing")?
        .paths()
        .iter()
        .find(|path| path.id.repetition_index == 0)
        .ok_or("zero-rank guide missing")?
        .path
        .clone())
}

/// Keeps original scaled cubic controls and feasible derivative continuity across Feature padding.
///
/// # Panics
/// Panics if actual family generation changes the core or leaves a canvas-edge coverage gap.
#[test]
fn authored_guide_core_and_tangent_continuity_stay_fixed_across_feature() {
    let recipe = skinny_recipe();
    let mut core = None;
    for side in [32, 64, 128] {
        let path = derived_authored_guide(&recipe, side, 1024.0 / f64::from(side)).unwrap();
        assert_eq!(path.segments().len(), 3);
        assert!(path.start().x < 0.0 && path.end().x > 1024.0);
        if let Some(core) = core {
            assert_eq!(path.segments()[1], core);
        } else {
            core = Some(path.segments()[1]);
        }
        for pair in path.segments().windows(2) {
            let incoming = pair[0].limiting_unit_tangent_at(1.0).unwrap();
            let outgoing = pair[1].limiting_unit_tangent_at(0.0).unwrap();
            assert!((incoming.dot(outgoing) - 1.0).abs() < 1.0e-12);
        }
    }
}

/// Bounds near-vertical tangent fallback and rejects overflowing coverage through the real producer.
///
/// # Panics
/// Panics if endpoint coverage moves the core, grows an unbounded ray, or silently accepts overflow.
#[test]
fn authored_guide_tangent_fallback_keeps_core_and_finite_edge_coverage() {
    let mut recipe = skinny_recipe();
    let PatternStructureRecipe::AuthoredResources { resources, .. } = &mut recipe.structure else {
        panic!("authored root");
    };
    resources[0] = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: AuthoredPoint2 { x: -450.0, y: 0.0 },
            control_1: AuthoredPoint2 { x: -450.0, y: 50.0 },
            control_2: AuthoredPoint2 { x: 450.0, y: 50.0 },
            end: AuthoredPoint2 { x: 450.0, y: 0.0 },
        }],
    )
    .unwrap();
    let first = derived_authored_guide(&recipe, 32, 32.0).unwrap();
    let fine = derived_authored_guide(&recipe, 128, 8.0).unwrap();
    assert_eq!(first.segments()[1], fine.segments()[1]);
    assert!(first.start().x < 0.0 && first.end().x > 1024.0);
    assert!(fine.start().x < 0.0 && fine.end().x > 1024.0);
    assert!(derived_authored_guide(&recipe, 32, f64::MAX).is_err());
}
