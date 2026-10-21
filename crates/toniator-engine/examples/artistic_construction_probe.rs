//! Counter-off paired complete/canvas current production recipes with separate construction diagnostics.
use serde_json::json;
use std::{error::Error, fs, path::PathBuf, time::Instant};
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::{ConstructionContext, ConstructionMode};
#[allow(dead_code)]
#[path = "artistic_connection_probe.rs"]
mod connection_fixture;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Builds the fixed-grid, authored-mark or full exterior curve recipe without source authority.
/// # Errors
/// Returns current authored resource/recipe validation failures.
fn recipe(kind: &str) -> Result<PatternDefinitionRecipe> {
    if kind == "round" || kind == "motif" {
        return tone_recipe(kind, 1.);
    }
    let structure = PatternStructureRecipe::StraightGrid(PatternDefinitionDraft {
        name: format!("Construction {kind}"),
        coverage: CoveragePolicy {
            guard_steps: if kind == "marks" { 12 } else { 2 },
            additional_margin: 0.,
        },
    });
    if kind != "marks" {
        return Ok(PatternDefinitionRecipe::marks(structure));
    }
    let vertices = (0..96)
        .map(|i| {
            let theta = std::f64::consts::TAU * i as f64 / 96.;
            AuthoredPoint2 {
                x: theta.cos(),
                y: theta.sin(),
            }
        })
        .collect::<Vec<_>>();
    let segments = (0..96)
        .map(|i| AuthoredCurveSegment::Line {
            start: vertices[i],
            end: vertices[(i + 1) % 96],
        })
        .collect();
    Ok(PatternDefinitionRecipe::marks(
        PatternStructureRecipe::AuthoredResources {
            resources: vec![AuthoredStructureDraft::new(
                AuthoredStructureKind::ClosedShape,
                segments,
            )?],
            definition: Box::new(PatternStructureRecipe::AuthoredClosedShapeMarks {
                definition: Box::new(structure),
                resource_index: 0,
            }),
        },
    ))
}
/// Resolves fresh intrinsic-size current intent and a declared synthetic transfer, never a measured fit.
/// # Errors
/// Returns document, recipe or exact recipient calibration binding failures.
pub(crate) fn fixture(kind: &str, canvas: CanvasSpec, id: SourceReferenceId) -> Result<Document> {
    if kind == "connection" {
        return connection_fixture::fixture(
            "nearest",
            ConnectionSelectionIntent {
                maximum_final_degree: Some(2),
                cycles: ConnectionCyclePolicy::Forbid,
                minimum_component_points: 1,
            },
            80.,
            canvas,
            HalftoneChannelModel::Cmyk,
            id,
        );
    }
    let base = Document::new_default_document(canvas, SourceReference::Assigned(id))?;
    let mut history = DocumentHistory::new(DocumentSession::new(base.clone())?);
    history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: base.pattern_settings().clone(),
        base_definition: base.pattern_definition_bundles()[0].definition.clone(),
        recipe: recipe(kind)?,
    })?;
    let properties = if kind == "marks" || kind == "grid" {
        vec![
            (
                PropertyFieldId::Density,
                if kind == "marks" { 35. } else { 90. },
            ),
            (PropertyFieldId::RotationDegrees, 45.),
            (PropertyFieldId::MarkMinimumFill, 0.4),
            (PropertyFieldId::MarkMaximumFill, 0.6),
        ]
    } else {
        vec![
            (PropertyFieldId::Density, 25.),
            (PropertyFieldId::ConnectedMinimumThickness, 0.25),
            (PropertyFieldId::ConnectedMaximumThickness, 0.75),
        ]
    };
    let mut document = history
        .document()
        .edit_all_channel_start_configuration(&properties)?
        .bind(history.document())?;
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
    let channels = document
        .channel_topology()
        .ok_or("missing topology")?
        .channels()
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    for channel in channels {
        let artifact = SavedCalibration::new(
            CalibrationBinding::for_channel(&document, channel)?,
            raw.clone(),
            None,
        )?;
        document = document.with_saved_calibration(channel, artifact)?;
    }
    let artifact = SavedCalibration::new(document.baseline_calibration_binding()?, raw, None)?;
    Ok(document.with_saved_baseline_calibration(artifact)?)
}
/// Builds full globally prepared tone/curve intent including exterior turns and motifs.
/// # Errors
/// Returns authored finite recipe validation errors.
fn tone_recipe(kind: &str, multiplier: f64) -> Result<PatternDefinitionRecipe> {
    let coverage = CoveragePolicy {
        guard_steps: 2,
        additional_margin: 0.0,
    };
    let tone_sample_spacing = ToneSampleSpacing { multiplier };
    let style = PathStrokeStyle::default();
    let structure = if kind == "round" || kind == "square" {
        PatternStructureRecipe::ParametricCurve {
            name: format!("Tone {kind}"),
            coverage,
            curve: ParametricCurve::Spiral(SpiralCurve {
                shape: if kind == "round" {
                    SpiralShape::Round
                } else {
                    SpiralShape::Square
                },
                turns: 18.375,
                radial_spacing: 64.0,
                phase_degrees: 37.0,
                winding: CurveWinding::Clockwise,
            }),
            spiral_coverage: SpiralCoveragePolicy::Fixed,
            repetition: GuideRepetition::Single,
            sites: None,
        }
    } else {
        PatternStructureRecipe::GeneralizedStraightGuides {
            name: format!("Tone {kind}"),
            coverage,
            dimensions: vec![GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.125,
                spacing_multiplier: 1.0,
            }],
            product: GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices: vec![0],
                interval_multiplier: 1.0,
                phase: 0.25,
            },
            orientation: MarkOrientationDraft::Fixed,
        }
    };
    if kind == "motif" || kind == "curved-guide" {
        let path = AuthoredStructureDraft::new(
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                control_1: AuthoredPoint2 { x: 0.25, y: 0.6 },
                control_2: AuthoredPoint2 { x: 0.75, y: -0.6 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            }],
        )?;
        if kind == "motif" {
            return Ok(PatternDefinitionRecipe::connected(
                PatternStructureRecipe::AuthoredResources {
                    resources: vec![path],
                    definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
                        tone_sample_spacing,
                        definition: Box::new(structure),
                        resource_index: 0,
                        style,
                        mirror_alternate_rows: true,
                        alternate_row_phase: Some(0.25),
                    }),
                },
            ));
        }
        let guide = AuthoredStructureDraft::new(
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 { x: -50.0, y: 0.0 },
                control_1: AuthoredPoint2 { x: -25.0, y: 35.0 },
                control_2: AuthoredPoint2 { x: 25.0, y: -35.0 },
                end: AuthoredPoint2 { x: 50.0, y: 0.0 },
            }],
        )?;
        return Ok(PatternDefinitionRecipe::connected(
            PatternStructureRecipe::AuthoredResources {
                resources: vec![guide],
                definition: Box::new(PatternStructureRecipe::OrderedOutputs {
                    definition: Box::new(PatternStructureRecipe::GenericGuides {
                        name: "Tone curved guide".into(),
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
                        tone_sample_spacing,
                        style,
                    }],
                }),
            },
        ));
    }
    Ok(PatternDefinitionRecipe::connected(
        PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(structure),
            outputs: vec![PatternOutputRealizationRecipe::StructuralPaths {
                tone_sample_spacing,
                style,
            }],
        },
    ))
}

/// Evaluates native, larger PNG, preview, streaming PNG and SVG through one ordinary accepted cache.
/// # Errors
/// Propagates current full pipeline/consumer failures before evidence publication.
fn pipeline(
    request: &EvaluationRequest,
    limits: EvaluationLimits,
    cache: &mut EvaluationProfileCache,
    native_only: bool,
) -> Result<(EvaluationResult, Vec<u8>, String)> {
    let native = evaluate_profiled_cached_with_limits(
        request
            .clone()
            .for_output(RasterBackground::Transparent, None, RasterAntialiasing::On),
        limits,
        cache,
    )?
    .result;
    if native_only {
        return Ok((native, Vec::new(), String::new()));
    }
    let target = OutputRasterTarget::new(
        (native.scene().canvas().width * 1.25) as u32,
        (native.scene().canvas().height * 1.125) as u32,
    )?;
    let larger = toniator_render::rasterize_output(
        native.scene(),
        RasterBackground::Transparent,
        Some(target),
        RasterAntialiasing::On,
    )?;
    let preview = evaluate_profiled_cached_with_limits(
        request
            .clone()
            .for_preview(PreviewRasterTarget::new(320, 240)?),
        limits,
        cache,
    )?
    .result;
    let mut stream = Vec::new();
    toniator_render::write_png_streaming(
        native.scene(),
        toniator_render::StreamingRasterTarget::new(337, 219)?,
        toniator_render::PngStreamOptions {
            maximum_band_rows: 31,
            ..Default::default()
        },
        Default::default(),
        &mut stream,
        &|| false,
        &|_, _| {},
    )?;
    let svg = write_svg(native.scene())?;
    std::hint::black_box((larger, preview));
    Ok((native, stream, svg))
}
/// Runs one warmup and five cold/warm measured end-to-end pairs with diagnostic counters opt-in only.
/// # Errors
/// Returns source, pipeline or evidence I/O failures; immutable intrinsic assets remain unchanged.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let mode = if args.get(1).map(String::as_str) == Some("canvas") {
        ConstructionMode::Canvas
    } else {
        ConstructionMode::Complete
    };
    let source = args.get(2).map(String::as_str).unwrap_or("raster");
    let kind = args.get(3).map(String::as_str).unwrap_or("marks");
    let diagnostic = args.get(4).map(String::as_str).unwrap_or("timing");
    let measured = matches!(diagnostic, "metrics" | "native-metrics");
    let native_only = matches!(diagnostic, "native" | "native-metrics");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join(format!(
        "target/validation/artistic-geometry-20261019/phase4/final/{source}-{kind}/{mode:?}/{}",
        match (native_only, measured) {
            (false, false) => "timing",
            (false, true) => "metrics",
            (true, false) => "native-timing",
            (true, true) => "native-metrics",
        }
    ));
    fs::create_dir_all(&out)?;
    let (input, hint, canvas) = if source == "vector" {
        (
            "vector-sample.svg",
            SourceFormatHint::Svg,
            CanvasSpec {
                width: 900.,
                height: 620.,
            },
        )
    } else {
        (
            "raster-sample.png",
            SourceFormatHint::Png,
            CanvasSpec {
                width: 1024.,
                height: 1024.,
            },
        )
    };
    let id = SourceReferenceId::new(format!("construction-{source}-{kind}"))?;
    let document = fixture(kind, canvas.clone(), id.clone())?;
    let bytes = fs::read(root.join("assets").join(input))?;
    let bundle = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
        id.clone(),
        if source == "vector" {
            toniator_io::EmbeddedSourceFormat::Svg
        } else {
            toniator_io::EmbeddedSourceFormat::Png
        },
        bytes.clone(),
        Some(input.into()),
    )?])?;
    toniator_io::save(&out.join("recipe.toniator"), &document, &bundle)?;
    let request = EvaluationRequest::new(
        DocumentSession::new(document)?.document_evaluation_snapshot(),
        ResolvedSource::new(id, bytes, hint)?,
    );
    let limits = EvaluationLimits::default().with_construction_mode(mode);
    let context = ConstructionContext::new(mode, None, measured);
    let mut times = Vec::new();
    let iterations = if measured { 1 } else { 6 };
    for iteration in 0..iterations {
        let mut cache = EvaluationProfileCache::default();
        let start = Instant::now();
        let cold = context.run(|| pipeline(&request, limits, &mut cache, native_only))?;
        let cold_seconds = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let warm = context.run(|| pipeline(&request, limits, &mut cache, native_only))?;
        let warm_seconds = start.elapsed().as_secs_f64();
        assert_eq!(cold.0.raster().pixels(), warm.0.raster().pixels());
        if iteration > 0 || measured {
            times.push(json!({"cold_seconds":cold_seconds,"warm_seconds":warm_seconds}));
        }
        if iteration == iterations - 1 {
            fs::write(out.join("native.rgba"), cold.0.raster().pixels())?;
            fs::write(out.join("native.png"), encode_png(cold.0.raster())?)?;
            fs::write(out.join("stream.png"), &cold.1)?;
            fs::write(out.join("native.svg"), &cold.2)?;
        }
    }
    let record = json!({"source":input,"case":kind,"mode":format!("{mode:?}"),"canvas":[canvas.width,canvas.height],"counter_instrumented":measured,"scope":if native_only{"native evaluation"}else{"native + largerPNG + preview + stream + SVG"},"warmup_pairs":if measured{0}else{1},"measured_pairs":times,"construction":context.measurements.as_ref().map(|v|v.snapshot()),"transfer":"synthetic fixture transfer; not measured calibration","native_rgba":"unchanged unflattened"});
    fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    println!("{record}");
    Ok(())
}
