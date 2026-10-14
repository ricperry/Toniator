//! Native immutable-frame evaluation and export using the two project-wide source inputs.
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use toniator_domain::*;
use toniator_engine::{
    export::{SequenceExportJob, SequenceExportOptions},
    *,
};
use toniator_io::{sequence::SequenceFormat, *};

/// Locates stage-owned native outputs without altering input sources or Presets.
///
/// # Panics
/// Panics if the current stage directory cannot be created.
fn stage() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-animation-response-20261013/native");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Binds one shipped measured Preset to an immutable source at its intrinsic native dimensions.
///
/// # Panics
/// Panics if current source/configuration authority cannot bind or validate.
fn fixture(name: &str, preset: &str) -> (Document, SourceBundle, ResolvedSource) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (file, width, height, format, hint) = match name {
        "raster" => (
            "raster-sample.png",
            1024,
            1024,
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
        ),
        _ => (
            "vector-sample.svg",
            900,
            620,
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
        ),
    };
    let id = SourceReferenceId::new(format!("ton018-animation-{name}")).unwrap();
    let bytes: Arc<[u8]> = fs::read(root.join("assets").join(file)).unwrap().into();
    let destination = Document::new_default_document(
        CanvasSpec {
            width: f64::from(width),
            height: f64::from(height),
        },
        SourceReference::Assigned(id.clone()),
    )
    .unwrap();
    let document = if preset == "triagrid-custom-shape-marks" {
        let loaded=load(&root.join(format!("target/validation/ton018-family-regression-20261013/native-projects/{preset}-{name}-f1.toniator"))).unwrap();
        DocumentConfiguration::capture(loaded.document())
            .bind(&destination)
            .unwrap()
    } else {
        load_document_preset(
            &root.join(format!("assets/document-presets/{preset}.toniator-preset")),
            &destination,
        )
        .unwrap()
        .bind(&destination)
        .unwrap()
    };
    let document = document
        .edit_all_channel_start_configuration(&[(PropertyFieldId::Density, 36.0)])
        .unwrap()
        .bind(&document)
        .unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        format,
        bytes.to_vec(),
        Some(file.into()),
    )
    .unwrap()])
    .unwrap();
    (
        document,
        sources,
        ResolvedSource::new(id, bytes, hint).unwrap(),
    )
}

/// Renders noncircular static/animated Shape rotation and continuous aspect with exact saved data.
///
/// # Panics
/// Panics on missing/swapped artifacts, stale clone privilege, failed native renders or exports.
/// This opt-in evidence check requires retained TON-018 current family matrix validation
/// artifacts; those files are not shipped source fixtures.
#[test]
#[ignore = "requires retained TON-018 current family matrix validation artifacts"]
fn native_rotating_diamonds_and_captured_exports_retain_start_companions() {
    let directory = stage().join("marks-final");
    fs::create_dir_all(&directory).unwrap();
    for name in ["raster", "vector"] {
        let (start, sources, source) = fixture(name, "triagrid-custom-shape-marks");
        start.require_saved_calibration_authority().unwrap();
        let first = start.channel_topology().unwrap().channels()[0].id;
        let start_angle = start
            .effective_channel_pattern(first)
            .unwrap()
            .shape_rotation_degrees;
        let rotated = start
            .edit_all_channel_start_configuration(&[(
                PropertyFieldId::ShapeRotationDegrees,
                start_angle + 45.0,
            )])
            .unwrap()
            .bind(&start)
            .unwrap();
        assert_eq!(
            rotated
                .effective_channel_pattern(first)
                .unwrap()
                .shape_rotation_degrees,
            start_angle + 45.0
        );
        let original_render = evaluate(EvaluationRequest::new(
            DocumentSession::new(start.clone())
                .unwrap()
                .document_evaluation_snapshot(),
            source.clone(),
        ))
        .unwrap();
        fs::write(
            directory.join(format!("{name}-angle0.png")),
            encode_png(original_render.raster()).unwrap(),
        )
        .unwrap();
        assert_eq!(rotated.saved_calibrations(), start.saved_calibrations());
        save(
            &directory.join(format!("{name}-static-shape45.toniator")),
            &rotated,
            &sources,
        )
        .unwrap();
        let evaluated = evaluate(EvaluationRequest::new(
            DocumentSession::new(rotated.clone())
                .unwrap()
                .document_evaluation_snapshot(),
            source.clone(),
        ))
        .unwrap();
        assert_ne!(
            encode_png(evaluated.raster()).unwrap(),
            encode_png(original_render.raster()).unwrap(),
            "noncircular orientation changes pixels"
        );
        assert_ne!(
            evaluated.scene(),
            original_render.scene(),
            "angle-only canonical geometry changes"
        );
        fs::write(
            directory.join(format!("{name}-static-shape45.png")),
            encode_png(evaluated.raster()).unwrap(),
        )
        .unwrap();
        match write_svg(evaluated.scene()) {
            Ok(svg) => {
                fs::write(directory.join(format!("{name}-static-shape45.svg")), svg).unwrap()
            }
            Err(error) => fs::write(
                directory.join(format!("{name}-static-shape45-svg-diagnostic.txt")),
                error.to_string(),
            )
            .unwrap(),
        }
        // An angle-only intermediate frame independently witnesses animated rotation.
        let angle_only = start
            .clone()
            .with_temporal_authority(
                ProjectTiming::new(
                    FrameRate::new(24, 1).unwrap(),
                    FrameRange::new(0, 9).unwrap(),
                ),
                vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: PropertyTarget::Document,
                    field: PropertyFieldId::ShapeRotationDegrees,
                    end: start.pattern_settings().shape_rotation_degrees + 360.0,
                    easing: Easing::Linear,
                })],
            )
            .unwrap();
        let intermediate = evaluate(EvaluationRequest::new(
            DocumentSession::new(angle_only.clone())
                .unwrap()
                .document_evaluation_snapshot_at_frame(1)
                .unwrap(),
            source.clone(),
        ))
        .unwrap();
        assert_ne!(
            encode_png(intermediate.raster()).unwrap(),
            encode_png(original_render.raster()).unwrap()
        );
        assert_eq!(
            angle_only
                .materialize_frame(1)
                .unwrap()
                .document()
                .saved_calibrations(),
            start.saved_calibrations()
        );
        fs::write(
            directory.join(format!("{name}-angle-only-frame1.png")),
            encode_png(intermediate.raster()).unwrap(),
        )
        .unwrap();
        let angle = start.pattern_settings().shape_rotation_degrees;
        let aspect = start.pattern_settings().density.aspect;
        let animated = start
            .clone()
            .with_temporal_authority(
                ProjectTiming::new(
                    FrameRate::new(24, 1).unwrap(),
                    FrameRange::new(0, 9).unwrap(),
                ),
                vec![
                    TemporalEndOverride::Scalar(ScalarEndOverride {
                        target: PropertyTarget::Document,
                        field: PropertyFieldId::ShapeRotationDegrees,
                        end: angle + 360.0,
                        easing: Easing::Linear,
                    }),
                    TemporalEndOverride::Scalar(ScalarEndOverride {
                        target: PropertyTarget::Document,
                        field: PropertyFieldId::DensityAspect,
                        end: aspect * 1.3,
                        easing: Easing::SmoothStep,
                    }),
                ],
            )
            .unwrap();
        assert!(animated.has_animated_shape_rotation().unwrap());
        let session = DocumentSession::new(animated.clone()).unwrap();
        for frame in [0, 1, 8] {
            let snapshot = session
                .document_evaluation_snapshot_at_frame(frame)
                .unwrap();
            snapshot.validate().unwrap();
            assert_eq!(
                snapshot.document().saved_calibrations(),
                start.saved_calibrations()
            );
            for entry in start.saved_calibrations() {
                assert_eq!(
                    snapshot.saved_calibration_for(entry.channel_id).unwrap(),
                    Some(&entry.artifact)
                );
            }
            if frame > 0 {
                assert!(DocumentSession::new(snapshot.document().clone()).is_err());
            }
            let result = evaluate(EvaluationRequest::new(snapshot, source.clone())).unwrap();
            fs::write(
                directory.join(format!("{name}-frame{frame}.png")),
                encode_png(result.raster()).unwrap(),
            )
            .unwrap();
            match write_svg(result.scene()) {
                Ok(svg) => {
                    fs::write(directory.join(format!("{name}-frame{frame}.svg")), svg).unwrap()
                }
                Err(error) => fs::write(
                    directory.join(format!("{name}-frame{frame}-svg-diagnostic.txt")),
                    error.to_string(),
                )
                .unwrap(),
            }
        }
        // Three captured frames exercise the actual shared export job and its materialized snapshot.
        let captured = animated
            .with_temporal_authority(
                ProjectTiming::new(
                    FrameRate::new(24, 1).unwrap(),
                    FrameRange::new(0, 3).unwrap(),
                ),
                vec![
                    TemporalEndOverride::Scalar(ScalarEndOverride {
                        target: PropertyTarget::Document,
                        field: PropertyFieldId::DensityAspect,
                        end: 1.2,
                        easing: Easing::Linear,
                    }),
                    TemporalEndOverride::Scalar(ScalarEndOverride {
                        target: PropertyTarget::Document,
                        field: PropertyFieldId::ShapeRotationDegrees,
                        end: 90.0,
                        easing: Easing::Linear,
                    }),
                ],
            )
            .unwrap();
        save(
            &directory.join(format!("{name}-animated.toniator")),
            &captured,
            &sources,
        )
        .unwrap();
        let format = SequenceFormat::Png;
        let result = SequenceExportJob::new(
            captured,
            sources,
            SequenceExportOptions {
                destination: directory.join(format!("{name}-sequence")),
                format,
                background: Some(RasterBackground::Transparent),
                target: None,
                antialiasing: RasterAntialiasing::On,
                limits: EvaluationLimits::default(),
            },
        )
        .unwrap()
        .run(MediaTools::default(), &AtomicBool::new(false), &|_| {})
        .unwrap();
        assert_eq!(result.frame_count, 3);
    }
}

/// Retains saved connected calibration through animated bias, while static edits require a fit.
///
/// # Panics
/// Panics if continuous bias loses its immutable assignment or an ordinary static clone validates.
#[test]
fn continuous_connected_bias_uses_start_saved_assignment() {
    for name in ["raster", "vector"] {
        let (start, _, source) = fixture(name, "skinny-wave-rgb");
        let channel = start.channel_topology().unwrap().channels()[0].id;
        let output = start
            .effective_channel_pattern(channel)
            .unwrap()
            .output_settings[0]
            .output_layer_id;
        let animated = start
            .clone()
            .with_temporal_authority(
                ProjectTiming::new(
                    FrameRate::new(24, 1).unwrap(),
                    FrameRange::new(0, 3).unwrap(),
                ),
                vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                    target: PropertyTarget::ChannelOutput(channel, output),
                    field: PropertyFieldId::CurveResponseBias,
                    end: 0.4,
                    easing: Easing::Linear,
                })],
            )
            .unwrap();
        let session = DocumentSession::new(animated).unwrap();
        let snapshot = session.document_evaluation_snapshot_at_frame(1).unwrap();
        assert_eq!(
            snapshot.saved_calibration_for(channel).unwrap(),
            start.saved_calibration_for(channel).unwrap()
        );
        assert!(snapshot.document().validate().is_err());
        let result = evaluate(EvaluationRequest::new(snapshot, source)).unwrap();
        fs::write(
            stage().join(format!("{name}-bias-frame1.png")),
            encode_png(result.raster()).unwrap(),
        )
        .unwrap();
        let edited = start
            .edit_channel_start_configuration(&[(
                PropertyTarget::ChannelOutput(channel, output),
                PropertyFieldId::CurveResponseBias,
                0.2,
            )])
            .unwrap()
            .bind(&start)
            .unwrap();
        assert!(edited.saved_calibration_for(channel).unwrap().is_none());
    }
}

/// Converts former raw intent with compatible angle-only donors while preserving independent owners.
///
/// # Panics
/// Panics if a missing assignment fits a new angle, existing owners swap, or ALL loses provenance.
#[test]
fn former_raw_conversion_uses_compatible_donors_only_for_missing_assignments() {
    let start = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 128.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let first = ChannelId(1);
    let second = ChannelId(2);
    let third = ChannelId(3);
    let original = start.saved_calibration_for(first).unwrap().unwrap().clone();
    let mut raw = original.raw().to_vec();
    raw[1].coverage += 1e-13;
    let distinct = SavedCalibration::with_profile(
        original.profile().into(),
        original.binding().clone(),
        raw,
        original.uniform().cloned(),
    )
    .unwrap();
    let rotated = start
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: start.pattern_settings().clone(),
            settings: DocumentPatternSettings {
                shape_rotation_degrees: 45.0,
                ..start.pattern_settings().clone()
            },
        })
        .unwrap()
        .0;
    let neutral = rotated
        .with_saved_calibration_authority(
            vec![
                AttachedCalibration {
                    channel_id: first,
                    artifact: original.clone(),
                },
                AttachedCalibration {
                    channel_id: second,
                    artifact: distinct.clone(),
                },
            ],
            None,
        )
        .unwrap();
    let pending = PendingRawPresetImport::new(
        DocumentConfiguration::capture(&neutral),
        DocumentResponseSettings::default(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let converted =
        prepare_pending_raw_preset(pending, &neutral, EvaluationLimits::default(), &|| false)
            .unwrap()
            .bind(&neutral)
            .unwrap();
    assert_eq!(
        converted.saved_calibration_for(first).unwrap(),
        Some(&original)
    );
    assert_eq!(
        converted.saved_calibration_for(second).unwrap(),
        Some(&distinct)
    );
    assert_eq!(
        converted.saved_calibration_for(third).unwrap(),
        Some(&original)
    );
    assert_eq!(
        converted.baseline_saved_calibration().unwrap(),
        Some(&original)
    );
    assert_eq!(
        converted
            .baseline_saved_calibration()
            .unwrap()
            .unwrap()
            .binding()
            .shape_rotation_degrees()
            .to_bits(),
        0.0f64.to_bits()
    );
}

/// Derives warning applicability from existing domain controls for pure regions and mixed marks.
///
/// # Panics
/// Panics if inert region Shape tracks warn or an active mark channel loses its full-turn warning.
#[test]
fn warning_uses_existing_shape_rotation_capability_in_mixed_documents() {
    let (region, _, _) = fixture("vector", "diamond-diamond-rgb");
    let overrides = vec![TemporalEndOverride::Scalar(ScalarEndOverride {
        target: PropertyTarget::Document,
        field: PropertyFieldId::ShapeRotationDegrees,
        end: 360.0,
        easing: Easing::Linear,
    })];
    let timing = ProjectTiming::new(
        FrameRate::new(24, 1).unwrap(),
        FrameRange::new(0, 3).unwrap(),
    );
    let animated = region
        .clone()
        .with_temporal_authority(timing.clone(), overrides.clone())
        .unwrap();
    assert!(!animated.has_animated_shape_rotation().unwrap());
    let channel = region.channel_topology().unwrap().channels()[0].id;
    let mut history = DocumentHistory::new(DocumentSession::new(region).unwrap());
    toniator_patterns::PresetRegistry::bundled()
        .apply_to_selected(&mut history, channel, "straight-grid-circles")
        .unwrap();
    let mixed = history
        .document()
        .clone()
        .with_temporal_authority(timing, overrides)
        .unwrap();
    assert!(mixed.has_animated_shape_rotation().unwrap());
}

/// Exports continuous-aspect SVG frames through captured immutable authority and final SVG joining.
///
/// # Panics
/// Panics if frame validation incorrectly requires fresh fits or SVG publication fails.
#[test]
fn captured_region_svg_export_uses_immutable_frame_authority() {
    let (start, sources, _) = fixture("vector", "diamond-diamond-rgb");
    let document = start
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(24, 1).unwrap(),
                FrameRange::new(0, 3).unwrap(),
            ),
            vec![TemporalEndOverride::Scalar(ScalarEndOverride {
                target: PropertyTarget::Document,
                field: PropertyFieldId::DensityAspect,
                end: 1.2,
                easing: Easing::Linear,
            })],
        )
        .unwrap();
    let result = SequenceExportJob::new(
        document,
        sources,
        SequenceExportOptions {
            destination: stage().join("region-aspect-svg-sequence"),
            format: SequenceFormat::Svg,
            background: None,
            target: None,
            antialiasing: RasterAntialiasing::On,
            limits: EvaluationLimits::default(),
        },
    )
    .unwrap()
    .run(MediaTools::default(), &AtomicBool::new(false), &|_| {})
    .unwrap();
    assert_eq!(result.frame_count, 3);
}

/// Populates default and Pattern Preset recipients explicitly without restoring implicit fanout.
///
/// # Panics
/// Panics if named publication touches another owner or ALL omits an active/baseline assignment.
#[test]
fn default_and_pattern_preset_publication_enumerate_assignment_recipients() {
    let source = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 128.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    source.require_saved_calibration_authority().unwrap();
    assert_eq!(source.saved_calibrations().len(), 3);
    let mut history = DocumentHistory::new(DocumentSession::new(source.clone()).unwrap());
    let registry = toniator_patterns::PresetRegistry::bundled();
    let record = registry.find("triagrid-custom-shape-marks").unwrap();
    registry
        .apply_to_selected(&mut history, ChannelId(1), "triagrid-custom-shape-marks")
        .unwrap();
    assert_eq!(
        history
            .document()
            .saved_calibration_for(ChannelId(1))
            .unwrap(),
        record.saved_calibration.as_ref()
    );
    for channel in [ChannelId(2), ChannelId(3)] {
        assert_eq!(
            history.document().saved_calibration_for(channel).unwrap(),
            source.saved_calibration_for(channel).unwrap()
        );
    }
    assert_eq!(
        history.document().baseline_saved_calibration().unwrap(),
        source.baseline_saved_calibration().unwrap()
    );
    registry
        .apply_to_document_base(&mut history, "triagrid-custom-shape-marks")
        .unwrap();
    history
        .document()
        .require_saved_calibration_authority()
        .unwrap();
    for entry in history.document().saved_calibrations() {
        assert_eq!(Some(&entry.artifact), record.saved_calibration.as_ref());
    }
    assert_eq!(
        history.document().baseline_saved_calibration().unwrap(),
        record.saved_calibration.as_ref()
    );
}
