#![forbid(unsafe_code)]

use std::{error::Error, fmt, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelState, ColorValue,
    DensityMetric2D, Document, DocumentId, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, ValidationError,
};
use toniator_engine::{DocumentSession, GridError, GridInspectRequest, inspect_straight_grid};

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
    /// Inspect deterministic family output without realizing marks or rendering.
    Inspect(InspectArgs),
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

#[derive(Debug, clap::Args)]
struct InspectArgs {
    #[command(subcommand)]
    command: InspectCommand,
}

#[derive(Debug, Subcommand)]
enum InspectCommand {
    /// Emit the two-dimension straight-guide family and its intersection sites.
    Grid(GridArgs),
}

#[derive(Debug, clap::Args)]
struct GridArgs {
    /// Write JSON to a file instead of standard output.
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    canvas: String,
    #[arg(long)]
    density_x: f64,
    #[arg(long)]
    density_y: f64,
    #[arg(long, allow_hyphen_values = true)]
    rotation: f64,
    #[arg(long, allow_hyphen_values = true)]
    offset_x: f64,
    #[arg(long, allow_hyphen_values = true)]
    offset_y: f64,
    #[arg(long)]
    guard_steps: u32,
    #[arg(long)]
    support_radius: f64,
    #[arg(long, value_enum)]
    format: InspectFormat,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InspectFormat {
    Json,
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

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Some(Command::Validate(arguments)) => validate(arguments).map_err(CliError::from),
        Some(Command::Inspect(arguments)) => inspect(arguments),
        None => Ok(()),
    }
}

fn inspect(arguments: InspectArgs) -> Result<(), CliError> {
    match arguments.command {
        InspectCommand::Grid(arguments) => inspect_grid(arguments),
    }
}

fn inspect_grid(arguments: GridArgs) -> Result<(), CliError> {
    let request = GridInspectRequest {
        canvas: parse_canvas(&arguments.canvas)?,
        density: DensityMetric2D {
            across_x: arguments.density_x,
            across_y: arguments.density_y,
            aspect_locked: true,
        },
        rotation_degrees: arguments.rotation,
        translation_x: arguments.offset_x,
        translation_y: arguments.offset_y,
        guard_steps: arguments.guard_steps,
        support_radius: arguments.support_radius,
    };
    let output = inspect_straight_grid(&request)?;
    let serialized = match arguments.format {
        InspectFormat::Json => serde_json::to_string_pretty(&output)
            .map_err(|_| CliError::new("inspect.format", "could not serialize JSON"))?,
    };
    if let Some(path) = arguments.output {
        std::fs::write(path, serialized)
            .map_err(|_| CliError::new("inspect.output", "could not write output file"))?;
    } else {
        println!("{serialized}");
    }
    Ok(())
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct CliError {
    path: &'static str,
    message: &'static str,
}

impl CliError {
    const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    const fn path(&self) -> &'static str {
        self.path
    }

    const fn message(&self) -> &'static str {
        self.message
    }
}

impl From<ValidationError> for CliError {
    fn from(error: ValidationError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl From<GridError> for CliError {
    fn from(error: GridError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for CliError {}
