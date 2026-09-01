//! Opt-in native stress profiling for one Stage 21A bundled-preset case.
//!
//! Run one preset/source/mutation per release process, preferably under an external
//! timeout and RSS supervisor. This executable is diagnostic only: it uses the
//! authoritative document commands and canonical engine/export path, but it neither
//! changes product limits nor writes persistent document state.

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Component, Path, PathBuf},
    process,
    time::Instant,
};

use sha2::{Digest, Sha256};
use toniator_domain::{
    CanvasSpec, ChannelId, ChannelTopologyTemplate, DensityEditedField, Document, DocumentCommand,
    DocumentHistory, DocumentSession, HalftoneChannelModel, MarkPrototype, PatternCapabilityScope,
    PatternGeometryResponse, PatternOutputRealization, PropertyFieldId, RegionGeometryFieldEdit,
    RegionSamplingStrategy, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationLimits, EvaluationPerformanceMetrics, EvaluationProfileCache, EvaluationRequest,
    GeometryOutput, ProfiledEvaluation, ResolvedSource, SourceFormatHint, encode_png,
    evaluate_profiled_cached_with_limits, write_svg,
};
use toniator_patterns::PresetRegistry;

/// Represents the immutable source baseline and its intrinsic canvas dimensions.
#[derive(Clone, Copy, Debug)]
struct SourceCase {
    name: &'static str,
    file_name: &'static str,
    format: SourceFormatHint,
    width: f64,
    height: f64,
}

/// Selects the authoritative channel topology used by one isolated stress process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StressModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

impl StressModel {
    /// Parses one exact model spelling without accepting presentation aliases.
    ///
    /// # Errors
    ///
    /// Returns an argument error before a document/history is built when the supplied model is not
    /// `rgb`, `cmyk`, or `source-color-alpha`.
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "rgb" => Ok(Self::Rgb),
            "cmyk" => Ok(Self::Cmyk),
            "source-color-alpha" => Ok(Self::SourceColorAlpha),
            _ => Err("--model must be rgb, cmyk, or source-color-alpha".into()),
        }
    }

    /// Returns the stable lowercase model identity used in diagnostic output and artifact names.
    const fn name(self) -> &'static str {
        match self {
            Self::Rgb => "rgb",
            Self::Cmyk => "cmyk",
            Self::SourceColorAlpha => "source-color-alpha",
        }
    }

    /// Returns the one domain-owned topology model selected by this process.
    const fn domain(self) -> HalftoneChannelModel {
        match self {
            Self::Rgb => HalftoneChannelModel::Rgb,
            Self::Cmyk => HalftoneChannelModel::Cmyk,
            Self::SourceColorAlpha => HalftoneChannelModel::SourceColorAlpha,
        }
    }
}

/// Parses the two immutable still-image inputs supported by the runner.
///
/// # Errors
///
/// Returns an argument error without reading either asset when the supplied source is not `png` or
/// `svg`.
fn parse_source(value: &str) -> Result<SourceCase, String> {
    match value {
        "png" => Ok(SourceCase {
            name: "png",
            file_name: "raster-sample.png",
            format: SourceFormatHint::Png,
            width: 1024.0,
            height: 1024.0,
        }),
        "svg" => Ok(SourceCase {
            name: "svg",
            file_name: "vector-sample.svg",
            format: SourceFormatHint::Svg,
            width: 900.0,
            height: 620.0,
        }),
        _ => Err("--source must be png or svg".into()),
    }
}

/// Identifies one capability-respecting stress edit applied after preset publication.
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
    ShapeRotation37,
    ZoomOut080Rotation17,
    RegionReferencePoint,
    RegionAreaAverage,
}

impl Mutation {
    /// Parses one stable command-line mutation spelling without accepting aliases.
    ///
    /// # Errors
    ///
    /// Returns an argument error without changing document authority for an unsupported spelling.
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
            "shape-rotation-37" => Ok(Self::ShapeRotation37),
            "zoom-out-080-rotation-17" => Ok(Self::ZoomOut080Rotation17),
            "region-reference-point" => Ok(Self::RegionReferencePoint),
            "region-area-average" => Ok(Self::RegionAreaAverage),
            _ => Err("unknown --mutation; run with --help for the supported values".into()),
        }
    }

    /// Returns the stable mutation spelling used in evidence and artifact names without allocation.
    fn name(self) -> &'static str {
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
            Self::ShapeRotation37 => "shape-rotation-37",
            Self::ZoomOut080Rotation17 => "zoom-out-080-rotation-17",
            Self::RegionReferencePoint => "region-reference-point",
            Self::RegionAreaAverage => "region-area-average",
        }
    }
}

/// Stores validated command-line settings without retaining unparsed user input.
#[derive(Debug)]
struct Arguments {
    preset_id: String,
    source: SourceCase,
    model: StressModel,
    mutation: Mutation,
    canvas_scale: f64,
    artifacts: Option<PathBuf>,
    skip_exports: bool,
}

/// Parses the runner's exact required and optional command-line arguments.
///
/// # Errors
///
/// Returns a human-readable usage or validation message for an unknown flag, a missing value,
/// duplicate required argument, conflicting export options, non-finite scale, or unsupported
/// source/model/mutation spelling.
fn parse_arguments() -> Result<Arguments, String> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
    let mut skip_exports = false;
    let mut iterator = env::args().skip(1);
    while let Some(flag) = iterator.next() {
        if flag == "--help" {
            return Err(usage());
        }
        if flag == "--skip-exports" {
            if skip_exports {
                return Err(format!("{flag} was supplied more than once\n{}", usage()));
            }
            skip_exports = true;
            continue;
        }
        if !matches!(
            flag.as_str(),
            "--preset" | "--source" | "--model" | "--mutation" | "--canvas-scale" | "--artifacts"
        ) {
            return Err(format!("unknown argument {flag:?}\n{}", usage()));
        }
        let value = iterator
            .next()
            .ok_or_else(|| format!("{flag} requires a value\n{}", usage()))?;
        if values.insert(flag.clone(), value).is_some() {
            return Err(format!("{flag} was supplied more than once\n{}", usage()));
        }
    }
    let required = |name: &str| {
        values
            .get(name)
            .cloned()
            .ok_or_else(|| format!("{name} is required\n{}", usage()))
    };
    let preset_id = required("--preset")?;
    let source = parse_source(&required("--source")?)?;
    let model = StressModel::parse(&required("--model")?)?;
    let mutation = Mutation::parse(&required("--mutation")?)?;
    let canvas_scale = values.get("--canvas-scale").map_or(Ok(1.0), |value| {
        value
            .parse::<f64>()
            .map_err(|_| "--canvas-scale must be a finite positive number".to_owned())
    })?;
    if !canvas_scale.is_finite() || canvas_scale <= 0.0 {
        return Err("--canvas-scale must be a finite positive number".into());
    }
    if skip_exports && values.contains_key("--artifacts") {
        return Err("--skip-exports cannot be combined with --artifacts".into());
    }
    Ok(Arguments {
        preset_id,
        source,
        model,
        mutation,
        canvas_scale,
        artifacts: values.get("--artifacts").map(PathBuf::from),
        skip_exports,
    })
}

/// Returns the complete compact CLI usage string without inspecting the filesystem or document authority.
fn usage() -> String {
    [
        "usage: cargo run -p toniator-engine --release --example stage21a_preset_stress -- ",
        "--preset <id> --source png|svg --model rgb|cmyk|source-color-alpha --mutation <baseline|zoom-out-080|zoom-in-140|rotation-17|rotation-895|aspect-wide-2|aspect-tall-05|response-min-0|response-max-1|response-min-025|response-max-075|shape-rotation-37|zoom-out-080-rotation-17|region-reference-point|region-area-average> [--canvas-scale <finite-positive>] [--artifacts target/validation/<child>] [--skip-exports]",
    ]
    .join("\n")
}

/// Returns the workspace root implied by this crate's fixed checked-in location.
///
/// # Panics
///
/// Panics only if the Cargo manifest directory has no parent hierarchy, which would mean the
/// checked-in workspace layout no longer matches this executable's declared location.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("engine manifest stays two levels below workspace root")
        .to_path_buf()
}

/// Resolves an optional artifact directory and refuses paths outside `target/validation`.
///
/// # Errors
///
/// Returns a filesystem or containment error without creating output files. The function creates
/// only the supplied validation child directory and rejects a canonicalized symlink escape.
fn resolve_artifact_directory(argument: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let Some(argument) = argument else {
        return Ok(None);
    };
    let root = workspace_root();
    let validation = root.join("target/validation");
    fs::create_dir_all(&validation)
        .map_err(|error| format!("could not create target/validation: {error}"))?;
    let validation = validation
        .canonicalize()
        .map_err(|error| format!("could not canonicalize target/validation: {error}"))?;
    let child = if argument.is_absolute() {
        argument
            .strip_prefix(&validation)
            .map_err(|_| format!("--artifacts must name a child of {}", validation.display()))?
    } else {
        argument
            .strip_prefix("target/validation")
            .map_err(|_| "relative --artifacts must begin with target/validation/".to_owned())?
    };
    validate_artifact_child(child)?;
    let candidate = validation.join(child);
    fs::create_dir_all(&candidate).map_err(|error| {
        format!(
            "could not create artifact directory {}: {error}",
            candidate.display()
        )
    })?;
    let candidate = candidate
        .canonicalize()
        .map_err(|error| format!("could not canonicalize artifact directory: {error}"))?;
    if candidate == validation || !candidate.starts_with(&validation) {
        return Err(format!(
            "--artifacts must name a child of {}",
            validation.display()
        ));
    }
    Ok(Some(candidate))
}

/// Rejects empty, parent-traversing, or non-normal artifact child paths before directory creation.
///
/// # Errors
///
/// Returns a containment error without touching the filesystem. Normal components are retained so
/// the caller can later canonicalize and reject a pre-existing symlink escape as a second defense.
fn validate_artifact_child(child: &Path) -> Result<(), String> {
    if child.as_os_str().is_empty()
        || child
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("--artifacts must name a nonempty normal child of target/validation".into());
    }
    Ok(())
}

/// Rounds scaled intrinsic dimensions to the integer canvas units required by raster evaluation.
///
/// # Errors
///
/// Returns an error when finite-positive input cannot produce at least one finite integral unit on
/// either axis. The resolved dimensions are always reported with the requested scale, so this
/// diagnostic runner never hides the engine's integer-raster contract.
fn scaled_canvas(source: SourceCase, canvas_scale: f64) -> Result<CanvasSpec, String> {
    let width = (source.width * canvas_scale).round();
    let height = (source.height * canvas_scale).round();
    if !width.is_finite() || !height.is_finite() || width < 1.0 || height < 1.0 {
        return Err("--canvas-scale produces a canvas smaller than one raster unit".into());
    }
    Ok(CanvasSpec { width, height })
}

/// Builds a fresh model-selected document through the canonical topology/history authority.
///
/// # Errors
///
/// Returns an authoritative domain/history error when the requested model topology or supplied
/// preset cannot be published. Incompatible SourceColorAlpha path recipes deliberately return this
/// error rather than being coerced into a different output or model.
fn build_history(
    registry: &PresetRegistry,
    preset_id: &str,
    source: SourceCase,
    model: StressModel,
    canvas_scale: f64,
) -> Result<DocumentHistory, String> {
    let canvas = scaled_canvas(source, canvas_scale)?;
    let source_id = SourceReferenceId::new(format!("stage21a-preset-stress-{}", source.name))
        .map_err(|error| format!("could not build source ID: {error}"))?;
    let document = Document::new_default_document(canvas, SourceReference::Assigned(source_id))
        .map_err(|error| format!("could not build default document: {error}"))?;
    let mut history = DocumentHistory::new(
        DocumentSession::new(document)
            .map_err(|error| format!("could not start session: {error}"))?,
    );
    if history.document().channel_model() != Some(model.domain()) {
        let template = {
            let channel = history
                .document()
                .channel_topology()
                .and_then(|topology| topology.channels().first())
                .ok_or_else(|| {
                    "document.channel_topology: model selection requires stored topology".to_owned()
                })?;
            ChannelTopologyTemplate {
                pattern_instance: channel.pattern_instance.clone(),
            }
        };
        let topology = history
            .document()
            .canonical_channel_topology(model.domain(), template)
            .map_err(|error| format!("could not build {} topology: {error}", model.name()))?;
        history
            .apply(&DocumentCommand::ReplaceChannelTopology {
                model: model.domain(),
                topology,
            })
            .map_err(|error| format!("could not publish {} topology: {error}", model.name()))?;
    }
    registry
        .apply_to_document_base(&mut history, preset_id)
        .map_err(|error| {
            format!(
                "could not apply preset {preset_id:?} for {}: {error}",
                model.name()
            )
        })?;
    Ok(history)
}

/// Derives the currently authoritative channel IDs in topology order for diagnostic mutations.
///
/// # Errors
///
/// Returns a stable topology error when the selected model failed to publish a stored channel
/// topology. The returned IDs are read from the current document rather than inferred from RGB
/// defaults, so CMYK and SourceColorAlpha stay model-valid.
fn channel_ids(history: &DocumentHistory) -> Result<Vec<ChannelId>, String> {
    history
        .document()
        .channel_topology()
        .map(|topology| {
            topology
                .channels()
                .iter()
                .map(|channel| channel.id)
                .collect()
        })
        .ok_or_else(|| "document.channel_topology: stress runner requires stored topology".into())
}

/// Reports whether one active channel capability exposes a named field.
///
/// # Errors
///
/// Returns the domain capability error when the effective channel cannot be projected.
fn channel_supports(
    history: &DocumentHistory,
    channel_id: ChannelId,
    field: PropertyFieldId,
) -> Result<bool, String> {
    Ok(history
        .document()
        .pattern_capabilities(PatternCapabilityScope::Channel(channel_id))
        .map_err(|error| {
            format!(
                "could not project channel {}/capabilities: {error}",
                channel_id.0
            )
        })?
        .active_controls
        .iter()
        .any(|descriptor| descriptor.field == field))
}

/// Reports whether an effective channel owns an authored closed-shape mark whose rotation changes geometry.
///
/// # Errors
///
/// Returns a stable missing-channel/definition error without constructing a command. Circle marks
/// and every non-mark realization deliberately return `false` because their shape rotation is not
/// a meaningful geometry stress input.
fn channel_has_rotatable_authored_shape(
    history: &DocumentHistory,
    channel_id: ChannelId,
) -> Result<bool, String> {
    let definition = history
        .document()
        .pattern_definition_for(channel_id)
        .ok_or_else(|| format!("channel {} resolves no pattern definition", channel_id.0))?;
    Ok(definition.output_layers.iter().any(|output| {
        matches!(
            output.realization,
            PatternOutputRealization::MarkPrototype {
                prototype: MarkPrototype::AuthoredClosedShape { .. },
                ..
            }
        )
    }))
}

/// Records mutation outcomes that preserve typed no-op authority while making stress coverage explicit.
#[derive(Default)]
struct MutationReport {
    rotation_rejection: Option<String>,
    region_sampling_applied: usize,
    region_sampling_noop: usize,
}

/// Converts one positive zoom factor to authoritative base density and publishes that history command.
///
/// # Errors
///
/// Returns the domain command construction or publication error without altering a failed
/// transition. The caller supplies only the runner's finite positive zoom fixtures.
fn apply_zoom(history: &mut DocumentHistory, zoom: f64) -> Result<(), String> {
    let default_density = history.document().pattern_settings().density.density;
    let command = history
        .document()
        .set_document_density_field(DensityEditedField::Density, default_density / zoom)
        .map_err(|error| format!("could not convert zoom {zoom} to density: {error}"))?;
    history
        .apply(&command)
        .map_err(|error| format!("could not publish zoom mutation: {error}"))?;
    Ok(())
}

/// Applies eligible model-selected pattern rotations and records the required artwork-weighted rejection.
///
/// # Errors
///
/// Returns a typed command construction/publication or effective-pattern resolution error without
/// continuing to later channels. Eligible channels receive the same requested rotation in stable
/// topology order.
///
/// # Panics
///
/// Panics only if the domain unexpectedly accepts an inactive artwork-weighted rotation command,
/// which violates the typed rotation contract being audited.
fn apply_rotation(
    history: &mut DocumentHistory,
    rotation: f64,
    report: &mut MutationReport,
) -> Result<(), String> {
    for channel_id in channel_ids(history)? {
        if channel_supports(history, channel_id, PropertyFieldId::RotationDegrees)? {
            let command = history
                .document()
                .set_channel_pattern_rotation_for_effective(channel_id, rotation)
                .map_err(|error| {
                    format!(
                        "could not build rotation for channel {}: {error}",
                        channel_id.0
                    )
                })?;
            history.apply(&command).map_err(|error| {
                format!(
                    "could not publish rotation for channel {}: {error}",
                    channel_id.0
                )
            })?;
        } else {
            let error = history
                .document()
                .set_channel_pattern_rotation_for_effective(channel_id, rotation)
                .expect_err(
                    "inactive artwork-weighted rotation must reject through the typed command",
                );
            if error.path() != "channel.pattern.rotation" {
                return Err(format!(
                    "channel {} rejected rotation at unexpected path {}",
                    channel_id.0,
                    error.path()
                ));
            }
            let effective = history
                .document()
                .effective_channel_pattern(channel_id)
                .map_err(|error| format!("could not resolve rejected rotation: {error}"))?;
            if effective.pattern_rotation_degrees != 0.0 {
                return Err(format!(
                    "channel {} retained nonzero artwork-weighted rotation",
                    channel_id.0
                ));
            }
            report.rotation_rejection = Some(format!("expected:{}", error.path()));
        }
    }
    Ok(())
}

/// Applies one requested typed mutation without bypassing effective-pattern or history authority.
///
/// # Errors
///
/// Returns a command-construction or command-publication error. No successful command is rolled
/// back because each transition is intentionally part of the isolated stress case. Region-sampling
/// requests already selected by one effective output are counted and skipped so the domain's
/// authoritative semantic no-op rejection remains intact. The combined zoom/rotation fixture
/// always publishes zoom before asking the domain to rotate each eligible topology channel.
///
/// # Panics
///
/// Panics only if an inactive artwork-weighted rotation command unexpectedly succeeds, which
/// violates the typed domain rotation contract this runner is explicitly auditing.
fn apply_mutation(
    history: &mut DocumentHistory,
    mutation: Mutation,
) -> Result<MutationReport, String> {
    let mut report = MutationReport::default();
    match mutation {
        Mutation::Baseline => {}
        Mutation::ZoomOut080 => apply_zoom(history, 0.80)?,
        Mutation::ZoomIn140 => apply_zoom(history, 1.40)?,
        Mutation::AspectWide2 | Mutation::AspectTall05 => {
            let aspect = if mutation == Mutation::AspectWide2 {
                2.0
            } else {
                0.5
            };
            let command = history
                .document()
                .set_document_density_field(DensityEditedField::Aspect, aspect)
                .map_err(|error| format!("could not build aspect mutation: {error}"))?;
            history
                .apply(&command)
                .map_err(|error| format!("could not publish aspect mutation: {error}"))?;
        }
        Mutation::Rotation17 => apply_rotation(history, 17.0, &mut report)?,
        Mutation::Rotation895 => apply_rotation(history, 89.5, &mut report)?,
        Mutation::ShapeRotation37 => {
            for channel_id in channel_ids(history)? {
                if channel_supports(history, channel_id, PropertyFieldId::ShapeRotationDegrees)?
                    && channel_has_rotatable_authored_shape(history, channel_id)?
                {
                    let command = history
                        .document()
                        .set_channel_shape_rotation_for_effective(channel_id, 37.0)
                        .map_err(|error| {
                            format!(
                                "could not build shape rotation for channel {}: {error}",
                                channel_id.0
                            )
                        })?;
                    history.apply(&command).map_err(|error| {
                        format!(
                            "could not publish shape rotation for channel {}: {error}",
                            channel_id.0
                        )
                    })?;
                }
            }
        }
        Mutation::ResponseMinimum0
        | Mutation::ResponseMaximum1
        | Mutation::ResponseMinimum025
        | Mutation::ResponseMaximum075 => {
            let (edits_minimum, endpoint) = match mutation {
                Mutation::ResponseMinimum0 => (true, 0.0),
                Mutation::ResponseMaximum1 => (false, 1.0),
                Mutation::ResponseMinimum025 => (true, 0.25),
                Mutation::ResponseMaximum075 => (false, 0.75),
                _ => unreachable!("the enclosing mutation branch is response-only"),
            };
            for channel_id in channel_ids(history)? {
                let outputs = history
                    .document()
                    .effective_channel_pattern(channel_id)
                    .map_err(|error| {
                        format!(
                            "could not resolve output response for channel {}: {error}",
                            channel_id.0
                        )
                    })?
                    .output_settings;
                for output in outputs {
                    let command = match output.response {
                        PatternGeometryResponse::Marks(response) => history
                            .document()
                            .set_channel_output_response_for_effective(
                                channel_id,
                                output.output_layer_id,
                                PatternGeometryResponse::Marks(
                                    toniator_domain::MarkGeometryResponse {
                                        minimum_fill: if edits_minimum {
                                            endpoint
                                        } else {
                                            response.minimum_fill
                                        },
                                        maximum_fill: if edits_minimum {
                                            response.maximum_fill
                                        } else {
                                            endpoint
                                        },
                                    },
                                ),
                            ),
                        PatternGeometryResponse::Connected(response) => history
                            .document()
                            .set_channel_output_response_for_effective(
                                channel_id,
                                output.output_layer_id,
                                PatternGeometryResponse::Connected(
                                    toniator_domain::ConnectedGeometryResponse {
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
                                        bias: response.bias,
                                    },
                                ),
                            ),
                        PatternGeometryResponse::Regions(_) => {
                            if edits_minimum {
                                history
                                    .document()
                                    .set_channel_region_response_field_for_effective(
                                        channel_id,
                                        output.output_layer_id,
                                        RegionGeometryFieldEdit::MinimumFill(endpoint),
                                    )
                            } else {
                                history
                                    .document()
                                    .set_channel_region_response_field_for_effective(
                                        channel_id,
                                        output.output_layer_id,
                                        RegionGeometryFieldEdit::MaximumFill(endpoint),
                                    )
                            }
                        }
                    }
                    .map_err(|error| {
                        format!(
                            "could not build response mutation for channel {} output {}: {error}",
                            channel_id.0, output.output_layer_id.0
                        )
                    })?;
                    history.apply(&command).map_err(|error| {
                        format!(
                            "could not publish response mutation for channel {} output {}: {error}",
                            channel_id.0, output.output_layer_id.0
                        )
                    })?;
                }
            }
        }
        Mutation::ZoomOut080Rotation17 => {
            apply_zoom(history, 0.80)?;
            apply_rotation(history, 17.0, &mut report)?;
        }
        Mutation::RegionReferencePoint | Mutation::RegionAreaAverage => {
            let sampling = if mutation == Mutation::RegionReferencePoint {
                RegionSamplingStrategy::ReferencePoint
            } else {
                RegionSamplingStrategy::AreaAverage
            };
            for channel_id in channel_ids(history)? {
                let outputs = history
                    .document()
                    .effective_channel_pattern(channel_id)
                    .map_err(|error| {
                        format!(
                            "could not resolve region output for channel {}: {error}",
                            channel_id.0
                        )
                    })?
                    .output_settings;
                for output in outputs {
                    let PatternGeometryResponse::Regions(mut response) = output.response else {
                        continue;
                    };
                    if response.sampling == sampling {
                        report.region_sampling_noop += 1;
                        continue;
                    }
                    response.sampling = sampling;
                    let command = history
                        .document()
                        .set_selected_channel_region_response_for_effective(
                            channel_id,
                            output.output_layer_id,
                            response,
                        )
                        .map_err(|error| {
                            format!(
                                "could not build selected-copy region sampling mutation for channel {} output {}: {error}",
                                channel_id.0, output.output_layer_id.0
                            )
                        })?;
                    history.apply(&command).map_err(|error| {
                        format!(
                            "could not publish selected-copy region sampling mutation for channel {} output {}: {error}",
                            channel_id.0, output.output_layer_id.0
                        )
                    })?;
                    report.region_sampling_applied += 1;
                }
            }
        }
    }
    Ok(report)
}

/// Builds one exact-byte authoritative engine request for the supplied document history.
///
/// # Errors
///
/// Returns a filesystem/source-boundary error without decoding, scaling, or altering the immutable asset bytes.
fn build_request(
    history: &DocumentHistory,
    source: SourceCase,
) -> Result<EvaluationRequest, String> {
    let source_path = workspace_root().join("assets").join(source.file_name);
    let bytes = fs::read(&source_path).map_err(|error| {
        format!(
            "could not read immutable source {}: {error}",
            source_path.display()
        )
    })?;
    let source_id = SourceReferenceId::new(format!("stage21a-preset-stress-{}", source.name))
        .map_err(|error| format!("could not build request source ID: {error}"))?;
    let resolved = ResolvedSource::new(source_id, bytes, source.format)
        .map_err(|error| format!("could not build resolved source: {error}"))?;
    Ok(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        resolved,
    ))
}

/// Prints the stable active field projection for each current model channel.
///
/// # Errors
///
/// Returns the capability projection error without evaluating or exporting the document.
fn print_active_fields(history: &DocumentHistory) -> Result<(), String> {
    for channel_id in channel_ids(history)? {
        let mut fields = BTreeSet::new();
        for descriptor in history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(channel_id))
            .map_err(|error| format!("could not project active fields: {error}"))?
            .active_controls
        {
            fields.insert(format!("{:?}", descriptor.field));
        }
        println!(
            "active channel={} fields={}",
            channel_id.0,
            fields.into_iter().collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}

/// Prints the effective layout authority used by each current model evaluator request.
///
/// The diagnostic reads the same resolved density, aspect, rotation, and translation values that
/// enter family cache identity. It does not derive an alternate layout or mutate document history.
///
/// # Errors
///
/// Returns effective-pattern resolution errors before canonical evaluation.
fn print_effective_layout(history: &DocumentHistory) -> Result<(), String> {
    for channel_id in channel_ids(history)? {
        let effective = history
            .document()
            .effective_channel_pattern(channel_id)
            .map_err(|error| format!("could not resolve effective layout: {error}"))?;
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

/// Prints deterministic worker and stage/workload records from one profiled invocation.
///
/// The projection reads diagnostic-only metrics and never enters cache keys, document state, or
/// export identity.
fn print_profile_records(run: &str, metrics: &EvaluationPerformanceMetrics) {
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
            "stage run={run} kind={:?} channel={} output={} cache={:?} execution={:?} elapsed_us={} workloads={}",
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
            workloads
        );
    }
}

/// Counts canonical primitive categories without merging or changing painter order.
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

/// Computes the lowercase SHA-256 identity of already-produced export bytes without retaining them.
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Converts evidence components to a portable filename segment without retaining arbitrary path syntax.
///
/// Non-ASCII-alphanumeric path syntax becomes `_`, preserving the artifact-directory containment
/// invariant enforced before this component is used.
fn sanitized_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// Writes successful exports below the already-validated artifact directory.
///
/// # Errors
///
/// Returns a filesystem error after canonical evaluation/export succeeds. It writes only the two
/// deterministic files rooted in `directory`; it never writes source bytes or document state.
fn write_artifacts(
    directory: Option<&Path>,
    arguments: &Arguments,
    png: &[u8],
    svg: &[u8],
) -> Result<(), String> {
    let Some(directory) = directory else {
        return Ok(());
    };
    let scale = sanitized_component(&format!("{:.6}", arguments.canvas_scale));
    let stem = format!(
        "preset-{}-{}-model-{}-{}-scale-{}",
        sanitized_component(&arguments.preset_id),
        arguments.source.name,
        arguments.model.name(),
        arguments.mutation.name(),
        scale
    );
    fs::write(directory.join(format!("{stem}.png")), png)
        .map_err(|error| format!("could not write stress PNG artifact: {error}"))?;
    fs::write(directory.join(format!("{stem}.svg")), svg)
        .map_err(|error| format!("could not write stress SVG artifact: {error}"))?;
    Ok(())
}

/// Executes one model-selected cold/export/warm stress case through the canonical authority path.
///
/// # Errors
///
/// Returns setup, domain, evaluator, exporter, or artifact errors without reporting a successful
/// case. With `--skip-exports`, it intentionally omits PNG/SVG construction and artifact writes
/// so external RSS supervision isolates canonical preview evaluation. The requested model remains
/// visible in the case and artifact identity. It does not catch process OOM, signals, or external
/// timeouts; supervisors classify those process-level outcomes.
fn run(arguments: Arguments) -> Result<(), String> {
    let registry = PresetRegistry::bundled();
    if registry.find(&arguments.preset_id).is_none() {
        return Err(format!(
            "unknown bundled preset ID {:?}",
            arguments.preset_id
        ));
    }
    let artifact_directory = resolve_artifact_directory(arguments.artifacts.as_deref())?;
    let mut history = build_history(
        &registry,
        &arguments.preset_id,
        arguments.source,
        arguments.model,
        arguments.canvas_scale,
    )?;
    let mutation_report = apply_mutation(&mut history, arguments.mutation)?;
    let canvas = history.document().canvas();
    println!(
        "case preset={} source={} model={} mutation={} canvas_width={} canvas_height={} canvas_scale={}",
        arguments.preset_id,
        arguments.source.name,
        arguments.model.name(),
        arguments.mutation.name(),
        canvas.width,
        canvas.height,
        arguments.canvas_scale
    );
    print_active_fields(&history)?;
    print_effective_layout(&history)?;
    let request = build_request(&history, arguments.source)?;
    let mut cache = EvaluationProfileCache::default();
    let cold = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )
    .map_err(|error| format!("cold evaluation failed: {error}"))?;
    print_profile_records("cold", &cold.performance);
    let (circles, marks, strokes, regions) = geometry_counts(&cold);
    println!(
        "geometry circles={circles} marks={marks} strokes={strokes} regions={regions} total={}",
        circles + marks + strokes + regions
    );
    if arguments.skip_exports {
        println!("export skipped=true");
    } else {
        let png_started = Instant::now();
        let png = encode_png(cold.result.raster())
            .map_err(|error| format!("PNG export failed: {error}"))?;
        let png_elapsed = png_started.elapsed();
        let svg_started = Instant::now();
        let svg = write_svg(cold.result.scene()).into_bytes();
        let svg_elapsed = svg_started.elapsed();
        println!(
            "export png_bytes={} png_sha256={} png_elapsed_us={} svg_bytes={} svg_sha256={} svg_elapsed_us={}",
            png.len(),
            sha256_hex(&png),
            png_elapsed.as_micros(),
            svg.len(),
            sha256_hex(&svg),
            svg_elapsed.as_micros()
        );
        write_artifacts(artifact_directory.as_deref(), &arguments, &png, &svg)?;
    }
    println!(
        "rotation_rejection={}",
        mutation_report
            .rotation_rejection
            .as_deref()
            .unwrap_or("none")
    );
    println!(
        "region_sampling_mutation applied={} noop={}",
        mutation_report.region_sampling_applied, mutation_report.region_sampling_noop
    );
    let warm =
        evaluate_profiled_cached_with_limits(request, EvaluationLimits::default(), &mut cache)
            .map_err(|error| format!("warm evaluation failed: {error}"))?;
    print_profile_records("warm", &warm.performance);
    Ok(())
}

/// Parses arguments, runs one isolated case, and reports only a stable process-level error line.
///
/// The process exits with code `2` for setup/evaluation/export errors so an external supervisor can
/// distinguish it from a signal, OOM kill, or timeout.
fn main() {
    match parse_arguments().and_then(run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("stage21a_preset_stress error={error}");
            process::exit(2);
        }
    }
}
