//! Bounded, project-only JSON entry point for garment print-preparation advisories.

use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use clap::ValueEnum;
use serde::{Serialize, Serializer, ser::SerializeSeq};
use toniator_domain::{
    DocumentCommand, DocumentHistory, DocumentSession, PhysicalPrintSizeMm,
    PrintPreparationSettings,
};
use toniator_engine::{
    EvaluationLimits, MediaTools, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    open_source_media,
    print_preflight::{
        PREFLIGHT_ALGORITHM_ID, PreflightLimits, PreflightOutcome, PreflightReport,
        PreflightSelection, ProbeCandidate, ProbeCandidateKind, SupportComponent, WidthCheckStatus,
        preflight_current_frame,
    },
};
use toniator_io::load as load_document;

use crate::{CliAntialiasing, CliBackground, CliError};

/// Supplies project input, output selection, optional session-local measurements, and detector bounds.
#[derive(Debug, clap::Args)]
pub(super) struct PreflightArgs {
    /// Current `.toniator` project; direct source files are not accepted.
    #[arg(short = 'i', long)]
    input: PathBuf,
    /// Absolute output frame; omitted selects the project's range start.
    #[arg(long)]
    frame: Option<u64>,
    /// Final raster extent in WIDTHxHEIGHT pixels; omitted uses the renderer's native canvas target.
    #[arg(long, value_name = "WIDTHxHEIGHT")]
    output_size: Option<String>,
    /// PNG edge antialiasing used by the transparent analysis render.
    #[arg(long, value_enum, default_value_t = CliAntialiasing::On)]
    antialiasing: CliAntialiasing,
    /// Export backing recorded as metadata; alpha analysis always uses transparent output.
    #[arg(long, value_enum)]
    background: Option<CliBackground>,
    /// Unit for physical dimensions and width thresholds supplied on this command.
    #[arg(long, value_enum, default_value_t = EntryUnit::Mm)]
    unit: EntryUnit,
    /// Optional complete-canvas physical width, interpreted using --unit.
    #[arg(long, allow_hyphen_values = true)]
    print_width: Option<f64>,
    /// Optional complete-canvas physical height, interpreted using --unit.
    #[arg(long, allow_hyphen_values = true)]
    print_height: Option<f64>,
    /// Optional positive-feature warning threshold; zero disables this check.
    #[arg(long, allow_hyphen_values = true)]
    positive_threshold: Option<f64>,
    /// Optional negative-gap warning threshold; zero disables this check.
    #[arg(long, allow_hyphen_values = true)]
    gap_threshold: Option<f64>,
    /// Maximum final-output pixels admitted by the detector.
    #[arg(long)]
    max_pixels: Option<u64>,
    /// Maximum detector scratch estimate in bytes.
    #[arg(long)]
    max_scratch_bytes: Option<u64>,
    /// Maximum sampled physical-disk stencil offsets.
    #[arg(long)]
    max_stencil_offsets: Option<u64>,
    /// Maximum connected-component, candidate, and highlight records.
    #[arg(long)]
    max_records: Option<u64>,
    /// Maximum charged analysis operations.
    #[arg(long)]
    max_operations: Option<u64>,
}

/// Selects the unit used to interpret command-local physical values.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum EntryUnit {
    /// Millimetres, also used for canonical report values.
    Mm,
    /// Inches, converted to canonical millimetres before measurement.
    In,
}

impl EntryUnit {
    /// Returns the stable lowercase name written to JSON.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::In => "in",
        }
    }
}

/// Loads one authoritative project, measures its current transparent frame, and streams JSON.
///
/// Session-local G1a overrides use reversible document history and are never saved. A result is
/// serialized only after the engine rechecks source freshness and the complete selection and a
/// final cancellation check passes. Publication then runs synchronously to completion; a signal
/// arriving after that commit point does not truncate an already-complete report.
///
/// # Errors
/// Rejects project, argument, history, source, cancellation, stale-result, and JSON-output failures.
pub(super) fn run(arguments: PreflightArgs) -> Result<(), CliError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let _signals = crate::temporal::ExportSignals::new(Arc::clone(&cancelled))?;
    let loaded = load_document(&arguments.input)
        .map_err(|error| CliError::new(error.path(), error.context()))?;
    let session = DocumentSession::new(loaded.document().clone())?;
    let mut history = DocumentHistory::new(session);
    let base_settings = history.document().print_preparation().clone();
    let settings = resolve_settings(&base_settings, &arguments)?;
    if settings != base_settings {
        history.apply(&DocumentCommand::SetPrintPreparation {
            base: base_settings,
            settings: settings.clone(),
        })?;
    }
    let settings = history.document().print_preparation().clone();
    let target = match arguments.output_size.as_deref() {
        Some(value) => parse_output_target(value)?,
        None => OutputRasterTarget::for_canvas(history.document().canvas())
            .map_err(crate::render_error)?,
    };
    let frame = arguments
        .frame
        .unwrap_or_else(|| history.document().project_timing().frame_range().start());
    let antialiasing = RasterAntialiasing::from(arguments.antialiasing);
    let background = arguments
        .background
        .map(RasterBackground::from)
        .unwrap_or_else(|| RasterBackground::default_for_model(history.document().channel_model()));
    let selection = PreflightSelection {
        frame,
        target,
        antialiasing,
        selected_background: background,
        settings,
    };
    let limits = preflight_limits(&arguments);
    crate::check_cli_cancelled(&cancelled)?;
    let mut media = open_source_media(loaded.sources(), MediaTools::default(), &|| {
        cancelled.load(Ordering::Acquire)
    })
    .map_err(|error| CliError::new(error.path(), error.message()))?;
    let outcome = preflight_current_frame(
        history.session(),
        &mut media,
        &selection,
        EvaluationLimits::default(),
        limits,
        &cancelled,
    )
    .map_err(|error| CliError::new(error.path(), error.message()))?;
    match outcome {
        PreflightOutcome::Complete(report) => {
            let current = report
                .identity
                .is_current(history.session(), &mut media, &selection, &cancelled)
                .map_err(|error| CliError::new(error.path(), error.message()))?;
            if !current {
                return Err(CliError::new(
                    "preflight.stale",
                    "report identity changed before publication",
                ));
            }
            let envelope = CompleteEnvelope::new(&report, limits, arguments.unit);
            crate::check_cli_cancelled(&cancelled)?;
            write_json(&envelope)
        }
        PreflightOutcome::Unavailable { reason } => {
            let envelope = UnavailableEnvelope {
                report_format_version: 1,
                algorithm_id: PREFLIGHT_ALGORITHM_ID,
                status: "unavailable",
                limits: LimitsJson::from(limits),
                reason: &reason,
                report: None::<()>,
            };
            write_json(&envelope)?;
            Err(CliError::new("preflight.unavailable", reason))
        }
        PreflightOutcome::Cancelled => Err(CliError::new(
            "preflight.cancelled",
            "analysis was cancelled before publication",
        )),
    }
}

/// Converts optional placement and threshold overrides into checked canonical project settings.
///
/// # Errors
/// Rejects incomplete dimensions and all negative, zero-size, non-finite, or unrepresentable values.
fn resolve_settings(
    current: &PrintPreparationSettings,
    arguments: &PreflightArgs,
) -> Result<PrintPreparationSettings, CliError> {
    let size_mm = match (arguments.print_width, arguments.print_height) {
        (None, None) => current.size_mm(),
        (Some(_), None) | (None, Some(_)) => {
            return Err(CliError::new(
                "preflight.print_size",
                "--print-width and --print-height must be supplied together",
            ));
        }
        (Some(width), Some(height)) => Some(match arguments.unit {
            EntryUnit::Mm => PhysicalPrintSizeMm::new(width, height)?,
            EntryUnit::In => PhysicalPrintSizeMm::from_inches(width, height)?,
        }),
    };
    let positive = match arguments.positive_threshold {
        Some(value) => threshold_mm(value, arguments.unit)?,
        None => current.minimum_positive_feature_width_mm(),
    };
    let gap = match arguments.gap_threshold {
        Some(value) => threshold_mm(value, arguments.unit)?,
        None => current.minimum_negative_gap_width_mm(),
    };
    PrintPreparationSettings::new(size_mm, positive, gap).map_err(Into::into)
}

/// Converts one command-local threshold to millimetres through the domain's checked unit helper.
///
/// # Errors
/// Returns checked inch-conversion diagnostics; complete-setting validation checks millimetres.
fn threshold_mm(value: f64, unit: EntryUnit) -> Result<f64, CliError> {
    match unit {
        EntryUnit::Mm => Ok(value),
        EntryUnit::In => {
            PrintPreparationSettings::threshold_mm_from_inches(value).map_err(Into::into)
        }
    }
}

/// Parses one explicit positive `WIDTHxHEIGHT` target and delegates raster bounds to the renderer.
///
/// # Errors
/// Rejects malformed, zero, overflowing, or renderer-over-budget dimensions.
fn parse_output_target(value: &str) -> Result<OutputRasterTarget, CliError> {
    let (width, height) = value.split_once('x').ok_or_else(|| {
        CliError::new(
            "preflight.output_size",
            "expected WIDTHxHEIGHT using positive integers",
        )
    })?;
    let width = width.parse::<u32>().map_err(|_| {
        CliError::new(
            "preflight.output_size",
            "width must be a positive 32-bit integer",
        )
    })?;
    let height = height.parse::<u32>().map_err(|_| {
        CliError::new(
            "preflight.output_size",
            "height must be a positive 32-bit integer",
        )
    })?;
    OutputRasterTarget::new(width, height).map_err(crate::render_error)
}

/// Replaces only explicitly supplied detector ceilings over the engine's bounded defaults.
///
/// # Errors
/// This conversion is total; zero values remain explicit values so the engine reports unavailable.
fn preflight_limits(arguments: &PreflightArgs) -> PreflightLimits {
    let defaults = PreflightLimits::default();
    PreflightLimits {
        max_pixels: arguments.max_pixels.unwrap_or(defaults.max_pixels),
        max_scratch_bytes: arguments
            .max_scratch_bytes
            .unwrap_or(defaults.max_scratch_bytes),
        max_stencil_offsets: arguments
            .max_stencil_offsets
            .unwrap_or(defaults.max_stencil_offsets),
        max_records: arguments.max_records.unwrap_or(defaults.max_records),
        max_operations: arguments.max_operations.unwrap_or(defaults.max_operations),
    }
}

/// Streams one stable JSON envelope without cloning the engine report's record and pixel-run data.
///
/// Publication is synchronous and begins only after the caller's freshness and cancellation
/// commit point; it is not polled for signals mid-stream.
///
/// # Errors
/// Returns a stable CLI output diagnostic if serialization, writing, or flushing fails.
fn write_json(value: &impl Serialize) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)
        .map_err(|error| CliError::new("preflight.output", error.to_string()))?;
    stdout
        .write_all(b"\n")
        .and_then(|()| stdout.flush())
        .map_err(|error| CliError::new("preflight.output", error.to_string()))
}

/// Describes one completed advisory report with stable names and borrowed finding arrays.
#[derive(Serialize)]
struct CompleteEnvelope<'a> {
    report_format_version: u32,
    algorithm_id: &'static str,
    status: &'static str,
    analysis_scope: ScopeJson,
    identity: IdentityJson<'a>,
    export: ExportJson,
    measurement: MeasurementJson,
    alpha: AlphaJson<'a>,
    resources: ResourcesJson,
    sampling: SamplingJson<'a>,
    limitations: [&'static str; 4],
}

impl<'a> CompleteEnvelope<'a> {
    /// Projects one accepted engine report into the stable CLI format without copying run arrays.
    fn new(report: &'a PreflightReport, limits: PreflightLimits, unit: EntryUnit) -> Self {
        let background = report.identity.selected_background();
        Self {
            report_format_version: 1,
            algorithm_id: report.identity.algorithm(),
            status: "completed_advisory",
            analysis_scope: ScopeJson {
                name: "final-transparent-composition-alpha",
                channel_separations_analyzed: false,
                alpha_is_measured_before_export_backing: true,
            },
            identity: IdentityJson::new(report),
            export: ExportJson {
                selected_background: background_name(background),
                flattens_transparency: !matches!(background, RasterBackground::Transparent),
            },
            measurement: MeasurementJson::new(report, unit),
            alpha: AlphaJson::new(report),
            resources: ResourcesJson {
                limits: LimitsJson::from(limits),
                charged_operations: report.operations,
            },
            sampling: SamplingJson {
                guard_mm_axes: report.sampling_guard_mm_axes.map(AxesJson::new),
                note: report.sampling_note.as_deref(),
            },
            limitations: [
                "Advisory only; the report does not certify print safety or predict a supplier process.",
                "Alpha analysis describes the final transparent composition, not separate channel plates.",
                "Opaque color-only details are invisible to alpha coverage analysis.",
                "Sampling guard and pixel probes are provisional and do not establish continuous feature width.",
            ],
        }
    }
}

/// Represents unavailable work explicitly, with no partial report or clean-looking empty findings.
#[derive(Serialize)]
struct UnavailableEnvelope<'a> {
    report_format_version: u32,
    algorithm_id: &'static str,
    status: &'static str,
    limits: LimitsJson,
    reason: &'a str,
    report: Option<()>,
}

/// Defines the bounded measurement's semantic scope independently of selected export backing.
#[derive(Serialize)]
struct ScopeJson {
    name: &'static str,
    channel_separations_analyzed: bool,
    alpha_is_measured_before_export_backing: bool,
}

/// Carries stable document, source, frame, scene, and final transparent-raster identities.
#[derive(Serialize)]
struct IdentityJson<'a> {
    document: DocumentIdentityJson,
    source_reference_id: &'a str,
    source: SourceIdentityJson<'a>,
    source_frame: Option<SourceFrameIdentityJson<'a>>,
    frame: u64,
    scene_fingerprint: &'a str,
    transparent_raster_identity: &'a str,
    alpha_sha256: &'a str,
    target: TargetJson,
    antialiasing: &'static str,
    selected_background: &'static str,
    alpha_policy: &'a str,
}

impl<'a> IdentityJson<'a> {
    /// Maps engine identity accessors into stable structured JSON fields.
    fn new(report: &'a PreflightReport) -> Self {
        let identity = &report.identity;
        let token = identity.token();
        Self {
            document: DocumentIdentityJson {
                id: token.document_id().0,
                revision: token.revision().0,
            },
            source_reference_id: identity.source_reference_id(),
            source: SourceIdentityJson::new(identity.source()),
            source_frame: identity.source_frame().map(SourceFrameIdentityJson::new),
            frame: identity.frame(),
            scene_fingerprint: identity.scene_fingerprint(),
            transparent_raster_identity: identity.transparent_raster_identity(),
            alpha_sha256: identity.alpha_sha256(),
            target: TargetJson::new(identity.target()),
            antialiasing: antialiasing_name(identity.antialiasing()),
            selected_background: background_name(identity.selected_background()),
            alpha_policy: identity.alpha_policy(),
        }
    }
}

/// Records the numeric project document and revision authority.
#[derive(Serialize)]
struct DocumentIdentityJson {
    id: u64,
    revision: u64,
}

/// Exposes stable source decoding fields without serializing Rust debug representations.
#[derive(Serialize)]
struct SourceIdentityJson<'a> {
    format: &'static str,
    width: u32,
    height: u32,
    content_hash: &'a str,
    decoded_pixel_hash: &'a str,
    svg_text: Option<SvgTextJson<'a>>,
}

impl<'a> SourceIdentityJson<'a> {
    /// Maps the exact engine source identity to its versioned wire fields.
    fn new(source: &'a toniator_engine::SourceIdentity) -> Self {
        Self {
            format: source_format_name(source.format),
            width: source.width,
            height: source.height,
            content_hash: &source.content_hash,
            decoded_pixel_hash: &source.decoded_pixel_hash,
            svg_text: source.svg_text.as_ref().map(|text| SvgTextJson {
                has_live_text_node: text.has_live_text_node,
                font_policy: &text.font_policy,
                rendered_glyph_coverage: text.rendered_glyph_coverage,
            }),
        }
    }
}

/// Records SVG text provenance with explicit stable field names.
#[derive(Serialize)]
struct SvgTextJson<'a> {
    has_live_text_node: bool,
    font_policy: &'a str,
    rendered_glyph_coverage: bool,
}

/// Exposes timed-media frame identity and rational time without floating-point conversion.
#[derive(Serialize)]
struct SourceFrameIdentityJson<'a> {
    source_fingerprint: &'a str,
    stream_index: u32,
    original_pts: i64,
    time_base: RationalJson,
    decoder_contract: &'a str,
    color_policy: &'a str,
    decoded_pixel_hash: &'a str,
}

impl<'a> SourceFrameIdentityJson<'a> {
    /// Borrows one timed-source identity and keeps its exact rational time base.
    fn new(frame: &'a toniator_engine::FrameIdentity) -> Self {
        Self {
            source_fingerprint: &frame.source_fingerprint,
            stream_index: frame.stream_index,
            original_pts: frame.original_pts,
            time_base: RationalJson {
                numerator: frame.time_base.numerator(),
                denominator: frame.time_base.denominator(),
            },
            decoder_contract: &frame.decoder_contract,
            color_policy: &frame.color_policy,
            decoded_pixel_hash: &frame.decoded_pixel_hash,
        }
    }
}

/// Encodes an exact nonnegative rational as integer fields.
#[derive(Serialize)]
struct RationalJson {
    numerator: u64,
    denominator: u64,
}

/// Records the final raster dimensions selected for identity and PPI reporting.
#[derive(Serialize)]
struct TargetJson {
    width: u32,
    height: u32,
}

impl TargetJson {
    /// Copies only the small dimensions from the renderer-owned target.
    fn new(target: OutputRasterTarget) -> Self {
        Self {
            width: target.width(),
            height: target.height(),
        }
    }
}

/// Separates selected consumer backing from transparent-alpha analysis.
#[derive(Serialize)]
struct ExportJson {
    selected_background: &'static str,
    flattens_transparency: bool,
}

/// Reports canonical physical settings and per-axis resolution when placement is known.
#[derive(Serialize)]
struct MeasurementJson {
    entry_unit: &'static str,
    placement_mm: Option<PlacementJson>,
    ppi_axes: Option<AxesJson>,
    positive_feature_threshold_mm: f64,
    negative_gap_threshold_mm: f64,
}

impl MeasurementJson {
    /// Projects the captured setting values and actual report PPI into canonical units.
    fn new(report: &PreflightReport, unit: EntryUnit) -> Self {
        let settings = report.identity.settings();
        Self {
            entry_unit: unit.as_str(),
            placement_mm: settings.size_mm().map(|size| {
                let (width, height) = size.millimetres();
                PlacementJson { width, height }
            }),
            ppi_axes: report.ppi_axes.map(AxesJson::new),
            positive_feature_threshold_mm: settings.minimum_positive_feature_width_mm(),
            negative_gap_threshold_mm: settings.minimum_negative_gap_width_mm(),
        }
    }
}

/// Stores canonical complete-canvas width and height in millimetres.
#[derive(Serialize)]
struct PlacementJson {
    width: f64,
    height: f64,
}

/// Stores explicit horizontal and vertical measurement axes.
#[derive(Serialize)]
struct AxesJson {
    horizontal: f64,
    vertical: f64,
}

impl AxesJson {
    /// Preserves unequal axis values instead of averaging physical anisotropy.
    fn new((horizontal, vertical): (f64, f64)) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

/// Contains the complete alpha inventory and independently configured width findings.
#[derive(Serialize)]
struct AlphaJson<'a> {
    policy: &'a str,
    support_alpha_minimum: u8,
    core_alpha_minimum: u8,
    histogram_by_alpha: &'a [u64],
    occupied_bounds_pixels: Option<BoundsJson>,
    support_components: SupportComponentsJson<'a>,
    positive_feature_check: WidthCheckJson<'a>,
    negative_gap_check: WidthCheckJson<'a>,
}

impl<'a> AlphaJson<'a> {
    /// Borrows the exact full histogram, component runs, and width-candidate runs.
    fn new(report: &'a PreflightReport) -> Self {
        Self {
            policy: report.identity.alpha_policy(),
            support_alpha_minimum: 1,
            core_alpha_minimum: 128,
            histogram_by_alpha: &report.alpha_histogram,
            occupied_bounds_pixels: report.occupied_bounds.map(BoundsJson::new),
            support_components: SupportComponentsJson(&report.support_components),
            positive_feature_check: WidthCheckJson::new(
                report.positive_status,
                &report.positive_candidates,
            ),
            negative_gap_check: WidthCheckJson::new(
                report.negative_status,
                &report.negative_candidates,
            ),
        }
    }
}

/// Describes one independently available physical width check and its exact pixel findings.
#[derive(Serialize)]
struct WidthCheckJson<'a> {
    status: &'static str,
    candidates: CandidateSliceJson<'a>,
}

impl<'a> WidthCheckJson<'a> {
    /// Converts the engine's closed status enum using explicit wire strings.
    fn new(status: WidthCheckStatus, candidates: &'a [ProbeCandidate]) -> Self {
        Self {
            status: width_status_name(status),
            candidates: CandidateSliceJson(candidates),
        }
    }
}

/// Encodes exclusive high-bound pixel coordinates.
#[derive(Serialize)]
struct BoundsJson {
    x0: u32,
    y0: u32,
    x1_exclusive: u32,
    y1_exclusive: u32,
}

impl BoundsJson {
    /// Copies one compact coordinate rectangle from the engine's exact finding bounds.
    fn new(bounds: toniator_engine::print_preflight::PixelBounds) -> Self {
        Self {
            x0: bounds.x0,
            y0: bounds.y0,
            x1_exclusive: bounds.x1_exclusive,
            y1_exclusive: bounds.y1_exclusive,
        }
    }
}

/// Streams support records directly from the boxed report without cloning their run vectors.
struct SupportComponentsJson<'a>(&'a [SupportComponent]);

impl Serialize for SupportComponentsJson<'_> {
    /// Serializes each borrowed component as one stable sequence element.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for component in self.0 {
            sequence.serialize_element(&SupportComponentJson::new(component))?;
        }
        sequence.end()
    }
}

/// Projects one component and its exact support and unresolved highlight runs.
#[derive(Serialize)]
struct SupportComponentJson<'a> {
    id: u32,
    pixels: u64,
    bounds: BoundsJson,
    runs: PixelRunsJson<'a>,
    max_alpha: u8,
    core_pixels: u64,
    low_coverage_pixels: u64,
    fractional_alpha_pixels: u64,
    boundary_associated_low_coverage_pixels: u64,
    unresolved_low_coverage_pixels: u64,
    unresolved_low_coverage_runs: PixelRunsJson<'a>,
    touches_canvas: bool,
}

impl<'a> SupportComponentJson<'a> {
    /// Borrows both exact run arrays and copies only scalar metadata.
    fn new(component: &'a SupportComponent) -> Self {
        Self {
            id: component.id,
            pixels: component.pixels,
            bounds: BoundsJson::new(component.bounds),
            runs: PixelRunsJson(&component.runs),
            max_alpha: component.max_alpha,
            core_pixels: component.core_pixels,
            low_coverage_pixels: component.low_coverage_pixels,
            fractional_alpha_pixels: component.fractional_alpha_pixels,
            boundary_associated_low_coverage_pixels: component
                .boundary_associated_low_coverage_pixels,
            unresolved_low_coverage_pixels: component.unresolved_low_coverage_pixels,
            unresolved_low_coverage_runs: PixelRunsJson(&component.unresolved_low_coverage_runs),
            touches_canvas: component.touches_canvas,
        }
    }
}

/// Streams exact horizontal output-pixel runs as small structured coordinates.
struct PixelRunsJson<'a>(&'a [toniator_engine::print_preflight::PixelRun]);

impl Serialize for PixelRunsJson<'_> {
    /// Serializes each borrowed run without allocating a parallel run vector.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for run in self.0 {
            sequence.serialize_element(&PixelRunJson {
                y: run.y,
                x0: run.x0,
                x1_exclusive: run.x1_exclusive,
            })?;
        }
        sequence.end()
    }
}

/// Encodes one exact run using an exclusive high x coordinate.
#[derive(Serialize)]
struct PixelRunJson {
    y: u32,
    x0: u32,
    x1_exclusive: u32,
}

/// Streams advisory candidates directly from their report-owned slice.
struct CandidateSliceJson<'a>(&'a [ProbeCandidate]);

impl Serialize for CandidateSliceJson<'_> {
    /// Serializes each borrowed candidate without copying candidate or run vectors.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for candidate in self.0 {
            sequence.serialize_element(&CandidateJson::new(candidate))?;
        }
        sequence.end()
    }
}

/// Projects one advisory candidate with an explicit basis, kind, and exact highlight runs.
#[derive(Serialize)]
struct CandidateJson<'a> {
    basis: &'static str,
    negative_gap: bool,
    kind: &'static str,
    original_component_id: u32,
    pixels: u64,
    bounds: BoundsJson,
    runs: PixelRunsJson<'a>,
    exterior_connected: bool,
}

impl<'a> CandidateJson<'a> {
    /// Borrows one candidate's exact highlight runs and maps enums to stable strings.
    fn new(candidate: &'a ProbeCandidate) -> Self {
        Self {
            basis: match candidate.basis {
                toniator_engine::print_preflight::MaskBasis::Support => "support",
                toniator_engine::print_preflight::MaskBasis::Core => "core",
            },
            negative_gap: candidate.negative_gap,
            kind: candidate_kind_name(candidate.kind),
            original_component_id: candidate.original_component_id,
            pixels: candidate.pixels,
            bounds: BoundsJson::new(candidate.bounds),
            runs: PixelRunsJson(&candidate.runs),
            exterior_connected: candidate.exterior_connected,
        }
    }
}

/// Records the exact engine limits and charged detector work.
#[derive(Serialize)]
struct ResourcesJson {
    limits: LimitsJson,
    charged_operations: u64,
}

/// Stores each independently configurable positive resource ceiling.
#[derive(Serialize)]
struct LimitsJson {
    max_pixels: u64,
    max_scratch_bytes: u64,
    max_stencil_offsets: u64,
    max_records: u64,
    max_operations: u64,
}

impl From<PreflightLimits> for LimitsJson {
    /// Copies the five small checked limits into their explicit JSON fields.
    fn from(limits: PreflightLimits) -> Self {
        Self {
            max_pixels: limits.max_pixels,
            max_scratch_bytes: limits.max_scratch_bytes,
            max_stencil_offsets: limits.max_stencil_offsets,
            max_records: limits.max_records,
            max_operations: limits.max_operations,
        }
    }
}

/// Records provisional physical sampling guard axes and engine qualification.
#[derive(Serialize)]
struct SamplingJson<'a> {
    guard_mm_axes: Option<AxesJson>,
    note: Option<&'a str>,
}

/// Returns a stable lowercase spelling for source formats.
fn source_format_name(value: toniator_engine::SourceFormat) -> &'static str {
    match value {
        toniator_engine::SourceFormat::Png => "png",
        toniator_engine::SourceFormat::Svg => "svg",
        toniator_engine::SourceFormat::Jpeg => "jpeg",
        toniator_engine::SourceFormat::Webp => "webp",
        toniator_engine::SourceFormat::Bmp => "bmp",
        toniator_engine::SourceFormat::Tiff => "tiff",
        toniator_engine::SourceFormat::OpenExr => "openexr",
        toniator_engine::SourceFormat::Avif => "avif",
        toniator_engine::SourceFormat::RawRgba => "raw_rgba",
    }
}

/// Returns a stable name for the exact selected edge-raster policy.
fn antialiasing_name(value: RasterAntialiasing) -> &'static str {
    match value {
        RasterAntialiasing::On => "on",
        RasterAntialiasing::Off => "off",
    }
}

/// Returns a stable name for the selected consumer backing.
fn background_name(value: RasterBackground) -> &'static str {
    match value {
        RasterBackground::Transparent => "transparent",
        RasterBackground::OpaqueBlack => "black",
        RasterBackground::OpaqueWhite => "white",
    }
}

/// Returns a stable name for one independent width-check outcome.
fn width_status_name(value: WidthCheckStatus) -> &'static str {
    match value {
        WidthCheckStatus::Disabled => "disabled",
        WidthCheckStatus::UnavailablePlacement => "unavailable_placement",
        WidthCheckStatus::SamplingLimited => "sampling_limited",
        WidthCheckStatus::AdvisoryCandidates => "advisory_candidates",
        WidthCheckStatus::NoWidthCandidatesWithUnresolved => "no_width_candidates_with_unresolved",
    }
}

/// Returns a stable name for one sampled morphology candidate type.
fn candidate_kind_name(value: ProbeCandidateKind) -> &'static str {
    match value {
        ProbeCandidateKind::NoErodedCenter => "no_eroded_center",
        ProbeCandidateKind::ProbeSensitiveResidual => "probe_sensitive_residual",
        ProbeCandidateKind::PossibleConstriction => "possible_constriction",
        ProbeCandidateKind::CanvasBoundary => "canvas_boundary",
    }
}
