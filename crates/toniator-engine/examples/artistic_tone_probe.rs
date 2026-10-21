//! Current native tone-detail fixtures; synthetic transfer isolates cadence from coverage refitting.
use std::{error::Error, fs, path::PathBuf};
use serde_json::json;
use toniator_domain::*;
use toniator_engine::*;
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Builds a current curve recipe with explicit output-owned artistic spacing.
/// # Errors
/// Returns authored-resource validation errors without writing product data.
fn recipe(kind: &str, multiplier: f64) -> Result<PatternDefinitionRecipe> {
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
                turns: 5.375,
                radial_spacing: 24.0,
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

/// Materializes a current recipe through history and binds a declared synthetic monotone transfer.
/// This fixture transfer is not a measured calibration. Equal transfers isolate artistic cadence.
/// # Errors
/// Returns domain/source/recipe/calibration validation errors before any native output is written.
pub(crate) fn fixture(
    kind: &str,
    multiplier: f64,
    source_id: SourceReferenceId,
) -> Result<Document> {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 512.0,
            height: 384.0,
        },
        SourceReference::Assigned(source_id),
    )?;
    let mut history = DocumentHistory::new(DocumentSession::new(base.clone())?);
    history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: base.pattern_settings().clone(),
        base_definition: base.pattern_definition_bundles()[0].definition.clone(),
        recipe: recipe(kind, multiplier)?,
    })?;
    let mut document = history.document().clone();
    document = document
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::Density, 25.0),
            (PropertyFieldId::ConnectedMinimumThickness, 0.0),
            (PropertyFieldId::ConnectedMaximumThickness, 0.75),
        ])?
        .bind(&document)?;
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
        .ok_or("modeled channels missing")?
        .channels()
        .iter()
        .map(|channel| channel.id)
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

/// Writes exact current preview/native transparent RGBA PNG and editable SVG for both immutable inputs.
/// Optional `fit` mode separately exercises strict real calibration without changing fixture transfers.
/// # Errors
/// Returns current source/domain/evaluation/export or stage-artifact I/O errors; inputs remain immutable.
fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/validation/artistic-geometry-20261019/phase2/native");
    fs::create_dir_all(&out)?;
    if std::env::args().nth(1).as_deref() == Some("fit") {
        for multiplier in [0.5, 1.0, 2.0] {
            let document = fixture(
                "straight-guide",
                multiplier,
                SourceReferenceId::new("tone-fit")?,
            )?;
            let binding = document.baseline_calibration_binding()?;
            let started = std::time::Instant::now();
            let result = reference_calibration::calibrate_saved_binding(
                &binding,
                EvaluationLimits::default(),
                &|| false,
            );
            let record = match result {
                Ok(artifact) => {
                    json!({"multiplier":multiplier,"seconds":started.elapsed().as_secs_f64(),"fit":"passed","artifact":format!("{artifact:#?}")})
                }
                Err(error) => {
                    json!({"multiplier":multiplier,"seconds":started.elapsed().as_secs_f64(),"fit":"failed","path":error.path(),"message":error.message()})
                }
            };
            fs::write(
                out.join(format!("strict-fit-{multiplier}.json")),
                serde_json::to_vec_pretty(&record)?,
            )?;
            println!("{record}");
        }
        return Ok(());
    }
    for (source_name, input, format, hint) in [
        (
            "raster",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
        ),
        (
            "vector",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
        ),
    ] {
        let bytes = fs::read(root.join("assets").join(input))?;
        for kind in ["round", "square", "straight-guide", "curved-guide", "motif"] {
            for multiplier in [0.5, 1.0, 2.0] {
                let stem = format!("{source_name}-{kind}-{multiplier}");
                let id = SourceReferenceId::new(format!("tone-{source_name}"))?;
                let document = fixture(kind, multiplier, id.clone())?;
                let bundle = SourceBundle::new([EmbeddedSource::new(
                    id.clone(),
                    format,
                    bytes.clone(),
                    Some(input.into()),
                )?])?;
                toniator_io::save(&out.join(format!("{stem}.toniator")), &document, &bundle)?;
                let request = EvaluationRequest::new(
                    DocumentSession::new(document)?.document_evaluation_snapshot(),
                    ResolvedSource::new(id, bytes.clone(), hint)?,
                );
                let native = evaluate(request.clone().for_output(
                    RasterBackground::Transparent,
                    None,
                    RasterAntialiasing::On,
                ))?;
                fs::write(
                    out.join(format!("{stem}.png")),
                    encode_png(native.raster())?,
                )?;
                fs::write(out.join(format!("{stem}.rgba")), native.raster().pixels())?;
                fs::write(out.join(format!("{stem}.svg")), write_svg(native.scene())?)?;
                let preview = evaluate(request.for_preview(PreviewRasterTarget::new(320, 240)?))?;
                fs::write(
                    out.join(format!("{stem}-preview.png")),
                    encode_png(preview.raster())?,
                )?;
                let alpha = native.raster().pixels().chunks_exact(4).fold(
                    [0_usize; 3],
                    |mut counts, pixel| {
                        counts[if pixel[3] == 0 {
                            0
                        } else if pixel[3] == 255 {
                            2
                        } else {
                            1
                        }] += 1;
                        counts
                    },
                );
                fs::write(
                    out.join(format!("{stem}.json")),
                    serde_json::to_vec_pretty(
                        &json!({"source":input,"kind":kind,"tone_multiplier":multiplier,"canvas":[512,384],"feature_size":DensityMetric2D::default_for_canvas(&CanvasSpec {width:512.0,height:384.0})?.density/25.0,"transfer":"explicit synthetic raw0..2 tocoverage0..1; not a measured calibration","native_background":"transparent","alpha_counts_zero_partial_full":alpha,"scene":native.scene().identity().scene_fingerprint(),"preview_scene_same":native.scene()==preview.scene()}),
                    )?,
                )?;
                println!("{stem}");
            }
        }
    }
    Ok(())
}
