//! Opt-in release stress profiling for one current headless Curve Motif case.
//!
//! Each process selects exactly one immutable source, materializes the asymmetric composed
//! mirror-and-phase recipe through document history, and evaluates it through the ordinary
//! canonical engine path.  It is intentionally diagnostic-only: it neither creates a catalog
//! entry nor changes product limits, persistent state, or renderer behavior.

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process,
    sync::Arc,
    time::Instant,
};

use sha2::{Digest, Sha256};
use toniator_domain::{
    ChannelId, ConnectedGeometryResponse, DensityEditedField, DocumentHistory,
    PatternCapabilityScope, PatternGeometryResponse, PatternOutputRealization, SourceReferenceId,
};
use toniator_engine::{
    EvaluationLimits, EvaluationPerformanceMetrics, EvaluationProfileCache, EvaluationRequest,
    GeometryOutput, ProfiledEvaluation, ResolvedSource, SourceFormatHint, encode_png,
    evaluate_profiled_cached_with_limits, write_svg,
};

/// Reuses the current asymmetric recipe and history-backed materialization used for validation.
///
/// The included example remains the sole owner of full visual evidence generation; this runner
/// only consumes its public current-recipe helpers and suppresses unused helper warnings.
#[allow(dead_code)]
#[path = "stage21b_prerequisite_curve_motif_validation.rs"]
mod validation_recipe;

/// The ordinary RGB channels installed by the authoritative default document constructor.
const RGB_CHANNELS: [ChannelId; 3] = [ChannelId(1), ChannelId(2), ChannelId(3)];

/// Selects exactly one immutable project-wide source for one isolated process invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceSelection {
    Png,
    Svg,
}

impl SourceSelection {
    /// Parses the runner's two explicit immutable-source spellings.
    ///
    /// # Errors
    ///
    /// Returns an argument error without reading either source when `value` is not `png` or
    /// `svg`.
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "png" => Ok(Self::Png),
            "svg" => Ok(Self::Svg),
            _ => Err("--source must be exactly png or svg".into()),
        }
    }

    /// Builds the intrinsic source/canvas case without reading or mutating the immutable input.
    fn case(self) -> validation_recipe::SourceCase {
        match self {
            Self::Png => validation_recipe::SourceCase {
                label: "stress-raster-1024x1024",
                input: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/raster-sample.png"
                ),
                width: 1024.0,
                height: 1024.0,
                hint: SourceFormatHint::Png,
            },
            Self::Svg => validation_recipe::SourceCase {
                label: "stress-vector-900x620",
                input: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/vector-sample.svg"
                ),
                width: 900.0,
                height: 620.0,
                hint: SourceFormatHint::Svg,
            },
        }
    }

    /// Returns the stable artifact filename component for this immutable source selection.
    const fn artifact_source_stem(self) -> &'static str {
        match self {
            Self::Png => "raster-1024x1024",
            Self::Svg => "vector-900x620",
        }
    }
}

/// Selects one bounded, typed Curve Motif stress mutation without accepting aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    Baseline,
    ZoomOut080,
    ZoomIn140,
    Rotation17,
    Rotation895,
    AspectWide2,
    AspectTall05,
    ResponseMinimum0,
    ResponseMaximum1,
    ResponseMinimum025,
    ResponseMaximum075,
    ZoomOut080Rotation17,
    MirrorOnly,
    PhaseOnly,
}

impl Mutation {
    /// Parses one stable mutation spelling without accepting a broader product-edit surface.
    ///
    /// # Errors
    ///
    /// Returns an argument error for a spelling outside the bounded Curve Motif stress matrix.
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "zoom-out-080" => Ok(Self::ZoomOut080),
            "zoom-in-140" => Ok(Self::ZoomIn140),
            "rotation-17" => Ok(Self::Rotation17),
            "rotation-895" => Ok(Self::Rotation895),
            "aspect-wide-2" => Ok(Self::AspectWide2),
            "aspect-tall-05" => Ok(Self::AspectTall05),
            "response-min-0" => Ok(Self::ResponseMinimum0),
            "response-max-1" => Ok(Self::ResponseMaximum1),
            "response-min-025" => Ok(Self::ResponseMinimum025),
            "response-max-075" => Ok(Self::ResponseMaximum075),
            "zoom-out-080-rotation-17" => Ok(Self::ZoomOut080Rotation17),
            "mirror-only" => Ok(Self::MirrorOnly),
            "phase-only" => Ok(Self::PhaseOnly),
            _ => Err("unknown --mutation; run with --help for supported values".into()),
        }
    }

    /// Returns the stable mutation spelling used in records and artifact filenames.
    const fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::ZoomOut080 => "zoom-out-080",
            Self::ZoomIn140 => "zoom-in-140",
            Self::Rotation17 => "rotation-17",
            Self::Rotation895 => "rotation-895",
            Self::AspectWide2 => "aspect-wide-2",
            Self::AspectTall05 => "aspect-tall-05",
            Self::ResponseMinimum0 => "response-min-0",
            Self::ResponseMaximum1 => "response-max-1",
            Self::ResponseMinimum025 => "response-min-025",
            Self::ResponseMaximum075 => "response-max-075",
            Self::ZoomOut080Rotation17 => "zoom-out-080-rotation-17",
            Self::MirrorOnly => "mirror-only",
            Self::PhaseOnly => "phase-only",
        }
    }

    /// Returns the authored odd-row transform recipe used before document-history materialization.
    ///
    /// Ordinary layout and response mutations retain the current composed mirror-plus-phase
    /// authority.  Only the two dedicated cases remove one component of that authored recipe.
    fn motif_layout(self) -> (bool, Option<f64>) {
        match self {
            Self::MirrorOnly => (true, None),
            Self::PhaseOnly => (false, Some(0.25)),
            _ => (true, Some(0.25)),
        }
    }
}

/// Stores validated, value-free runner choices without retaining unparsed command-line input.
#[derive(Clone, Copy, Debug)]
struct Arguments {
    source: SourceSelection,
    mutation: Mutation,
    skip_exports: bool,
}

/// Returns the compact stable command-line usage string without inspecting the filesystem.
fn usage() -> &'static str {
    "usage: cargo run -p toniator-engine --release --example stage21b_prerequisite_curve_motif_stress -- --source png|svg --mutation <baseline|zoom-out-080|zoom-in-140|rotation-17|rotation-895|aspect-wide-2|aspect-tall-05|response-min-0|response-max-1|response-min-025|response-max-075|zoom-out-080-rotation-17|mirror-only|phase-only> [--skip-exports]"
}

/// Parses one explicit source, one required bounded mutation, and the export omission switch.
///
/// # Errors
///
/// Returns a usage error for missing, duplicate, unknown, or malformed arguments without
/// materializing a document, decoding source bytes, or creating validation artifacts.
fn parse_arguments() -> Result<Arguments, String> {
    let mut source = None;
    let mut mutation = None;
    let mut skip_exports = false;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--source" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("--source requires a value\n{}", usage()))?;
                if source.replace(SourceSelection::parse(&value)?).is_some() {
                    return Err(format!("--source was supplied more than once\n{}", usage()));
                }
            }
            "--mutation" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("--mutation requires a value\n{}", usage()))?;
                if mutation.replace(Mutation::parse(&value)?).is_some() {
                    return Err(format!(
                        "--mutation was supplied more than once\n{}",
                        usage()
                    ));
                }
            }
            "--skip-exports" if !skip_exports => skip_exports = true,
            "--skip-exports" => {
                return Err(format!(
                    "--skip-exports was supplied more than once\n{}",
                    usage()
                ));
            }
            "--help" => return Err(usage().to_owned()),
            _ => return Err(format!("unknown argument {argument:?}\n{}", usage())),
        }
    }
    let source = source.ok_or_else(|| format!("--source is required\n{}", usage()))?;
    let mutation = mutation.ok_or_else(|| format!("--mutation is required\n{}", usage()))?;
    Ok(Arguments {
        source,
        mutation,
        skip_exports,
    })
}

/// Returns the workspace root implied by this checked-in engine example location.
///
/// # Panics
///
/// Panics only when the engine manifest no longer has the fixed two-parent relationship to the
/// workspace root required by this repository layout.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("engine manifest remains two levels below the workspace root")
        .to_path_buf()
}

/// Returns the fixed derived-artifact directory, creating no path outside `target/validation`.
///
/// # Errors
///
/// Returns a filesystem error before any export is written.  This runner owns only its fixed
/// child and never accepts an arbitrary output path from the command line.
fn artifact_directory() -> Result<PathBuf, String> {
    let directory = workspace_root()
        .join("target/validation")
        .join("stage21b-prerequisite-curve-motif/stress");
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "could not create Curve Motif stress artifact directory {}: {error}",
            directory.display()
        )
    })?;
    Ok(directory)
}

/// Builds an exact-byte authoritative request from the selected immutable source and session.
///
/// # Errors
///
/// Returns a source-boundary error without decoding, scaling, or modifying immutable source
/// bytes.  The source identifier exactly matches the history-backed validation materialization.
fn build_request(
    session: &toniator_domain::DocumentSession,
    case: &validation_recipe::SourceCase,
) -> Result<EvaluationRequest, String> {
    let bytes = Arc::<[u8]>::from(
        fs::read(case.input)
            .map_err(|error| format!("could not read immutable source {}: {error}", case.input))?,
    );
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .map_err(|error| format!("could not construct source identifier: {error}"))?;
    let source = ResolvedSource::new(source_id, bytes, case.hint)
        .map_err(|error| format!("could not resolve immutable source: {error}"))?;
    Ok(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        source,
    ))
}

/// Publishes one positive zoom factor through the document-wide density command.
///
/// # Errors
///
/// Returns command construction or history-publication errors without directly assigning an
/// effective channel field.  The fixed callers supply finite positive stress fixtures only.
fn apply_zoom(history: &mut DocumentHistory, zoom: f64) -> Result<(), String> {
    let default_density = history.document().pattern_settings().density.density;
    let command = history
        .document()
        .set_document_density_field(DensityEditedField::Density, default_density / zoom)
        .map_err(|error| format!("could not construct zoom {zoom}: {error}"))?;
    history
        .apply(&command)
        .map_err(|error| format!("could not publish zoom {zoom}: {error}"))?;
    Ok(())
}

/// Publishes one channel rotation through the effective-pattern typed command on every RGB channel.
///
/// # Errors
///
/// Returns the first authoritative command construction or history-publication error without
/// continuing to subsequent channels.  It never mutates resolved layout fields directly.
fn apply_rotation(history: &mut DocumentHistory, rotation: f64) -> Result<(), String> {
    for channel_id in RGB_CHANNELS {
        let command = history
            .document()
            .set_channel_pattern_rotation_for_effective(channel_id, rotation)
            .map_err(|error| {
                format!(
                    "could not construct rotation {rotation} for channel {}: {error}",
                    channel_id.0
                )
            })?;
        history.apply(&command).map_err(|error| {
            format!(
                "could not publish rotation {rotation} for channel {}: {error}",
                channel_id.0
            )
        })?;
    }
    Ok(())
}

/// Publishes one bounded connected-stroke response endpoint through effective output commands.
///
/// # Errors
///
/// Returns a stable output-kind, effective-pattern, command, or history error without making a
/// direct effective-field assignment.  Every current Curve Motif output must retain the existing
/// canonical connected response; any other output kind is a runner/setup error.
fn apply_response_endpoint(
    history: &mut DocumentHistory,
    edits_minimum: bool,
    endpoint: f64,
) -> Result<(), String> {
    for channel_id in RGB_CHANNELS {
        let outputs = history
            .document()
            .effective_channel_pattern(channel_id)
            .map_err(|error| {
                format!(
                    "could not resolve response outputs for channel {}: {error}",
                    channel_id.0
                )
            })?
            .output_settings;
        for output in outputs {
            let PatternGeometryResponse::Connected(response) = output.response else {
                return Err(format!(
                    "Curve Motif channel {} output {} is not a connected response",
                    channel_id.0, output.output_layer_id.0
                ));
            };
            let response = ConnectedGeometryResponse {
                minimum_thickness: if edits_minimum {
                    endpoint
                } else {
                    response.minimum_thickness
                },
                maximum_thickness: if edits_minimum {
                    response.maximum_thickness
                } else {
                    endpoint
                },
            };
            let command = history
                .document()
                .set_channel_output_response_for_effective(
                    channel_id,
                    output.output_layer_id,
                    PatternGeometryResponse::Connected(response),
                )
                .map_err(|error| {
                    format!(
                        "could not construct response for channel {} output {}: {error}",
                        channel_id.0, output.output_layer_id.0
                    )
                })?;
            history.apply(&command).map_err(|error| {
                format!(
                    "could not publish response for channel {} output {}: {error}",
                    channel_id.0, output.output_layer_id.0
                )
            })?;
        }
    }
    Ok(())
}

/// Applies one bounded ordinary layout or connected-response mutation through document history.
///
/// # Errors
///
/// Returns the first typed command or history failure without applying any subsequent part of a
/// compound mutation.  Mirror-only and phase-only select their authored recipe before history
/// materialization and deliberately produce no extra layout command.
fn apply_mutation(history: &mut DocumentHistory, mutation: Mutation) -> Result<(), String> {
    match mutation {
        Mutation::Baseline | Mutation::MirrorOnly | Mutation::PhaseOnly => {}
        Mutation::ZoomOut080 => apply_zoom(history, 0.80)?,
        Mutation::ZoomIn140 => apply_zoom(history, 1.40)?,
        Mutation::Rotation17 => apply_rotation(history, 17.0)?,
        Mutation::Rotation895 => apply_rotation(history, 89.5)?,
        Mutation::AspectWide2 | Mutation::AspectTall05 => {
            let aspect = if mutation == Mutation::AspectWide2 {
                2.0
            } else {
                0.5
            };
            let command = history
                .document()
                .set_document_density_field(DensityEditedField::Aspect, aspect)
                .map_err(|error| format!("could not construct aspect {aspect}: {error}"))?;
            history
                .apply(&command)
                .map_err(|error| format!("could not publish aspect {aspect}: {error}"))?;
        }
        Mutation::ResponseMinimum0 => apply_response_endpoint(history, true, 0.0)?,
        Mutation::ResponseMaximum1 => apply_response_endpoint(history, false, 1.0)?,
        Mutation::ResponseMinimum025 => apply_response_endpoint(history, true, 0.25)?,
        Mutation::ResponseMaximum075 => apply_response_endpoint(history, false, 0.75)?,
        Mutation::ZoomOut080Rotation17 => {
            apply_zoom(history, 0.80)?;
            apply_rotation(history, 17.0)?;
        }
    }
    Ok(())
}

/// Prints the active capability fields and exact composed Curve Motif output authority.
///
/// # Errors
///
/// Returns a domain capability or effective-definition error without evaluating, exporting, or
/// changing the materialized history-backed document.
fn print_active_fields(session: &toniator_domain::DocumentSession) -> Result<(), String> {
    let document = session.document();
    for channel_id in RGB_CHANNELS {
        let fields = document
            .pattern_capabilities(PatternCapabilityScope::Channel(channel_id))
            .map_err(|error| format!("could not project channel {} fields: {error}", channel_id.0))?
            .active_controls
            .into_iter()
            .map(|descriptor| format!("{:?}", descriptor.field))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        println!("active channel={} fields={fields}", channel_id.0);

        let definition = document
            .pattern_definition_for(channel_id)
            .ok_or_else(|| format!("channel {} resolves no definition", channel_id.0))?;
        for output in &definition.output_layers {
            if let PatternOutputRealization::CurveMotifPaths {
                structure_id,
                style,
                mirror_alternate_rows,
                alternate_row_phase,
                ..
            } = &output.realization
            {
                println!(
                    "motif channel={} output={} structure={} style={style:?} mirror_alternate_rows={} alternate_row_phase={alternate_row_phase:?}",
                    channel_id.0, output.id.0, structure_id.0, mirror_alternate_rows
                );
            }
        }
    }
    Ok(())
}

/// Prints the effective density-10 layout inputs that enter ordinary family identity.
///
/// # Errors
///
/// Returns an effective-pattern resolution error before canonical evaluation.  It reads the
/// resolved projection and does not construct a runner-owned motif-size or layout authority.
fn print_effective_layout(session: &toniator_domain::DocumentSession) -> Result<(), String> {
    for channel_id in RGB_CHANNELS {
        let effective = session
            .document()
            .effective_channel_pattern(channel_id)
            .map_err(|error| {
                format!("could not resolve channel {} layout: {error}", channel_id.0)
            })?;
        println!(
            "layout channel={} density={} aspect={} across_x={} across_y={} rotation={} translation_x={} translation_y={}",
            channel_id.0,
            effective.density.density,
            effective.density.aspect,
            effective.resolved_density.across_x,
            effective.resolved_density.across_y,
            effective.pattern_rotation_degrees,
            effective.translation_x,
            effective.translation_y,
        );
    }
    Ok(())
}

/// Prints deterministic cache, worker, stage, and workload records for one profiled invocation.
///
/// The diagnostic projection is read-only: it neither changes cache keys nor retains evaluation
/// geometry beyond the supplied profiled record.
fn print_profile_records(run: &str, profile: &ProfiledEvaluation) {
    println!(
        "cache run={run} aggregate={:?}",
        profile.diagnostics.aggregate
    );
    for channel in &profile.diagnostics.channels {
        let outputs = channel
            .outputs
            .iter()
            .map(|output| format!("{}:{:?}", output.output_layer_id.0, output.realization))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "cache channel={} family={:?} realization={:?} outputs={outputs}",
            channel.channel_id.0, channel.family, channel.realization
        );
    }
    print_performance_records(run, &profile.performance);
}

/// Prints deterministic shared-pool participation and coordinator-ordered workload records.
///
/// The metrics are diagnostic-only observations supplied by the ordinary evaluator and never
/// feed document authority, cached identity, or export content.
fn print_performance_records(run: &str, metrics: &EvaluationPerformanceMetrics) {
    println!(
        "workers run={run} configured={} observed={} registrations={}",
        metrics.configured_worker_count,
        metrics.observed_worker_count,
        metrics.worker_registration_count
    );
    for record in &metrics.records {
        let workloads = record
            .workloads
            .iter()
            .map(|workload| format!("{:?}:{}", workload.kind, workload.count))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "stage run={run} kind={:?} channel={} output={} cache={:?} execution={:?} elapsed_us={} workloads={workloads}",
            record.stage,
            record
                .channel_id
                .map_or_else(|| "-".to_owned(), |id| id.0.to_string()),
            record
                .output_layer_id
                .map_or_else(|| "-".to_owned(), |id| id.0.to_string()),
            record.cache,
            record.execution,
            record.elapsed.as_micros(),
        );
    }
}

/// Counts painter-owned canonical primitive categories without merging or reordering geometry.
///
/// The count is diagnostic-only and does not retain, mutate, or clone canonical geometry.
fn geometry_counts(profile: &ProfiledEvaluation) -> (usize, usize, usize, usize) {
    let mut circles = 0;
    let mut marks = 0;
    let mut strokes = 0;
    let mut regions = 0;
    for output in profile
        .result
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
    {
        match output.geometry() {
            GeometryOutput::CircularMarks(value) => circles += value.len(),
            GeometryOutput::CanonicalMarks(value) => marks += value.len(),
            GeometryOutput::CanonicalStrokes(value) => strokes += value.len(),
            GeometryOutput::CanonicalRegions(value) => regions += value.regions().len(),
        }
    }
    (circles, marks, strokes, regions)
}

/// Computes the lowercase SHA-256 identity of export bytes without retaining a second copy.
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Encodes and writes mutation-labeled canonical PNG/SVG exports below the fixed validation directory.
///
/// # Errors
///
/// Returns an encoder or filesystem error after successful canonical evaluation.  It writes no
/// source data and does not modify document/history state.
fn export_artifacts(
    source: SourceSelection,
    mutation: Mutation,
    profile: &ProfiledEvaluation,
) -> Result<(), String> {
    let directory = artifact_directory()?;
    let png_started = Instant::now();
    let png = encode_png(profile.result.raster())
        .map_err(|error| format!("PNG export failed: {error}"))?;
    let png_encode_elapsed = png_started.elapsed();
    let svg_started = Instant::now();
    let svg = write_svg(profile.result.scene()).into_bytes();
    let svg_encode_elapsed = svg_started.elapsed();
    let stem = format!("{}-{}", source.artifact_source_stem(), mutation.name());
    let png_path = directory.join(format!("{stem}.png"));
    let png_write_started = Instant::now();
    fs::write(&png_path, &png)
        .map_err(|error| format!("could not write stress PNG {}: {error}", png_path.display()))?;
    let png_write_elapsed = png_write_started.elapsed();
    let svg_path = directory.join(format!("{stem}.svg"));
    let svg_write_started = Instant::now();
    fs::write(&svg_path, &svg)
        .map_err(|error| format!("could not write stress SVG {}: {error}", svg_path.display()))?;
    let svg_write_elapsed = svg_write_started.elapsed();
    println!(
        "export png_path={} png_bytes={} png_sha256={} png_encode_us={} png_write_us={} svg_path={} svg_bytes={} svg_sha256={} svg_encode_us={} svg_write_us={}",
        png_path.display(),
        png.len(),
        sha256_hex(&png),
        png_encode_elapsed.as_micros(),
        png_write_elapsed.as_micros(),
        svg_path.display(),
        svg.len(),
        sha256_hex(&svg),
        svg_encode_elapsed.as_micros(),
        svg_write_elapsed.as_micros(),
    );
    Ok(())
}

/// Runs one density-10 Curve Motif cold/warm mutation through ordinary history and engine authority.
///
/// # Errors
///
/// Returns setup, document/history, evaluator, encoder, or filesystem errors without reporting a
/// successful case.  `--skip-exports` intentionally avoids PNG/SVG construction and writes so an
/// external RSS supervisor observes evaluator memory rather than exporter memory.
fn run(arguments: Arguments) -> Result<(), String> {
    let case = arguments.source.case();
    let (mirror_alternate_rows, alternate_row_phase) = arguments.mutation.motif_layout();
    let session = validation_recipe::materialized_session(
        &case,
        validation_recipe::curve_recipe(mirror_alternate_rows, alternate_row_phase),
    );
    let mut history = DocumentHistory::new(session);
    apply_mutation(&mut history, arguments.mutation)?;
    let session = history.session();
    let canvas = session.document().canvas();
    println!(
        "case source={} mutation={} canvas_width={} canvas_height={} density=10 mirror_alternate_rows={} alternate_row_phase={alternate_row_phase:?}",
        case.label,
        arguments.mutation.name(),
        canvas.width,
        canvas.height,
        mirror_alternate_rows,
    );
    print_active_fields(session)?;
    print_effective_layout(session)?;
    let request = build_request(session, &case)?;
    let mut cache = EvaluationProfileCache::default();
    let cold = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )
    .map_err(|error| format!("cold evaluation failed: {error}"))?;
    print_profile_records("cold", &cold);
    let (circles, marks, strokes, regions) = geometry_counts(&cold);
    println!(
        "geometry circles={circles} marks={marks} strokes={strokes} regions={regions} total={}",
        circles + marks + strokes + regions
    );
    if arguments.skip_exports {
        println!("export skipped=true");
    } else {
        export_artifacts(arguments.source, arguments.mutation, &cold)?;
    }
    let warm =
        evaluate_profiled_cached_with_limits(request, EvaluationLimits::default(), &mut cache)
            .map_err(|error| format!("warm evaluation failed: {error}"))?;
    print_profile_records("warm", &warm);
    Ok(())
}

/// Parses arguments, runs one case, and emits one stable process-level diagnostic on failure.
///
/// The process exits with code `2` for every argument, materialization, evaluator, export, or
/// filesystem failure so external supervisors can distinguish these from signals, OOM kills, and
/// timeout termination.
fn main() {
    match parse_arguments().and_then(run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("stage21b_prerequisite_curve_motif_stress error={error}");
            process::exit(2);
        }
    }
}
