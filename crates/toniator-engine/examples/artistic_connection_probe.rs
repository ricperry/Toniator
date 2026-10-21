//! Native current connected-points witnesses; fixture transfers are explicitly synthetic.
use serde_json::json;
use std::{error::Error, fs, path::PathBuf};
use toniator_domain::*;
use toniator_engine::*;
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Creates one current connection recipe with independent final constraints.
fn recipe(
    kind: &str,
    selection: ConnectionSelectionIntent,
    distance: f64,
) -> PatternDefinitionRecipe {
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 4,
        maximum_distance: distance,
    };
    let program = match kind {
        "random" | "random-visible" => ConnectionProgram::RandomLinks {
            adjacency,
            minimum_degree: 0,
            seed: 17,
        },
        "tree" => ConnectionProgram::GridSpanningTree {
            adjacency,
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 17,
        },
        _ => ConnectionProgram::NearestLinks { adjacency },
    };
    PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
        selection,
        program,
        style: PathStrokeStyle::default(),
        definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
            name: "Artistic connection fixture".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            dimensions: vec![
                GuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                },
                GuideDimensionDraft {
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    spacing_multiplier: 1.0,
                },
            ],
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-9,
            },
            orientation: MarkOrientationDraft::Fixed,
        }),
    })
}
/// Binds synthetic raw-size-to-coverage transfer solely to isolate topology and mixed consumption.
/// # Errors
/// Returns current domain/calibration validation failures before external output.
pub(crate) fn fixture(
    kind: &str,
    selection: ConnectionSelectionIntent,
    distance: f64,
    canvas: CanvasSpec,
    model: HalftoneChannelModel,
    id: SourceReferenceId,
) -> Result<Document> {
    let base = Document::new_default_document(canvas, SourceReference::Assigned(id))?;
    let template = ChannelTopologyTemplate {
        pattern_instance: base
            .channel_topology()
            .ok_or("missing topology")?
            .channels()[0]
            .pattern_instance
            .clone(),
    };
    let base = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        base.pattern_definition_bundles().to_vec(),
        base.pattern_settings().clone(),
        model,
        ChannelTopology::canonical(model, template)?,
        vec![],
    )?;
    let mut history = DocumentHistory::new(DocumentSession::new(base.clone())?);
    history.apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: base.pattern_settings().clone(),
        base_definition: base.pattern_definition_bundles()[0].definition.clone(),
        recipe: recipe(kind, selection, distance),
    })?;
    let mut document = history
        .document()
        .edit_all_channel_start_configuration(&[
            (PropertyFieldId::Density, 15.0),
            (PropertyFieldId::ConnectedMinimumThickness, 0.0),
            (PropertyFieldId::ConnectedMaximumThickness, 0.65),
        ])?
        .bind(history.document())?;
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
/// Exports intrinsic native RGBA, preview, editable SVG and streaming PNG for immutable source fixtures.
/// Optional fit mode performs one genuine strict singleton-producing calibration, without fixture transfer reuse.
/// # Errors
/// Returns validation, strict fit, evaluation, native consumer or stage I/O errors.
fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/validation/artistic-geometry-20261019/phase3/native");
    fs::create_dir_all(&out)?;
    if std::env::args().nth(1).as_deref() == Some("fit") {
        let document = fixture(
            "dots",
            Default::default(),
            0.001,
            CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            HalftoneChannelModel::Rgb,
            SourceReferenceId::new("connection-fit")?,
        )?;
        let binding = document.baseline_calibration_binding()?;
        let start = std::time::Instant::now();
        let fit = reference_calibration::calibrate_saved_binding(
            &binding,
            EvaluationLimits::default(),
            &|| false,
        );
        let record = match fit {
            Ok(artifact) => {
                json!({"fit":"passed","seconds":start.elapsed().as_secs_f64(),"topology":"unresolved singleton circles; no strokes","artifact":format!("{artifact:#?}")})
            }
            Err(e) => {
                json!({"fit":"failed","seconds":start.elapsed().as_secs_f64(),"path":e.path(),"message":e.message()})
            }
        };
        fs::write(
            out.join("strict-singleton-fit.json"),
            serde_json::to_vec_pretty(&record)?,
        )?;
        println!("{record}");
        if record["fit"] != "passed" {
            return Err("strict singleton fit failed".into());
        }
        return Ok(());
    }
    for (source_name, input, format, hint, width, height) in [
        (
            "raster",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
            1024.,
            1024.,
        ),
        (
            "vector",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
            900.,
            620.,
        ),
    ] {
        let bytes = fs::read(root.join("assets").join(input))?;
        for (kind, model, degree, cycles, minimum, distance) in [
            (
                "dots",
                HalftoneChannelModel::Rgb,
                None,
                ConnectionCyclePolicy::Allow,
                1,
                0.001,
            ),
            (
                "nearest",
                HalftoneChannelModel::Rgb,
                Some(2),
                ConnectionCyclePolicy::Forbid,
                1,
                18.,
            ),
            (
                "random",
                HalftoneChannelModel::Rgb,
                Some(1),
                ConnectionCyclePolicy::Allow,
                3,
                18.,
            ),
            (
                "random-visible",
                HalftoneChannelModel::Rgb,
                None,
                ConnectionCyclePolicy::Allow,
                1,
                18.,
            ),
            (
                "tree",
                HalftoneChannelModel::Cmyk,
                Some(2),
                ConnectionCyclePolicy::Allow,
                1,
                18.,
            ),
        ] {
            let stem = format!("{source_name}-{kind}");
            let id = SourceReferenceId::new(format!("connection-{stem}"))?;
            let document = fixture(
                kind,
                ConnectionSelectionIntent {
                    maximum_final_degree: degree,
                    cycles,
                    minimum_component_points: minimum,
                },
                distance,
                CanvasSpec { width, height },
                model,
                id.clone(),
            )?;
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
            let start = std::time::Instant::now();
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
            let mut counts = [0usize; 2];
            for layer in native.scene().layers() {
                for output in layer.outputs() {
                    if let toniator_render::GeometryOutput::CanonicalConnections(value) =
                        output.geometry()
                    {
                        counts[0] += value.strokes.len();
                        counts[1] += value.isolated_points.len();
                    }
                }
            }
            toniator_render::write_png_streaming(
                native.scene(),
                toniator_render::StreamingRasterTarget::new(width as u32, height as u32)?,
                toniator_render::PngStreamOptions {
                    maximum_band_rows: 31,
                    ..Default::default()
                },
                toniator_render::RasterizationLimits::default(),
                fs::File::create(out.join(format!("{stem}-stream.png")))?,
                &|| false,
                &|_, _| {},
            )?;
            let record = json!({"source":input,"canvas":[width,height],"model":format!("{model:?}"),"selection":{"maximum_final_degree":degree,"cycles":format!("{cycles:?}"),"minimum_component_points":minimum},"strokes":counts[0],"singleton_circles":counts[1],"seconds":start.elapsed().as_secs_f64(),"transfer":"synthetic raw0..2 tocoverage0..1; not a measured calibration","native_rgba":"unflattened","preview_scene_same":native.scene()==preview.scene()});
            fs::write(
                out.join(format!("{stem}.json")),
                serde_json::to_vec_pretty(&record)?,
            )?;
            println!("{record}");
        }
    }
    Ok(())
}
