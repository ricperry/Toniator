//! Native Curves recipe operations. The retained renderer remains live during
//! the migration; these bodies use its atomic geometry helpers only.

use crate::artwork_pipeline::{
    ArtworkPipelineSettings, OutputChannelId, PreparedSource, ResolvedChannelField,
    ResolvedChannelFields, resolve_channel_fields_cancellable,
};
use crate::curve_render::{
    CurveDeformationLimits, CurveDeformationRequest, CurveGeometry, CurveInkLayer,
    CurveModulationLimits, CurveModulationRequest, CurveOutline, deform_curve_paths_cancellable,
    modulate_curve_paths_cancellable,
};
use crate::model::{
    AlternateTileTransform, CurveLayout, CurvePath, MotifCoverage, WebCurveChannel,
    WebCurveSettings, parse_hex_color,
};
use crate::pattern::{ArtboardSpace, CanonicalPatternOutput, PathPatternOutput};
use crate::pattern_definition::{
    DefinitionParameterScope, LiteralValue, NativeRecipeOperationError,
    NativeRecipeOperationRegistry, PatternDefinition, PatternInstanceParameters,
    REGISTERED_OPERATIONS, RecipeArgument, RecipeExecutionContext, RecipeNode,
    RecipeOperationInputs, RecipeOperationParameters, RecipeRuntimeValue,
    RegisteredNativeRecipeOperation,
};
use crate::render::{Channel, InkLayer, WebGrid, calculate_web_grid};
use crate::site_distribution::DistributionField;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct CurvesPlacement {
    pub artboard: ArtboardSpace,
    pub grid: WebGrid,
    /// Retained downstream placement transform inputs, without a duplicate
    /// `WebCurveChannel` facade.
    pub grid_rotation: f64,
    pub grid_pivot_x: f64,
    pub grid_pivot_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CurvesSamples {
    pub placement: CurvesPlacement,
    pub field: DistributionField,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CurvesMotif {
    pub path: CurvePath,
    pub close_ends: bool,
    pub smooth_join: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CurvesDeformedPaths {
    pub placement: CurvesPlacement,
    pub paths: Vec<Vec<crate::CurvePoint>>,
    /// Modulation needs this retained close/open decision when it builds final
    /// variable-width outlines; smoothness was already consumed by sampling.
    pub closed: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CurvesModulatedPaths {
    pub artboard: ArtboardSpace,
    pub outlines: Vec<CurveOutline>,
}

pub static CURVES_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 6] = [
    RegisteredNativeRecipeOperation {
        id: "curves.placement",
        version: 1,
        execute: curves_placement,
    },
    RegisteredNativeRecipeOperation {
        id: "curves.source-sample",
        version: 1,
        execute: curves_source_sample,
    },
    RegisteredNativeRecipeOperation {
        id: "curves.motif-selection",
        version: 1,
        execute: curves_motif_selection,
    },
    RegisteredNativeRecipeOperation {
        id: "curves.deformation",
        version: 1,
        execute: curves_deformation,
    },
    RegisteredNativeRecipeOperation {
        id: "curves.width-modulation",
        version: 1,
        execute: curves_width_modulation,
    },
    RegisteredNativeRecipeOperation {
        id: "curves.emit-paths",
        version: 1,
        execute: curves_emit_paths,
    },
];
pub static CURVES_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::with_preflight(
        REGISTERED_OPERATIONS.entries(),
        &CURVES_NATIVE_OPERATIONS,
        validate_curves_recipe_execution_assets,
    );

#[cfg(test)]
thread_local! {
    static NATIVE_NODE_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_native_node_invocation() {
    NATIVE_NODE_INVOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_native_node_invocation() {}

#[cfg(test)]
fn reset_native_node_invocations() {
    NATIVE_NODE_INVOCATIONS.with(|count| count.set(0));
}

#[cfg(test)]
fn native_node_invocations() -> usize {
    NATIVE_NODE_INVOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static ORCHESTRATION_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROVIDER_CACHE_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CANCEL_AFTER_FIRST_ORCHESTRATED_CHANNEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn record_orchestration_invocation() {
    ORCHESTRATION_INVOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_orchestration_invocation() {}

#[cfg(test)]
pub(crate) fn reset_curves_recipe_orchestration_instrumentation() {
    ORCHESTRATION_INVOCATIONS.with(|count| count.set(0));
    PROVIDER_CACHE_MISSES.with(|count| count.set(0));
    CANCEL_AFTER_FIRST_ORCHESTRATED_CHANNEL.with(|value| value.set(false));
}

#[cfg(test)]
pub(crate) fn curves_recipe_orchestration_invocations() -> usize {
    ORCHESTRATION_INVOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
fn provider_cache_misses() -> usize {
    PROVIDER_CACHE_MISSES.with(std::cell::Cell::get)
}

#[cfg(test)]
fn cancel_after_first_orchestrated_channel_for_test() {
    CANCEL_AFTER_FIRST_ORCHESTRATED_CHANNEL.with(|value| value.set(true));
}

pub(crate) fn validate_curves_recipe_execution_assets(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    context: &RecipeExecutionContext<'_>,
) -> Result<(), NativeRecipeOperationError> {
    for node in
        definition.recipe.nodes.iter().filter(|node| {
            node.operation.id == "curves.motif-selection" && node.operation.version == 1
        })
    {
        let shared = boolean_value(resolve_node_argument(
            definition,
            instance,
            context,
            node,
            "use-shared-curve",
        )?)?;
        let asset_parameter = if shared { "shared-path" } else { "path" };
        let LiteralValue::SvgAsset(digest) =
            resolve_node_argument(definition, instance, context, node, asset_parameter)?
        else {
            return Err(NativeRecipeOperationError::new(format!(
                "Curves motif node `{}` {asset_parameter} must reference an SVG asset",
                node.id
            )));
        };
        decode_curve_asset(digest, &definition.assets)?;
    }
    Ok(())
}

/// Executes the immutable bundled Curves recipe for the authoritative settings
/// and artwork pipeline without changing production render dispatch. Each
/// enabled semantic output channel runs the common generic recipe once; this
/// function only merges those one-layer canonical results in retained order.
pub fn execute_bundled_curves_recipe_cancellable(
    prepared: &PreparedSource,
    settings: &WebCurveSettings,
    pipeline: &ArtworkPipelineSettings,
    token: &crate::CancellationToken,
) -> anyhow::Result<CanonicalPatternOutput> {
    record_orchestration_invocation();
    token.checkpoint()?;
    let output_channels = if matches!(
        pipeline.assignment,
        crate::artwork_pipeline::ChannelAssignment::LegacyCompatibility(_)
    ) {
        OutputChannelId::CMYK.to_vec()
    } else {
        pipeline.output_model.channels().to_vec()
    };
    let enabled = output_channels
        .iter()
        .copied()
        .filter(|channel| settings.channels.get(channel.to_legacy_ink()).enabled)
        .collect::<Vec<_>>();
    let adaptation = crate::curves_recipe::adapt_curves_settings_to_recipe(settings)?;
    let provider = CurvesRecipeSourceProvider {
        prepared,
        pipeline,
        enabled: &enabled,
        fields: RefCell::new(HashMap::new()),
    };
    let artboard = ArtboardSpace {
        width: settings.output_width,
        height: settings.output_height,
    };
    let crosshatch_color = if matches!(
        pipeline.assignment,
        crate::artwork_pipeline::ChannelAssignment::LegacyCompatibility(_)
    ) {
        Some(
            parse_hex_color(&settings.crosshatch_color)
                .ok_or_else(|| anyhow::anyhow!("invalid crosshatch curve color"))?,
        )
    } else {
        None
    };
    let mut layers = Vec::new();
    for (semantic_channel_index, channel) in output_channels.iter().copied().enumerate() {
        token.checkpoint()?;
        if !settings.channels.get(channel.to_legacy_ink()).enabled {
            continue;
        }
        let context = RecipeExecutionContext {
            artboard,
            output_channel: Some(channel),
            source_field_provider: Some(&provider),
            source_field: None,
            source_generation: prepared.generation,
            resolved_field_generation: prepared.generation,
            semantic_channel_index: semantic_channel_index as u32,
            enabled_layer_index: layers.len() as u32,
            definition_assets: &[],
            cancellation: token,
        };
        let CanonicalPatternOutput::Paths(output) = adaptation.definition.execute_recipe(
            &adaptation.instance,
            &context,
            &CURVES_NATIVE_OPERATION_REGISTRY,
        )?
        else {
            anyhow::bail!("bundled Curves recipe did not emit canonical paths");
        };
        if output.geometry.layers.len() != 1 {
            anyhow::bail!("bundled Curves recipe must emit exactly one semantic layer per channel");
        }
        let mut layer = output
            .geometry
            .layers
            .into_iter()
            .next()
            .expect("length checked");
        if layer.layer.channel != Channel::from(channel.to_legacy_ink()) {
            anyhow::bail!("bundled Curves recipe emitted a layer for the wrong semantic channel");
        }
        if !layer.layer.enabled {
            anyhow::bail!("bundled Curves recipe emitted an unexpectedly disabled channel");
        }
        if let Some(color) = crosshatch_color {
            layer.layer.color = color;
        }
        layers.push(layer);
        #[cfg(test)]
        if CANCEL_AFTER_FIRST_ORCHESTRATED_CHANNEL.with(|value| value.replace(false)) {
            token.cancel();
        }
    }
    token.checkpoint()?;
    Ok(CanonicalPatternOutput::Paths(PathPatternOutput {
        geometry: CurveGeometry {
            width: artboard.width,
            height: artboard.height,
            layers,
        },
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CurvesFieldCacheKey {
    columns: u32,
    rows: u32,
    source_generation: u64,
    resolved_field_generation: u64,
    enabled: Vec<&'static str>,
}

struct CurvesRecipeSourceProvider<'a> {
    prepared: &'a PreparedSource,
    pipeline: &'a ArtworkPipelineSettings,
    enabled: &'a [OutputChannelId],
    fields: RefCell<HashMap<CurvesFieldCacheKey, ResolvedChannelFields>>,
}

impl crate::RecipeSourceFieldProvider for CurvesRecipeSourceProvider<'_> {
    fn resolve_source_field(
        &self,
        channel: OutputChannelId,
        columns: u32,
        rows: u32,
        cancellation: &crate::CancellationToken,
    ) -> Result<DistributionField, NativeRecipeOperationError> {
        cancellation.checkpoint().map_err(cancelled)?;
        let key = CurvesFieldCacheKey {
            columns,
            rows,
            source_generation: self.prepared.generation,
            resolved_field_generation: self.prepared.generation,
            enabled: self
                .enabled
                .iter()
                .map(|channel| channel.stable_id())
                .collect(),
        };
        if !self.fields.borrow().contains_key(&key) {
            #[cfg(test)]
            PROVIDER_CACHE_MISSES.with(|count| count.set(count.get() + 1));
            let fields = resolve_channel_fields_cancellable(
                self.prepared,
                self.pipeline,
                columns,
                rows,
                self.prepared.generation,
                self.enabled,
                cancellation,
            )
            .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            self.fields.borrow_mut().insert(key.clone(), fields);
        }
        let fields = self.fields.borrow();
        let field = fields
            .get(&key)
            .and_then(|fields| fields.field(channel))
            .ok_or_else(|| {
                NativeRecipeOperationError::new(
                    "Curves source field provider has no requested semantic channel",
                )
            })?;
        distribution_field(field)
    }
}

fn distribution_field(
    field: &ResolvedChannelField,
) -> Result<DistributionField, NativeRecipeOperationError> {
    let values = field
        .values()
        .iter()
        .zip(field.coverage())
        .map(|(value, coverage)| f64::from(*value * *coverage))
        .collect();
    DistributionField::new(field.bounds.width, field.bounds.height, values)
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))
}

fn curves_placement(
    context: &RecipeExecutionContext<'_>,
    _: &RecipeOperationInputs<'_>,
    p: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(cancelled)?;
    let width = u32::try_from(integer(p, "output-width")?).map_err(|_| {
        NativeRecipeOperationError::new("Curves placement output width exceeds u32 artboard limit")
    })?;
    let height = u32::try_from(integer(p, "output-height")?).map_err(|_| {
        NativeRecipeOperationError::new("Curves placement output height exceeds u32 artboard limit")
    })?;
    if width == 0 || height == 0 || context.artboard != (ArtboardSpace { width, height }) {
        return Err(NativeRecipeOperationError::new(
            "Curves placement artboard must match execution context",
        ));
    }
    let long = number(p, "long-edge-cells")?;
    let resolution = number(p, "resolution-scale")?;
    if !(2.0..=10_000.0).contains(&long)
        || !(0.0..=100.0).contains(&resolution)
        || resolution <= 0.0
    {
        return Err(NativeRecipeOperationError::new(
            "Curves placement density or resolution is outside supported range",
        ));
    }
    let grid_rotation = number(p, "grid-rotation")?;
    let grid_pivot_x = number(p, "grid-pivot-x")?;
    let grid_pivot_y = number(p, "grid-pivot-y")?;
    let offset_x = number(p, "offset-x")?;
    let offset_y = number(p, "offset-y")?;
    let grid = calculate_web_grid(
        width,
        height,
        (long * resolution.max(0.05)).round().max(2.0),
    );
    if u64::from(grid.cols) * u64::from(grid.rows) > 1_000_000 {
        return Err(NativeRecipeOperationError::new(
            "Curves placement exceeds bounded source grid",
        ));
    }
    Ok(RecipeRuntimeValue::CurvesPlacement(CurvesPlacement {
        artboard: context.artboard,
        grid,
        grid_rotation,
        grid_pivot_x,
        grid_pivot_y,
        offset_x,
        offset_y,
    }))
}

fn curves_source_sample(
    context: &RecipeExecutionContext<'_>,
    i: &RecipeOperationInputs<'_>,
    _: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    let RecipeRuntimeValue::CurvesPlacement(placement) = input(i, "placement")? else {
        return Err(NativeRecipeOperationError::new(
            "input `placement` must be Curves placement",
        ));
    };
    context.cancellation.checkpoint().map_err(cancelled)?;
    let field = match context.source_field_provider {
        Some(provider) => provider.resolve_source_field(
            channel(context)?,
            placement.grid.cols,
            placement.grid.rows,
            context.cancellation,
        )?,
        None => context.source_field.cloned().ok_or_else(|| {
            NativeRecipeOperationError::new(
                "Curves source-sample requires source field or provider",
            )
        })?,
    };
    if field.dimensions() != (placement.grid.cols, placement.grid.rows) {
        return Err(NativeRecipeOperationError::new(
            "Curves source field dimensions do not match placement grid",
        ));
    }
    Ok(RecipeRuntimeValue::CurvesSamples(CurvesSamples {
        placement: placement.clone(),
        field,
    }))
}

fn curves_motif_selection(
    context: &RecipeExecutionContext<'_>,
    _: &RecipeOperationInputs<'_>,
    p: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(cancelled)?;
    let shared = boolean(p, "use-shared-curve")?;
    let (key, close, smooth) = if shared {
        (
            "shared-path",
            boolean(p, "shared-close-ends")?,
            boolean(p, "shared-smooth-join")?,
        )
    } else {
        (
            "path",
            boolean(p, "close-ends")?,
            boolean(p, "smooth-join")?,
        )
    };
    let path = decode_curve_asset(svg(p, key)?, context.definition_assets)?;
    Ok(RecipeRuntimeValue::CurvesMotif(CurvesMotif {
        path,
        close_ends: close,
        smooth_join: smooth,
    }))
}

fn curves_deformation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(cancelled)?;
    let RecipeRuntimeValue::CurvesPlacement(placement) = input(inputs, "placement")? else {
        return Err(NativeRecipeOperationError::new(
            "input `placement` must be Curves placement",
        ));
    };
    let RecipeRuntimeValue::CurvesMotif(motif) = input(inputs, "motif")? else {
        return Err(NativeRecipeOperationError::new(
            "input `motif` must be Curves motif",
        ));
    };
    let settings = temporary_deformation_settings(placement, parameters)?;
    let channel = temporary_deformation_channel(placement, parameters)?;
    let paths = deform_curve_paths_cancellable(
        CurveDeformationRequest {
            path: &motif.path,
            close_ends: motif.close_ends,
            smooth_join: motif.smooth_join,
            settings: &settings,
            channel: &channel,
            grid: &placement.grid,
        },
        context.cancellation,
        Some(CurveDeformationLimits {
            max_paths: 10_000,
            max_points_per_path: 20_000,
            max_total_points: 1_000_000,
        }),
    )
    .map_err(|error| NativeRecipeOperationError::new(format!("Curves deformation: {error}")))?;
    context.cancellation.checkpoint().map_err(cancelled)?;
    Ok(RecipeRuntimeValue::CurvesDeformedPaths(
        CurvesDeformedPaths {
            placement: placement.clone(),
            paths,
            closed: motif.close_ends,
        },
    ))
}

fn curves_width_modulation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(cancelled)?;
    let RecipeRuntimeValue::CurvesDeformedPaths(deformed) = input(inputs, "paths")? else {
        return Err(NativeRecipeOperationError::new(
            "input `paths` must be Curves deformed paths",
        ));
    };
    let RecipeRuntimeValue::CurvesSamples(samples) = input(inputs, "samples")? else {
        return Err(NativeRecipeOperationError::new(
            "input `samples` must be Curves samples",
        ));
    };
    if deformed.placement != samples.placement {
        return Err(NativeRecipeOperationError::new(
            "Curves width modulation placement provenance must match",
        ));
    }
    if deformed.placement.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "Curves width modulation artboard must match execution context",
        ));
    }
    if samples.field.dimensions() != (deformed.placement.grid.cols, deformed.placement.grid.rows) {
        return Err(NativeRecipeOperationError::new(
            "Curves width modulation source dimensions must match placement grid",
        ));
    }
    let settings = temporary_modulation_settings(&deformed.placement, parameters)?;
    let channel = temporary_modulation_channel(parameters)?;
    let sample_value = |index| samples.field.values()[index];
    let outlines = modulate_curve_paths_cancellable(
        CurveModulationRequest {
            paths: &deformed.paths,
            closed: deformed.closed,
            grid: &deformed.placement.grid,
            settings: &settings,
            channel: &channel,
            sample_value: &sample_value,
        },
        context.cancellation,
        Some(CurveModulationLimits {
            max_points_per_path: 20_000,
            max_total_points: 1_000_000,
            max_outlines: 10_000,
            max_commands: 4_000_000,
        }),
    )
    .map_err(|error| {
        NativeRecipeOperationError::new(format!("Curves width modulation: {error}"))
    })?;
    context.cancellation.checkpoint().map_err(cancelled)?;
    Ok(RecipeRuntimeValue::CurvesModulatedPaths(
        CurvesModulatedPaths {
            artboard: deformed.placement.artboard,
            outlines,
        },
    ))
}

fn curves_emit_paths(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(cancelled)?;
    let RecipeRuntimeValue::CurvesModulatedPaths(modulated) = input(inputs, "paths")? else {
        return Err(NativeRecipeOperationError::new(
            "input `paths` must be Curves modulated paths",
        ));
    };
    if modulated.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "Curves modulated paths artboard must match execution context",
        ));
    }
    let enabled = boolean(parameters, "enabled")?;
    let color = match parameters.get("color") {
        Some(LiteralValue::Text(color)) => parse_hex_color(color).ok_or_else(|| {
            NativeRecipeOperationError::new("Curves output color must be a six-digit hex color")
        })?,
        Some(_) => {
            return Err(NativeRecipeOperationError::new(
                "Curves parameter `color` must be text",
            ));
        }
        None => {
            return Err(NativeRecipeOperationError::new(
                "missing Curves parameter `color`",
            ));
        }
    };
    let opacity = bounded_number(parameters, "opacity", 0.0, 1.0)?;
    let channel = channel(context)?.to_legacy_ink();
    validate_final_outlines(&modulated.outlines)?;
    context.cancellation.checkpoint().map_err(cancelled)?;
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Paths(PathPatternOutput {
            geometry: CurveGeometry {
                width: modulated.artboard.width,
                height: modulated.artboard.height,
                layers: vec![CurveInkLayer {
                    layer: InkLayer {
                        channel: channel.into(),
                        enabled,
                        color,
                        opacity: opacity as f32,
                    },
                    outlines: if enabled {
                        modulated.outlines.clone()
                    } else {
                        Vec::new()
                    },
                }],
            },
        }),
    ))
}

const CURVES_MAX_FINAL_OUTLINES: usize = 10_000;
const CURVES_MAX_FINAL_COMMANDS: usize = 4_000_000;

fn validate_final_outlines(outlines: &[CurveOutline]) -> Result<(), NativeRecipeOperationError> {
    validate_final_outlines_with_limits(
        outlines,
        CURVES_MAX_FINAL_OUTLINES,
        CURVES_MAX_FINAL_COMMANDS,
    )
}

fn validate_final_outlines_with_limits(
    outlines: &[CurveOutline],
    max_outlines: usize,
    max_commands: usize,
) -> Result<(), NativeRecipeOperationError> {
    if outlines.len() > max_outlines {
        return Err(NativeRecipeOperationError::new(
            "Curves emit exceeds bounded outline count",
        ));
    }
    let command_count = outlines.iter().try_fold(0usize, |total, outline| {
        total
            .checked_add(outline.commands.len())
            .ok_or_else(|| NativeRecipeOperationError::new("Curves emit command count overflow"))
    })?;
    if command_count > max_commands {
        return Err(NativeRecipeOperationError::new(
            "Curves emit exceeds bounded command count",
        ));
    }
    Ok(())
}

fn temporary_modulation_settings(
    placement: &CurvesPlacement,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<WebCurveSettings, NativeRecipeOperationError> {
    let min_mark = bounded_number(parameters, "min-mark", 0.0, 1_000.0)?;
    let max_mark = bounded_number(parameters, "max-mark", 0.0, 1_000.0)?;
    if max_mark < min_mark {
        return Err(NativeRecipeOperationError::new(
            "Curves width modulation maximum width must not be less than minimum width",
        ));
    }
    Ok(WebCurveSettings {
        output_width: placement.artboard.width,
        output_height: placement.artboard.height,
        min_mark,
        max_mark,
        ..WebCurveSettings::default()
    })
}

fn temporary_modulation_channel(
    parameters: &RecipeOperationParameters<'_>,
) -> Result<WebCurveChannel, NativeRecipeOperationError> {
    Ok(WebCurveChannel {
        threshold: bounded_number(parameters, "threshold", 0.0, 1.0)?,
        max_size: bounded_number(parameters, "max-size", 0.0, 10_000.0)?,
        scale: bounded_number(parameters, "scale", 0.0, 100.0)?,
        output_quality: positive_number(parameters, "output-quality", 100.0)?,
        ..WebCurveChannel::default()
    })
}

fn temporary_deformation_settings(
    placement: &CurvesPlacement,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<WebCurveSettings, NativeRecipeOperationError> {
    let min_mark = bounded_number(parameters, "min-mark", 0.0, 1_000.0)?;
    let max_mark = bounded_number(parameters, "max-mark", 0.0, 1_000.0)?;
    if max_mark < min_mark {
        return Err(NativeRecipeOperationError::new(
            "Curves deformation maximum width must not be less than minimum width",
        ));
    }
    Ok(WebCurveSettings {
        output_width: placement.artboard.width,
        output_height: placement.artboard.height,
        min_mark,
        max_mark,
        layout: curve_layout(parameters)?,
        ..WebCurveSettings::default()
    })
}

fn temporary_deformation_channel(
    placement: &CurvesPlacement,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<WebCurveChannel, NativeRecipeOperationError> {
    Ok(WebCurveChannel {
        grid_rotation: placement.grid_rotation,
        grid_pivot_x: placement.grid_pivot_x,
        grid_pivot_y: placement.grid_pivot_y,
        offset_x: placement.offset_x,
        offset_y: placement.offset_y,
        curve_scale: bounded_number(parameters, "curve-scale", 0.1, 500.0)?,
        motif_coverage: motif_coverage(parameters)?,
        motif_bleed: bounded_number(parameters, "motif-bleed", 0.0, 100.0)?,
        tile_count: bounded_count(parameters, "tile-count")?,
        tile_angle: number(parameters, "tile-angle")?,
        tile_offset: number(parameters, "tile-offset")?,
        stack_count: bounded_count(parameters, "stack-count")?,
        stack_spacing: bounded_number(parameters, "stack-spacing", -10_000.0, 10_000.0)?,
        stack_angle: number(parameters, "stack-angle")?,
        stack_offset: number(parameters, "stack-offset")?,
        alternate_stack_offset: number(parameters, "alternate-stack-offset")?,
        alternate_tile_transform: alternate_tile_transform(parameters)?,
        scale: bounded_number(parameters, "scale", 0.0, 100.0)?,
        max_size: bounded_number(parameters, "max-size", 0.0, 10_000.0)?,
        output_quality: positive_number(parameters, "output-quality", 100.0)?,
        ..WebCurveChannel::default()
    })
}

fn curve_layout(
    parameters: &RecipeOperationParameters<'_>,
) -> Result<CurveLayout, NativeRecipeOperationError> {
    match choice(parameters, "layout")? {
        "full-width" => Ok(CurveLayout::FullWidth),
        "motif-pattern" => Ok(CurveLayout::MotifPattern),
        value => Err(NativeRecipeOperationError::new(format!(
            "Curves deformation layout `{value}` is unsupported"
        ))),
    }
}

fn motif_coverage(
    parameters: &RecipeOperationParameters<'_>,
) -> Result<MotifCoverage, NativeRecipeOperationError> {
    match choice(parameters, "motif-coverage")? {
        "auto" => Ok(MotifCoverage::Auto),
        "manual" => Ok(MotifCoverage::Manual),
        value => Err(NativeRecipeOperationError::new(format!(
            "Curves deformation motif coverage `{value}` is unsupported"
        ))),
    }
}

fn alternate_tile_transform(
    parameters: &RecipeOperationParameters<'_>,
) -> Result<AlternateTileTransform, NativeRecipeOperationError> {
    match choice(parameters, "alternate-tile-transform")? {
        "none" => Ok(AlternateTileTransform::None),
        "flip" => Ok(AlternateTileTransform::Flip),
        "rotate-180" => Ok(AlternateTileTransform::Rotate180),
        value => Err(NativeRecipeOperationError::new(format!(
            "Curves deformation alternate transform `{value}` is unsupported"
        ))),
    }
}

fn decode_curve_asset(
    digest: &str,
    assets: &[crate::EmbeddedSvgAsset],
) -> Result<CurvePath, NativeRecipeOperationError> {
    let asset = assets.iter().find(|a| a.digest == digest).ok_or_else(|| {
        NativeRecipeOperationError::new(format!("Curves SVG asset digest `{digest}` is missing"))
    })?;
    let prefix = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"";
    let suffix = "\"/></svg>";
    let d = asset
        .svg
        .strip_prefix(prefix)
        .and_then(|v| v.strip_suffix(suffix))
        .ok_or_else(|| {
            NativeRecipeOperationError::new(
                "Curves SVG must contain exactly one untransformed cubic path",
            )
        })?;
    let tokens = d
        .replace(',', " ")
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if tokens.len() < 10 || tokens[0] != "M" || tokens[3] != "C" {
        return Err(NativeRecipeOperationError::new(
            "Curves SVG path must start with M followed by cubic C commands",
        ));
    }
    let number_at = |index: usize| {
        tokens
            .get(index)
            .ok_or_else(|| {
                NativeRecipeOperationError::new("Curves SVG path has incomplete cubic command")
            })
            .and_then(|v| {
                v.parse::<f64>().map_err(|_| {
                    NativeRecipeOperationError::new("Curves SVG path contains malformed coordinate")
                })
            })
    };
    let start = crate::CurvePoint {
        x: number_at(1)?,
        y: number_at(2)?,
    };
    let mut segments = Vec::new();
    let mut index = 3;
    while index < tokens.len() {
        if tokens[index] != "C" {
            return Err(NativeRecipeOperationError::new(
                "Curves SVG path may contain only cubic C commands",
            ));
        }
        if index + 6 >= tokens.len() {
            return Err(NativeRecipeOperationError::new(
                "Curves SVG path has incomplete cubic command",
            ));
        }
        segments.push(crate::CubicCurveSegment {
            control_1: crate::CurvePoint {
                x: number_at(index + 1)?,
                y: number_at(index + 2)?,
            },
            control_2: crate::CurvePoint {
                x: number_at(index + 3)?,
                y: number_at(index + 4)?,
            },
            end: crate::CurvePoint {
                x: number_at(index + 5)?,
                y: number_at(index + 6)?,
            },
        });
        index += 7;
    }
    if !(1..=64).contains(&segments.len())
        || !std::iter::once(start)
            .chain(
                segments
                    .iter()
                    .flat_map(|s| [s.control_1, s.control_2, s.end]),
            )
            .all(|p| p.x.is_finite() && p.y.is_finite())
    {
        return Err(NativeRecipeOperationError::new(
            "Curves SVG path must contain 1..=64 finite cubic segments",
        ));
    }
    Ok(CurvePath { start, segments })
}
fn input<'a>(
    i: &'a RecipeOperationInputs<'a>,
    key: &str,
) -> Result<&'a RecipeRuntimeValue, NativeRecipeOperationError> {
    i.get(key)
        .copied()
        .ok_or_else(|| NativeRecipeOperationError::new(format!("missing Curves input `{key}")))
}
fn resolve_node_argument<'a>(
    definition: &'a PatternDefinition,
    instance: &'a PatternInstanceParameters,
    context: &RecipeExecutionContext<'_>,
    node: &'a RecipeNode,
    operation_parameter: &str,
) -> Result<&'a LiteralValue, NativeRecipeOperationError> {
    let argument = node.parameters.get(operation_parameter).ok_or_else(|| {
        NativeRecipeOperationError::new(format!(
            "Curves motif node `{}` is missing `{operation_parameter}` binding",
            node.id
        ))
    })?;
    let RecipeArgument::Parameter(parameter_key) = argument else {
        let RecipeArgument::Literal(value) = argument else {
            unreachable!("recipe arguments have only literal and parameter variants")
        };
        return Ok(value);
    };
    let parameter = definition
        .parameters
        .iter()
        .find(|parameter| parameter.key == *parameter_key)
        .ok_or_else(|| {
            NativeRecipeOperationError::new(format!(
                "Curves motif node `{}` binds unknown parameter `{parameter_key}`",
                node.id
            ))
        })?;
    let values = match parameter.scope {
        DefinitionParameterScope::Pattern => &instance.pattern_values,
        DefinitionParameterScope::OutputChannel => {
            let channel = context.output_channel.ok_or_else(|| {
                NativeRecipeOperationError::new(format!(
                    "Curves motif node `{}` requires a semantic output channel for `{parameter_key}`",
                    node.id
                ))
            })?;
            &instance
                .output_channel_values
                .iter()
                .find(|values| values.channel == channel.stable_id())
                .ok_or_else(|| {
                    NativeRecipeOperationError::new(format!(
                        "Curves motif node `{}` has no values for output channel `{}`",
                        node.id,
                        channel.stable_id()
                    ))
                })?
                .values
        }
    };
    values
        .iter()
        .find(|value| value.key == *parameter_key)
        .map(|value| &value.value)
        .ok_or_else(|| {
            NativeRecipeOperationError::new(format!(
                "Curves motif node `{}` has no value for `{parameter_key}`",
                node.id
            ))
        })
}

fn boolean_value(value: &LiteralValue) -> Result<bool, NativeRecipeOperationError> {
    match value {
        LiteralValue::Boolean(value) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(
            "Curves motif selection binding must resolve to boolean",
        )),
    }
}
fn channel(
    context: &RecipeExecutionContext<'_>,
) -> Result<OutputChannelId, NativeRecipeOperationError> {
    context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new("Curves operation requires semantic output channel")
    })
}
fn cancelled(_: crate::OperationCancelled) -> NativeRecipeOperationError {
    NativeRecipeOperationError::new("Curves operation cancelled")
}
fn number(p: &RecipeOperationParameters<'_>, key: &str) -> Result<f64, NativeRecipeOperationError> {
    match p.get(key) {
        Some(LiteralValue::Number(value)) if value.is_finite() => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be finite number"
        ))),
    }
}

fn bounded_number(
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Result<f64, NativeRecipeOperationError> {
    let value = number(parameters, key)?;
    if !(minimum..=maximum).contains(&value) {
        return Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be within {minimum}..={maximum}"
        )));
    }
    Ok(value)
}

fn positive_number(
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
    maximum: f64,
) -> Result<f64, NativeRecipeOperationError> {
    let value = number(parameters, key)?;
    if !(0.0 < value && value <= maximum) {
        return Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be within (0, {maximum}]"
        )));
    }
    Ok(value)
}
fn integer(
    p: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<u64, NativeRecipeOperationError> {
    match p.get(key) {
        Some(LiteralValue::Integer(value)) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be integer"
        ))),
    }
}

fn bounded_count(
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<u32, NativeRecipeOperationError> {
    let value = u32::try_from(integer(parameters, key)?).map_err(|_| {
        NativeRecipeOperationError::new(format!("Curves parameter `{key}` exceeds u32 limit"))
    })?;
    if !(1..=10_000).contains(&value) {
        return Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be within 1..=10000"
        )));
    }
    Ok(value)
}
fn boolean(
    p: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<bool, NativeRecipeOperationError> {
    match p.get(key) {
        Some(LiteralValue::Boolean(value)) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be boolean"
        ))),
    }
}

fn choice<'a>(
    parameters: &'a RecipeOperationParameters<'_>,
    key: &str,
) -> Result<&'a str, NativeRecipeOperationError> {
    match parameters.get(key) {
        Some(LiteralValue::Choice(value)) => Ok(value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be choice"
        ))),
    }
}
fn svg<'a>(
    p: &'a RecipeOperationParameters<'_>,
    key: &str,
) -> Result<&'a str, NativeRecipeOperationError> {
    match p.get(key) {
        Some(LiteralValue::SvgAsset(value)) => Ok(value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "Curves parameter `{key}` must be SVG asset"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve_render::CurveCommand;
    use crate::render::{Channel, legacy_pipeline_from_facade};
    use crate::{
        CancellationToken, EmbeddedSvgAsset, RecipeSourceFieldProvider, WebCurveSettings,
        adapt_curves_settings_to_recipe,
    };
    use crate::{OutputMode, ValueMode};
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};

    fn context<'a>(
        token: &'a CancellationToken,
        field: Option<&'a DistributionField>,
        assets: &'a [EmbeddedSvgAsset],
    ) -> RecipeExecutionContext<'a> {
        RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 900,
                height: 620,
            },
            output_channel: Some(OutputChannelId::CmykCyan),
            source_field_provider: None,
            source_field: field,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: assets,
            cancellation: token,
        }
    }
    fn placement_parameters() -> BTreeMap<&'static str, LiteralValue> {
        BTreeMap::from([
            ("output-width", LiteralValue::Integer(900)),
            ("output-height", LiteralValue::Integer(620)),
            ("long-edge-cells", LiteralValue::Number(90.0)),
            ("resolution-scale", LiteralValue::Number(1.0)),
            ("grid-rotation", LiteralValue::Number(15.0)),
            ("grid-pivot-x", LiteralValue::Number(0.0)),
            ("grid-pivot-y", LiteralValue::Number(0.0)),
            ("offset-x", LiteralValue::Number(0.0)),
            ("offset-y", LiteralValue::Number(0.0)),
        ])
    }

    fn parameter_references<'a>(
        values: &'a BTreeMap<&'static str, LiteralValue>,
    ) -> RecipeOperationParameters<'a> {
        values.iter().map(|(key, value)| (*key, value)).collect()
    }

    fn deformation_values(
        layout: &str,
        coverage: &str,
        output_quality: f64,
    ) -> BTreeMap<&'static str, LiteralValue> {
        BTreeMap::from([
            ("layout", LiteralValue::Choice(layout.into())),
            ("curve-scale", LiteralValue::Number(32.0)),
            ("motif-coverage", LiteralValue::Choice(coverage.into())),
            ("motif-bleed", LiteralValue::Number(2.0)),
            ("tile-count", LiteralValue::Integer(3)),
            ("tile-angle", LiteralValue::Number(20.0)),
            ("tile-offset", LiteralValue::Number(4.0)),
            ("stack-count", LiteralValue::Integer(2)),
            ("stack-spacing", LiteralValue::Number(36.0)),
            ("stack-angle", LiteralValue::Number(12.0)),
            ("stack-offset", LiteralValue::Number(3.0)),
            ("alternate-stack-offset", LiteralValue::Number(5.0)),
            (
                "alternate-tile-transform",
                LiteralValue::Choice("flip".into()),
            ),
            ("min-mark", LiteralValue::Number(0.0)),
            ("max-mark", LiteralValue::Number(85.0)),
            ("max-size", LiteralValue::Number(100.0)),
            ("scale", LiteralValue::Number(1.0)),
            ("output-quality", LiteralValue::Number(output_quality)),
        ])
    }

    fn typed_placement(token: &CancellationToken) -> CurvesPlacement {
        let values = placement_parameters();
        let parameters = parameter_references(&values);
        let RecipeRuntimeValue::CurvesPlacement(placement) =
            curves_placement(&context(token, None, &[]), &BTreeMap::new(), &parameters).unwrap()
        else {
            panic!("wrong runtime type")
        };
        placement
    }

    fn deformation_output(
        token: &CancellationToken,
        placement: CurvesPlacement,
        motif: CurvesMotif,
        values: &BTreeMap<&'static str, LiteralValue>,
    ) -> Result<CurvesDeformedPaths, NativeRecipeOperationError> {
        let placement_value = RecipeRuntimeValue::CurvesPlacement(placement);
        let motif_value = RecipeRuntimeValue::CurvesMotif(motif);
        let inputs = BTreeMap::from([("placement", &placement_value), ("motif", &motif_value)]);
        let parameters = parameter_references(values);
        let RecipeRuntimeValue::CurvesDeformedPaths(paths) =
            curves_deformation(&context(token, None, &[]), &inputs, &parameters)?
        else {
            panic!("wrong runtime type")
        };
        Ok(paths)
    }

    fn modulation_values(
        min_mark: f64,
        max_mark: f64,
        threshold: f64,
        max_size: f64,
        scale: f64,
        output_quality: f64,
    ) -> BTreeMap<&'static str, LiteralValue> {
        BTreeMap::from([
            ("min-mark", LiteralValue::Number(min_mark)),
            ("max-mark", LiteralValue::Number(max_mark)),
            ("threshold", LiteralValue::Number(threshold)),
            ("max-size", LiteralValue::Number(max_size)),
            ("scale", LiteralValue::Number(scale)),
            ("output-quality", LiteralValue::Number(output_quality)),
        ])
    }

    fn samples(placement: CurvesPlacement, values: Vec<f64>) -> CurvesSamples {
        let field =
            DistributionField::new(placement.grid.cols, placement.grid.rows, values).unwrap();
        CurvesSamples { placement, field }
    }

    fn modulation_output(
        token: &CancellationToken,
        deformed: CurvesDeformedPaths,
        samples: CurvesSamples,
        values: &BTreeMap<&'static str, LiteralValue>,
    ) -> Result<CurvesModulatedPaths, NativeRecipeOperationError> {
        let paths_value = RecipeRuntimeValue::CurvesDeformedPaths(deformed);
        let samples_value = RecipeRuntimeValue::CurvesSamples(samples);
        let inputs = BTreeMap::from([("paths", &paths_value), ("samples", &samples_value)]);
        let parameters = parameter_references(values);
        let RecipeRuntimeValue::CurvesModulatedPaths(paths) =
            curves_width_modulation(&context(token, None, &[]), &inputs, &parameters)?
        else {
            panic!("wrong runtime type")
        };
        Ok(paths)
    }

    fn emit_values(
        enabled: bool,
        color: &str,
        opacity: f64,
    ) -> BTreeMap<&'static str, LiteralValue> {
        BTreeMap::from([
            ("enabled", LiteralValue::Boolean(enabled)),
            ("color", LiteralValue::Text(color.into())),
            ("opacity", LiteralValue::Number(opacity)),
        ])
    }

    fn emit_output(
        token: &CancellationToken,
        output_channel: OutputChannelId,
        modulated: CurvesModulatedPaths,
        values: &BTreeMap<&'static str, LiteralValue>,
    ) -> Result<CanonicalPatternOutput, NativeRecipeOperationError> {
        let paths_value = RecipeRuntimeValue::CurvesModulatedPaths(modulated);
        let inputs = BTreeMap::from([("paths", &paths_value)]);
        let context = RecipeExecutionContext {
            output_channel: Some(output_channel),
            ..context(token, None, &[])
        };
        let parameters = parameter_references(values);
        let RecipeRuntimeValue::CanonicalOutput(output) =
            curves_emit_paths(&context, &inputs, &parameters)?
        else {
            panic!("wrong runtime type")
        };
        Ok(output)
    }

    fn fixture_outline() -> CurveOutline {
        CurveOutline {
            commands: vec![CurveCommand::Move(crate::CurvePoint::default())],
        }
    }

    fn orchestration_settings() -> WebCurveSettings {
        let mut settings = WebCurveSettings {
            output_width: 96,
            output_height: 64,
            long_edge_cells: 9.0,
            min_mark: 8.0,
            max_mark: 82.0,
            ..WebCurveSettings::default()
        };
        settings.channels.c.color = "#1256a8".into();
        settings.channels.c.opacity = 0.4;
        settings.channels.m.color = "#a81256".into();
        settings.channels.m.opacity = 0.6;
        settings.channels.y.color = "#56a812".into();
        settings.channels.y.opacity = 0.8;
        settings.channels.k.color = "#222222".into();
        settings
    }

    fn assert_orchestration_matches_retained(
        label: &str,
        settings: &WebCurveSettings,
        pipeline: &ArtworkPipelineSettings,
    ) {
        let source = image::RgbaImage::from_fn(32, 16, |x, y| {
            image::Rgba([(x * 7) as u8, (y * 13) as u8, 255 - (x * 3) as u8, 255])
        });
        assert_orchestration_matches_retained_source(label, &source, settings, pipeline);
    }

    fn assert_orchestration_matches_retained_source(
        label: &str,
        source: &image::RgbaImage,
        settings: &WebCurveSettings,
        pipeline: &ArtworkPipelineSettings,
    ) {
        let prepared = PreparedSource::from_rgba_image(source, 19);
        let token = CancellationToken::new();
        let retained = crate::curve_render::generate_curve_geometry_for_pipeline(
            &prepared, settings, pipeline, &token,
        )
        .unwrap();
        let expected = CanonicalPatternOutput::Paths(PathPatternOutput { geometry: retained });
        let first =
            execute_bundled_curves_recipe_cancellable(&prepared, settings, pipeline, &token)
                .unwrap();
        let second =
            execute_bundled_curves_recipe_cancellable(&prepared, settings, pipeline, &token)
                .unwrap();
        assert_eq!(first, expected, "orchestration case: {label}");
        assert_eq!(second, first, "deterministic repeat: {label}");
    }

    fn custom_cubic_path(amplitude: f64) -> CurvePath {
        CurvePath {
            start: crate::CurvePoint { x: -0.5, y: 0.0 },
            segments: vec![
                crate::CubicCurveSegment {
                    control_1: crate::CurvePoint {
                        x: -0.34,
                        y: -amplitude,
                    },
                    control_2: crate::CurvePoint {
                        x: -0.16,
                        y: -amplitude,
                    },
                    end: crate::CurvePoint { x: 0.0, y: 0.0 },
                },
                crate::CubicCurveSegment {
                    control_1: crate::CurvePoint {
                        x: 0.16,
                        y: amplitude,
                    },
                    control_2: crate::CurvePoint {
                        x: 0.34,
                        y: amplitude,
                    },
                    end: crate::CurvePoint { x: 0.5, y: 0.0 },
                },
            ],
        }
    }

    // These are the exact recipe descriptor ids accepted by the Curves
    // adapter. Keeping this manifest beside the equivalence cases makes a
    // changed descriptor impossible to overlook during retained-parity review.
    const CURVES_RECIPE_PARAMETER_COVERAGE: &[(&str, &[&str])] = &[
        ("output-width", &["cmyk-shared-full-response-grid"]),
        ("output-height", &["cmyk-shared-full-response-grid"]),
        ("long-edge-cells", &["cmyk-shared-full-response-grid"]),
        ("resolution-scale", &["cmyk-per-channel-manual-motif"]),
        ("grid-rotation", &["cmyk-per-channel-manual-motif"]),
        ("grid-pivot-x", &["cmyk-per-channel-manual-motif"]),
        ("grid-pivot-y", &["cmyk-per-channel-manual-motif"]),
        ("offset-x", &["cmyk-per-channel-manual-motif"]),
        ("offset-y", &["cmyk-per-channel-manual-motif"]),
        (
            "layout",
            &[
                "cmyk-shared-full-response-grid",
                "cmyk-per-channel-manual-motif",
            ],
        ),
        (
            "use-shared-curve",
            &[
                "cmyk-shared-full-response-grid",
                "cmyk-per-channel-manual-motif",
            ],
        ),
        ("shared-path", &["cmyk-shared-full-response-grid"]),
        ("shared-close-ends", &["cmyk-shared-full-response-grid"]),
        ("shared-smooth-join", &["cmyk-shared-full-response-grid"]),
        ("path", &["cmyk-per-channel-manual-motif"]),
        ("close-ends", &["cmyk-per-channel-manual-motif"]),
        ("smooth-join", &["cmyk-per-channel-manual-motif"]),
        ("curve-scale", &["cmyk-per-channel-manual-motif"]),
        (
            "motif-coverage",
            &["cmyk-per-channel-manual-motif", "rgb-auto-motif-alpha"],
        ),
        (
            "motif-bleed",
            &["cmyk-per-channel-manual-motif", "rgb-auto-motif-alpha"],
        ),
        ("tile-count", &["cmyk-per-channel-manual-motif"]),
        ("tile-angle", &["cmyk-per-channel-manual-motif"]),
        ("tile-offset", &["cmyk-per-channel-manual-motif"]),
        ("stack-count", &["cmyk-per-channel-manual-motif"]),
        ("stack-spacing", &["cmyk-per-channel-manual-motif"]),
        ("stack-angle", &["cmyk-per-channel-manual-motif"]),
        ("stack-offset", &["cmyk-per-channel-manual-motif"]),
        ("alternate-stack-offset", &["cmyk-per-channel-manual-motif"]),
        (
            "alternate-tile-transform",
            &["cmyk-per-channel-manual-motif"],
        ),
        (
            "min-mark",
            &[
                "cmyk-shared-full-response-grid",
                "response-boundaries-zero-width",
            ],
        ),
        (
            "max-mark",
            &[
                "cmyk-shared-full-response-grid",
                "response-boundaries-zero-width",
            ],
        ),
        (
            "max-size",
            &[
                "cmyk-per-channel-manual-motif",
                "response-boundaries-zero-width",
            ],
        ),
        (
            "scale",
            &[
                "cmyk-per-channel-manual-motif",
                "response-boundaries-zero-width",
            ],
        ),
        (
            "threshold",
            &[
                "cmyk-per-channel-manual-motif",
                "response-boundaries-zero-width",
            ],
        ),
        (
            "output-quality",
            &[
                "cmyk-per-channel-manual-motif",
                "response-boundaries-zero-width",
            ],
        ),
        ("enabled", &["cmyk-per-channel-manual-motif"]),
        (
            "color",
            &["cmyk-per-channel-manual-motif", "rgb-auto-motif-alpha"],
        ),
        (
            "opacity",
            &["cmyk-per-channel-manual-motif", "rgb-auto-motif-alpha"],
        ),
    ];

    const CURVES_RECIPE_MATRIX_CASES: &[&str] = &[
        "cmyk-shared-full-response-grid",
        "cmyk-per-channel-manual-motif",
        "rgb-auto-motif-alpha",
        "legacy-crosshatch-external-color",
        "response-boundaries-zero-width",
    ];

    #[test]
    fn bundled_orchestrator_recipe_coverage_manifest_matches_current_descriptor_contract() {
        let expected = BTreeSet::from([
            "output-width",
            "output-height",
            "long-edge-cells",
            "resolution-scale",
            "grid-rotation",
            "grid-pivot-x",
            "grid-pivot-y",
            "offset-x",
            "offset-y",
            "layout",
            "use-shared-curve",
            "shared-path",
            "shared-close-ends",
            "shared-smooth-join",
            "path",
            "close-ends",
            "smooth-join",
            "curve-scale",
            "motif-coverage",
            "motif-bleed",
            "tile-count",
            "tile-angle",
            "tile-offset",
            "stack-count",
            "stack-spacing",
            "stack-angle",
            "stack-offset",
            "alternate-stack-offset",
            "alternate-tile-transform",
            "min-mark",
            "max-mark",
            "max-size",
            "scale",
            "threshold",
            "output-quality",
            "enabled",
            "color",
            "opacity",
        ]);
        let actual = CURVES_RECIPE_PARAMETER_COVERAGE
            .iter()
            .map(|(parameter, _)| *parameter)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "descriptor coverage needs review");
        for (parameter, cases) in CURVES_RECIPE_PARAMETER_COVERAGE {
            assert!(
                !cases.is_empty()
                    && cases
                        .iter()
                        .all(|case| CURVES_RECIPE_MATRIX_CASES.contains(case)),
                "{parameter} must name a reviewed equivalence case"
            );
        }
    }

    struct RecordingProvider {
        field: DistributionField,
        channels: RefCell<Vec<OutputChannelId>>,
    }

    impl RecipeSourceFieldProvider for RecordingProvider {
        fn resolve_source_field(
            &self,
            channel: OutputChannelId,
            columns: u32,
            rows: u32,
            cancellation: &CancellationToken,
        ) -> Result<DistributionField, NativeRecipeOperationError> {
            cancellation.checkpoint().map_err(cancelled)?;
            if self.field.dimensions() != (columns, rows) {
                return Err(NativeRecipeOperationError::new(
                    "unexpected source request dimensions",
                ));
            }
            self.channels.borrow_mut().push(channel);
            Ok(self.field.clone())
        }
    }
    #[test]
    fn placement_and_source_sample_are_typed_bounded_and_identity_preserving() {
        let token = CancellationToken::new();
        let ctx = context(&token, None, &[]);
        let placement_values = placement_parameters();
        let placement_parameter_refs = parameter_references(&placement_values);
        let value = curves_placement(&ctx, &BTreeMap::new(), &placement_parameter_refs).unwrap();
        let RecipeRuntimeValue::CurvesPlacement(placement) = value else {
            panic!("wrong runtime type")
        };
        assert_eq!(placement.grid, calculate_web_grid(900, 620, 90.0));
        let field = DistributionField::new(
            placement.grid.cols,
            placement.grid.rows,
            vec![0.5; (placement.grid.cols * placement.grid.rows) as usize],
        )
        .unwrap();
        let ctx = context(&token, Some(&field), &[]);
        let placement_value = RecipeRuntimeValue::CurvesPlacement(placement.clone());
        let inputs = BTreeMap::from([("placement", &placement_value)]);
        let sampled = curves_source_sample(&ctx, &inputs, &BTreeMap::new()).unwrap();
        assert!(
            matches!(sampled,RecipeRuntimeValue::CurvesSamples(CurvesSamples{field: ref actual,..}) if actual==&field)
        );
        assert!(curves_source_sample(&ctx, &BTreeMap::new(), &BTreeMap::new()).is_err());
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_values = placement_parameters();
        let cancelled_parameters = parameter_references(&cancelled_values);
        assert!(
            curves_placement(
                &context(&cancelled, None, &[]),
                &BTreeMap::new(),
                &cancelled_parameters
            )
            .is_err()
        );
        let mut oversized_values = placement_parameters();
        oversized_values.insert("output-width", LiteralValue::Integer(u64::MAX));
        let oversized_parameters = parameter_references(&oversized_values);
        assert!(curves_placement(&ctx, &BTreeMap::new(), &oversized_parameters).is_err());
    }

    #[test]
    fn source_sample_preserves_semantic_rgb_and_cmyk_channel_identity() {
        let token = CancellationToken::new();
        let values = placement_parameters();
        let parameters = parameter_references(&values);
        let base =
            curves_placement(&context(&token, None, &[]), &BTreeMap::new(), &parameters).unwrap();
        let RecipeRuntimeValue::CurvesPlacement(placement) = base else {
            panic!("wrong runtime type")
        };
        let field = DistributionField::new(
            placement.grid.cols,
            placement.grid.rows,
            vec![0.25; (placement.grid.cols * placement.grid.rows) as usize],
        )
        .unwrap();
        let provider = RecordingProvider {
            field,
            channels: RefCell::new(Vec::new()),
        };
        let placement_value = RecipeRuntimeValue::CurvesPlacement(placement);
        let inputs = BTreeMap::from([("placement", &placement_value)]);
        for output_channel in [OutputChannelId::CmykCyan, OutputChannelId::RgbRed] {
            let ctx = RecipeExecutionContext {
                output_channel: Some(output_channel),
                source_field_provider: Some(&provider),
                ..context(&token, None, &[])
            };
            curves_source_sample(&ctx, &inputs, &BTreeMap::new()).unwrap();
        }
        assert_eq!(
            *provider.channels.borrow(),
            vec![OutputChannelId::CmykCyan, OutputChannelId::RgbRed]
        );
    }

    #[test]
    fn placement_rejects_an_unbounded_source_grid_before_sampling() {
        let token = CancellationToken::new();
        let mut values = placement_parameters();
        values.insert("output-width", LiteralValue::Integer(100_000));
        values.insert("output-height", LiteralValue::Integer(100_000));
        values.insert("long-edge-cells", LiteralValue::Number(10_000.0));
        values.insert("resolution-scale", LiteralValue::Number(100.0));
        let parameters = parameter_references(&values);
        let ctx = RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 100_000,
                height: 100_000,
            },
            ..context(&token, None, &[])
        };
        assert!(curves_placement(&ctx, &BTreeMap::new(), &parameters).is_err());
    }

    #[test]
    fn motif_decoder_and_preflight_are_strict() {
        let adapted = adapt_curves_settings_to_recipe(&WebCurveSettings::default()).unwrap();
        let token = CancellationToken::new();
        let ctx = context(&token, None, &adapted.definition.assets);
        validate_curves_recipe_execution_assets(&adapted.definition, &adapted.instance, &ctx)
            .unwrap();
        let mut params = BTreeMap::new();
        params.insert("use-shared-curve", LiteralValue::Boolean(true));
        params.insert(
            "shared-path",
            adapted
                .instance
                .pattern_values
                .iter()
                .find(|v| v.key == "shared-path")
                .unwrap()
                .value
                .clone(),
        );
        params.insert("shared-close-ends", LiteralValue::Boolean(false));
        params.insert("shared-smooth-join", LiteralValue::Boolean(false));
        params.insert("path", LiteralValue::SvgAsset("sha256:missing".into()));
        params.insert("close-ends", LiteralValue::Boolean(false));
        params.insert("smooth-join", LiteralValue::Boolean(false));
        let parameter_refs = parameter_references(&params);
        let motif = curves_motif_selection(&ctx, &BTreeMap::new(), &parameter_refs).unwrap();
        assert!(
            matches!(motif,RecipeRuntimeValue::CurvesMotif(CurvesMotif{path,..}) if path==WebCurveSettings::default().shared_path)
        );
        let decoded = decode_curve_asset(
            match params.get("shared-path") {
                Some(LiteralValue::SvgAsset(digest)) => digest,
                _ => panic!("expected shared SVG digest"),
            },
            &adapted.definition.assets,
        )
        .unwrap();
        assert_eq!(
            crate::curve_render::sample_curve_path(&decoded, 24, false, false),
            crate::curve_render::sample_curve_path(
                &WebCurveSettings::default().shared_path,
                24,
                false,
                false,
            )
        );
        assert!(
            CURVES_NATIVE_OPERATION_REGISTRY
                .get("curves.placement", 1)
                .is_some()
        );
        assert!(
            CURVES_NATIVE_OPERATION_REGISTRY
                .get("curves.source-sample", 1)
                .is_some()
        );
        assert!(
            CURVES_NATIVE_OPERATION_REGISTRY
                .get("curves.motif-selection", 1)
                .is_some()
        );
        assert!(decode_curve_asset("sha256:x", &[]).is_err());
        params.insert("use-shared-curve", LiteralValue::Boolean(false));
        let per_channel_refs = parameter_references(&params);
        assert!(curves_motif_selection(&ctx, &BTreeMap::new(), &per_channel_refs).is_err());
        let bad = [EmbeddedSvgAsset {
            digest: "sha256:x".into(),
            svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"M 0 0 L 1 1\"/></svg>".into(),
        }];
        assert!(decode_curve_asset("sha256:x", &bad).is_err());
    }

    #[test]
    fn deformation_matches_retained_full_width_and_motif_helpers() {
        let token = CancellationToken::new();
        let cases = [
            (
                "full-width",
                "auto",
                CurvesMotif {
                    path: WebCurveSettings::default().shared_path,
                    close_ends: true,
                    smooth_join: true,
                },
            ),
            (
                "motif-pattern",
                "manual",
                CurvesMotif {
                    path: crate::CurvePath::straight(),
                    close_ends: false,
                    smooth_join: false,
                },
            ),
        ];
        for (layout, coverage, motif) in cases {
            let mut placement = typed_placement(&token);
            placement.grid_pivot_x = 8.0;
            placement.grid_pivot_y = -5.0;
            placement.offset_x = 12.0;
            placement.offset_y = -7.0;
            let values = deformation_values(layout, coverage, 2.5);
            let actual =
                deformation_output(&token, placement.clone(), motif.clone(), &values).unwrap();
            let parameters = parameter_references(&values);
            let settings = temporary_deformation_settings(&placement, &parameters).unwrap();
            let channel = temporary_deformation_channel(&placement, &parameters).unwrap();
            let expected = deform_curve_paths_cancellable(
                CurveDeformationRequest {
                    path: &motif.path,
                    close_ends: motif.close_ends,
                    smooth_join: motif.smooth_join,
                    settings: &settings,
                    channel: &channel,
                    grid: &placement.grid,
                },
                &token,
                None,
            )
            .unwrap();
            assert_eq!(actual.paths, expected);
            assert_eq!(actual.closed, motif.close_ends);
            assert_eq!(actual.placement, placement);
        }
    }

    #[test]
    fn deformation_preserves_shared_and_per_channel_decoded_motif_semantics() {
        let mut settings = WebCurveSettings::default();
        settings.channels.c.path = crate::CurvePath::straight();
        settings.channels.c.close_ends = true;
        settings.channels.c.smooth_join = true;
        let adapted = adapt_curves_settings_to_recipe(&settings).unwrap();
        let token = CancellationToken::new();
        let global = |key| {
            adapted
                .instance
                .pattern_values
                .iter()
                .find(|value| value.key == key)
                .unwrap()
                .value
                .clone()
        };
        let channel = adapted
            .instance
            .output_channel_values
            .iter()
            .find(|channel| channel.channel == OutputChannelId::CmykCyan.stable_id())
            .unwrap();
        let channel_value = |key| {
            channel
                .values
                .iter()
                .find(|value| value.key == key)
                .unwrap()
                .value
                .clone()
        };
        let mut values = BTreeMap::from([
            ("use-shared-curve", global("use-shared-curve")),
            ("shared-path", global("shared-path")),
            ("shared-close-ends", global("shared-close-ends")),
            ("shared-smooth-join", global("shared-smooth-join")),
            ("path", channel_value("path")),
            ("close-ends", channel_value("close-ends")),
            ("smooth-join", channel_value("smooth-join")),
        ]);
        let placement = typed_placement(&token);
        let deformation = deformation_values("full-width", "auto", 1.0);
        let motif_parameters = parameter_references(&values);
        let RecipeRuntimeValue::CurvesMotif(shared) = curves_motif_selection(
            &context(&token, None, &adapted.definition.assets),
            &BTreeMap::new(),
            &motif_parameters,
        )
        .unwrap() else {
            panic!("wrong runtime type")
        };
        assert!(!shared.close_ends);
        assert!(
            !deformation_output(&token, placement.clone(), shared, &deformation)
                .unwrap()
                .closed
        );

        values.insert("use-shared-curve", LiteralValue::Boolean(false));
        let motif_parameters = parameter_references(&values);
        let RecipeRuntimeValue::CurvesMotif(per_channel) = curves_motif_selection(
            &context(&token, None, &adapted.definition.assets),
            &BTreeMap::new(),
            &motif_parameters,
        )
        .unwrap() else {
            panic!("wrong runtime type")
        };
        let output = deformation_output(&token, placement, per_channel, &deformation).unwrap();
        assert!(output.closed);
        assert!(!output.paths.is_empty());
    }

    #[test]
    fn deformation_preserves_coverage_guards_and_rejects_path_expansion_before_allocation() {
        let token = CancellationToken::new();
        let placement = typed_placement(&token);
        let motif = CurvesMotif {
            path: crate::CurvePath::straight(),
            close_ends: false,
            smooth_join: false,
        };
        let narrow = deformation_values("motif-pattern", "auto", 1.0);
        let mut guarded = narrow.clone();
        guarded.insert("min-mark", LiteralValue::Number(500.0));
        guarded.insert("max-mark", LiteralValue::Number(1_000.0));
        guarded.insert("max-size", LiteralValue::Number(100.0));
        guarded.insert("scale", LiteralValue::Number(1.0));
        let narrow_output =
            deformation_output(&token, placement.clone(), motif.clone(), &narrow).unwrap();
        let guarded_output =
            deformation_output(&token, placement.clone(), motif.clone(), &guarded).unwrap();
        assert!(guarded_output.paths.len() >= narrow_output.paths.len());

        let mut manual = deformation_values("motif-pattern", "manual", 1.0);
        manual.insert("tile-count", LiteralValue::Integer(7));
        manual.insert("stack-count", LiteralValue::Integer(5));
        let manual_output =
            deformation_output(&token, placement.clone(), motif.clone(), &manual).unwrap();
        assert!(manual_output.paths.len() >= 5);

        let mut pathological = manual;
        pathological.insert("tile-count", LiteralValue::Integer(10_000));
        pathological.insert("stack-count", LiteralValue::Integer(10_000));
        let error = deformation_output(&token, placement, motif, &pathological).unwrap_err();
        assert!(error.to_string().contains("bounded"));
    }

    #[test]
    fn deformation_cancellation_and_emit_reject_missing_typed_paths() {
        let token = CancellationToken::new();
        let values = deformation_values("full-width", "auto", 1.0);
        let parameters = parameter_references(&values);
        assert!(
            curves_deformation(&context(&token, None, &[]), &BTreeMap::new(), &parameters).is_err()
        );

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let error = deformation_output(
            &cancelled,
            typed_placement(&token),
            CurvesMotif {
                path: crate::CurvePath::straight(),
                close_ends: false,
                smooth_join: false,
            },
            &values,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));

        assert!(
            curves_emit_paths(
                &context(&token, None, &[]),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn width_modulation_matches_the_retained_outline_seam_for_layouts_and_closed_paths() {
        let token = CancellationToken::new();
        let placement = typed_placement(&token);
        let field_values = (0..placement.grid.cols * placement.grid.rows)
            .map(|index| (index % placement.grid.cols) as f64 / placement.grid.cols as f64)
            .collect::<Vec<_>>();
        let parameters = modulation_values(8.0, 85.0, 0.2, 90.0, 1.3, 2.5);
        for (layout, closed) in [
            ("full-width", false),
            ("full-width", true),
            ("motif-pattern", false),
            ("motif-pattern", true),
        ] {
            let mut deformation = deformation_values(layout, "auto", 2.5);
            deformation.insert("min-mark", LiteralValue::Number(8.0));
            deformation.insert("max-mark", LiteralValue::Number(85.0));
            deformation.insert("max-size", LiteralValue::Number(90.0));
            deformation.insert("scale", LiteralValue::Number(1.3));
            let deformed = deformation_output(
                &token,
                placement.clone(),
                CurvesMotif {
                    path: crate::CurvePath::straight(),
                    close_ends: closed,
                    smooth_join: true,
                },
                &deformation,
            )
            .unwrap();
            let settings =
                temporary_modulation_settings(&placement, &parameter_references(&parameters))
                    .unwrap();
            let channel = temporary_modulation_channel(&parameter_references(&parameters)).unwrap();
            let sample_value = |index| field_values[index];
            let expected = modulate_curve_paths_cancellable(
                CurveModulationRequest {
                    paths: &deformed.paths,
                    closed: deformed.closed,
                    grid: &placement.grid,
                    settings: &settings,
                    channel: &channel,
                    sample_value: &sample_value,
                },
                &token,
                None,
            )
            .unwrap();
            let actual = modulation_output(
                &token,
                deformed,
                samples(placement.clone(), field_values.clone()),
                &parameters,
            )
            .unwrap();
            assert_eq!(actual.artboard, placement.artboard);
            assert_eq!(actual.outlines, expected, "{layout}, closed={closed}");
        }
    }

    #[test]
    fn width_modulation_is_parameter_sensitive_and_handles_zero_and_clipped_paths() {
        let token = CancellationToken::new();
        let placement = typed_placement(&token);
        let values = vec![0.5; (placement.grid.cols * placement.grid.rows) as usize];
        let deformed = CurvesDeformedPaths {
            placement: placement.clone(),
            paths: vec![vec![
                crate::CurvePoint { x: 20.0, y: 20.0 },
                crate::CurvePoint { x: 880.0, y: 600.0 },
            ]],
            closed: false,
        };
        let visible = modulation_output(
            &token,
            deformed.clone(),
            samples(placement.clone(), values.clone()),
            &modulation_values(10.0, 90.0, 0.0, 100.0, 1.0, 1.0),
        )
        .unwrap();
        let thresholded = modulation_output(
            &token,
            deformed.clone(),
            samples(placement.clone(), values.clone()),
            &modulation_values(10.0, 90.0, 0.8, 100.0, 1.0, 1.0),
        )
        .unwrap();
        assert_ne!(visible.outlines, thresholded.outlines);
        assert!(thresholded.outlines.is_empty());

        let zero = modulation_output(
            &token,
            deformed.clone(),
            samples(
                placement.clone(),
                vec![0.0; (placement.grid.cols * placement.grid.rows) as usize],
            ),
            &modulation_values(0.0, 90.0, 0.0, 100.0, 1.0, 1.0),
        )
        .unwrap();
        assert!(zero.outlines.is_empty());

        let clipped = CurvesDeformedPaths {
            placement: placement.clone(),
            paths: vec![vec![
                crate::CurvePoint {
                    x: -10_000.0,
                    y: -10_000.0,
                },
                crate::CurvePoint {
                    x: -9_000.0,
                    y: -9_000.0,
                },
            ]],
            closed: false,
        };
        assert!(
            modulation_output(
                &token,
                clipped,
                samples(placement, values),
                &modulation_values(10.0, 90.0, 0.0, 100.0, 1.0, 1.0),
            )
            .unwrap()
            .outlines
            .is_empty()
        );
    }

    #[test]
    fn width_modulation_rejects_typed_provenance_dimension_cancellation_and_limit_failures() {
        let token = CancellationToken::new();
        let placement = typed_placement(&token);
        let values = modulation_values(0.0, 80.0, 0.0, 100.0, 1.0, 1.0);
        let deformed = CurvesDeformedPaths {
            placement: placement.clone(),
            paths: vec![vec![
                crate::CurvePoint::default(),
                crate::CurvePoint { x: 5.0, y: 5.0 },
            ]],
            closed: false,
        };
        assert!(
            curves_width_modulation(
                &context(&token, None, &[]),
                &BTreeMap::new(),
                &parameter_references(&values),
            )
            .is_err()
        );

        let mut mismatched = placement.clone();
        mismatched.offset_x = 1.0;
        let provenance = modulation_output(
            &token,
            deformed.clone(),
            samples(
                mismatched,
                vec![0.5; (placement.grid.cols * placement.grid.rows) as usize],
            ),
            &values,
        )
        .unwrap_err();
        assert!(provenance.to_string().contains("provenance"));

        let wrong_dimensions = CurvesSamples {
            placement: placement.clone(),
            field: DistributionField::new(1, 1, vec![0.5]).unwrap(),
        };
        let dimensions =
            modulation_output(&token, deformed.clone(), wrong_dimensions, &values).unwrap_err();
        assert!(dimensions.to_string().contains("dimensions"));

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_error = modulation_output(
            &cancelled,
            deformed.clone(),
            samples(
                placement.clone(),
                vec![0.5; (placement.grid.cols * placement.grid.rows) as usize],
            ),
            &values,
        )
        .unwrap_err();
        assert!(cancelled_error.to_string().contains("cancelled"));

        let oversized = CurvesDeformedPaths {
            placement: placement.clone(),
            paths: vec![vec![crate::CurvePoint::default(); 20_001]],
            closed: false,
        };
        let bounded = modulation_output(
            &token,
            oversized,
            samples(
                placement,
                vec![0.5; (deformed.placement.grid.cols * deformed.placement.grid.rows) as usize],
            ),
            &values,
        )
        .unwrap_err();
        assert!(bounded.to_string().contains("bounded"));
    }

    #[test]
    fn emit_paths_returns_exact_single_semantic_layer_for_cmyk_and_rgb() {
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 900,
            height: 620,
        };
        for (output_channel, expected_channel, color, opacity) in [
            (OutputChannelId::CmykCyan, Channel::Cyan, "#123456", 0.5),
            (OutputChannelId::RgbRed, Channel::Red, "#abcdef", 0.75),
        ] {
            let outlines = vec![fixture_outline()];
            let output = emit_output(
                &token,
                output_channel,
                CurvesModulatedPaths {
                    artboard,
                    outlines: outlines.clone(),
                },
                &emit_values(true, color, opacity),
            )
            .unwrap();
            assert_eq!(
                output,
                CanonicalPatternOutput::Paths(PathPatternOutput {
                    geometry: CurveGeometry {
                        width: artboard.width,
                        height: artboard.height,
                        layers: vec![CurveInkLayer {
                            layer: InkLayer {
                                channel: expected_channel,
                                enabled: true,
                                color: parse_hex_color(color).unwrap(),
                                opacity: opacity as f32,
                            },
                            outlines,
                        }],
                    },
                })
            );
        }
    }

    #[test]
    fn emit_paths_keeps_one_disabled_layer_and_matches_retained_path_fixture() {
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 900,
            height: 620,
        };
        let retained = CurveGeometry {
            width: artboard.width,
            height: artboard.height,
            layers: vec![CurveInkLayer {
                layer: InkLayer {
                    channel: Channel::Magenta,
                    enabled: true,
                    color: (0x55, 0x33, 0x11),
                    opacity: 0.92,
                },
                outlines: vec![fixture_outline()],
            }],
        };
        let enabled = emit_output(
            &token,
            OutputChannelId::CmykMagenta,
            CurvesModulatedPaths {
                artboard,
                outlines: retained.layers[0].outlines.clone(),
            },
            &emit_values(true, "#553311", 0.92),
        )
        .unwrap();
        assert_eq!(
            enabled,
            CanonicalPatternOutput::Paths(PathPatternOutput {
                geometry: retained.clone(),
            })
        );

        let disabled = emit_output(
            &token,
            OutputChannelId::CmykMagenta,
            CurvesModulatedPaths {
                artboard,
                outlines: retained.layers[0].outlines.clone(),
            },
            &emit_values(false, "#553311", 0.92),
        )
        .unwrap();
        let CanonicalPatternOutput::Paths(disabled) = disabled else {
            panic!("Curves emit must return paths")
        };
        assert_eq!(disabled.geometry.layers.len(), 1);
        assert!(!disabled.geometry.layers[0].layer.enabled);
        assert!(disabled.geometry.layers[0].outlines.is_empty());

        let empty = emit_output(
            &token,
            OutputChannelId::CmykMagenta,
            CurvesModulatedPaths {
                artboard,
                outlines: Vec::new(),
            },
            &emit_values(true, "#553311", 0.92),
        )
        .unwrap();
        let CanonicalPatternOutput::Paths(empty) = empty else {
            panic!("Curves emit must return paths")
        };
        assert!(empty.geometry.layers[0].layer.enabled);
        assert!(empty.geometry.layers[0].outlines.is_empty());
    }

    #[test]
    fn emit_paths_rejects_invalid_inputs_context_parameters_cancellation_and_limits() {
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 900,
            height: 620,
        };
        let modulated = CurvesModulatedPaths {
            artboard,
            outlines: vec![fixture_outline()],
        };
        let values = emit_values(true, "#123456", 0.5);
        let paths_value = RecipeRuntimeValue::CurvesModulatedPaths(modulated.clone());
        let inputs = BTreeMap::from([("paths", &paths_value)]);
        assert!(
            curves_emit_paths(
                &context(&token, None, &[]),
                &inputs,
                &parameter_references(&values),
            )
            .is_ok()
        );

        let no_channel = RecipeExecutionContext {
            output_channel: None,
            ..context(&token, None, &[])
        };
        assert!(curves_emit_paths(&no_channel, &inputs, &parameter_references(&values)).is_err());
        let mismatched = CurvesModulatedPaths {
            artboard: ArtboardSpace {
                width: 1,
                height: 1,
            },
            outlines: vec![],
        };
        assert!(
            emit_output(&token, OutputChannelId::CmykCyan, mismatched, &values,)
                .unwrap_err()
                .to_string()
                .contains("artboard")
        );
        for invalid in [
            emit_values(true, "not-a-color", 0.5),
            emit_values(true, "#123456", -0.1),
            emit_values(true, "#123456", 1.1),
        ] {
            assert!(
                emit_output(
                    &token,
                    OutputChannelId::CmykCyan,
                    modulated.clone(),
                    &invalid,
                )
                .is_err()
            );
        }
        let too_many = CurvesModulatedPaths {
            artboard,
            outlines: vec![fixture_outline(); 10_001],
        };
        assert!(
            emit_output(&token, OutputChannelId::CmykCyan, too_many, &values,)
                .unwrap_err()
                .to_string()
                .contains("bounded")
        );
        let command_limited = CurveOutline {
            commands: vec![
                CurveCommand::Move(crate::CurvePoint::default()),
                CurveCommand::Close,
            ],
        };
        assert!(
            validate_final_outlines_with_limits(&[command_limited], 1, 1)
                .unwrap_err()
                .to_string()
                .contains("command")
        );
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(
            emit_output(&cancelled, OutputChannelId::CmykCyan, modulated, &values,)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }

    #[test]
    fn retained_renderer_does_not_invoke_curves_recipe_operations() {
        let source = image::RgbaImage::from_pixel(16, 12, image::Rgba([20, 40, 60, 255]));
        reset_native_node_invocations();
        crate::curve_render::generate_curve_geometry(&source, &WebCurveSettings::default())
            .unwrap();
        assert_eq!(native_node_invocations(), 0);
    }

    #[test]
    fn generic_preflight_rejects_a_custom_motif_literal_before_any_native_node() {
        let adapted = adapt_curves_settings_to_recipe(&WebCurveSettings::default()).unwrap();
        let invalid_svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"M 0 0 L 1 1\"/></svg>";
        let invalid_digest = format!("sha256:{:x}", Sha256::digest(invalid_svg.as_bytes()));
        let mut definition = adapted.definition.clone();
        definition.assets.push(EmbeddedSvgAsset {
            digest: invalid_digest.clone(),
            svg: invalid_svg.into(),
        });
        let motif = definition
            .recipe
            .nodes
            .iter_mut()
            .find(|node| node.operation.id == "curves.motif-selection")
            .unwrap();
        motif.parameters.insert(
            "use-shared-curve".into(),
            RecipeArgument::Literal(LiteralValue::Boolean(true)),
        );
        motif.parameters.insert(
            "shared-path".into(),
            RecipeArgument::Literal(LiteralValue::SvgAsset(invalid_digest)),
        );
        let token = CancellationToken::new();
        reset_native_node_invocations();
        let error = definition
            .execute_recipe(
                &adapted.instance,
                &context(&token, None, &definition.assets),
                &CURVES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("native recipe preflight failed"), "{error}");
        assert!(error.contains("start with M followed by cubic C commands"));
        assert_eq!(native_node_invocations(), 0);
    }

    #[test]
    fn generic_preflight_resolves_selected_output_channel_path_binding() {
        let adapted = adapt_curves_settings_to_recipe(&WebCurveSettings::default()).unwrap();
        let invalid_svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"M 0 0 L 1 1\"/></svg>";
        let invalid_digest = format!("sha256:{:x}", Sha256::digest(invalid_svg.as_bytes()));
        let mut definition = adapted.definition.clone();
        definition.assets.push(EmbeddedSvgAsset {
            digest: invalid_digest.clone(),
            svg: invalid_svg.into(),
        });
        let mut instance = adapted.instance.clone();
        instance
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "use-shared-curve")
            .unwrap()
            .value = LiteralValue::Boolean(false);
        instance
            .output_channel_values
            .iter_mut()
            .find(|channel| channel.channel == OutputChannelId::CmykCyan.stable_id())
            .unwrap()
            .values
            .iter_mut()
            .find(|value| value.key == "path")
            .unwrap()
            .value = LiteralValue::SvgAsset(invalid_digest);
        let token = CancellationToken::new();
        reset_native_node_invocations();
        let error = definition
            .execute_recipe(
                &instance,
                &context(&token, None, &definition.assets),
                &CURVES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("native recipe preflight failed"), "{error}");
        assert!(error.contains("start with M followed by cubic C commands"));
        assert_eq!(native_node_invocations(), 0);
    }

    #[test]
    fn generic_recipe_executes_all_six_native_bodies_and_returns_paths() {
        let settings = WebCurveSettings::default();
        let adapted = adapt_curves_settings_to_recipe(&settings).unwrap();
        let grid = calculate_web_grid(
            settings.output_width,
            settings.output_height,
            settings.long_edge_cells,
        );
        let field = DistributionField::new(
            grid.cols,
            grid.rows,
            vec![0.5; (grid.cols * grid.rows) as usize],
        )
        .unwrap();
        let token = CancellationToken::new();
        reset_native_node_invocations();
        let output = adapted
            .definition
            .execute_recipe(
                &adapted.instance,
                &context(&token, Some(&field), &adapted.definition.assets),
                &CURVES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap();
        assert!(matches!(output, CanonicalPatternOutput::Paths(_)));
        assert_eq!(native_node_invocations(), 6);
    }

    #[test]
    fn bundled_orchestrator_matches_retained_complete_canonical_recipe_matrix() {
        let opaque_source = image::RgbaImage::from_fn(43, 29, |x, y| {
            image::Rgba([
                (x.wrapping_mul(13) % 256) as u8,
                (y.wrapping_mul(17) % 256) as u8,
                ((x * 5 + y * 3) % 256) as u8,
                255,
            ])
        });
        let alpha_source = image::RgbaImage::from_fn(43, 29, |x, y| {
            image::Rgba([
                (x.wrapping_mul(11) % 256) as u8,
                (y.wrapping_mul(19) % 256) as u8,
                ((x * 7 + y * 5) % 256) as u8,
                if (x + y) % 3 == 0 { 48 } else { 192 },
            ])
        });

        let mut shared = orchestration_settings();
        shared.output_width = 117;
        shared.output_height = 79;
        shared.long_edge_cells = 13.0;
        shared.min_mark = 7.0;
        shared.max_mark = 93.0;
        shared.layout = CurveLayout::FullWidth;
        shared.use_shared_curve = true;
        shared.shared_path = custom_cubic_path(0.23);
        shared.shared_close_ends = true;
        shared.shared_smooth_join = true;
        for channel in [
            &mut shared.channels.c,
            &mut shared.channels.m,
            &mut shared.channels.y,
            &mut shared.channels.k,
        ] {
            channel.enabled = true;
        }
        shared.channels.c.grid_rotation = 11.0;
        shared.channels.c.resolution_scale = 0.75;
        shared.channels.m.grid_rotation = 47.0;
        shared.channels.m.resolution_scale = 1.25;
        shared.channels.y.grid_rotation = 83.0;
        shared.channels.y.resolution_scale = 1.75;
        shared.channels.k.grid_rotation = 137.0;
        shared.channels.k.resolution_scale = 2.0;
        let cmyk_pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            shared.single_channel,
        );
        assert_orchestration_matches_retained_source(
            "cmyk-shared-full-response-grid",
            &opaque_source,
            &shared,
            &cmyk_pipeline,
        );

        let mut manual = shared.clone();
        manual.layout = CurveLayout::MotifPattern;
        manual.use_shared_curve = false;
        manual.channels.c.path = custom_cubic_path(0.12);
        manual.channels.c.close_ends = false;
        manual.channels.c.smooth_join = false;
        manual.channels.c.grid_pivot_x = -13.0;
        manual.channels.c.grid_pivot_y = 7.0;
        manual.channels.c.offset_x = -9.0;
        manual.channels.c.offset_y = 4.0;
        manual.channels.c.curve_scale = 18.0;
        manual.channels.c.motif_coverage = MotifCoverage::Manual;
        manual.channels.c.motif_bleed = 1.0;
        manual.channels.c.tile_count = 2;
        manual.channels.c.tile_angle = 12.0;
        manual.channels.c.tile_offset = -3.0;
        manual.channels.c.stack_count = 2;
        manual.channels.c.stack_spacing = 21.0;
        manual.channels.c.stack_angle = -8.0;
        manual.channels.c.stack_offset = 4.0;
        manual.channels.c.alternate_stack_offset = 7.0;
        manual.channels.c.alternate_tile_transform = AlternateTileTransform::None;
        manual.channels.c.scale = 0.61;
        manual.channels.c.threshold = 0.11;
        manual.channels.c.max_size = 42.0;
        manual.channels.c.output_quality = 0.7;

        manual.channels.m.path = custom_cubic_path(0.18);
        manual.channels.m.close_ends = true;
        manual.channels.m.smooth_join = false;
        manual.channels.m.grid_pivot_x = 5.0;
        manual.channels.m.grid_pivot_y = -8.0;
        manual.channels.m.offset_x = 6.0;
        manual.channels.m.offset_y = -3.0;
        manual.channels.m.curve_scale = 25.0;
        manual.channels.m.motif_coverage = MotifCoverage::Manual;
        manual.channels.m.motif_bleed = 3.0;
        manual.channels.m.tile_count = 3;
        manual.channels.m.tile_angle = 31.0;
        manual.channels.m.tile_offset = 2.0;
        manual.channels.m.stack_count = 2;
        manual.channels.m.stack_spacing = 28.0;
        manual.channels.m.stack_angle = 9.0;
        manual.channels.m.stack_offset = -6.0;
        manual.channels.m.alternate_stack_offset = -4.0;
        manual.channels.m.alternate_tile_transform = AlternateTileTransform::Flip;
        manual.channels.m.scale = 0.83;
        manual.channels.m.threshold = 0.23;
        manual.channels.m.max_size = 58.0;
        manual.channels.m.output_quality = 1.3;

        manual.channels.y.path = custom_cubic_path(0.27);
        manual.channels.y.close_ends = false;
        manual.channels.y.smooth_join = true;
        manual.channels.y.grid_pivot_x = 12.0;
        manual.channels.y.grid_pivot_y = 3.0;
        manual.channels.y.offset_x = 2.0;
        manual.channels.y.offset_y = 9.0;
        manual.channels.y.curve_scale = 31.0;
        manual.channels.y.motif_coverage = MotifCoverage::Manual;
        manual.channels.y.motif_bleed = 5.0;
        manual.channels.y.tile_count = 2;
        manual.channels.y.tile_angle = -19.0;
        manual.channels.y.tile_offset = 5.0;
        manual.channels.y.stack_count = 3;
        manual.channels.y.stack_spacing = 17.0;
        manual.channels.y.stack_angle = 14.0;
        manual.channels.y.stack_offset = 3.0;
        manual.channels.y.alternate_stack_offset = 6.0;
        manual.channels.y.alternate_tile_transform = AlternateTileTransform::Rotate180;
        manual.channels.y.scale = 0.72;
        manual.channels.y.threshold = 0.37;
        manual.channels.y.max_size = 67.0;
        manual.channels.y.output_quality = 2.2;

        manual.channels.k.path = custom_cubic_path(0.33);
        manual.channels.k.close_ends = true;
        manual.channels.k.smooth_join = true;
        manual.channels.k.grid_pivot_x = -4.0;
        manual.channels.k.grid_pivot_y = -11.0;
        manual.channels.k.offset_x = 11.0;
        manual.channels.k.offset_y = -7.0;
        manual.channels.k.curve_scale = 39.0;
        manual.channels.k.motif_coverage = MotifCoverage::Manual;
        manual.channels.k.motif_bleed = 2.0;
        manual.channels.k.tile_count = 4;
        manual.channels.k.tile_angle = 46.0;
        manual.channels.k.tile_offset = -1.0;
        manual.channels.k.stack_count = 2;
        manual.channels.k.stack_spacing = 33.0;
        manual.channels.k.stack_angle = -13.0;
        manual.channels.k.stack_offset = 8.0;
        manual.channels.k.alternate_stack_offset = -9.0;
        manual.channels.k.alternate_tile_transform = AlternateTileTransform::Flip;
        manual.channels.k.scale = 0.95;
        manual.channels.k.threshold = 0.49;
        manual.channels.k.max_size = 79.0;
        manual.channels.k.output_quality = 3.1;
        assert_orchestration_matches_retained_source(
            "cmyk-per-channel-manual-motif",
            &alpha_source,
            &manual,
            &cmyk_pipeline,
        );

        let mut rgb = manual.clone();
        rgb.value_mode = ValueMode::Rgb;
        rgb.layout = CurveLayout::MotifPattern;
        rgb.channels.c.enabled = false;
        rgb.channels.m.enabled = false;
        rgb.channels.y.enabled = false;
        rgb.channels.k.enabled = false;
        rgb.channels.r.enabled = true;
        rgb.channels.r.color = "#1946c8".into();
        rgb.channels.r.opacity = 0.31;
        rgb.channels.r.path = custom_cubic_path(0.15);
        rgb.channels.r.motif_coverage = MotifCoverage::Auto;
        rgb.channels.r.motif_bleed = 0.0;
        rgb.channels.r.resolution_scale = 0.6;
        rgb.channels.g.enabled = true;
        rgb.channels.g.color = "#38b849".into();
        rgb.channels.g.opacity = 0.57;
        rgb.channels.g.path = custom_cubic_path(0.24);
        rgb.channels.g.motif_coverage = MotifCoverage::Auto;
        rgb.channels.g.motif_bleed = 4.0;
        rgb.channels.g.resolution_scale = 1.4;
        rgb.channels.b.enabled = true;
        rgb.channels.b.color = "#d1792d".into();
        rgb.channels.b.opacity = 0.89;
        rgb.channels.b.path = custom_cubic_path(0.3);
        rgb.channels.b.motif_coverage = MotifCoverage::Auto;
        rgb.channels.b.motif_bleed = 8.0;
        rgb.channels.b.resolution_scale = 2.1;
        let rgb_pipeline =
            legacy_pipeline_from_facade(ValueMode::Rgb, OutputMode::RgbScreen, rgb.single_channel);
        assert_orchestration_matches_retained_source(
            "rgb-auto-motif-alpha",
            &alpha_source,
            &rgb,
            &rgb_pipeline,
        );

        let mut crosshatch = orchestration_settings();
        crosshatch.configure_crosshatch();
        crosshatch.output_width = 117;
        crosshatch.output_height = 79;
        crosshatch.long_edge_cells = 13.0;
        crosshatch.crosshatch_color = "#345678".into();
        let crosshatch_pipeline = legacy_pipeline_from_facade(
            ValueMode::CrosshatchLuminance,
            OutputMode::CmykInks,
            crosshatch.single_channel,
        );
        assert_orchestration_matches_retained_source(
            "legacy-crosshatch-external-color",
            &opaque_source,
            &crosshatch,
            &crosshatch_pipeline,
        );

        let mut boundaries = orchestration_settings();
        boundaries.output_width = 17;
        boundaries.output_height = 11;
        boundaries.long_edge_cells = 2.0;
        boundaries.min_mark = 0.0;
        boundaries.max_mark = 0.0;
        boundaries.channels.m.enabled = false;
        boundaries.channels.y.enabled = false;
        boundaries.channels.k.enabled = false;
        boundaries.channels.c.resolution_scale = 0.05;
        boundaries.channels.c.scale = 0.0;
        boundaries.channels.c.threshold = 1.0;
        boundaries.channels.c.max_size = 0.0;
        boundaries.channels.c.output_quality = 0.1;
        assert_orchestration_matches_retained_source(
            "response-boundaries-zero-width",
            &opaque_source,
            &boundaries,
            &cmyk_pipeline,
        );
    }

    #[test]
    fn bundled_orchestrator_proves_retained_noop_fields_do_not_change_canonical_paths() {
        let mut settings = orchestration_settings();
        settings.layout = CurveLayout::MotifPattern;
        settings.use_shared_curve = false;
        settings.channels.c.path = custom_cubic_path(0.21);
        settings.channels.c.motif_coverage = MotifCoverage::Manual;
        settings.channels.c.tile_count = 3;
        settings.channels.c.stack_count = 2;
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            settings.single_channel,
        );
        let source = image::RgbaImage::from_fn(31, 17, |x, y| {
            image::Rgba([(x * 9) as u8, (y * 11) as u8, 127, 175])
        });
        let prepared = PreparedSource::from_rgba_image(&source, 37);
        let token = CancellationToken::new();
        let retained = crate::curve_render::generate_curve_geometry_for_pipeline(
            &prepared, &settings, &pipeline, &token,
        )
        .unwrap();
        let recipe =
            execute_bundled_curves_recipe_cancellable(&prepared, &settings, &pipeline, &token)
                .unwrap();

        let mut changed_noops = settings.clone();
        changed_noops.show_background = !changed_noops.show_background;
        changed_noops.channels.c.tile_spacing = -1234.5;
        changed_noops.channels.m.tile_spacing = 9876.5;
        let retained_changed = crate::curve_render::generate_curve_geometry_for_pipeline(
            &prepared,
            &changed_noops,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        let recipe_changed = execute_bundled_curves_recipe_cancellable(
            &prepared,
            &changed_noops,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(retained_changed, retained, "retained no-op evidence");
        assert_eq!(recipe_changed, recipe, "recipe no-op evidence");
    }

    #[test]
    fn bundled_orchestrator_rejects_pathological_recipe_expansion_before_allocation() {
        let mut settings = orchestration_settings();
        settings.layout = CurveLayout::MotifPattern;
        settings.use_shared_curve = false;
        settings.channels.m.enabled = false;
        settings.channels.y.enabled = false;
        settings.channels.k.enabled = false;
        settings.channels.c.path = custom_cubic_path(0.2);
        settings.channels.c.motif_coverage = MotifCoverage::Manual;
        settings.channels.c.tile_count = 10_000;
        settings.channels.c.stack_count = 10_000;
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            settings.single_channel,
        );
        let prepared = PreparedSource::from_rgba_image(
            &image::RgbaImage::from_pixel(16, 16, image::Rgba([10, 20, 30, 255])),
            41,
        );
        let error = execute_bundled_curves_recipe_cancellable(
            &prepared,
            &settings,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("bounded"), "{error}");
    }

    #[test]
    fn bundled_orchestrator_matches_retained_cmyk_rgb_layout_motif_disabled_and_crosshatch() {
        let mut cmyk = orchestration_settings();
        for channel in [
            &mut cmyk.channels.c,
            &mut cmyk.channels.m,
            &mut cmyk.channels.y,
            &mut cmyk.channels.k,
        ] {
            channel.enabled = true;
        }
        let cmyk_pipeline =
            legacy_pipeline_from_facade(ValueMode::Cmyk, OutputMode::CmykInks, cmyk.single_channel);
        assert_orchestration_matches_retained("CMYK shared full-width", &cmyk, &cmyk_pipeline);

        let mut motif = cmyk.clone();
        motif.layout = CurveLayout::MotifPattern;
        motif.use_shared_curve = false;
        motif.channels.c.path = CurvePath::straight();
        motif.channels.m.path = CurvePath::soft_wave();
        motif.channels.y.enabled = false;
        assert_orchestration_matches_retained(
            "CMYK per-channel motif with disabled layer",
            &motif,
            &cmyk_pipeline,
        );

        let mut rgb = orchestration_settings();
        rgb.value_mode = ValueMode::Rgb;
        for channel in [
            &mut rgb.channels.r,
            &mut rgb.channels.g,
            &mut rgb.channels.b,
        ] {
            channel.enabled = true;
        }
        rgb.channels.r.color = "#0011ff".into();
        rgb.channels.g.color = "#00ff11".into();
        rgb.channels.b.color = "#ff1100".into();
        let rgb_pipeline =
            legacy_pipeline_from_facade(ValueMode::Rgb, OutputMode::RgbScreen, rgb.single_channel);
        assert_orchestration_matches_retained("RGB shared full-width", &rgb, &rgb_pipeline);

        let mut crosshatch = orchestration_settings();
        crosshatch.configure_crosshatch();
        crosshatch.output_width = 96;
        crosshatch.output_height = 64;
        crosshatch.long_edge_cells = 9.0;
        crosshatch.crosshatch_color = "#345678".into();
        let crosshatch_pipeline = legacy_pipeline_from_facade(
            ValueMode::CrosshatchLuminance,
            OutputMode::CmykInks,
            crosshatch.single_channel,
        );
        assert_orchestration_matches_retained(
            "Crosshatch compatibility color",
            &crosshatch,
            &crosshatch_pipeline,
        );
    }

    #[test]
    fn bundled_orchestrator_reuses_requested_fields_and_skips_disabled_work() {
        let mut settings = orchestration_settings();
        for channel in [
            &mut settings.channels.c,
            &mut settings.channels.m,
            &mut settings.channels.y,
            &mut settings.channels.k,
        ] {
            channel.enabled = true;
        }
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            settings.single_channel,
        );
        let source = image::RgbaImage::from_pixel(32, 16, image::Rgba([10, 20, 30, 255]));
        let prepared = PreparedSource::from_rgba_image(&source, 23);
        reset_curves_recipe_orchestration_instrumentation();
        reset_native_node_invocations();
        let output = execute_bundled_curves_recipe_cancellable(
            &prepared,
            &settings,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Paths(output) = output else {
            panic!("Curves orchestration must emit paths")
        };
        assert_eq!(output.geometry.layers.len(), 4);
        assert_eq!(provider_cache_misses(), 1);
        assert_eq!(native_node_invocations(), 24);

        settings.channels.c.resolution_scale = 1.0;
        settings.channels.m.resolution_scale = 2.0;
        settings.channels.y.resolution_scale = 1.0;
        settings.channels.k.resolution_scale = 2.0;
        reset_curves_recipe_orchestration_instrumentation();
        execute_bundled_curves_recipe_cancellable(
            &prepared,
            &settings,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(provider_cache_misses(), 2);

        for channel in [
            &mut settings.channels.c,
            &mut settings.channels.m,
            &mut settings.channels.y,
            &mut settings.channels.k,
        ] {
            channel.enabled = false;
        }
        reset_curves_recipe_orchestration_instrumentation();
        reset_native_node_invocations();
        let output = execute_bundled_curves_recipe_cancellable(
            &prepared,
            &settings,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Paths(output) = output else {
            panic!("Curves orchestration must emit paths")
        };
        assert!(output.geometry.layers.is_empty());
        assert_eq!(provider_cache_misses(), 0);
        assert_eq!(native_node_invocations(), 0);
    }

    #[test]
    fn bundled_orchestrator_honors_cancellation_before_channel_execution() {
        let settings = orchestration_settings();
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            settings.single_channel,
        );
        let source = image::RgbaImage::from_pixel(32, 16, image::Rgba([10, 20, 30, 255]));
        let prepared = PreparedSource::from_rgba_image(&source, 29);
        let token = CancellationToken::new();
        token.cancel();
        reset_curves_recipe_orchestration_instrumentation();
        reset_native_node_invocations();
        let error =
            execute_bundled_curves_recipe_cancellable(&prepared, &settings, &pipeline, &token)
                .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(curves_recipe_orchestration_invocations(), 1);
        assert_eq!(native_node_invocations(), 0);
        assert_eq!(provider_cache_misses(), 0);

        let token = CancellationToken::new();
        reset_curves_recipe_orchestration_instrumentation();
        reset_native_node_invocations();
        cancel_after_first_orchestrated_channel_for_test();
        let error =
            execute_bundled_curves_recipe_cancellable(&prepared, &settings, &pipeline, &token)
                .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(curves_recipe_orchestration_invocations(), 1);
        assert_eq!(native_node_invocations(), 6);
        assert_eq!(provider_cache_misses(), 1);
    }
}
