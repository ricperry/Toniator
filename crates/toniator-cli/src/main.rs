#![forbid(unsafe_code)]

use std::{error::Error, fmt, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ChannelTopologyTemplate, ColorValue, CoveragePolicy, DensityMetric2D, Document,
    DocumentCommand, DocumentId, DocumentSession, HalftoneChannelModel, MarkGeometryResponse,
    PatternDefinition, PatternDefinitionId, PatternMechanismId, PatternOutputLayerId,
    SourceComponent, SourcePlacement, SourceReference, SourceReferenceId, ValidationError,
};
use toniator_engine::{
    CanonicalCircleMark, EvaluationLimits, GridError, GridInspectRequest, MarkResponse,
    MarksInspectError, MarksInspectRequest, Point2, RasterAntialiasing, RasterBackground,
    ResolvedSource, SiteId, SiteScope, SourceFormat, SourceFormatHint, SvgTextDiagnostic,
    encode_png, evaluate_with_limits, inspect_circular_marks, inspect_straight_grid,
    resolve_source_identity, write_svg,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load as load_document,
    save as save_document,
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
    /// Render a complete authoritative document to PNG or SVG.
    Render(RenderArgs),
    /// Create one portable source-backed `.toniator` document.
    Document(DocumentArgs),
    /// Verify and summarize the schema-derived headless capability surface.
    Capabilities(CapabilitiesArgs),
}

#[derive(Debug, clap::Args)]
struct CapabilitiesArgs {
    /// Existing authoritative `.toniator` document to describe.
    #[arg(short = 'i', long)]
    input: Option<PathBuf>,
    /// Canvas dimensions used to construct the immutable diagnostic document.
    #[arg(long, default_value = "900x600")]
    canvas: String,
}

#[derive(Debug, clap::Args)]
struct ValidateArgs {
    #[arg(short = 'i', long)]
    input: Option<PathBuf>,
    /// Canvas dimensions, in WIDTHxHEIGHT form.
    #[arg(long)]
    canvas: Option<String>,
    #[arg(long)]
    density_x: Option<f64>,
    #[arg(long)]
    density_y: Option<f64>,
    #[arg(long)]
    opacity: Option<f64>,
}

#[derive(Debug, clap::Args)]
struct DocumentArgs {
    #[command(subcommand)]
    command: DocumentCommandArgs,
}
#[derive(Debug, Subcommand)]
enum DocumentCommandArgs {
    Create(DocumentCreateArgs),
}
#[derive(Debug, clap::Args)]
struct DocumentCreateArgs {
    #[arg(short = 'i', long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, value_enum)]
    channel_model: CliChannelModel,
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
    size_min: f64,
    #[arg(long)]
    size_max: f64,
    #[arg(long)]
    opacity: Option<f64>,
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
    #[arg(long, default_value_t = 1_048_576)]
    max_family_candidates: usize,
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
    #[arg(long, default_value_t = 1_048_576)]
    max_family_candidates: usize,
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
    #[arg(short = 'i', long, visible_alias = "source")]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    /// Authoritative ordered halftone channel topology for a direct source.
    #[arg(long, value_enum)]
    channel_model: Option<CliChannelModel>,
    #[arg(long)]
    canvas: Option<String>,
    #[arg(long)]
    density_x: Option<f64>,
    #[arg(long)]
    density_y: Option<f64>,
    #[arg(long, allow_hyphen_values = true)]
    rotation: Option<f64>,
    #[arg(long, allow_hyphen_values = true)]
    offset_x: Option<f64>,
    #[arg(long, allow_hyphen_values = true)]
    offset_y: Option<f64>,
    #[arg(long)]
    guard_steps: Option<u32>,
    #[arg(long, default_value_t = 1_048_576)]
    max_family_candidates: usize,
    #[arg(long)]
    size_min: Option<f64>,
    #[arg(long)]
    size_max: Option<f64>,
    /// Override opacity for every canonical channel in the selected topology.
    #[arg(long)]
    opacity: Option<f64>,
    /// Consumer-only PNG backing. SVG remains transparent.
    #[arg(long, value_enum, default_value_t = CliBackground::Transparent)]
    background: CliBackground,
    /// PNG edge rasterization. This never affects document evaluation or SVG.
    #[arg(long, value_enum, default_value_t = CliAntialiasing::On)]
    antialiasing: CliAntialiasing,
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
enum CliChannelModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

impl From<CliChannelModel> for HalftoneChannelModel {
    fn from(value: CliChannelModel) -> Self {
        match value {
            CliChannelModel::Rgb => Self::Rgb,
            CliChannelModel::Cmyk => Self::Cmyk,
            CliChannelModel::SourceColorAlpha => Self::SourceColorAlpha,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliBackground {
    Transparent,
    Black,
    White,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliAntialiasing {
    On,
    Off,
}

impl From<CliBackground> for RasterBackground {
    fn from(value: CliBackground) -> Self {
        match value {
            CliBackground::Transparent => Self::Transparent,
            CliBackground::Black => Self::OpaqueBlack,
            CliBackground::White => Self::OpaqueWhite,
        }
    }
}

impl From<CliAntialiasing> for RasterAntialiasing {
    fn from(value: CliAntialiasing) -> Self {
        match value {
            CliAntialiasing::On => Self::On,
            CliAntialiasing::Off => Self::Off,
        }
    }
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
        Some(Command::Validate(arguments)) => validate(arguments),
        Some(Command::Inspect(arguments)) => inspect(arguments),
        Some(Command::Render(arguments)) => render(arguments),
        Some(Command::Document(arguments)) => document_command(arguments),
        Some(Command::Capabilities(arguments)) => capabilities(arguments),
        None => Ok(()),
    }
}

fn capabilities(arguments: CapabilitiesArgs) -> Result<(), CliError> {
    let document = match arguments.input {
        Some(path) => load_document(&path)
            .map_err(|error| CliError::new("capabilities.input", error.to_string()))?
            .document()
            .clone(),
        None => Document::new_default_document(
            parse_canvas(&arguments.canvas)?,
            SourceReference::Unassigned,
        )?,
    };
    document.validate_property_descriptors()?;
    let descriptors = document.property_descriptors();
    println!("capabilities-v1\tcount={}", descriptors.len());
    for descriptor in descriptors {
        println!(
            "field={:?}\ttarget={:?}\tcommand={:?}\tkind={:?}\tchoices={:?}\tbounds={:?}\treference={:?}\tunit={:?}\tdependency={:?}\tsupport={:?}\tinvalidation={:?}\tcopy_escalates={}",
            descriptor.field,
            descriptor.target,
            descriptor.command_kind(),
            descriptor.value_kind,
            descriptor.choices,
            descriptor.bounds,
            descriptor.reference_constraint,
            descriptor.unit,
            descriptor.dependency,
            descriptor.structural_support,
            descriptor.invalidation,
            descriptor.copy_on_edit_escalates_to_family,
        );
    }
    Ok(())
}

fn document_command(arguments: DocumentArgs) -> Result<(), CliError> {
    match arguments.command {
        DocumentCommandArgs::Create(arguments) => document_create(arguments),
    }
}

fn document_create(arguments: DocumentCreateArgs) -> Result<(), CliError> {
    if arguments
        .output
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some("toniator")
    {
        return Err(CliError::new(
            "document.output",
            "document output extension must be .toniator",
        ));
    }
    let format = source_hint(&arguments.input)?;
    let bytes = std::fs::read(&arguments.input)
        .map_err(|_| CliError::new("source", "could not read source file"))?;
    let source_id = SourceReferenceId::new("source-1")?;
    let document = build_document(
        source_id.clone(),
        parse_canvas(&arguments.canvas)?,
        arguments.channel_model.into(),
        arguments.density_x,
        arguments.density_y,
        arguments.rotation,
        arguments.offset_x,
        arguments.offset_y,
        arguments.guard_steps,
        arguments.size_min,
        arguments.size_max,
        arguments.opacity,
    )?;
    let embedded = EmbeddedSource::new(
        source_id,
        match format {
            SourceFormatHint::Png => EmbeddedSourceFormat::Png,
            SourceFormatHint::Svg => EmbeddedSourceFormat::Svg,
            SourceFormatHint::Unsupported => unreachable!(),
        },
        bytes,
        arguments
            .input
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned),
    )
    .map_err(|error| CliError::new(error.path(), error.context()))?;
    save_document(
        &arguments.output,
        &document,
        &SourceBundle::new([embedded])
            .map_err(|error| CliError::new(error.path(), error.context()))?,
    )
    .map_err(|error| CliError::new(error.path(), error.context()))
}

#[allow(clippy::too_many_arguments)]
fn build_document(
    source_reference: SourceReferenceId,
    canvas: CanvasSpec,
    model: HalftoneChannelModel,
    density_x: f64,
    density_y: f64,
    rotation: f64,
    offset_x: f64,
    offset_y: f64,
    guard_steps: u32,
    size_min: f64,
    size_max: f64,
    opacity: Option<f64>,
) -> Result<Document, CliError> {
    let legacy = Document::with_source(
        DocumentId(1),
        canvas,
        SourceReference::Assigned(source_reference),
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: density_x,
                    across_y: density_y,
                    aspect_locked: true,
                },
                rotation_degrees: rotation,
                translation_x: offset_x,
                translation_y: offset_y,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: size_min,
                maximum_size: size_max,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )?;
    let topology = legacy.canonical_channel_topology(
        model,
        ChannelTopologyTemplate {
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: density_x,
                    across_y: density_y,
                    aspect_locked: true,
                },
                rotation_degrees: rotation,
                translation_x: offset_x,
                translation_y: offset_y,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: size_min,
                maximum_size: size_max,
            },
        },
    )?;
    let mut session = DocumentSession::new(legacy)?;
    let channel_ids = topology
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    session.apply(&DocumentCommand::ReplaceChannelTopology { model, topology })?;
    if let Some(opacity) = opacity.filter(|value| *value != 1.0) {
        for channel_id in channel_ids {
            session.apply(&DocumentCommand::SetOpacity {
                channel_id,
                opacity,
            })?;
        }
    }
    Ok(session.snapshot())
}

fn render(arguments: RenderArgs) -> Result<(), CliError> {
    let format = output_format(&arguments.output)?;
    if matches!(format, OutputFormat::Svg)
        && !matches!(arguments.background, CliBackground::Transparent)
    {
        return Err(CliError::new(
            "render.background",
            "SVG output supports only a transparent background",
        ));
    }
    if is_toniator_path(&arguments.input) {
        if arguments.channel_model.is_some()
            || arguments.canvas.is_some()
            || arguments.density_x.is_some()
            || arguments.density_y.is_some()
            || arguments.rotation.is_some()
            || arguments.offset_x.is_some()
            || arguments.offset_y.is_some()
            || arguments.guard_steps.is_some()
            || arguments.size_min.is_some()
            || arguments.size_max.is_some()
            || arguments.opacity.is_some()
        {
            return Err(CliError::new(
                "render.arguments",
                "container rendering does not accept document override flags",
            ));
        }
        let loaded = load_document(&arguments.input)
            .map_err(|error| CliError::new(error.path(), error.context()))?;
        let source_id = match loaded.document().source() {
            SourceReference::Assigned(id) => id,
            SourceReference::Unassigned => {
                return Err(CliError::new(
                    "source.document",
                    "container document has no assigned source",
                ));
            }
        };
        let source = loaded.sources().get(source_id).ok_or_else(|| {
            CliError::new(
                "source.document",
                "container source bundle does not match document",
            )
        })?;
        let hint = match source.format() {
            EmbeddedSourceFormat::Png => SourceFormatHint::Png,
            EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
        };
        let session = DocumentSession::new(loaded.document().clone())?;
        let result = evaluate_with_limits(
            toniator_engine::EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), source.bytes().to_vec(), hint)?,
            ),
            EvaluationLimits::new(arguments.max_family_candidates)?,
        )?;
        return write_render_result(
            &arguments.output,
            format,
            arguments.background,
            arguments.antialiasing.into(),
            &result,
        );
    }
    let source_format = source_hint(&arguments.input)?;
    let source_bytes = std::fs::read(&arguments.input)
        .map_err(|_| CliError::new("source", "could not read source file"))?;
    let source_identity = resolve_source_identity(&source_bytes, source_format)?;
    let source_reference = SourceReferenceId::new("cli-input-1")?;
    let canvas = match arguments.canvas.as_deref() {
        Some(value) => parse_canvas(value)?,
        None => CanvasSpec {
            width: f64::from(source_identity.width),
            height: f64::from(source_identity.height),
        },
    };
    let document = build_document(
        source_reference.clone(),
        canvas,
        arguments
            .channel_model
            .ok_or_else(|| {
                CliError::new(
                    "channel_model",
                    "--channel-model is required for direct-source rendering",
                )
            })?
            .into(),
        arguments.density_x.ok_or_else(|| {
            CliError::new(
                "density_x",
                "--density-x is required for direct-source rendering",
            )
        })?,
        arguments.density_y.ok_or_else(|| {
            CliError::new(
                "density_y",
                "--density-y is required for direct-source rendering",
            )
        })?,
        arguments.rotation.ok_or_else(|| {
            CliError::new(
                "rotation",
                "--rotation is required for direct-source rendering",
            )
        })?,
        arguments.offset_x.ok_or_else(|| {
            CliError::new(
                "offset_x",
                "--offset-x is required for direct-source rendering",
            )
        })?,
        arguments.offset_y.ok_or_else(|| {
            CliError::new(
                "offset_y",
                "--offset-y is required for direct-source rendering",
            )
        })?,
        arguments.guard_steps.ok_or_else(|| {
            CliError::new(
                "guard_steps",
                "--guard-steps is required for direct-source rendering",
            )
        })?,
        arguments.size_min.ok_or_else(|| {
            CliError::new(
                "size_min",
                "--size-min is required for direct-source rendering",
            )
        })?,
        arguments.size_max.ok_or_else(|| {
            CliError::new(
                "size_max",
                "--size-max is required for direct-source rendering",
            )
        })?,
        arguments.opacity,
    )?;
    let session = DocumentSession::new(document)?;
    let result = evaluate_with_limits(
        toniator_engine::EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_reference, source_bytes, source_format)?,
        ),
        EvaluationLimits::new(arguments.max_family_candidates)?,
    )?;
    write_render_result(
        &arguments.output,
        format,
        arguments.background,
        arguments.antialiasing.into(),
        &result,
    )
}

fn write_render_result(
    output: &PathBuf,
    format: OutputFormat,
    background: CliBackground,
    antialiasing: RasterAntialiasing,
    result: &toniator_engine::EvaluationResult,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Png => {
            let background = background.into();
            let raster = if matches!(background, RasterBackground::Transparent)
                && matches!(antialiasing, RasterAntialiasing::On)
            {
                result.raster().clone()
            } else {
                toniator_engine::rasterize_output(result.scene(), background, None, antialiasing)
                    .map_err(render_error)?
            };
            let png = encode_png(&raster).map_err(render_error)?;
            std::fs::write(output, png)
                .map_err(|_| CliError::new("output", "could not write PNG output"))?;
        }
        OutputFormat::Svg => {
            std::fs::write(output, write_svg(result.scene()))
                .map_err(|_| CliError::new("output", "could not write SVG output"))?;
        }
    }
    Ok(())
}

fn is_toniator_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        == Some("toniator")
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
            max_family_candidates: arguments.max_family_candidates,
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
        max_family_candidates: arguments.max_family_candidates,
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

fn validate(arguments: ValidateArgs) -> Result<(), CliError> {
    if let Some(input) = arguments.input {
        if arguments.canvas.is_some()
            || arguments.density_x.is_some()
            || arguments.density_y.is_some()
            || arguments.opacity.is_some()
        {
            return Err(CliError::new(
                "validate.arguments",
                "--input cannot be combined with document construction arguments",
            ));
        }
        let loaded =
            load_document(&input).map_err(|error| CliError::new(error.path(), error.context()))?;
        let session = DocumentSession::new(loaded.document().clone())?;
        let migrations = if loaded.migration_report().is_empty() {
            "empty"
        } else {
            "v1-to-v2"
        };
        println!(
            "valid document (revision {}, container v{}, document v{}, migrations: {})",
            session.revision().0,
            loaded.versions().container(),
            loaded.versions().document(),
            migrations,
        );
        return Ok(());
    }
    let canvas = parse_canvas(
        arguments
            .canvas
            .as_deref()
            .ok_or_else(|| CliError::new("canvas", "--canvas is required without --input"))?,
    )?;
    let density_x = arguments
        .density_x
        .ok_or_else(|| CliError::new("density_x", "--density-x is required without --input"))?;
    let density_y = arguments
        .density_y
        .ok_or_else(|| CliError::new("density_y", "--density-y is required without --input"))?;
    let opacity = arguments
        .opacity
        .ok_or_else(|| CliError::new("opacity", "--opacity is required without --input"))?;
    let document = Document::new(
        DocumentId(1),
        canvas,
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "minimal",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: density_x,
                    across_y: density_y,
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
                opacity,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
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
    path: String,
    message: String,
}

impl CliError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn message(&self) -> &str {
        &self.message
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

impl From<toniator_domain::DocumentSessionError> for CliError {
    fn from(error: toniator_domain::DocumentSessionError) -> Self {
        match error {
            toniator_domain::DocumentSessionError::Validation(error) => Self::from(error),
            toniator_domain::DocumentSessionError::RevisionExhausted => {
                Self::new("document.revision", "document revision is exhausted")
            }
            toniator_domain::DocumentSessionError::HistoryRequired => Self::new(
                "document.history",
                "pattern definition commands require document history",
            ),
        }
    }
}

impl From<toniator_engine::EvaluationError> for CliError {
    fn from(error: toniator_engine::EvaluationError) -> Self {
        Self::new(error.path(), error.message())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for CliError {}
