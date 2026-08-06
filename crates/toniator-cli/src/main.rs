#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelState, ColorValue,
    DensityMetric2D, Document, DocumentId, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, ValidationError,
};
use toniator_engine::DocumentSession;

/// Headless Toniator command-line frontend.
#[derive(Debug, Parser)]
#[command(name = "toniator", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build and validate the minimal authoritative in-memory document.
    Validate(ValidateArgs),
}

#[derive(Debug, clap::Args)]
struct ValidateArgs {
    /// Canvas dimensions, in WIDTHxHEIGHT form.
    #[arg(long)]
    canvas: String,
    #[arg(long)]
    density_x: f64,
    #[arg(long)]
    density_y: f64,
    #[arg(long)]
    opacity: f64,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error at {}: {}", error.path(), error.message());
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), ValidationError> {
    match cli.command {
        Some(Command::Validate(arguments)) => validate(arguments),
        None => Ok(()),
    }
}

fn validate(arguments: ValidateArgs) -> Result<(), ValidationError> {
    let canvas = parse_canvas(&arguments.canvas)?;
    let document = Document::new(
        DocumentId(1),
        canvas,
        vec![PatternDefinition {
            id: PatternDefinitionId(1),
            name: "minimal".to_owned(),
        }],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: arguments.density_x,
                    across_y: arguments.density_y,
                    aspect_locked: true,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: arguments.opacity,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 0.0,
                maximum_size: 1.0,
            },
        }],
    )?;
    let session = DocumentSession::new(document)?;
    session.document().validate()?;
    println!("valid document (revision {})", session.revision().0);
    Ok(())
}

fn parse_canvas(value: &str) -> Result<CanvasSpec, ValidationError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(ValidationError::new("canvas", "expected WIDTHxHEIGHT"));
    };
    let width = width
        .parse::<f64>()
        .map_err(|_| ValidationError::new("canvas.width", "expected a number"))?;
    let height = height
        .parse::<f64>()
        .map_err(|_| ValidationError::new("canvas.height", "expected a number"))?;
    Ok(CanvasSpec { width, height })
}
