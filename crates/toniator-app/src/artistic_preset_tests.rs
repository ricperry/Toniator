//! Current artistic milestone preparation witnesses use the real app worker and guarded publication.
use super::*;
use crate::*;
use std::{fs, time::Instant};
use toniator_domain::{CalibrationBinding, SavedCalibration};

/// Returns the phase-owned artifact directory, requiring an explicit absolute destination.
/// # Panics
/// Panics if the caller omits the evidence destination or directory creation fails.
fn output() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("TONIATOR_ARTISTIC_PHASE5").expect("explicit evidence root"),
    );
    assert!(path.is_absolute());
    fs::create_dir_all(&path).unwrap();
    path
}

/// Supplies only genuinely prepared exact-binding companions collected earlier in this run.
/// # Panics
/// Panics if typed binding reconstruction or validated assignment fails.
fn reuse(document: Document, pool: &[SavedCalibration]) -> (Document, usize) {
    let mut document = document;
    let mut reused = 0;
    for channel in authoritative_channel_ids(&document) {
        if document.saved_calibration_for(channel).unwrap().is_none() {
            let binding = CalibrationBinding::for_channel(&document, channel).unwrap();
            if let Some(artifact) = pool
                .iter()
                .find(|artifact| artifact.binding().is_compatible_with(&binding))
            {
                document = document
                    .with_saved_calibration(channel, artifact.clone())
                    .unwrap();
                reused += 1;
            }
        }
    }
    if document.baseline_saved_calibration().unwrap().is_none() {
        let binding = document.baseline_calibration_binding().unwrap();
        if let Some(artifact) = pool
            .iter()
            .find(|artifact| artifact.binding().is_compatible_with(&binding))
        {
            document = document
                .with_saved_baseline_calibration(artifact.clone())
                .unwrap();
            reused += 1;
        }
    }
    (document, reused)
}

/// Records validated current companions without replacing an earlier exact-binding measurement.
/// # Panics
/// Panics if the supplied document cannot reconstruct its current baseline authority.
fn remember(document: &Document, pool: &mut Vec<SavedCalibration>) {
    for artifact in document
        .saved_calibrations()
        .iter()
        .map(|entry| &entry.artifact)
        .chain(document.baseline_saved_calibration().unwrap())
    {
        if !pool.iter().any(|old| old.binding() == artifact.binding()) {
            pool.push(artifact.clone());
        }
    }
}

/// Prepares all shipped archives through the real app helper, preserving both source kinds and history.
/// Genuine exact-binding reuse is explicit; synthetic artistic fixtures never enter this test.
/// # Panics
/// Panics on any strict fitter, source, endpoint, archive, history or persistence regression.
#[test]
#[ignore = "explicit real calibration and native evidence stage"]
fn artistic_all14_prepare_and_apply() {
    let root = output();
    let mut pool = Vec::new();
    let mut records = Vec::new();
    let mut failures = Vec::new();
    assert_eq!(builtin_document_presets().len(), 14);
    for preset in builtin_document_presets().iter().filter(|preset| {
        std::env::var("TONIATOR_ARTISTIC_PRESET_ID").is_ok_and(|id| id == preset.id)
            || std::env::var_os("TONIATOR_ARTISTIC_PRESET_ID").is_none()
    }) {
        let start = Instant::now();
        let bytes = preset.archive_bytes().to_vec();
        let mut raster = Workspace::from_direct(
            include_bytes!("../../../assets/raster-sample.png")
                .as_slice()
                .into(),
            SourceFormatHint::Png,
            "Raster".into(),
        )
        .unwrap();
        raster.accept_saved_snapshot(PathBuf::from("destination.toniator"), raster.snapshot());
        let before = raster.snapshot();
        let staged = preset.load_staged(raster.document()).unwrap();
        let origin = match &staged {
            StagedDocumentPreset::Current(_) => "current",
            StagedDocumentPreset::PendingCalibration(_) => "coverage",
            StagedDocumentPreset::PendingRaw(_) => "raw",
        };
        let configuration = match staged {
            StagedDocumentPreset::Current(value)
            | StagedDocumentPreset::PendingCalibration(value) => value,
            StagedDocumentPreset::PendingRaw(value) => {
                match toniator_engine::prepare_pending_raw_preset(
                    value,
                    raster.document(),
                    toniator_engine::EvaluationLimits::default(),
                    &|| false,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        failures.push(format!("{} raw: {error}", preset.id));
                        continue;
                    }
                }
            }
        };
        let (candidate, reused) = reuse(configuration.bind(raster.document()).unwrap(), &pool);
        let expected = DocumentConfiguration::capture(&candidate);
        let prepared = match prepare_configuration(
            raster.document(),
            expected.clone(),
            &AtomicBool::new(false),
        ) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("{}: {error}", preset.id));
                records.push(serde_json::json!({"id":preset.id,"error":error,"seconds":start.elapsed().as_secs_f64()}));
                fs::write(
                    root.join("presets.json"),
                    serde_json::to_vec_pretty(&records).unwrap(),
                )
                .unwrap();
                continue;
            }
        };
        let capture = LoadCapture {
            base: raster.document().clone(),
            revision: raster.history.revision(),
        };
        assert!(apply_configuration(&mut raster, &capture, &prepared).unwrap());
        raster
            .document()
            .require_saved_calibration_authority()
            .unwrap();
        assert_eq!(raster.sources, before.sources);
        assert_eq!(raster.document().canvas(), before.document.canvas());
        assert_eq!(raster.document().source(), before.document.source());
        assert_eq!(raster.document().id(), before.document.id());
        assert_eq!(raster.location, Some(PathBuf::from("destination.toniator")));
        // Preparation attaches measurements; it never changes desired start/end response or authored recipes.
        let mut without = raster
            .document()
            .clone()
            .with_saved_calibration_authority(
                candidate.saved_calibrations().to_vec(),
                candidate.baseline_saved_calibration().unwrap().cloned(),
            )
            .unwrap();
        assert_eq!(DocumentConfiguration::capture(&without), expected);
        without = raster.document().clone();
        let after = raster.snapshot();
        raster.history.undo().unwrap();
        assert_eq!(raster.document(), &before.document);
        raster.history.redo().unwrap();
        assert_eq!(raster.snapshot(), after);
        let path = root.join(format!("{}-raster.toniator", preset.id));
        save_container(&path, raster.document(), &raster.sources).unwrap();
        let reopened = Workspace::from_container(&path).unwrap();
        assert_eq!(reopened.document(), raster.document());
        assert_eq!(reopened.sources, raster.sources);
        remember(&without, &mut pool);
        let mut vector = Workspace::from_direct(
            include_bytes!("../../../assets/vector-sample.svg")
                .as_slice()
                .into(),
            SourceFormatHint::Svg,
            "Vector".into(),
        )
        .unwrap();
        let source = vector.snapshot();
        let repeat =
            prepare_configuration(vector.document(), prepared.clone(), &AtomicBool::new(false))
                .unwrap();
        assert_eq!(repeat, prepared);
        let capture = LoadCapture {
            base: vector.document().clone(),
            revision: vector.history.revision(),
        };
        assert!(apply_configuration(&mut vector, &capture, &repeat).unwrap());
        assert_eq!(vector.sources, source.sources);
        assert_eq!(vector.document().canvas(), source.document.canvas());
        assert_eq!(vector.document().source(), source.document.source());
        save_container(
            &root.join(format!("{}-vector.toniator", preset.id)),
            vector.document(),
            &vector.sources,
        )
        .unwrap();
        assert_eq!(preset.archive_bytes(), bytes);
        records.push(serde_json::json!({"id":preset.id,"archive_sha256":preset.archive_sha256,"origin":origin,"genuine_pool_assignments_reused":reused,"pool_bindings":pool.len(),"seconds":start.elapsed().as_secs_f64(),"profiles":raster.document().saved_calibrations().iter().map(|entry|entry.artifact.profile()).collect::<Vec<_>>(),"both_intrinsic_sources":true,"history_save_reopen":true}));
        fs::write(
            root.join("presets.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        eprintln!(
            "{} prepared in {:.3}s",
            preset.id,
            start.elapsed().as_secs_f64()
        );
    }
    fs::write(
        root.join("failures.json"),
        serde_json::to_vec_pretty(&failures).unwrap(),
    )
    .unwrap();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Reopens genuine prepared catalog outputs and proves the opposite-source repeat schedules no fits.
/// # Panics
/// Panics if a completed catalog artifact is absent, unmeasured, or source-specific in binding identity.
#[test]
#[ignore = "runs after actual catalog preparation"]
fn artistic_prepared_catalog_reopens_without_fresh_fits() {
    let root = output();
    for preset in builtin_document_presets() {
        for source in ["raster", "vector"] {
            let workspace =
                Workspace::from_container(&root.join(format!("{}-{source}.toniator", preset.id)))
                    .unwrap();
            workspace
                .document()
                .require_saved_calibration_authority()
                .unwrap();
            assert!(
                crate::calibration_preparation::plan_to_prepare(
                    workspace.document(),
                    &authoritative_channel_ids(workspace.document()),
                    false,
                    false
                )
                .unwrap()
                .is_empty()
            );
            let capture = DocumentConfiguration::capture(workspace.document());
            let prepared = prepare_configuration(
                workspace.document(),
                capture.clone(),
                &AtomicBool::new(false),
            )
            .unwrap();
            assert_eq!(prepared, capture);
        }
    }
}

/// Fits each affected gallery producer through the same missing-companion preparation used by Apply.
/// # Panics
/// Panics on strict fit failure, wrong producer identity, or unprepared publication authority.
#[test]
#[ignore = "explicit real gallery calibration"]
fn artistic_gallery_prepares_affected_producers() {
    let root = output();
    let registry = toniator_patterns::PresetRegistry::bundled();
    let mut records = Vec::new();
    let mut failures = Vec::new();
    let mut pool = Vec::new();
    for preset in registry
        .entries()
        .iter()
        .filter(|preset| preset.saved_calibration.is_none())
        .filter(|preset| {
            std::env::var("TONIATOR_ARTISTIC_GALLERY_ID").is_ok_and(|id| id == preset.metadata.id)
                || std::env::var_os("TONIATOR_ARTISTIC_GALLERY_ID").is_none()
        })
    {
        let workspace = Workspace::from_direct(
            include_bytes!("../../../assets/raster-sample.png")
                .as_slice()
                .into(),
            SourceFormatHint::Png,
            "Raster".into(),
        )
        .unwrap();
        let base = workspace.document();
        let candidate = base
            .apply_command(
                &toniator_domain::DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                    base: base.pattern_settings().clone(),
                    base_definition: base.pattern_definition_bundles()[0].definition.clone(),
                    recipe: preset.recipe.clone(),
                },
            )
            .unwrap()
            .0;
        let (candidate, reused) = reuse(candidate, &pool);
        let start = Instant::now();
        match prepare_configuration(
            base,
            DocumentConfiguration::capture(&candidate),
            &AtomicBool::new(false),
        ) {
            Ok(prepared) => {
                let document = prepared.bind(base).unwrap();
                document.require_saved_calibration_authority().unwrap();
                remember(&document, &mut pool);
                save_container(
                    &root.join(format!("gallery-{}.toniator", preset.metadata.id)),
                    &document,
                    &workspace.sources,
                )
                .unwrap();
                records.push(serde_json::json!({"id":preset.metadata.id,"fit":"passed","seconds":start.elapsed().as_secs_f64(),"genuine_pool_assignments_reused":reused,"pool_bindings":pool.len(),"profiles":document.saved_calibrations().iter().map(|entry|entry.artifact.profile()).collect::<Vec<_>>()}));
            }
            Err(error) => {
                failures.push(format!("{}: {error}", preset.metadata.id));
                records.push(serde_json::json!({"id":preset.metadata.id,"fit":"failed","error":error,"seconds":start.elapsed().as_secs_f64()}));
            }
        }
        fs::write(
            root.join("gallery.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        eprintln!(
            "gallery {} {:.3}s",
            preset.metadata.id,
            start.elapsed().as_secs_f64()
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Replaces all synthetic fixture companions with actual strict current fits on intrinsic canvases.
/// Produces ordinary projects and native exports for private GTK, never modifies previous evidence.
/// # Panics
/// Panics if fitting, intrinsic binding, saved authority, or normal export fails.
#[test]
#[ignore = "explicit genuine GUI fixtures"]
fn artistic_genuine_gui_fixtures() {
    let root = output().join("gui");
    fs::create_dir_all(&root).unwrap();
    let mut pool = Vec::new();
    let mut records = Vec::new();
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (kind, prior) in [
        ("tone", "phase5/cleared-inputs/tone.toniator"),
        ("dots", "phase5/cleared-inputs/dots.toniator"),
        ("connections", "phase5/cleared-inputs/connections.toniator"),
        ("tree", "phase5/cleared-inputs/tree.toniator"),
    ] {
        let previous = Workspace::from_container(
            &repository
                .join("target/validation/artistic-geometry-20261019")
                .join(prior),
        )
        .unwrap();
        let cleared = previous
            .document()
            .clone()
            .with_saved_calibration_authority(vec![], None)
            .unwrap();
        for (source, bytes, hint) in [
            (
                "raster",
                include_bytes!("../../../assets/raster-sample.png").as_slice(),
                SourceFormatHint::Png,
            ),
            (
                "vector",
                include_bytes!("../../../assets/vector-sample.svg").as_slice(),
                SourceFormatHint::Svg,
            ),
        ] {
            let mut workspace = Workspace::from_direct(bytes.into(), hint, source.into()).unwrap();
            let candidate = DocumentConfiguration::capture(&cleared)
                .bind(workspace.document())
                .unwrap();
            let (candidate, reused) = reuse(candidate, &pool);
            let start = Instant::now();
            let prepared = prepare_configuration(
                workspace.document(),
                DocumentConfiguration::capture(&candidate),
                &AtomicBool::new(false),
            )
            .unwrap();
            let capture = LoadCapture {
                base: workspace.document().clone(),
                revision: workspace.history.revision(),
            };
            apply_configuration(&mut workspace, &capture, &prepared).unwrap();
            workspace
                .document()
                .require_saved_calibration_authority()
                .unwrap();
            remember(workspace.document(), &mut pool);
            let stem = format!("{source}-{kind}");
            save_container(
                &root.join(format!("{stem}.toniator")),
                workspace.document(),
                &workspace.sources,
            )
            .unwrap();
            for (format, extension) in [(ExportFormat::Png, "png"), (ExportFormat::Svg, "svg")] {
                export_snapshot_frame(
                    workspace.snapshot(),
                    root.join(format!("{stem}.{extension}")),
                    ExportSettings {
                        format,
                        background: RasterBackground::Transparent,
                        antialiasing: RasterAntialiasing::On,
                        output_target: None,
                        prepare_for_print: false,
                        pixels_per_metre: None,
                        cleanup: print_cleanup::Settings::default(),
                    },
                    0,
                )
                .unwrap();
            }
            records.push(serde_json::json!({"fixture":stem,"synthetic_assignments_cleared":true,"seconds":start.elapsed().as_secs_f64(),"genuine_pool_assignments_reused":reused,"profiles":workspace.document().saved_calibrations().iter().map(|entry|entry.artifact.profile()).collect::<Vec<_>>()}));
            fs::write(
                root.join("preparation.json"),
                serde_json::to_vec_pretty(&records).unwrap(),
            )
            .unwrap();
            eprintln!("genuine {stem} {:.3}s", start.elapsed().as_secs_f64());
        }
    }
}

/// Projects tone help from effective channel appearance rather than the dormant shared base.
/// # Panics
/// Panics if ALL/named help diverges from authored Feature size or hides mixed channel spacing.
#[test]
fn artistic_tone_help_tracks_effective_feature_size() {
    let base = Document::new_default_document(
        toniator_domain::CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        toniator_domain::SourceReference::Unassigned,
    )
    .unwrap();
    let recipe = toniator_patterns::PresetRegistry::bundled()
        .find("one-guide-lines")
        .unwrap()
        .recipe
        .clone();
    let base = base
        .apply_command(
            &toniator_domain::DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base: base.pattern_settings().clone(),
                base_definition: base.pattern_definition_bundles()[0].definition.clone(),
                recipe,
            },
        )
        .unwrap()
        .0;
    let definition = base
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.pattern_settings().definition_id)
        .unwrap()
        .definition
        .clone();
    let base = base
        .apply_command(
            &toniator_domain::DocumentCommand::EditSharedPatternDefinition {
                definition_id: definition.id,
                base_definition: definition.clone(),
                edit: toniator_domain::PatternDefinitionEdit::SetToneSampleSpacing {
                    output_layer_id: definition.output_layers[0].id,
                    value: toniator_domain::ToneSampleSpacing { multiplier: 2.0 },
                },
            },
        )
        .unwrap()
        .0;
    let first = base
        .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, 25.0)])
        .unwrap()
        .bind(&base)
        .unwrap();
    let help = |document: &Document, target| {
        tone_spacing_help(
            document,
            target,
            PropertyFieldId::ToneSampleSpacing,
            &PropertyCurrentValueKind::FiniteF64(2.0),
            String::new(),
        )
    };
    assert!(help(&first, InspectorTarget::DocumentAll).contains("81.920000"));
    let second = first
        .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, 12.5)])
        .unwrap()
        .bind(&first)
        .unwrap();
    assert!(help(&second, InspectorTarget::DocumentAll).contains("163.840000"));
    let id = authoritative_channel_ids(&second)[0];
    let target = second
        .channel_scalar_batch(PropertyFieldId::Density)
        .unwrap()
        .values
        .into_iter()
        .find(|value| matches!(value.target, PropertyTarget::Channel(channel) | PropertyTarget::ChannelOutput(channel, _) if channel == id))
        .unwrap()
        .target;
    let mixed = second
        .edit_channel_start_configuration(&[(target, PropertyFieldId::Density, 25.0)])
        .unwrap()
        .bind(&second)
        .unwrap();
    assert!(help(&mixed, InspectorTarget::DocumentAll).contains("varies"));
    assert!(help(&mixed, InspectorTarget::Channel(id)).contains("81.920000"));
    let definition = second.pattern_definition_for(id).unwrap().clone();
    let different = second
        .apply_command(
            &toniator_domain::DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: id,
                base_definition: definition.clone(),
                edit: toniator_domain::PatternDefinitionEdit::SetToneSampleSpacing {
                    output_layer_id: definition.output_layers[0].id,
                    value: toniator_domain::ToneSampleSpacing { multiplier: 3.0 },
                },
            },
        )
        .unwrap()
        .0;
    assert!(help(&different, InspectorTarget::DocumentAll).contains("varies"));
    assert!(help(&different, InspectorTarget::Channel(id)).contains("245.760000"));
}

/// Builds explicit ALL/named/End desired-coverage intent without changing connection binding.
/// # Panics
/// Panics if current response or additive temporal authority cannot express the fixture.
#[test]
#[ignore = "requires genuine intrinsic fixture"]
fn artistic_endpoint_fixture() {
    let root = output();
    let workspace = Workspace::from_container(&root.join("gui/raster-dots.toniator")).unwrap();
    let base = workspace.document();
    let all = base
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::ConnectedMinimumThickness, 0.13),
            (PropertyFieldId::ConnectedMaximumThickness, 0.77),
        ])
        .unwrap()
        .bind(base)
        .unwrap();
    let target = all
        .channel_scalar_batch(PropertyFieldId::ConnectedMaximumThickness)
        .unwrap()
        .values[0]
        .target;
    let named = all
        .edit_channel_start_configuration(&[(
            target,
            PropertyFieldId::ConnectedMaximumThickness,
            0.68,
        )])
        .unwrap()
        .bind(&all)
        .unwrap();
    let document = named
        .with_temporal_authority(
            toniator_domain::ProjectTiming::new(
                toniator_domain::FrameRate::new(24, 1).unwrap(),
                toniator_domain::FrameRange::new(0, 3).unwrap(),
            ),
            vec![toniator_domain::TemporalEndOverride::Scalar(
                toniator_domain::ScalarEndOverride {
                    target,
                    field: PropertyFieldId::ConnectedMaximumThickness,
                    end: -0.24,
                    easing: toniator_domain::Easing::SmoothStep,
                },
            )],
        )
        .unwrap()
        .with_saved_calibration_authority(vec![], None)
        .unwrap();
    let path = root.join("endpoint-current.toniator-preset");
    save_document_preset(
        &path,
        &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&path).unwrap(),
    )
    .unwrap();
}

/// Prepares the converted desired endpoint fixture through the actual app helper without reinterpretation.
/// # Panics
/// Panics if ALL/named/End changes during strict measurement or genuine publication fails.
#[test]
#[ignore = "explicit converted endpoint strict preparation"]
fn artistic_converted_endpoint_prepares() {
    let root = output();
    let workspace = Workspace::from_container(&root.join("gui/raster-dots.toniator")).unwrap();
    let staged = load_document_preset_staged(
        &root.join("endpoint-schema16.toniator-preset"),
        workspace.document(),
    )
    .unwrap();
    let StagedDocumentPreset::PendingCalibration(configuration) = staged else {
        panic!("desired coverage requires calibration only")
    };
    let expected = configuration.bind(workspace.document()).unwrap();
    let prepared = prepare_configuration(
        workspace.document(),
        configuration.clone(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let document = prepared.bind(workspace.document()).unwrap();
    document.require_saved_calibration_authority().unwrap();
    let stripped = document
        .clone()
        .with_saved_calibration_authority(
            expected.saved_calibrations().to_vec(),
            expected.baseline_saved_calibration().unwrap().cloned(),
        )
        .unwrap();
    assert_eq!(DocumentConfiguration::capture(&stripped), configuration);
    let batch = document
        .channel_scalar_batch(PropertyFieldId::ConnectedMaximumThickness)
        .unwrap();
    assert!(
        batch
            .values
            .iter()
            .any(|value| (value.value - 0.68).abs() < 1e-12)
    );
    assert!(
        batch
            .values
            .iter()
            .any(|value| (value.value - 0.77).abs() < 1e-12)
    );
    assert_eq!(
        document.temporal_end_overrides(),
        expected.temporal_end_overrides()
    );
    assert!(!document.temporal_end_overrides().is_empty());
    save_container(
        &root.join("endpoint-genuine.toniator"),
        &document,
        &workspace.sources,
    )
    .unwrap();
}

/// Keeps Advanced tone locators and effective help valid across named copy-on-edit.
/// # Panics
/// Panics if an accepted typed Advanced edit loses its retained locator or resolved spacing.
#[test]
#[ignore = "uses genuine saved Phase5 GUI fixture"]
fn artistic_advanced_tone_locator_refreshes() {
    let workspace = Workspace::from_container(
        &std::env::var_os("TONIATOR_ARTISTIC_ADVANCED_FIXTURE")
            .map(PathBuf::from)
            .unwrap_or_else(|| output().join("gui/raster-tone.toniator")),
    )
    .unwrap();
    let mut history = DocumentHistory::new_draft(&workspace.history);
    let channel = authoritative_channel_ids(history.document())[0];
    let target = InspectorTarget::Channel(channel);
    let current = advanced_settings_values(history.document(), target)
        .into_iter()
        .find(|value| value.descriptor.field == PropertyFieldId::ToneSampleSpacing)
        .unwrap();
    let locator =
        advanced_descriptor_locator(history.document(), target, &current.descriptor).unwrap();
    assert!(
        advanced_temporal::apply_input(
            &mut history,
            temporal_preview::Endpoint::Start,
            target,
            locator,
            InspectorInput::FiniteF64(3.0)
        )
        .unwrap()
    );
    let current = resolve_current_advanced_value(history.document(), target, locator).unwrap();
    assert_eq!(current.value, PropertyCurrentValueKind::FiniteF64(3.0));
    let help = tone_spacing_help(
        history.document(),
        target,
        current.descriptor.field,
        &current.value,
        String::new(),
    );
    let spacing = toniator_domain::ToneSampleSpacing { multiplier: 3.0 }
        .resolve(
            history.document().canvas(),
            &history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .density,
        )
        .unwrap();
    assert!(help.contains(&format!("{spacing:.6}")), "{help}");
}

/// Prepares a changed Advanced tone binding before one atomic publication and exact Undo/Redo.
/// # Panics
/// Panics if missing-only preparation skips the changed producer, loses authority, or splits history.
#[test]
#[ignore = "measures the actual changed Advanced tone producer"]
fn artistic_advanced_apply_prepares_before_publication() {
    let root = output();
    let mut workspace = Workspace::from_container(&root.join("gui/raster-tone.toniator")).unwrap();
    let before = workspace.document().clone();
    let mut draft = DocumentHistory::new_draft(&workspace.history);
    let channel = authoritative_channel_ids(draft.document())[0];
    let target = InspectorTarget::Channel(channel);
    let current = advanced_settings_values(draft.document(), target)
        .into_iter()
        .find(|value| value.descriptor.field == PropertyFieldId::ToneSampleSpacing)
        .unwrap();
    let locator =
        advanced_descriptor_locator(draft.document(), target, &current.descriptor).unwrap();
    advanced_temporal::apply_input(
        &mut draft,
        temporal_preview::Endpoint::Start,
        target,
        locator,
        InspectorInput::FiniteF64(3.0),
    )
    .unwrap();
    assert!(
        draft
            .document()
            .require_saved_calibration_authority()
            .is_err()
    );
    assert_eq!(workspace.document(), &before);
    let channels = authoritative_channel_ids(draft.document());
    let plan = calibration_preparation::plan_to_prepare(draft.document(), &channels, false, false)
        .unwrap();
    assert_eq!(plan.active, vec![channel]);
    assert!(!plan.baseline);
    let artifacts = calibration_preparation::prepare(
        draft.document(),
        &channels,
        false,
        false,
        &AtomicBool::new(false),
    )
    .unwrap();
    attach_calibrations_to_history(&mut draft, artifacts).unwrap();
    draft
        .document()
        .require_saved_calibration_authority()
        .unwrap();
    assert!(
        calibration_preparation::plan_to_prepare(draft.document(), &channels, false, false)
            .unwrap()
            .is_empty()
    );
    workspace.history.squash_draft(&draft).unwrap();
    let after = workspace.document().clone();
    assert_ne!(after, before);
    workspace.history.undo().unwrap();
    assert_eq!(workspace.document(), &before);
    workspace.history.redo().unwrap();
    assert_eq!(workspace.document(), &after);
    save_container(
        &root.join("advanced-tone-genuine.toniator"),
        &after,
        &workspace.sources,
    )
    .unwrap();
}

/// Reopens the actual private-GTK Advanced Apply result with complete current calibration authority.
/// # Panics
/// Panics if the saved named tone edit loses its genuine fit or changes the other output.
#[test]
#[ignore = "requires the completed native Advanced Apply witness"]
fn artistic_native_advanced_saved_authority() {
    let root = output();
    let workspace = Workspace::from_container(
        &root
            .parent()
            .unwrap()
            .join("gtk/advanced-fixed-work.toniator"),
    )
    .unwrap();
    workspace
        .document()
        .require_saved_calibration_authority()
        .unwrap();
    let values: Vec<_> = authoritative_channel_ids(workspace.document())
        .into_iter()
        .map(|channel| {
            advanced_settings_values(workspace.document(), InspectorTarget::Channel(channel))
                .into_iter()
                .find(|value| value.descriptor.field == PropertyFieldId::ToneSampleSpacing)
                .unwrap()
                .value
        })
        .collect();
    assert_eq!(values[0], PropertyCurrentValueKind::FiniteF64(3.0));
    assert!(
        values[1..]
            .iter()
            .all(|value| *value == PropertyCurrentValueKind::FiniteF64(1.0))
    );
}
