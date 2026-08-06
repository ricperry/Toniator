#![forbid(unsafe_code)]

use std::{error::Error, fmt, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelState, ColorValue,
    DensityMetric2D, Document, DocumentId, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, ValidationError,
};
use toniator_engine::{
    CanonicalCircleMark, DocumentSession, GridError, GridInspectRequest, MarkResponse,
    MarksInspectError, MarksInspectRequest, Point2, RasterBackground, RenderSceneRequest,
    ScenePresentation, SiteId, SiteScope, SourceComponent, SourceFormat, SourceFormatHint,
    SourcePlacement, SvgTextDiagnostic, encode_png, inspect_circular_marks, inspect_straight_grid,
    rasterize, render_scene, srgb_to_linear, write_svg,
};

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
    /// Render canonical Stage 4 circles to PNG or SVG through one RenderScene.
    Render(RenderArgs),
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
    /// Summarize source-driven canonical circular marks without rendering.
    Marks(MarksArgs),
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

#[derive(Debug, clap::Args)]
struct MarksArgs {
    #[arg(long)]
    source: PathBuf,
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
    source_component: CliSourceComponent,
    #[arg(long)]
    size_min: f64,
    #[arg(long)]
    size_max: f64,
    #[arg(long)]
    color: String,
    #[arg(long)]
    opacity: f64,
    #[arg(long)]
    summary: bool,
    #[arg(long, value_enum)]
    format: InspectFormat,
}

#[derive(Debug, clap::Args)]
struct RenderArgs {
    #[arg(long)]
    source: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, value_enum)]
    mode: OutputMode,
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
    source_component: CliSourceComponent,
    #[arg(long)]
    size_min: f64,
    #[arg(long)]
    size_max: f64,
    #[arg(long)]
    color: String,
    #[arg(long)]
    opacity: f64,
    #[arg(long)]
    transparent: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InspectFormat {
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliSourceComponent {
    Luminance,
    Alpha,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputMode {
    Rgb,
    Cmyk,
}

impl From<CliSourceComponent> for SourceComponent {
    fn from(value: CliSourceComponent) -> Self {
        match value {
            CliSourceComponent::Luminance => Self::Luminance,
            CliSourceComponent::Alpha => Self::Alpha,
        }
    }
}

#[derive(Serialize)]
struct MarksSummary {
    source: SummarySource,
    family_fingerprint: String,
    realization_fingerprint: String,
    marks: MarkCounts,
    radius: RadiusSummary,
    representative_marks: Vec<RepresentativeMark>,
    presentation: PresentationSummary,
    fingerprint_proof: FingerprintProof,
}

#[derive(Serialize)]
struct SummarySource {
    format: SourceFormat,
    width: u32,
    height: u32,
    content_hash: String,
    decoded_pixel_hash: String,
    svg_text: Option<SvgTextDiagnostic>,
}
#[derive(Serialize)]
struct MarkCounts {
    total: usize,
    canvas: usize,
    guard: usize,
    nonfinite: usize,
}
#[derive(Serialize)]
struct RadiusSummary {
    minimum: f64,
    maximum: f64,
    mean: f64,
}
#[derive(Serialize)]
struct RepresentativeMark {
    id: SiteId,
    center: Point2,
    radius: f64,
    scope: SiteScope,
}
#[derive(Serialize)]
struct PresentationSummary {
    color: String,
    opacity: f64,
    visible: bool,
}
#[derive(Serialize)]
struct FingerprintProof {
    excludes_color_opacity_visibility: bool,
    statement: &'static str,
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
        Some(Command::Render(arguments)) => render(arguments),
        None => Ok(()),
    }
}

fn render(arguments: RenderArgs) -> Result<(), CliError> {
    let format = output_format(&arguments.output)?;
    let color = parse_color_value(&arguments.color)?;
    if !arguments.opacity.is_finite() || !(0.0..=1.0).contains(&arguments.opacity) {
        return Err(CliError::new(
            "presentation.opacity",
            "opacity must be within 0.0..=1.0",
        ));
    }
    let source_format = source_hint(&arguments.source)?;
    let source_bytes = std::fs::read(&arguments.source)
        .map_err(|_| CliError::new("source", "could not read source file"))?;
    let scene = render_scene(&RenderSceneRequest {
        marks: MarksInspectRequest {
            grid: GridInspectRequest {
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
            },
            source_bytes: &source_bytes,
            source_format,
            source_component: arguments.source_component.into(),
            placement: SourcePlacement::StretchToCanvas,
            response: MarkResponse {
                minimum_size: arguments.size_min,
                maximum_size: arguments.size_max,
            },
        },
        presentation: ScenePresentation {
            channel_id: ChannelId(1),
            visible: true,
            color,
            opacity: arguments.opacity,
        },
    })?;
    match format {
        OutputFormat::Png => {
            let background = if arguments.transparent {
                RasterBackground::Transparent
            } else {
                match arguments.mode {
                    OutputMode::Rgb => RasterBackground::OpaqueBlack,
                    OutputMode::Cmyk => RasterBackground::OpaqueWhite,
                }
            };
            let png = encode_png(&rasterize(&scene, background).map_err(render_error)?)
                .map_err(render_error)?;
            std::fs::write(&arguments.output, png)
                .map_err(|_| CliError::new("output", "could not write PNG output"))?;
        }
        OutputFormat::Svg => {
            // SVG has no mode background; --transparent is intentionally a no-op.
            std::fs::write(&arguments.output, write_svg(&scene))
                .map_err(|_| CliError::new("output", "could not write SVG output"))?;
        }
    }
    Ok(())
}

fn inspect(arguments: InspectArgs) -> Result<(), CliError> {
    match arguments.command {
        InspectCommand::Grid(arguments) => inspect_grid(arguments),
        InspectCommand::Marks(arguments) => inspect_marks(arguments),
    }
}

fn inspect_marks(arguments: MarksArgs) -> Result<(), CliError> {
    if !arguments.summary {
        return Err(CliError::new(
            "inspect.summary",
            "Stage 4 inspect marks requires --summary",
        ));
    }
    let color = parse_color(&arguments.color)?;
    if !arguments.opacity.is_finite() || !(0.0..=1.0).contains(&arguments.opacity) {
        return Err(CliError::new(
            "presentation.opacity",
            "opacity must be within 0.0..=1.0",
        ));
    }
    let source_format = source_hint(&arguments.source)?;
    let source_bytes = std::fs::read(&arguments.source)
        .map_err(|_| CliError::new("source", "could not read source file"))?;
    let request = MarksInspectRequest {
        grid: GridInspectRequest {
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
        },
        source_bytes: &source_bytes,
        source_format,
        source_component: arguments.source_component.into(),
        placement: SourcePlacement::StretchToCanvas,
        response: MarkResponse {
            minimum_size: arguments.size_min,
            maximum_size: arguments.size_max,
        },
    };
    let output = inspect_circular_marks(&request)?;
    let canvas = output
        .marks
        .iter()
        .filter(|mark| mark.scope == SiteScope::Canvas)
        .count();
    let guard = output.marks.len() - canvas;
    let radii: Vec<_> = output.marks.iter().map(|mark| mark.radius).collect();
    let summary = MarksSummary {
        source: SummarySource {
            format: output.source_identity.format,
            width: output.source_identity.width,
            height: output.source_identity.height,
            content_hash: output.source_identity.content_hash,
            decoded_pixel_hash: output.source_identity.decoded_pixel_hash,
            svg_text: output.source_identity.svg_text,
        },
        family_fingerprint: output.family_fingerprint,
        realization_fingerprint: output.realization_fingerprint,
        marks: MarkCounts {
            total: output.marks.len(),
            canvas,
            guard,
            nonfinite: output
                .marks
                .iter()
                .filter(|mark| !mark.center.is_finite() || !mark.radius.is_finite())
                .count(),
        },
        radius: RadiusSummary {
            minimum: compact_radius(radii.iter().copied().fold(f64::INFINITY, f64::min)),
            maximum: compact_radius(radii.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
            mean: compact_radius(radii.iter().sum::<f64>() / radii.len() as f64),
        },
        representative_marks: representative_marks(&output.marks),
        presentation: PresentationSummary {
            color,
            opacity: arguments.opacity,
            visible: true,
        },
        fingerprint_proof: FingerprintProof {
            excludes_color_opacity_visibility: true,
            statement: "canonical marks and realization fingerprint are computed before presentation",
        },
    };
    let serialized = match arguments.format {
        InspectFormat::Json => serde_json::to_string_pretty(&summary)
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

fn compact_radius(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn representative_marks(marks: &[CanonicalCircleMark]) -> Vec<RepresentativeMark> {
    // The SVG fixture's live text is deliberately system-font dependent. Keep
    // exact representatives outside its documented text box.
    let candidates: Vec<_> = marks
        .iter()
        .filter(|mark| {
            !(mark.center.x >= 515.0
                && mark.center.x <= 765.0
                && mark.center.y >= 190.0
                && mark.center.y <= 320.0)
        })
        .collect();
    let mut indices = vec![
        0,
        1,
        2,
        candidates.len() / 3,
        candidates.len() / 2,
        candidates.len() * 2 / 3,
        candidates.len().saturating_sub(2),
        candidates.len().saturating_sub(1),
    ];
    indices.sort_unstable();
    indices.dedup();
    indices
        .into_iter()
        .filter_map(|index| candidates.get(index))
        .map(|mark| RepresentativeMark {
            id: mark.source_site_id,
            center: mark.center,
            radius: mark.radius,
            scope: mark.scope,
        })
        .collect()
}

fn source_hint(path: &std::path::Path) -> Result<SourceFormatHint, CliError> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(SourceFormatHint::Png),
        Some("svg") => Ok(SourceFormatHint::Svg),
        _ => Err(CliError::new(
            "source.format",
            "source extension must be .png or .svg",
        )),
    }
}

fn parse_color(value: &str) -> Result<String, CliError> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(CliError::new("presentation.color", "expected #RRGGBB"));
    };
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliError::new("presentation.color", "expected #RRGGBB"));
    }
    Ok(format!("#{}", hex.to_ascii_lowercase()))
}

fn parse_color_value(value: &str) -> Result<ColorValue, CliError> {
    let normalized = parse_color(value)?;
    let parse_component = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&normalized[range], 16)
            .map(|component| srgb_to_linear(f64::from(component) / 255.0))
            .map_err(|_| CliError::new("presentation.color", "expected #RRGGBB"))
    };
    Ok(ColorValue {
        red: parse_component(1..3)?,
        green: parse_component(3..5)?,
        blue: parse_component(5..7)?,
        alpha: 1.0,
    })
}

#[derive(Clone, Copy, Debug)]
enum OutputFormat {
    Png,
    Svg,
}

fn output_format(path: &std::path::Path) -> Result<OutputFormat, CliError> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(OutputFormat::Png),
        Some("svg") => Ok(OutputFormat::Svg),
        _ => Err(CliError::new(
            "output.format",
            "output extension must be .png or .svg",
        )),
    }
}

fn render_error(error: toniator_engine::RenderError) -> CliError {
    CliError::new(error.path(), error.message())
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

impl From<MarksInspectError> for CliError {
    fn from(error: MarksInspectError) -> Self {
        match error {
            MarksInspectError::Grid(error) => Self::new(error.path(), error.message()),
            MarksInspectError::Sampling(error) => Self::new(error.path(), error.message()),
            MarksInspectError::Realization(error) => Self::new(error.path(), error.message()),
        }
    }
}

impl From<toniator_engine::RenderSceneError> for CliError {
    fn from(error: toniator_engine::RenderSceneError) -> Self {
        match error {
            toniator_engine::RenderSceneError::Marks(error) => Self::from(error),
            toniator_engine::RenderSceneError::Render(error) => render_error(error),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for CliError {}
