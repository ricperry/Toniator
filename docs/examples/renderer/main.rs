//! Reproduces README PNGs through Toniator's normal scene and streaming export APIs.
use std::{error::Error, fs::File, path::Path};
use toniator_domain::DocumentSession;
use toniator_engine::{
    EvaluationLimits, MediaTools, PngStreamOptions, RasterBackground, RasterizationLimits,
    StreamingRasterTarget, evaluate, frame_evaluation_request, open_source_media,
    write_png_streaming,
};

/// Renders one saved current project at twice its canvas dimensions without changing geometry.
///
/// Uses the same canonical scene and PNG consumer as the GUI; preserves the source and project.
/// The destination must be new, so a failed run cannot overwrite existing artwork.
///
/// # Errors
/// Returns invalid arguments, project/source validation, evaluation, or filesystem/PNG errors.
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: toniator-showcase-renderer INPUT.toniator OUTPUT.png".into());
    }
    let loaded = toniator_io::load(Path::new(&args[1]))?;
    let session = DocumentSession::new(loaded.document().clone())?;
    let canvas = session.document().canvas();
    let target = StreamingRasterTarget::new(
        (canvas.width * 2.0).round() as u32,
        (canvas.height * 2.0).round() as u32,
    )?;
    let background = RasterBackground::default_for_model(session.document().channel_model());
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| false)?;
    let frame = session.document().project_timing().frame_range().start();
    let request = frame_evaluation_request(&session, &mut media, frame, &|| false)?.for_scene()?;
    let result = evaluate(request)?;
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    write_png_streaming(
        result.scene(),
        target,
        PngStreamOptions {
            background,
            ..Default::default()
        },
        RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())?,
        file,
        &|| false,
        &|_, _| {},
    )?;
    Ok(())
}
