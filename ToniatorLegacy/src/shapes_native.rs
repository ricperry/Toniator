//! Bounded native bodies for the declarative compatibility Shapes recipe.
//!
//! The shipping renderer remains the live authority through 3D2. These
//! operations deliberately consume only recipe inputs, parameters, definition
//! assets, and the generic source-field request boundary.

use crate::artwork_pipeline::{
    ArtworkPipelineSettings, OutputChannelId, PreparedSource, ResolvedChannelField,
    ResolvedChannelFields, resolve_channel_fields_cancellable,
};
use crate::curve_render::{VariablePoint, outline_from_variable_points};
use crate::model::{Treatment, WebShapeSettings, parse_hex_color};
use crate::pattern::{
    AffineTransform, ArtboardSpace, CanonicalBlendMode, CanonicalColor, CanonicalLayer,
    CanonicalLayerId, CanonicalPatternOutput, GeometryPolarity, MarkPatternOutput, NetworkEdgeId,
    NetworkNode, NetworkNodeId, NetworkPatternOutput, NetworkStroke, NetworkStrokeId, PatternId,
    PatternOutputKind, SharedBoundaryEdge, adapt_legacy_shapes,
};
use crate::pattern_definition::{
    EmbeddedSvgAsset, LiteralValue, NativeRecipeOperationError, NativeRecipeOperationRegistry,
    PatternDefinition, PatternInstanceParameters, REGISTERED_OPERATIONS, RecipeExecutionContext,
    RecipeOperationInputs, RecipeOperationParameters, RecipeRuntimeValue,
    RegisteredNativeRecipeOperation,
};
use crate::render::{
    InkLayer, Mark, MarkGeometry, MarkSet, ResolvedWebShape, WebGrid, calculate_web_grid,
    map_web_threshold,
};
use crate::site_distribution::DistributionField;
use crate::site_distribution::{
    ArrangementPolicy, DistributionIdentity, DistributionLimits, DistributionMode,
    DistributionPolarity, DistributionRequest, DistributionRequestMetadata, OrderedPoint,
    generate_site_distribution_cancellable,
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;

/// Hard upper bound on source samples retained by one Shapes recipe execution.
pub const SHAPES_MAX_LATTICE_CELLS: usize = 1_000_000;
/// Hard upper bound on mark candidates expanded for one semantic channel.
pub const SHAPES_MAX_LATTICE_CANDIDATES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ShapesLattice {
    pub artboard: ArtboardSpace,
    pub grid: WebGrid,
    grid_rotation: f64,
    grid_pivot_x: f64,
    grid_pivot_y: f64,
    offset_x: f64,
    offset_y: f64,
    x_grid_curved: bool,
    y_grid_curved: bool,
    x_grid_curve: f64,
    y_grid_curve: f64,
    curve_function: ShapesCurveFunction,
    placement_strategy: ShapesPlacementStrategy,
    random_dispersion: ShapesRandomDispersion,
    point_definition: ShapesPointDefinition,
    sampler: ShapesPointSampler,
    seed: u64,
    weight_influence: f64,
    /// Random-pattern mark-size response: 0 is uniform, 1 is source-sized.
    random_size_response: f64,
    jitter_factor: f64,
    curve_spacing: f64,
    /// Editor-authored lattices use periodic site coordinates so rotated
    /// construction cannot leave a matching gap on the opposite edge. The
    /// immutable compatibility recipe keeps its historical out-of-artboard
    /// centers for parity and clipping.
    wrap_sites: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapesSamples {
    pub lattice: ShapesLattice,
    pub field: DistributionField,
    placements: Option<Vec<ShapesPlacement>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapesMappedValues {
    pub lattice: ShapesLattice,
    /// Size-response factors after threshold/minimum/maximum mapping.
    pub values: Vec<f64>,
    /// The configured upper response bound used for rotated lattice coverage,
    /// independent of the values sampled in this particular source field.
    pub maximum_extent_factor: f64,
    /// Baseline mark extent used when random patterns request uniform marks.
    pub uniform_extent_factor: f64,
    placements: Option<Vec<ShapesPlacement>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapesPointDefinition {
    Intersections,
    CurveSpacing,
    FullCurves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapesPointSampler {
    Grid,
    Uniform,
    Weighted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapesCurveFunction {
    Sine,
    Square,
    Spiral,
    Sawtooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapesPlacementStrategy {
    Grid,
    TriangularGrid,
    Curve,
    Random,
    MathFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapesRandomDispersion {
    Uniform,
    Gaussian,
    BlueNoise,
    PinkNoise,
    Poisson,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapesSelectedPrimitive {
    pub mapped_values: ShapesMappedValues,
    pub shape: ResolvedWebShape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapesTransformedMarks {
    pub artboard: ArtboardSpace,
    pub marks: Vec<Mark>,
    /// Whether each mark continues the previous authored path point. A
    /// thresholded-out sample starts a new segment instead of being silently
    /// bridged by the network emitter.
    pub continuation: Vec<bool>,
}

/// Fixed native bodies selected by `compat.shapes.v1`; definitions cannot add
/// code or select any implementation outside this finite registry.
pub static SHAPES_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 9] = [
    RegisteredNativeRecipeOperation {
        id: "shapes.lattice-placement",
        version: 1,
        execute: shapes_lattice_placement,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.lattice-placement-editor",
        version: 1,
        execute: shapes_lattice_placement_editor,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.lattice-placement-editor",
        version: 2,
        execute: shapes_lattice_placement_editor,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.source-sample",
        version: 1,
        execute: shapes_source_sample,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.mark-map",
        version: 1,
        execute: shapes_mark_map,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.primitive-selection",
        version: 1,
        execute: shapes_primitive_selection,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.transforms",
        version: 1,
        execute: shapes_transforms,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.emit-marks",
        version: 1,
        execute: shapes_emit_marks,
    },
    RegisteredNativeRecipeOperation {
        id: "shapes.emit-network",
        version: 1,
        execute: shapes_emit_network,
    },
];

pub static SHAPES_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::with_preflight(
        REGISTERED_OPERATIONS.entries(),
        &SHAPES_NATIVE_OPERATIONS,
        validate_shapes_recipe_execution_assets,
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
    NATIVE_NODE_INVOCATIONS.with(|count| count.get())
}

#[cfg(test)]
thread_local! {
    static ORCHESTRATION_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROVIDER_CACHE_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_orchestration_invocation() {
    ORCHESTRATION_INVOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_orchestration_invocation() {}

#[cfg(test)]
pub(crate) fn reset_shapes_recipe_orchestration_instrumentation() {
    ORCHESTRATION_INVOCATIONS.with(|count| count.set(0));
    PROVIDER_CACHE_MISSES.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn shapes_recipe_orchestration_invocations() -> usize {
    ORCHESTRATION_INVOCATIONS.with(|count| count.get())
}

#[cfg(test)]
fn provider_cache_misses() -> usize {
    PROVIDER_CACHE_MISSES.with(|count| count.get())
}

/// Preflight the one SVG asset a selected custom primitive can consume. This
/// is intentionally stricter than generic SVG safety validation and runs
/// before any Shapes recipe node executes.
pub(crate) fn validate_shapes_recipe_execution_assets(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    context: &RecipeExecutionContext<'_>,
) -> Result<(), NativeRecipeOperationError> {
    let global = |key: &str| {
        instance
            .pattern_values
            .iter()
            .find(|value| value.key == key)
            .map(|value| &value.value)
            .ok_or_else(|| {
                NativeRecipeOperationError::new(format!("missing Shapes parameter `{key}`"))
            })
    };
    let shared = match global("use-shared-mark")? {
        LiteralValue::Boolean(value) => *value,
        _ => {
            return Err(NativeRecipeOperationError::new(
                "Shapes parameter `use-shared-mark` must be boolean",
            ));
        }
    };
    let values = if shared {
        None
    } else {
        let channel = context.output_channel.ok_or_else(|| {
            NativeRecipeOperationError::new(
                "Shapes custom motif validation requires a semantic output channel",
            )
        })?;
        Some(
            instance
                .output_channel_values
                .iter()
                .find(|values| values.channel == channel.stable_id())
                .ok_or_else(|| {
                    NativeRecipeOperationError::new(
                        "Shapes recipe instance has no selected output-channel values",
                    )
                })?,
        )
    };
    let selected = |key: &str| -> Result<&LiteralValue, NativeRecipeOperationError> {
        if let Some(values) = values {
            values
                .values
                .iter()
                .find(|value| value.key == key)
                .map(|value| &value.value)
                .ok_or_else(|| {
                    NativeRecipeOperationError::new(format!("missing Shapes parameter `{key}`"))
                })
        } else {
            global(key)
        }
    };
    let shape_key = if shared { "shared-shape" } else { "shape" };
    let custom_key = if shared {
        "global-custom-motif"
    } else {
        "channel-custom-motif"
    };
    if !matches!(selected(shape_key)?, LiteralValue::Choice(shape) if shape == "user-defined") {
        return Ok(());
    }
    let LiteralValue::SvgAsset(digest) = selected(custom_key)? else {
        return Err(NativeRecipeOperationError::new(format!(
            "Shapes parameter `{custom_key}` must reference an SVG asset"
        )));
    };
    parse_supported_custom_motif(digest, &definition.assets).map(|_| ())
}

/// Executes the immutable Shapes recipe for the authoritative settings and
/// artwork pipeline without changing the live renderer dispatch. The provider
/// resolves exactly the lattice dimensions declared by each enabled recipe
/// channel, caching repeated dimensions for the whole orchestration.
pub fn execute_bundled_shapes_recipe_cancellable(
    prepared: &PreparedSource,
    settings: &WebShapeSettings,
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
    let adaptation = crate::shapes_recipe::adapt_shapes_settings_to_recipe(settings)?;
    let provider = ShapesRecipeSourceProvider {
        prepared,
        pipeline,
        enabled: &enabled,
        fields: RefCell::new(HashMap::new()),
    };
    let artboard = ArtboardSpace {
        width: settings.output_width,
        height: settings.output_height,
    };
    let mut marks = Vec::new();
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
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: token,
        };
        let CanonicalPatternOutput::Marks(output) = adaptation.definition.execute_recipe(
            &adaptation.instance,
            &context,
            &SHAPES_NATIVE_OPERATION_REGISTRY,
        )?
        else {
            unreachable!("the bundled Shapes recipe declares mark output")
        };
        marks.extend(output.geometry.marks);
    }
    let layers = output_channels
        .iter()
        .copied()
        .map(|channel| {
            let shape_channel = settings.channels.get(channel.to_legacy_ink());
            let color = if matches!(
                pipeline.assignment,
                crate::artwork_pipeline::ChannelAssignment::LegacyCompatibility(_)
            ) {
                parse_hex_color(&settings.crosshatch_color).unwrap_or((17, 17, 17))
            } else {
                parse_hex_color(&shape_channel.color).unwrap_or_else(|| {
                    crate::render::Channel::from(channel.to_legacy_ink()).color()
                })
            };
            InkLayer {
                channel: channel.to_legacy_ink().into(),
                enabled: shape_channel.enabled,
                color,
                opacity: shape_channel.opacity as f32,
            }
        })
        .collect();
    adapt_legacy_shapes(
        PatternId::COMPATIBILITY_SHAPES_V1,
        MarkSet {
            width: settings.output_width,
            height: settings.output_height,
            marks,
            layers,
        },
    )
    .map_err(anyhow::Error::new)
}

/// Validates the deliberately narrow custom runtime surface: a definition must
/// use only the finite Shapes operation registry and emit either discrete marks
/// or canonical connected networks. Both outputs remain data-only and bounded.
pub fn validate_shapes_definition_instance(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
) -> anyhow::Result<()> {
    definition
        .validate_with_registry(&SHAPES_NATIVE_OPERATION_REGISTRY.descriptors())
        .map_err(anyhow::Error::new)?;
    definition
        .validate_instance_parameters_with_registry(
            instance,
            &SHAPES_NATIVE_OPERATION_REGISTRY.descriptors(),
        )
        .map_err(anyhow::Error::new)?;
    anyhow::ensure!(
        definition.outputs == [PatternOutputKind::Marks]
            || definition.outputs == [PatternOutputKind::Networks],
        "custom Shapes definitions must declare mark or network output"
    );
    Ok(())
}

/// Resolves the required Shapes artboard from its validated global instance
/// values. A custom project recipe remains self-contained; no canvas size is
/// recovered from a legacy renderer facade.
pub fn shapes_instance_artboard(
    instance: &PatternInstanceParameters,
) -> anyhow::Result<ArtboardSpace> {
    let integer = |key: &str| {
        instance
            .pattern_values
            .iter()
            .find(|value| value.key == key)
            .and_then(|value| match value.value {
                LiteralValue::Integer(value) => Some(value),
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("custom Shapes instance is missing integer `{key}`"))
    };
    let width = u32::try_from(integer("output-width")?)?;
    let height = u32::try_from(integer("output-height")?)?;
    let artboard = ArtboardSpace { width, height };
    artboard.validate().map_err(anyhow::Error::new)?;
    Ok(artboard)
}

/// Executes any validated project-embedded Shapes-compatible definition with
/// the same production artwork pipeline, source provider, cancellation points,
/// and lattice bounds as bundled Shapes. The caller supplies an artboard so UI
/// and project-runtime callers share one explicit execution contract.
pub fn execute_shapes_definition_cancellable(
    prepared: &PreparedSource,
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    pipeline: &ArtworkPipelineSettings,
    artboard: ArtboardSpace,
    token: &crate::CancellationToken,
) -> anyhow::Result<CanonicalPatternOutput> {
    record_orchestration_invocation();
    token.checkpoint()?;
    validate_shapes_definition_instance(definition, instance)?;
    artboard.validate().map_err(anyhow::Error::new)?;
    let output_channels = if matches!(
        pipeline.assignment,
        crate::artwork_pipeline::ChannelAssignment::LegacyCompatibility(_)
    ) {
        OutputChannelId::CMYK.to_vec()
    } else {
        pipeline.output_model.channels().to_vec()
    };
    let provider = ShapesRecipeSourceProvider {
        prepared,
        pipeline,
        // Execute every selected output channel, including disabled layers,
        // because their declared recipe output retains the authoritative layer
        // order and opacity. The finite source resolver remains bounded.
        enabled: &output_channels,
        fields: RefCell::new(HashMap::new()),
    };
    let mut marks = Vec::new();
    let mut layers = Vec::new();
    let mut network_nodes = Vec::new();
    let mut network_edges = Vec::new();
    let mut network_strokes = Vec::new();
    let mut network_layers = Vec::new();
    for (semantic_channel_index, channel) in output_channels.iter().copied().enumerate() {
        token.checkpoint()?;
        let context = RecipeExecutionContext {
            artboard,
            output_channel: Some(channel),
            source_field_provider: Some(&provider),
            source_field: None,
            source_generation: prepared.generation,
            resolved_field_generation: prepared.generation,
            semantic_channel_index: semantic_channel_index as u32,
            enabled_layer_index: semantic_channel_index as u32,
            definition_assets: &[],
            cancellation: token,
        };
        match definition.execute_recipe(instance, &context, &SHAPES_NATIVE_OPERATION_REGISTRY)? {
            CanonicalPatternOutput::Marks(output) => {
                marks.extend(output.geometry.marks);
                layers.extend(output.geometry.layers);
            }
            CanonicalPatternOutput::Network(output) => {
                let node_offset = network_nodes.len() as u32;
                let edge_offset = network_edges.len() as u32;
                let stroke_offset = network_strokes.len() as u32;
                let layer_offset = network_layers.len() as u32;
                network_nodes.extend(output.nodes.into_iter().map(|mut node| {
                    node.id = NetworkNodeId(node.id.0.saturating_add(node_offset));
                    node
                }));
                network_edges.extend(output.edges.into_iter().map(|mut edge| {
                    edge.id = NetworkEdgeId(edge.id.0.saturating_add(edge_offset));
                    edge.layer_id = CanonicalLayerId(edge.layer_id.0.saturating_add(layer_offset));
                    edge.start = NetworkNodeId(edge.start.0.saturating_add(node_offset));
                    edge.end = NetworkNodeId(edge.end.0.saturating_add(node_offset));
                    edge
                }));
                network_strokes.extend(output.strokes.into_iter().map(|mut stroke| {
                    stroke.id = NetworkStrokeId(stroke.id.0.saturating_add(stroke_offset));
                    stroke.layer_id =
                        CanonicalLayerId(stroke.layer_id.0.saturating_add(layer_offset));
                    stroke.order = stroke.order.saturating_add(stroke_offset);
                    stroke
                }));
                network_layers.extend(output.layers.into_iter().map(|mut layer| {
                    layer.id = CanonicalLayerId(layer.id.0.saturating_add(layer_offset));
                    layer
                }));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "custom Shapes definition emitted unsupported canonical output"
                ));
            }
        }
    }
    let output = if definition.outputs == [PatternOutputKind::Networks] {
        CanonicalPatternOutput::Network(NetworkPatternOutput {
            artboard,
            layers: network_layers,
            nodes: network_nodes,
            edges: network_edges,
            strokes: network_strokes,
            transform: AffineTransform::IDENTITY,
        })
    } else {
        CanonicalPatternOutput::Marks(MarkPatternOutput {
            geometry: MarkSet {
                width: artboard.width,
                height: artboard.height,
                marks,
                layers,
            },
        })
    };
    output.validate().map_err(anyhow::Error::new)?;
    Ok(output)
}

struct ShapesRecipeSourceProvider<'a> {
    prepared: &'a PreparedSource,
    pipeline: &'a ArtworkPipelineSettings,
    enabled: &'a [OutputChannelId],
    fields: RefCell<HashMap<(u32, u32), ResolvedChannelFields>>,
}

impl crate::RecipeSourceFieldProvider for ShapesRecipeSourceProvider<'_> {
    fn resolve_source_field(
        &self,
        channel: OutputChannelId,
        columns: u32,
        rows: u32,
        cancellation: &crate::CancellationToken,
    ) -> Result<DistributionField, NativeRecipeOperationError> {
        cancellation.checkpoint().map_err(native_error)?;
        let key = (columns, rows);
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
            .map_err(native_error)?;
            self.fields.borrow_mut().insert(key, fields);
        }
        let fields = self.fields.borrow();
        let field = fields
            .get(&key)
            .and_then(|fields| fields.field(channel))
            .ok_or_else(|| {
                NativeRecipeOperationError::new(
                    "Shapes source field provider has no requested semantic channel",
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
    DistributionField::new(field.bounds.width, field.bounds.height, values).map_err(native_error)
}

fn native_error(error: impl std::fmt::Display) -> NativeRecipeOperationError {
    NativeRecipeOperationError::new(error.to_string())
}

fn required_input<'a>(
    inputs: &RecipeOperationInputs<'a>,
    name: &'static str,
) -> Result<&'a RecipeRuntimeValue, NativeRecipeOperationError> {
    inputs
        .get(name)
        .copied()
        .ok_or_else(|| NativeRecipeOperationError::new(format!("missing required input `{name}`")))
}

fn required_integer(
    parameters: &RecipeOperationParameters<'_>,
    name: &'static str,
) -> Result<u64, NativeRecipeOperationError> {
    match parameters.get(name) {
        Some(LiteralValue::Integer(value)) => Ok(*value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be an integer"
        ))),
        None => Err(NativeRecipeOperationError::new(format!(
            "missing required parameter `{name}`"
        ))),
    }
}

fn required_number(
    parameters: &RecipeOperationParameters<'_>,
    name: &'static str,
) -> Result<f64, NativeRecipeOperationError> {
    match parameters.get(name) {
        Some(LiteralValue::Number(value)) if value.is_finite() => Ok(*value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be a finite number"
        ))),
        None => Err(NativeRecipeOperationError::new(format!(
            "missing required parameter `{name}`"
        ))),
    }
}

fn required_boolean(
    parameters: &RecipeOperationParameters<'_>,
    name: &'static str,
) -> Result<bool, NativeRecipeOperationError> {
    match parameters.get(name) {
        Some(LiteralValue::Boolean(value)) => Ok(*value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be a boolean"
        ))),
        None => Err(NativeRecipeOperationError::new(format!(
            "missing required parameter `{name}`"
        ))),
    }
}

fn required_choice<'a>(
    parameters: &'a RecipeOperationParameters<'_>,
    name: &'static str,
) -> Result<&'a str, NativeRecipeOperationError> {
    match parameters.get(name) {
        Some(LiteralValue::Choice(value)) => Ok(value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be a choice"
        ))),
        None => Err(NativeRecipeOperationError::new(format!(
            "missing required parameter `{name}`"
        ))),
    }
}

fn optional_choice<'a>(
    parameters: &'a RecipeOperationParameters<'_>,
    name: &'static str,
    fallback: &'a str,
) -> Result<&'a str, NativeRecipeOperationError> {
    match parameters.get(name) {
        None => Ok(fallback),
        Some(LiteralValue::Choice(value)) => Ok(value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be a choice"
        ))),
    }
}

fn optional_number(
    parameters: &RecipeOperationParameters<'_>,
    name: &'static str,
    fallback: f64,
) -> Result<f64, NativeRecipeOperationError> {
    match parameters.get(name) {
        None => Ok(fallback),
        Some(LiteralValue::Number(value)) if value.is_finite() => Ok(*value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must be a finite number"
        ))),
    }
}

fn required_svg_asset<'a>(
    parameters: &'a RecipeOperationParameters<'_>,
    name: &'static str,
) -> Result<&'a str, NativeRecipeOperationError> {
    match parameters.get(name) {
        Some(LiteralValue::SvgAsset(value)) => Ok(value),
        Some(_) => Err(NativeRecipeOperationError::new(format!(
            "parameter `{name}` must reference an SVG asset"
        ))),
        None => Err(NativeRecipeOperationError::new(format!(
            "missing required parameter `{name}`"
        ))),
    }
}

fn required_channel(
    context: &RecipeExecutionContext<'_>,
) -> Result<OutputChannelId, NativeRecipeOperationError> {
    context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new("Shapes operation requires a semantic output channel")
    })
}

fn shapes_lattice_placement(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    shapes_lattice_placement_common(context, inputs, parameters, false)
}

fn shapes_lattice_placement_editor(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    shapes_lattice_placement_common(context, inputs, parameters, true)
}

fn shapes_lattice_placement_common(
    context: &RecipeExecutionContext<'_>,
    _: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
    editor: bool,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let width =
        u32::try_from(required_integer(parameters, "output-width")?).map_err(native_error)?;
    let height =
        u32::try_from(required_integer(parameters, "output-height")?).map_err(native_error)?;
    if width == 0
        || height == 0
        || context.artboard.width != width
        || context.artboard.height != height
    {
        return Err(NativeRecipeOperationError::new(
            "Shapes lattice artboard parameters must match the execution context",
        ));
    }
    let long_edge_cells = required_number(parameters, "long-edge-cells")?;
    let resolution_scale = required_number(parameters, "resolution-scale")?;
    if !(2.0..=10_000.0).contains(&long_edge_cells)
        || !(0.000_001..=100.0).contains(&resolution_scale)
    {
        return Err(NativeRecipeOperationError::new(
            "Shapes lattice density or resolution is outside the supported range",
        ));
    }
    let default_grid = calculate_web_grid(
        width,
        height,
        (long_edge_cells * resolution_scale.max(0.05))
            .round()
            .max(2.0),
    );
    let (
        grid,
        x_grid_curved,
        y_grid_curved,
        x_grid_curve,
        y_grid_curve,
        curve_function,
        placement_strategy,
        random_dispersion,
        point_definition,
        sampler,
        seed,
        weight_influence,
        random_size_response,
        jitter_factor,
        curve_spacing,
    ) = if editor {
        let x_spacing = required_number(parameters, "x-grid-spacing")?;
        let y_spacing = required_number(parameters, "y-grid-spacing")?;
        let x_mode = required_choice(parameters, "x-grid-mode")?;
        let y_mode = required_choice(parameters, "y-grid-mode")?;
        let x_curve = required_number(parameters, "x-grid-curve")?;
        let y_curve = required_number(parameters, "y-grid-curve")?;
        let curve_spacing = optional_number(parameters, "curve-spacing", x_spacing.min(y_spacing))?;
        let curve_function = match optional_choice(parameters, "curve-function", "sine")? {
            "sine" => ShapesCurveFunction::Sine,
            "square" => ShapesCurveFunction::Square,
            "spiral" => ShapesCurveFunction::Spiral,
            "sawtooth" => ShapesCurveFunction::Sawtooth,
            _ => {
                return Err(NativeRecipeOperationError::new(
                    "curve function must be sine, square, spiral, or sawtooth",
                ));
            }
        };
        let placement_strategy = match optional_choice(parameters, "placement-strategy", "grid")? {
            "grid" => ShapesPlacementStrategy::Grid,
            "triangular-grid" => ShapesPlacementStrategy::TriangularGrid,
            "curve" => ShapesPlacementStrategy::Curve,
            "random" => ShapesPlacementStrategy::Random,
            "math-function" => ShapesPlacementStrategy::MathFunction,
            _ => {
                return Err(NativeRecipeOperationError::new(
                    "placement strategy must be grid, triangular-grid, curve, random, or math-function",
                ));
            }
        };
        let random_dispersion = match optional_choice(parameters, "random-dispersion", "uniform")? {
            "uniform" => ShapesRandomDispersion::Uniform,
            "gaussian" => ShapesRandomDispersion::Gaussian,
            "blue-noise" => ShapesRandomDispersion::BlueNoise,
            "pink-noise" => ShapesRandomDispersion::PinkNoise,
            "poisson" => ShapesRandomDispersion::Poisson,
            _ => {
                return Err(NativeRecipeOperationError::new(
                    "random dispersion must be uniform, gaussian, blue-noise, pink-noise, or poisson",
                ));
            }
        };
        let point_definition = match optional_choice(
            parameters,
            "point-definition",
            "intersections",
        )? {
            "intersections" => ShapesPointDefinition::Intersections,
            "curve-spacing" => ShapesPointDefinition::CurveSpacing,
            "full-curves" => ShapesPointDefinition::FullCurves,
            _ => {
                return Err(NativeRecipeOperationError::new(
                    "editor point definition must be intersections, curve-spacing, or full-curves",
                ));
            }
        };
        let sampler = match required_choice(parameters, "point-sampler")? {
            "grid" => ShapesPointSampler::Grid,
            "uniform" => ShapesPointSampler::Uniform,
            "weighted" => ShapesPointSampler::Weighted,
            _ => {
                return Err(NativeRecipeOperationError::new(
                    "editor point sampler must be grid, uniform, or weighted",
                ));
            }
        };
        let channel_seed = required_integer(parameters, "channel-seed")?;
        let seed = channel_seed;
        let weight_influence = required_number(parameters, "channel-weight-influence")?;
        let random_size_response = optional_number(parameters, "random-size-response", 1.0)?;
        let jitter_factor = required_number(parameters, "jitter-factor")?;
        if !(0.0..=1.0).contains(&jitter_factor)
            || !(0.0..=1.0).contains(&random_size_response)
            || !(0.001..=16.0).contains(&weight_influence)
            || !(0.0..=100_000.0).contains(&x_spacing)
            || !(0.0..=100_000.0).contains(&y_spacing)
            || !(0.01..=100_000.0).contains(&curve_spacing)
            || !x_curve.is_finite()
            || !y_curve.is_finite()
            || !matches!(x_mode, "straight" | "curve")
            || !matches!(y_mode, "straight" | "curve")
        {
            return Err(NativeRecipeOperationError::new(
                "editor grid spacing, curves, or jitter are outside the supported range",
            ));
        }
        let (x_grid_curved, y_grid_curved, point_definition, sampler) = match placement_strategy {
            ShapesPlacementStrategy::Grid => (
                x_mode == "curve",
                y_mode == "curve",
                point_definition,
                sampler,
            ),
            ShapesPlacementStrategy::TriangularGrid => {
                (false, false, ShapesPointDefinition::Intersections, sampler)
            }
            ShapesPlacementStrategy::Curve => {
                (true, false, ShapesPointDefinition::CurveSpacing, sampler)
            }
            ShapesPlacementStrategy::Random => (
                false,
                false,
                ShapesPointDefinition::Intersections,
                if sampler == ShapesPointSampler::Grid {
                    // Grid is the neutral channel default for lattice-based
                    // patterns. A Random pattern has no lattice sites to
                    // reuse, so its neutral fallback is source-weighted
                    // random placement; users can still opt into explicitly
                    // uniform random sites from Channel Settings.
                    ShapesPointSampler::Weighted
                } else {
                    sampler
                },
            ),
            ShapesPlacementStrategy::MathFunction => (true, true, point_definition, sampler),
        };
        let grid = editor_grid(
            width,
            height,
            &default_grid,
            if placement_strategy == ShapesPlacementStrategy::Random {
                default_grid.cell_width
            } else if placement_strategy == ShapesPlacementStrategy::Curve {
                curve_spacing
            } else {
                x_spacing
            },
            if placement_strategy == ShapesPlacementStrategy::Random {
                default_grid.cell_height
            } else {
                y_spacing
            },
        )?;
        let grid = if placement_strategy == ShapesPlacementStrategy::Random {
            grid
        } else if placement_strategy == ShapesPlacementStrategy::TriangularGrid {
            // Triangular placement still honors the authored X spacing, but
            // Sampling Detail must scale its site population just like the
            // rectangular grid.  The old branch used only the explicit
            // spacing, so changing a channel's resolution multiplier had no
            // observable effect on triangular recipes.
            let authored_cells = u64::from(grid.cols).saturating_mul(u64::from(grid.rows));
            let density_cells =
                u64::from(default_grid.cols).saturating_mul(u64::from(default_grid.rows));
            let density_ratio = (density_cells as f64 / authored_cells.max(1) as f64)
                .sqrt()
                .clamp(0.05, 20.0);
            let cell_width = (grid.cell_width / density_ratio).max(0.01);
            let cols = (f64::from(width) / cell_width).ceil().max(1.0) as u32;
            let cell_width = f64::from(width) / f64::from(cols);
            let triangular_height = cell_width * (3.0_f64.sqrt() * 0.5);
            let rows = (f64::from(height) / triangular_height.max(f64::EPSILON))
                .ceil()
                .max(1.0) as u32;
            let cells = u64::from(cols).saturating_mul(u64::from(rows));
            if cells > SHAPES_MAX_LATTICE_CELLS as u64 {
                return Err(NativeRecipeOperationError::new(
                    "triangular grid exceeds the bounded lattice limit",
                ));
            }
            WebGrid {
                cols,
                rows,
                cell_width,
                cell_height: triangular_height,
            }
        } else {
            density_adjusted_editor_grid(
                width,
                height,
                &default_grid,
                grid.cell_width,
                grid.cell_height,
            )?
        };
        (
            grid,
            x_grid_curved,
            y_grid_curved,
            x_curve,
            y_curve,
            curve_function,
            placement_strategy,
            random_dispersion,
            point_definition,
            sampler,
            seed,
            weight_influence,
            random_size_response,
            jitter_factor,
            curve_spacing,
        )
    } else {
        (
            default_grid,
            false,
            false,
            0.0,
            0.0,
            ShapesCurveFunction::Sine,
            ShapesPlacementStrategy::Grid,
            ShapesRandomDispersion::Uniform,
            ShapesPointDefinition::Intersections,
            ShapesPointSampler::Grid,
            0,
            1.0,
            1.0,
            0.0,
            default_grid.cell_width.min(default_grid.cell_height),
        )
    };
    let cells = u64::from(grid.cols) * u64::from(grid.rows);
    if cells > SHAPES_MAX_LATTICE_CELLS as u64 {
        return Err(NativeRecipeOperationError::new(
            "Shapes lattice exceeds the bounded source-sample grid",
        ));
    }
    Ok(RecipeRuntimeValue::ShapesLattice(ShapesLattice {
        artboard: context.artboard,
        grid,
        grid_rotation: required_number(parameters, "grid-rotation")?,
        grid_pivot_x: required_number(parameters, "grid-pivot-x")?,
        grid_pivot_y: required_number(parameters, "grid-pivot-y")?,
        offset_x: required_number(parameters, "offset-x")?,
        offset_y: required_number(parameters, "offset-y")?,
        x_grid_curved,
        y_grid_curved,
        x_grid_curve,
        y_grid_curve,
        curve_function,
        placement_strategy,
        random_dispersion,
        point_definition,
        sampler,
        seed,
        weight_influence,
        random_size_response,
        jitter_factor,
        curve_spacing,
        wrap_sites: editor
            && matches!(
                placement_strategy,
                ShapesPlacementStrategy::Grid | ShapesPlacementStrategy::TriangularGrid
            ),
    }))
}

fn shapes_source_sample(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    _: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesLattice(lattice) = required_input(inputs, "lattice")? else {
        return Err(NativeRecipeOperationError::new(
            "input `lattice` must be a Shapes lattice",
        ));
    };
    let field = match context.source_field_provider {
        Some(provider) => provider.resolve_source_field(
            required_channel(context)?,
            lattice.grid.cols,
            lattice.grid.rows,
            context.cancellation,
        )?,
        None => context.source_field.cloned().ok_or_else(|| {
            NativeRecipeOperationError::new(
                "Shapes source-sample requires a source field or field-request provider",
            )
        })?,
    };
    if field.dimensions() != (lattice.grid.cols, lattice.grid.rows) {
        return Err(NativeRecipeOperationError::new(
            "Shapes source field dimensions do not match the declared lattice",
        ));
    }
    let placements = lattice
        .requires_explicit_placements()
        .then(|| build_explicit_placements(lattice, &field, context))
        .transpose()?;
    Ok(RecipeRuntimeValue::ShapesSamples(ShapesSamples {
        lattice: lattice.clone(),
        field,
        placements,
    }))
}

fn shapes_mark_map(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesSamples(samples) = required_input(inputs, "samples")? else {
        return Err(NativeRecipeOperationError::new(
            "input `samples` must be Shapes source samples",
        ));
    };
    let minimum = required_number(parameters, "min-mark")?;
    let maximum = required_number(parameters, "max-mark")?;
    let threshold = required_number(parameters, "threshold")?;
    let max_size = required_number(parameters, "max-size")?;
    if !(0.0..=1000.0).contains(&minimum)
        || !(minimum..=1000.0).contains(&maximum)
        || !(0.0..=1.0).contains(&threshold)
        || !(0.0..=10_000.0).contains(&max_size)
    {
        return Err(NativeRecipeOperationError::new(
            "Shapes mark mapping parameters are outside the supported range",
        ));
    }
    let minimum_factor = minimum / 100.0;
    let maximum_factor = (maximum / 100.0).max(minimum_factor) * (max_size / 100.0);
    let uniform_extent_factor = (minimum_factor + maximum_factor) * 0.5;
    let (values, placements) = if let Some(placements) = &samples.placements {
        let values = placements
            .iter()
            .map(|placement| samples.field.values()[placement.sample_index])
            .enumerate()
            .map(|(index, raw)| {
                if index % 1024 == 0 {
                    context.cancellation.checkpoint().map_err(native_error)?;
                }
                let response = map_web_threshold(raw, threshold);
                if response <= 0.0 {
                    return Ok(0.0);
                }
                Ok(minimum_factor + (maximum_factor - minimum_factor) * response)
            })
            .collect::<Result<Vec<_>, NativeRecipeOperationError>>()?;
        (values, Some(placements.clone()))
    } else {
        let values = samples
            .field
            .values()
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                if index % 1024 == 0 {
                    context.cancellation.checkpoint().map_err(native_error)?;
                }
                let response = map_web_threshold(*raw, threshold);
                if response <= 0.0 {
                    return Ok(0.0);
                }
                Ok(minimum_factor + (maximum_factor - minimum_factor) * response)
            })
            .collect::<Result<Vec<_>, NativeRecipeOperationError>>()?;
        (values, None)
    };
    let maximum_extent_factor = (maximum / 100.0).max(minimum / 100.0) * (max_size / 100.0);
    Ok(RecipeRuntimeValue::ShapesMappedValues(ShapesMappedValues {
        lattice: samples.lattice.clone(),
        values,
        maximum_extent_factor,
        uniform_extent_factor,
        placements,
    }))
}

fn shapes_primitive_selection(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesMappedValues(mapped_values) =
        required_input(inputs, "mapped-values")?
    else {
        return Err(NativeRecipeOperationError::new(
            "input `mapped-values` must be mapped Shapes values",
        ));
    };
    let shared = required_boolean(parameters, "use-shared-mark")?;
    let (shape, polygon_sides, custom_asset) = if shared {
        (
            required_choice(parameters, "shared-shape")?,
            required_integer(parameters, "polygon-sides")?,
            required_svg_asset(parameters, "global-custom-motif")?,
        )
    } else {
        (
            required_choice(parameters, "shape")?,
            required_integer(parameters, "channel-polygon-sides")?,
            required_svg_asset(parameters, "channel-custom-motif")?,
        )
    };
    let shape = resolve_shape(
        shape,
        polygon_sides,
        custom_asset,
        context.definition_assets,
    )?;
    Ok(RecipeRuntimeValue::ShapesPrimitive(
        ShapesSelectedPrimitive {
            mapped_values: mapped_values.clone(),
            shape,
        },
    ))
}

fn shapes_transforms(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesLattice(lattice) = required_input(inputs, "lattice")? else {
        return Err(NativeRecipeOperationError::new(
            "input `lattice` must be a Shapes lattice",
        ));
    };
    let RecipeRuntimeValue::ShapesPrimitive(primitive) = required_input(inputs, "primitive")?
    else {
        return Err(NativeRecipeOperationError::new(
            "input `primitive` must be a selected Shapes primitive",
        ));
    };
    if primitive.mapped_values.lattice != *lattice {
        return Err(NativeRecipeOperationError::new(
            "Shapes primitive mapped values do not belong to the input lattice",
        ));
    }
    let rotation = required_number(parameters, "rotation")?;
    let scale = required_number(parameters, "scale")?;
    let width_scale = required_number(parameters, "width-scale")?;
    let height_scale = required_number(parameters, "height-scale")?;
    let grid_scale = required_number(parameters, "grid-scale")?;
    if !(0.0..=100.0).contains(&scale)
        || !(0.01..=100.0).contains(&width_scale)
        || !(0.01..=100.0).contains(&height_scale)
        || !(0.001..=1000.0).contains(&grid_scale)
    {
        return Err(NativeRecipeOperationError::new(
            "Shapes transform parameters are outside the supported range",
        ));
    }
    let max_factor = primitive.mapped_values.maximum_extent_factor;
    let margin = lattice.max_extent(
        &primitive.shape,
        max_factor,
        scale,
        width_scale,
        height_scale,
        grid_scale,
    );
    let (min_col, max_col, min_row, max_row) = lattice.candidate_range(margin);
    let candidate_columns = i64::from(max_col)
        .checked_sub(i64::from(min_col))
        .and_then(|width| width.checked_add(1));
    let candidate_rows = i64::from(max_row)
        .checked_sub(i64::from(min_row))
        .and_then(|height| height.checked_add(1));
    let Some(lattice_candidate_count) = candidate_columns
        .zip(candidate_rows)
        .and_then(|(columns, rows)| columns.checked_mul(rows))
    else {
        return Err(NativeRecipeOperationError::new(
            "Shapes transformed lattice exceeds the bounded candidate count",
        ));
    };
    // Explicit paths/distributions already carry their bounded site list. Do
    // not reapply an inferred lattice candidate count based on mark extent;
    // that could reject an authored path solely because it intentionally
    // overflows the artboard for clipping.
    let candidate_count = primitive
        .mapped_values
        .placements
        .as_ref()
        .map(|placements| {
            i64::try_from(placements.len()).map_err(|_| {
                NativeRecipeOperationError::new(
                    "Shapes transformed explicit placements exceed the bounded candidate count",
                )
            })
        })
        .unwrap_or(Ok(lattice_candidate_count))?;
    if candidate_count <= 0 || candidate_count as u64 > SHAPES_MAX_LATTICE_CANDIDATES as u64 {
        return Err(NativeRecipeOperationError::new(
            "Shapes transformed lattice exceeds the bounded candidate count",
        ));
    }
    let channel = required_channel(context)?.to_legacy_ink();
    let mut marks = Vec::with_capacity(candidate_count as usize);
    let mut continuation = Vec::with_capacity(candidate_count as usize);
    let mut path_break = true;
    // Explicit placements are authored paths or bounded point distributions.
    // Keep their out-of-artboard samples so the canonical raster/SVG clip can
    // trim them at export; ordinary lattice candidates still use the
    // calculated extent margin to preserve the compatibility output oracle.
    let retain_explicit_placements = primitive.mapped_values.placements.is_some();
    let mut push_mark = |placement: ShapesPlacement, mapped: f64| {
        if mapped <= 0.0 {
            path_break = true;
            return;
        }
        if !retain_explicit_placements && !placement.visible {
            path_break = true;
            return;
        }
        let size = lattice.grid.cell_width.min(lattice.grid.cell_height) * mapped * scale;
        marks.push(Mark {
            channel: channel.into(),
            x: placement.x as f32,
            y: placement.y as f32,
            extent: (size * grid_scale / 100.0 * width_scale) as f32,
            thickness: (size * grid_scale / 100.0 * height_scale) as f32,
            angle: rotation as f32,
            treatment: Treatment::Dots,
            geometry: MarkGeometry::WebShape(primitive.shape.clone()),
        });
        continuation.push(!path_break);
        path_break = false;
    };
    if let Some(placements) = &primitive.mapped_values.placements {
        for (index, placement) in placements.iter().copied().enumerate() {
            if index % 1024 == 0 {
                context.cancellation.checkpoint().map_err(native_error)?;
            }
            let mapped = primitive.mapped_values.values[index];
            let mapped = if mapped <= 0.0 {
                0.0
            } else if lattice.placement_strategy == ShapesPlacementStrategy::Random {
                primitive.mapped_values.uniform_extent_factor
                    + (mapped - primitive.mapped_values.uniform_extent_factor)
                        * lattice.random_size_response
            } else {
                mapped
            };
            push_mark(placement, mapped);
        }
    } else {
        for row in min_row..=max_row {
            context.cancellation.checkpoint().map_err(native_error)?;
            for col in min_col..=max_col {
                let placement = lattice.placement(col, row, margin);
                push_mark(
                    placement,
                    primitive.mapped_values.values[placement.sample_index],
                );
            }
        }
    }
    Ok(RecipeRuntimeValue::ShapesTransformedMarks(
        ShapesTransformedMarks {
            artboard: lattice.artboard,
            marks,
            continuation,
        },
    ))
}

fn shapes_emit_marks(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesTransformedMarks(transformed) = required_input(inputs, "marks")?
    else {
        return Err(NativeRecipeOperationError::new(
            "input `marks` must be transformed Shapes marks",
        ));
    };
    if transformed.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "transformed Shapes marks artboard does not match the execution context",
        ));
    }
    let enabled = required_boolean(parameters, "enabled")?;
    let color = match parameters.get("color") {
        Some(LiteralValue::Text(color)) => parse_hex_color(color).ok_or_else(|| {
            NativeRecipeOperationError::new("Shapes output color must be a six-digit hex color")
        })?,
        Some(_) => {
            return Err(NativeRecipeOperationError::new(
                "parameter `color` must be text",
            ));
        }
        None => {
            return Err(NativeRecipeOperationError::new(
                "missing required parameter `color`",
            ));
        }
    };
    let opacity = required_number(parameters, "opacity")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(NativeRecipeOperationError::new(
            "Shapes output opacity is outside the supported range",
        ));
    }
    let channel = required_channel(context)?.to_legacy_ink();
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Marks(MarkPatternOutput {
            geometry: MarkSet {
                width: context.artboard.width,
                height: context.artboard.height,
                marks: if enabled {
                    transformed.marks.clone()
                } else {
                    Vec::new()
                },
                layers: vec![InkLayer {
                    channel: channel.into(),
                    enabled,
                    color,
                    opacity: opacity as f32,
                }],
            },
        }),
    ))
}

fn shapes_emit_network(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_native_node_invocation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::ShapesTransformedMarks(transformed) = required_input(inputs, "marks")?
    else {
        return Err(NativeRecipeOperationError::new(
            "input `marks` must be transformed Shapes marks",
        ));
    };
    if transformed.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "transformed Shapes marks artboard does not match the execution context",
        ));
    }
    let enabled = required_boolean(parameters, "enabled")?;
    let color = match parameters.get("color") {
        Some(LiteralValue::Text(color)) => parse_hex_color(color).ok_or_else(|| {
            NativeRecipeOperationError::new("Shapes network color must be a six-digit hex color")
        })?,
        Some(_) => {
            return Err(NativeRecipeOperationError::new(
                "parameter `color` must be text",
            ));
        }
        None => {
            return Err(NativeRecipeOperationError::new(
                "missing required parameter `color`",
            ));
        }
    };
    let opacity = required_number(parameters, "opacity")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(NativeRecipeOperationError::new(
            "Shapes network opacity is outside the supported range",
        ));
    }
    let connection_mode = required_choice(parameters, "connection-mode")?;
    if !matches!(connection_mode, "linear" | "maze") {
        return Err(NativeRecipeOperationError::new(
            "Shapes network connection mode must be linear or maze",
        ));
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut mark_nodes = Vec::with_capacity(transformed.marks.len());
    for (index, mark) in transformed.marks.iter().enumerate() {
        if index % 1024 == 0 {
            context.cancellation.checkpoint().map_err(native_error)?;
        }
        let id = NetworkNodeId(index as u32 + 1);
        nodes.push(NetworkNode {
            id,
            point: crate::pattern::CanonicalPoint {
                x: mark.x,
                y: mark.y,
            },
        });
        mark_nodes.push((id, mark));
    }
    let mut edge_id = 1u32;
    for (index, window) in mark_nodes.windows(2).enumerate() {
        let [(start, start_mark), (end, end_mark)] = window else {
            continue;
        };
        if !transformed
            .continuation
            .get(index + 1)
            .copied()
            // A missing continuation marker is treated as a hard boundary;
            // it must never silently recreate the old gap-bridging behavior.
            .unwrap_or(false)
        {
            continue;
        }
        let width = ((start_mark.thickness.abs() + end_mark.thickness.abs()) * 0.5).max(1.0);
        if connection_mode == "maze" {
            let mid_id = NetworkNodeId(nodes.len() as u32 + 1);
            nodes.push(NetworkNode {
                id: mid_id,
                point: crate::pattern::CanonicalPoint {
                    x: end_mark.x,
                    y: start_mark.y,
                },
            });
            edges.push(SharedBoundaryEdge {
                id: NetworkEdgeId(edge_id),
                layer_id: CanonicalLayerId(context.enabled_layer_index),
                order: edge_id,
                start: *start,
                end: mid_id,
                width,
                polarity: GeometryPolarity::Positive,
            });
            edge_id = edge_id.saturating_add(1);
            edges.push(SharedBoundaryEdge {
                id: NetworkEdgeId(edge_id),
                layer_id: CanonicalLayerId(context.enabled_layer_index),
                order: edge_id,
                start: mid_id,
                end: *end,
                width,
                polarity: GeometryPolarity::Positive,
            });
        } else {
            edges.push(SharedBoundaryEdge {
                id: NetworkEdgeId(edge_id),
                layer_id: CanonicalLayerId(context.enabled_layer_index),
                order: edge_id,
                start: *start,
                end: *end,
                width,
                polarity: GeometryPolarity::Positive,
            });
        }
        edge_id = edge_id.saturating_add(1);
    }
    let strokes = if enabled {
        build_network_strokes(
            &mark_nodes,
            &transformed.continuation,
            connection_mode,
            CanonicalLayerId(context.enabled_layer_index),
            context.cancellation,
        )
    } else {
        Vec::new()
    };
    let (red, green, blue) = color;
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Network(NetworkPatternOutput {
            artboard: context.artboard,
            layers: vec![CanonicalLayer {
                id: CanonicalLayerId(context.enabled_layer_index),
                channel: context.output_channel,
                label: context
                    .output_channel
                    .map(|channel| format!("{} connected", channel.stable_id()))
                    .unwrap_or_else(|| "Connected pattern".into()),
                order: context.enabled_layer_index,
                color: CanonicalColor { red, green, blue },
                opacity: opacity as f32,
                blend_mode: if context.output_channel.is_some_and(|channel| {
                    channel.belongs_to(crate::artwork_pipeline::OutputModel::RgbScreen)
                }) {
                    CanonicalBlendMode::Screen
                } else {
                    CanonicalBlendMode::Multiply
                },
            }],
            nodes: if enabled { nodes } else { Vec::new() },
            edges: if enabled { edges } else { Vec::new() },
            strokes,
            transform: AffineTransform::IDENTITY,
        }),
    ))
}

/// Converts each authored continuation run into one smooth, variable-width
/// filled contour. The topology edges above remain available for inspection,
/// but raster and SVG consumers use this outline so adjacent samples share a
/// continuous centerline and never show tiny independently capped segments.
fn build_network_strokes(
    mark_nodes: &[(NetworkNodeId, &Mark)],
    continuation: &[bool],
    connection_mode: &str,
    layer_id: CanonicalLayerId,
    token: &crate::CancellationToken,
) -> Vec<NetworkStroke> {
    let mut strokes = Vec::new();
    let mut run: Vec<&Mark> = Vec::new();
    let flush = |run: &mut Vec<&Mark>, strokes: &mut Vec<NetworkStroke>| {
        if run.len() < 2 {
            run.clear();
            return;
        }
        let mut points = Vec::with_capacity(if connection_mode == "maze" {
            run.len().saturating_mul(2).saturating_sub(1)
        } else {
            run.len()
        });
        for (index, mark) in run.iter().enumerate() {
            let width = mark.thickness.abs().max(1.0) as f64;
            points.push(VariablePoint {
                x: mark.x as f64,
                y: mark.y as f64,
                width,
            });
            if connection_mode == "maze" && index + 1 < run.len() {
                let next = run[index + 1];
                points.push(VariablePoint {
                    x: next.x as f64,
                    y: mark.y as f64,
                    width: ((mark.thickness.abs() + next.thickness.abs()) * 0.5).max(1.0) as f64,
                });
            }
        }
        let Some(outline) = outline_from_variable_points(&points, false) else {
            run.clear();
            return;
        };
        let centerline = points
            .iter()
            .map(|point| crate::pattern::CanonicalPoint {
                x: point.x as f32,
                y: point.y as f32,
            })
            .collect();
        let widths = points.iter().map(|point| point.width as f32).collect();
        let id = NetworkStrokeId(strokes.len() as u32 + 1);
        strokes.push(NetworkStroke {
            id,
            layer_id,
            order: id.0,
            centerline,
            widths,
            outline,
            polarity: GeometryPolarity::Positive,
        });
        run.clear();
    };

    for (index, (_, mark)) in mark_nodes.iter().enumerate() {
        if index % 1024 == 0 {
            let _ = token.checkpoint();
        }
        if index > 0 && !continuation.get(index).copied().unwrap_or(false) {
            flush(&mut run, &mut strokes);
        }
        run.push(*mark);
    }
    flush(&mut run, &mut strokes);
    strokes
}

fn editor_grid(
    width: u32,
    height: u32,
    fallback: &WebGrid,
    x_spacing: f64,
    y_spacing: f64,
) -> Result<WebGrid, NativeRecipeOperationError> {
    let cell_width = if x_spacing > 0.0 {
        x_spacing
    } else {
        fallback.cell_width
    };
    let cell_height = if y_spacing > 0.0 {
        y_spacing
    } else {
        fallback.cell_height
    };
    let cols = (f64::from(width) / cell_width).ceil().max(1.0) as u32;
    let rows = (f64::from(height) / cell_height).ceil().max(1.0) as u32;
    let cells = u64::from(cols).saturating_mul(u64::from(rows));
    if cells == 0 || cells > SHAPES_MAX_LATTICE_CELLS as u64 {
        return Err(NativeRecipeOperationError::new(
            "editor grid spacing exceeds the bounded lattice limit",
        ));
    }
    Ok(WebGrid {
        cols,
        rows,
        cell_width: f64::from(width) / f64::from(cols),
        cell_height: f64::from(height) / f64::from(rows),
    })
}

/// Preserve the authored X/Y spacing ratio while making Site density a real
/// construction control.  The old editor path only used explicit spacing, so
/// changing density after the first apply could leave the rendered site count
/// unchanged.  Density now sets the total lattice population; X/Y spacing
/// still controls its anisotropy.
fn density_adjusted_editor_grid(
    width: u32,
    height: u32,
    density_grid: &WebGrid,
    x_spacing: f64,
    y_spacing: f64,
) -> Result<WebGrid, NativeRecipeOperationError> {
    let target_cells = u64::from(density_grid.cols) * u64::from(density_grid.rows);
    let ratio = (x_spacing.max(f64::EPSILON) / y_spacing.max(f64::EPSILON)).max(0.000_001);
    let cols = ((target_cells as f64 * ratio).sqrt().round() as u32).max(1);
    let rows = ((target_cells as f64 / ratio).sqrt().round() as u32).max(1);
    let cells = u64::from(cols).saturating_mul(u64::from(rows));
    if cells == 0 || cells > SHAPES_MAX_LATTICE_CELLS as u64 {
        return Err(NativeRecipeOperationError::new(
            "site density exceeds the bounded lattice limit",
        ));
    }
    Ok(WebGrid {
        cols,
        rows,
        cell_width: f64::from(width) / f64::from(cols),
        cell_height: f64::from(height) / f64::from(rows),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ShapesPlacement {
    x: f64,
    y: f64,
    visible: bool,
    sample_index: usize,
}

impl ShapesLattice {
    fn requires_explicit_placements(&self) -> bool {
        self.sampler != ShapesPointSampler::Grid
            || self.jitter_factor > 0.0
            || self.point_definition != ShapesPointDefinition::Intersections
            || self.x_grid_curved
            || self.y_grid_curved
            || self.placement_strategy == ShapesPlacementStrategy::TriangularGrid
    }

    fn warp_point(&self, x: f64, y: f64) -> (f64, f64) {
        let mut x = x;
        let mut y = y;
        if self.curve_function == ShapesCurveFunction::Spiral
            && (self.x_grid_curved || self.y_grid_curved)
        {
            let normalized_x = x / self.artboard.width.max(1) as f64 - 0.5;
            let normalized_y = y / self.artboard.height.max(1) as f64 - 0.5;
            let radius = normalized_x.hypot(normalized_y);
            let angle = normalized_y.atan2(normalized_x);
            let spiral = (angle * 2.0 + radius * std::f64::consts::TAU * 3.0).sin() * radius;
            x += self.x_grid_curve * spiral * angle.cos();
            y += self.y_grid_curve * spiral * angle.sin();
            return (x, y);
        }
        let curve = |phase: f64, function: ShapesCurveFunction| match function {
            ShapesCurveFunction::Sine => phase.sin(),
            ShapesCurveFunction::Square => {
                if phase.sin().is_sign_negative() {
                    -1.0
                } else {
                    1.0
                }
            }
            ShapesCurveFunction::Spiral => phase.sin() * (phase.abs() / std::f64::consts::TAU),
            ShapesCurveFunction::Sawtooth => {
                let normalized = (phase / std::f64::consts::TAU).rem_euclid(1.0);
                normalized * 2.0 - 1.0
            }
        };
        if self.x_grid_curved {
            let wave = curve(
                y / self.artboard.height.max(1) as f64 * std::f64::consts::TAU,
                self.curve_function,
            );
            if self.curve_function == ShapesCurveFunction::Square {
                // A square-wave grid expands and contracts about the
                // artboard centre.  Scaling the distance from centre keeps
                // the two sides symmetric instead of translating every
                // track in the same direction.
                let center = self.artboard.width.max(1) as f64 * 0.5;
                let half = center.max(1.0);
                let factor = (1.0 + wave * self.x_grid_curve / half).max(0.0);
                x = center + (x - center) * factor;
            } else {
                x += self.x_grid_curve * wave;
            }
        }
        if self.y_grid_curved {
            let wave = curve(
                x / self.artboard.width.max(1) as f64 * std::f64::consts::TAU,
                self.curve_function,
            );
            if self.curve_function == ShapesCurveFunction::Square {
                let center = self.artboard.height.max(1) as f64 * 0.5;
                let half = center.max(1.0);
                let factor = (1.0 + wave * self.y_grid_curve / half).max(0.0);
                y = center + (y - center) * factor;
            } else {
                y += self.y_grid_curve * wave;
            }
        }
        (x, y)
    }

    fn candidate_range(&self, margin: f64) -> (i32, i32, i32, i32) {
        if self.grid_rotation.abs() <= 0.0001 {
            return (0, self.grid.cols as i32 - 1, 0, self.grid.rows as i32 - 1);
        }
        let corners = [
            (-margin, -margin),
            (self.artboard.width as f64 + margin, -margin),
            (
                self.artboard.width as f64 + margin,
                self.artboard.height as f64 + margin,
            ),
            (-margin, self.artboard.height as f64 + margin),
        ];
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in corners {
            let (x, y) = self.rotate(x, y, -self.grid_rotation);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let phase_x = wrap_signed_grid_offset(self.offset_x, self.grid.cell_width);
        let phase_y = wrap_signed_grid_offset(self.offset_y, self.grid.cell_height);
        (
            ((min_x - phase_x) / self.grid.cell_width - 0.5).floor() as i32,
            ((max_x - phase_x) / self.grid.cell_width - 0.5).ceil() as i32,
            ((min_y - phase_y) / self.grid.cell_height - 0.5).floor() as i32,
            ((max_y - phase_y) / self.grid.cell_height - 0.5).ceil() as i32,
        )
    }

    fn placement(&self, col: i32, row: i32, margin: f64) -> ShapesPlacement {
        let phase_x = wrap_signed_grid_offset(self.offset_x, self.grid.cell_width);
        let phase_y = wrap_signed_grid_offset(self.offset_y, self.grid.cell_height);
        let logical_x = if self.grid_rotation.abs() <= 0.0001 {
            positive_modulo(
                (col as f64 + 0.5) * self.grid.cell_width + phase_x,
                self.artboard.width as f64,
            )
        } else {
            (col as f64 + 0.5) * self.grid.cell_width + phase_x
        };
        let logical_y = if self.grid_rotation.abs() <= 0.0001 {
            positive_modulo(
                (row as f64 + 0.5) * self.grid.cell_height + phase_y,
                self.artboard.height as f64,
            )
        } else {
            (row as f64 + 0.5) * self.grid.cell_height + phase_y
        };
        let (logical_x, logical_y) = self.warp_point(logical_x, logical_y);
        let (mut x, mut y) = self.rotate(logical_x, logical_y, self.grid_rotation);
        if self.wrap_sites && self.grid_rotation.abs() > 0.0001 {
            x = positive_modulo(x, self.artboard.width.max(1) as f64);
            y = positive_modulo(y, self.artboard.height.max(1) as f64);
        }
        let sample_col =
            ((x / self.grid.cell_width).floor() as i64).clamp(0, self.grid.cols as i64 - 1) as u32;
        let sample_row =
            ((y / self.grid.cell_height).floor() as i64).clamp(0, self.grid.rows as i64 - 1) as u32;
        ShapesPlacement {
            x,
            y,
            visible: x >= -margin
                && x <= self.artboard.width as f64 + margin
                && y >= -margin
                && y <= self.artboard.height as f64 + margin,
            sample_index: (sample_row * self.grid.cols + sample_col) as usize,
        }
    }

    fn rotate(&self, x: f64, y: f64, degrees: f64) -> (f64, f64) {
        if degrees.abs() <= 0.0001 {
            return (x, y);
        }
        let pivot_x = self.artboard.width as f64 / 2.0 + self.grid_pivot_x;
        let pivot_y = self.artboard.height as f64 / 2.0 + self.grid_pivot_y;
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = x - pivot_x;
        let dy = y - pivot_y;
        (pivot_x + dx * cos - dy * sin, pivot_y + dx * sin + dy * cos)
    }

    fn max_extent(
        &self,
        shape: &ResolvedWebShape,
        max_factor: f64,
        scale: f64,
        width_scale: f64,
        height_scale: f64,
        grid_scale: f64,
    ) -> f64 {
        let radius = match shape {
            ResolvedWebShape::Circle => 0.5 * width_scale.max(height_scale),
            ResolvedWebShape::Polygon(points) => points
                .iter()
                .map(|(x, y)| (*x as f64 * width_scale).hypot(*y as f64 * height_scale))
                .fold(0.0, f64::max),
            ResolvedWebShape::Cubic { start, segments } => std::iter::once(*start)
                .chain(segments.iter().flat_map(|segment| {
                    [
                        (segment.0, segment.1),
                        (segment.2, segment.3),
                        (segment.4, segment.5),
                    ]
                }))
                .map(|(x, y)| (x as f64 * width_scale).hypot(y as f64 * height_scale))
                .fold(0.0, f64::max),
        };
        self.grid.cell_width.min(self.grid.cell_height) * max_factor * scale * grid_scale / 100.0
            * radius
    }
}

fn build_explicit_placements(
    lattice: &ShapesLattice,
    field: &DistributionField,
    context: &RecipeExecutionContext<'_>,
) -> Result<Vec<ShapesPlacement>, NativeRecipeOperationError> {
    let count = u64::from(lattice.grid.cols) * u64::from(lattice.grid.rows);
    if count == 0 {
        return Err(NativeRecipeOperationError::new(
            "editor sampling requires at least one point",
        ));
    }
    let domain = crate::site_distribution::DomainBounds {
        width: lattice.artboard.width,
        height: lattice.artboard.height,
    };
    let spiral_path = lattice.placement_strategy == ShapesPlacementStrategy::MathFunction
        && lattice.curve_function == ShapesCurveFunction::Spiral;
    let points_are_placed = spiral_path
        || (lattice.sampler == ShapesPointSampler::Grid
            && lattice.point_definition == ShapesPointDefinition::Intersections);
    let mut points = if spiral_path {
        // Spiral is an authored path construction.  A channel's sampling
        // mode controls source response, not the path's site topology.
        spiral_sample_points(
            lattice,
            lattice.point_definition == ShapesPointDefinition::FullCurves,
        )
    } else if lattice.sampler == ShapesPointSampler::Grid {
        match lattice.point_definition {
            ShapesPointDefinition::Intersections => {
                let mut points = Vec::with_capacity(count as usize);
                for row in 0..lattice.grid.rows {
                    for col in 0..lattice.grid.cols {
                        if lattice.placement_strategy == ShapesPlacementStrategy::TriangularGrid {
                            let offset = if row % 2 == 1 {
                                lattice.grid.cell_width * 0.5
                            } else {
                                0.0
                            };
                            // A triangular lattice uses a 60-degree axis pair.  Derive
                            // the vertical step from the authored horizontal spacing so
                            // manually constructed runtime lattices cannot accidentally
                            // fall back to a rectangular (90-degree) row spacing.
                            let triangular_cell_height =
                                lattice.grid.cell_width * (3.0_f64.sqrt() * 0.5);
                            let point = (
                                (f64::from(col) + 0.5) * lattice.grid.cell_width + offset,
                                (f64::from(row) + 0.5) * triangular_cell_height,
                            );
                            let (x, y) = lattice.rotate(point.0, point.1, lattice.grid_rotation);
                            points.push(OrderedPoint { x, y });
                        } else {
                            let placement = lattice.placement(col as i32, row as i32, 0.0);
                            points.push(OrderedPoint {
                                x: placement.x,
                                y: placement.y,
                            });
                        }
                    }
                }
                points
            }
            ShapesPointDefinition::CurveSpacing | ShapesPointDefinition::FullCurves => {
                curve_sample_points(
                    lattice,
                    lattice.point_definition == ShapesPointDefinition::FullCurves,
                )
            }
        }
    } else {
        let mode = match lattice.sampler {
            ShapesPointSampler::Uniform => DistributionMode::Uniform,
            ShapesPointSampler::Weighted => DistributionMode::SourceWeighted,
            ShapesPointSampler::Grid => unreachable!(),
        };
        let distribution = generate_site_distribution_cancellable(
            DistributionRequest {
                domain,
                count: count as usize,
                metadata: DistributionRequestMetadata {
                    seed: lattice.seed,
                    identity: DistributionIdentity(u64::from(context.semantic_channel_index)),
                    arrangement: ArrangementPolicy::Shared,
                    mode,
                    polarity: DistributionPolarity::HigherValuesMoreDense,
                    strength_milli: (lattice.weight_influence * 1_000.0).round() as u32,
                },
                field: (mode == DistributionMode::SourceWeighted).then_some(field),
                limits: DistributionLimits {
                    max_sites: count as usize,
                    max_candidates: (count as usize).saturating_mul(8),
                },
            },
            context.cancellation,
        )
        .map_err(native_error)?;
        distribution.points
    };
    if lattice.wrap_sites && lattice.placement_strategy == ShapesPlacementStrategy::TriangularGrid {
        for point in &mut points {
            point.x = positive_modulo(point.x, f64::from(lattice.artboard.width));
            point.y = positive_modulo(point.y, f64::from(lattice.artboard.height));
        }
    }
    // A weighted distribution is already source-directed. Applying a second
    // independent cell-sized displacement here would wash out the clustering
    // the channel's weight influence requested, so dispersion offsets are
    // reserved for uniform random placement.
    if lattice.sampler == ShapesPointSampler::Uniform && !spiral_path {
        for (index, point) in points.iter_mut().enumerate() {
            if index % 1024 == 0 {
                context.cancellation.checkpoint().map_err(native_error)?;
            }
            let seed = mix_editor_seed(lattice.seed ^ 0x51ed_5eed, index as u64);
            let raw_x = unit_from_seed(seed) * 2.0 - 1.0;
            let raw_y = unit_from_seed(seed ^ 0x9e37_79b9_7f4a_7c15) * 2.0 - 1.0;
            let (dx, dy) = match lattice.random_dispersion {
                ShapesRandomDispersion::Uniform => (raw_x, raw_y),
                ShapesRandomDispersion::Gaussian => {
                    let a = unit_from_seed(seed ^ 0xa5a5_a5a5_a5a5_a5a5);
                    let b = unit_from_seed(seed ^ 0x5a5a_5a5a_5a5a_5a5a);
                    let c = unit_from_seed(seed ^ 0x3c6e_f372_fe94_f82b);
                    let d = unit_from_seed(seed ^ 0xdaa6_6d2b_3f9a_1a7d);
                    ((a + b + c + d - 2.0) * 0.8, (raw_y + raw_x) * 0.4)
                }
                ShapesRandomDispersion::BlueNoise => (raw_x * 0.35, raw_y * 0.35),
                ShapesRandomDispersion::PinkNoise => {
                    let phase = index as f64 * 0.17 + f64::from((seed & 0xffff) as u32);
                    (phase.sin() * 0.55, (phase * 0.73).sin() * 0.55)
                }
                ShapesRandomDispersion::Poisson => (raw_x * 0.2, raw_y * 0.2),
            };
            point.x = (point.x + dx * lattice.grid.cell_width * 0.5)
                .clamp(0.0, f64::from(lattice.artboard.width));
            point.y = (point.y + dy * lattice.grid.cell_height * 0.5)
                .clamp(0.0, f64::from(lattice.artboard.height));
        }
    }
    let jitter = lattice.jitter_factor;
    if jitter > 0.0 {
        for (index, point) in points.iter_mut().enumerate() {
            if index % 1024 == 0 {
                context.cancellation.checkpoint().map_err(native_error)?;
            }
            let seed = mix_editor_seed(lattice.seed, index as u64);
            let dx = (unit_from_seed(seed) * 2.0 - 1.0) * lattice.grid.cell_width * 0.5 * jitter;
            let dy = (unit_from_seed(seed ^ 0x9e37_79b9_7f4a_7c15) * 2.0 - 1.0)
                * lattice.grid.cell_height
                * 0.5
                * jitter;
            if spiral_path {
                point.x += dx;
                point.y += dy;
            } else {
                point.x = (point.x + dx).clamp(0.0, f64::from(lattice.artboard.width));
                point.y = (point.y + dy).clamp(0.0, f64::from(lattice.artboard.height));
            }
        }
    }
    if points.len() > SHAPES_MAX_LATTICE_CANDIDATES {
        return Err(NativeRecipeOperationError::new(
            "editor curve sampling exceeds the bounded candidate count",
        ));
    }
    let (field_width, field_height) = field.dimensions();
    Ok(points
        .into_iter()
        .map(|point| {
            let (x, y) = if points_are_placed {
                (point.x, point.y)
            } else {
                lattice.warp_point(point.x, point.y)
            };
            let sample_col = ((x / f64::from(lattice.artboard.width) * f64::from(field_width))
                .floor()
                .clamp(0.0, f64::from(field_width - 1))) as u32;
            let sample_row = ((y / f64::from(lattice.artboard.height) * f64::from(field_height))
                .floor()
                .clamp(0.0, f64::from(field_height - 1))) as u32;
            ShapesPlacement {
                x,
                y,
                visible: x >= 0.0
                    && x <= f64::from(lattice.artboard.width)
                    && y >= 0.0
                    && y <= f64::from(lattice.artboard.height),
                sample_index: (sample_row * field_width + sample_col) as usize,
            }
        })
        .collect())
}

fn curve_sample_points(lattice: &ShapesLattice, dense: bool) -> Vec<OrderedPoint> {
    let x_step = if dense {
        (lattice.grid.cell_width * 0.5).max(1.0)
    } else {
        lattice.grid.cell_width.max(1.0)
    };
    let y_step = if dense {
        (lattice.grid.cell_height * 0.5).max(1.0)
    } else {
        lattice.grid.cell_height.max(1.0)
    };
    let mut points = Vec::new();
    for row in 0..lattice.grid.rows {
        let y = (f64::from(row) + 0.5) * lattice.grid.cell_height;
        let mut x = 0.0;
        while x <= f64::from(lattice.artboard.width) {
            points.push(OrderedPoint { x, y });
            x += x_step;
        }
    }
    for col in 0..lattice.grid.cols {
        let x = (f64::from(col) + 0.5) * lattice.grid.cell_width;
        let mut y = 0.0;
        while y <= f64::from(lattice.artboard.height) {
            points.push(OrderedPoint { x, y });
            y += y_step;
        }
    }
    points
}

/// Generate one ordered Archimedean spiral, starting at the artboard origin
/// (the pattern's center) and winding outward.  Unlike the other math
/// functions, Spiral is a path construction rather than a deformation of an
/// X/Y lattice; preserving this order lets the network emitter produce one
/// connected line instead of a grid of unrelated tracks.
fn spiral_sample_points(lattice: &ShapesLattice, dense: bool) -> Vec<OrderedPoint> {
    let width = f64::from(lattice.artboard.width);
    let height = f64::from(lattice.artboard.height);
    let origin = (width * 0.5, height * 0.5);
    // Reach every corner and continue by one authored curve spacing.  The
    // canonical/export clip trims that overrun, while the authored path
    // remains complete across the rectangular canvas rather than ending at
    // an inscribed circle.  Keeping the turn count based on the corner
    // radius means the extra spacing does not change the phase at the edge.
    let pitch = lattice.curve_spacing.max(0.01);
    let corner_radius = (width * 0.5).hypot(height * 0.5).max(1.0);
    let max_radius = (corner_radius + pitch).max(1.0);
    let turns = (corner_radius / pitch).clamp(1.0, 64.0);
    let theta_max = std::f64::consts::TAU * turns;
    let arc_step = (lattice.grid.cell_width.min(lattice.grid.cell_height)
        * if dense { 0.5 } else { 0.85 })
    .max(0.5);
    // For an Archimedean spiral r = a*theta, the derivative has magnitude
    // a*sqrt(1 + theta^2).  Use its closed-form arc length and invert it for
    // every sample instead of stepping uniformly in radians. Uniform angle
    // steps collapse many nodes near the origin and leave the outer turns
    // under-resolved, which is exactly the dense-center/stepped-edge failure
    // visible when the exported path is inspected in Inkscape.
    let radial_scale = max_radius / theta_max.max(f64::EPSILON);
    let total_length = archimedean_spiral_arc_length(radial_scale, theta_max);
    let sample_count = (total_length / arc_step)
        .ceil()
        .clamp(16.0, SHAPES_MAX_LATTICE_CANDIDATES as f64) as usize;
    let mut points = Vec::with_capacity(sample_count + 1);
    for index in 0..=sample_count {
        let target_length = total_length * index as f64 / sample_count as f64;
        let angle = archimedean_spiral_theta_at_length(radial_scale, target_length, theta_max);
        let radius = radial_scale * angle;
        points.push(OrderedPoint {
            x: origin.0 + radius * angle.cos(),
            y: origin.1 + radius * angle.sin(),
        });
    }
    points
}

fn archimedean_spiral_arc_length(radial_scale: f64, theta: f64) -> f64 {
    let theta = theta.max(0.0);
    0.5 * radial_scale * (theta * (1.0 + theta * theta).sqrt() + theta.asinh())
}

fn archimedean_spiral_theta_at_length(
    radial_scale: f64,
    target_length: f64,
    theta_max: f64,
) -> f64 {
    if target_length <= 0.0 {
        return 0.0;
    }
    let total_length = archimedean_spiral_arc_length(radial_scale, theta_max);
    if target_length >= total_length {
        return theta_max;
    }
    // Monotonic inversion is bounded and deterministic. A fixed iteration
    // count avoids platform-dependent convergence differences in persisted
    // geometry fingerprints.
    let mut low = 0.0;
    let mut high = theta_max;
    for _ in 0..48 {
        let mid = (low + high) * 0.5;
        if archimedean_spiral_arc_length(radial_scale, mid) < target_length {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) * 0.5
}

fn mix_editor_seed(mut value: u64, salt: u64) -> u64 {
    value ^= salt.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit_from_seed(seed: u64) -> f64 {
    (seed >> 11) as f64 / (1_u64 << 53) as f64
}

fn resolve_shape(
    shape: &str,
    polygon_sides: u64,
    custom_digest: &str,
    assets: &[EmbeddedSvgAsset],
) -> Result<ResolvedWebShape, NativeRecipeOperationError> {
    match shape {
        "circle" => Ok(ResolvedWebShape::Circle),
        "regular-polygon" => Ok(ResolvedWebShape::Polygon(
            regular_polygon_points(u8::try_from(polygon_sides).unwrap_or(u8::MAX)).into(),
        )),
        "rectangle" => Ok(ResolvedWebShape::Polygon(
            vec![(-0.45, -0.45), (0.45, -0.45), (0.45, 0.45), (-0.45, 0.45)].into(),
        )),
        "triangle" => Ok(ResolvedWebShape::Polygon(
            vec![(0.0, -0.52), (0.5, 0.4), (-0.5, 0.4)].into(),
        )),
        "pentagon" => Ok(ResolvedWebShape::Polygon(regular_polygon_points(5).into())),
        "hexagon" => Ok(ResolvedWebShape::Polygon(regular_polygon_points(6).into())),
        "user-defined" => parse_supported_custom_motif(custom_digest, assets),
        value => Err(NativeRecipeOperationError::new(format!(
            "unsupported Shapes primitive `{value}`"
        ))),
    }
}

fn regular_polygon_points(sides: u8) -> Vec<(f32, f32)> {
    let sides = sides.clamp(3, 6);
    let start = -std::f32::consts::FRAC_PI_2
        + if sides.is_multiple_of(2) {
            std::f32::consts::PI / sides as f32
        } else {
            0.0
        };
    (0..sides)
        .map(|index| {
            let angle = start + std::f32::consts::TAU * index as f32 / sides as f32;
            (angle.cos() * 0.5, angle.sin() * 0.5)
        })
        .collect()
}

fn parse_supported_custom_motif(
    digest: &str,
    assets: &[EmbeddedSvgAsset],
) -> Result<ResolvedWebShape, NativeRecipeOperationError> {
    let asset = assets
        .iter()
        .find(|asset| asset.digest == digest)
        .ok_or_else(|| {
            NativeRecipeOperationError::new(format!(
                "Shapes custom motif asset `{digest}` is missing"
            ))
        })?;
    let actual_digest = format!("sha256:{:x}", Sha256::digest(asset.svg.as_bytes()));
    if actual_digest != asset.digest {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif asset digest does not match its SVG bytes",
        ));
    }
    let prefix = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"";
    let suffix = "\"/></svg>";
    let path = asset
        .svg
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .ok_or_else(|| {
            NativeRecipeOperationError::new(
                "Shapes custom motif is safe SVG but outside the supported single-path M/L/C/Z subset",
            )
        })?;
    parse_closed_path(path)
}

#[derive(Clone, Copy)]
struct CubicSegment {
    start: (f64, f64),
    control_one: (f64, f64),
    control_two: (f64, f64),
    end: (f64, f64),
}

fn parse_closed_path(path: &str) -> Result<ResolvedWebShape, NativeRecipeOperationError> {
    let tokens = path_tokens(path)?;
    let mut index = 0usize;
    expect_command(&tokens, &mut index, 'M')?;
    let start = (
        next_number(&tokens, &mut index)?,
        next_number(&tokens, &mut index)?,
    );
    let mut current = start;
    let mut segments = Vec::new();
    let mut closed = false;
    while index < tokens.len() {
        let command = next_command(&tokens, &mut index)?;
        match command {
            'L' => {
                let end = (
                    next_number(&tokens, &mut index)?,
                    next_number(&tokens, &mut index)?,
                );
                segments.push(line_segment(current, end));
                current = end;
            }
            'C' => {
                let segment = CubicSegment {
                    start: current,
                    control_one: (
                        next_number(&tokens, &mut index)?,
                        next_number(&tokens, &mut index)?,
                    ),
                    control_two: (
                        next_number(&tokens, &mut index)?,
                        next_number(&tokens, &mut index)?,
                    ),
                    end: (
                        next_number(&tokens, &mut index)?,
                        next_number(&tokens, &mut index)?,
                    ),
                };
                current = segment.end;
                segments.push(segment);
            }
            'Z' => {
                if current != start {
                    segments.push(line_segment(current, start));
                }
                closed = true;
                if index != tokens.len() {
                    return Err(NativeRecipeOperationError::new(
                        "Shapes custom motif commands after Z are unsupported",
                    ));
                }
            }
            command => {
                return Err(NativeRecipeOperationError::new(format!(
                    "Shapes custom motif command `{command}` is unsupported; only M/L/C/Z are supported"
                )));
            }
        }
    }
    if !closed || segments.len() < 3 || segments.len() > 64 {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif must be a closed path with 3 through 64 segments",
        ));
    }
    if !segments.iter().all(|segment| {
        [
            segment.start,
            segment.control_one,
            segment.control_two,
            segment.end,
        ]
        .into_iter()
        .all(|point| {
            point.0.is_finite()
                && point.1.is_finite()
                && point.0.abs() <= f64::from(f32::MAX)
                && point.1.abs() <= f64::from(f32::MAX)
        })
    }) {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif contains non-finite or unsupported coordinates",
        ));
    }
    let area = segments
        .iter()
        .map(|segment| segment.start.0 * segment.end.1 - segment.end.0 * segment.start.1)
        .sum::<f64>();
    if area.abs() <= 1e-9 {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif has no usable area",
        ));
    }
    Ok(ResolvedWebShape::Cubic {
        start: (start.0 as f32, start.1 as f32),
        segments: segments
            .into_iter()
            .map(|segment| {
                (
                    segment.control_one.0 as f32,
                    segment.control_one.1 as f32,
                    segment.control_two.0 as f32,
                    segment.control_two.1 as f32,
                    segment.end.0 as f32,
                    segment.end.1 as f32,
                )
            })
            .collect::<Vec<_>>()
            .into(),
    })
}

fn line_segment(start: (f64, f64), end: (f64, f64)) -> CubicSegment {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    CubicSegment {
        start,
        control_one: (start.0 + dx / 3.0, start.1 + dy / 3.0),
        control_two: (start.0 + dx * 2.0 / 3.0, start.1 + dy * 2.0 / 3.0),
        end,
    }
}

#[derive(Clone, Copy)]
enum PathToken {
    Command(char),
    Number(f64),
}

fn path_tokens(path: &str) -> Result<Vec<PathToken>, NativeRecipeOperationError> {
    let bytes = path.as_bytes();
    let mut index = 0usize;
    let mut tokens = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b',') {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        let byte = bytes[index];
        if byte.is_ascii_alphabetic() {
            tokens.push(PathToken::Command(byte as char));
            index += 1;
            continue;
        }
        let start = index;
        if matches!(bytes[index], b'+' | b'-') {
            index += 1;
        }
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            index += 1;
            if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                index += 1;
            }
            let exponent_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if exponent_start == index {
                return Err(NativeRecipeOperationError::new(
                    "Shapes custom motif contains an invalid numeric exponent",
                ));
            }
        }
        if start == index {
            return Err(NativeRecipeOperationError::new(
                "Shapes custom motif contains an invalid path token",
            ));
        }
        let value = path[start..index].parse::<f64>().map_err(native_error)?;
        tokens.push(PathToken::Number(value));
    }
    Ok(tokens)
}

fn expect_command(
    tokens: &[PathToken],
    index: &mut usize,
    expected: char,
) -> Result<(), NativeRecipeOperationError> {
    if next_command(tokens, index)? == expected {
        Ok(())
    } else {
        Err(NativeRecipeOperationError::new(format!(
            "Shapes custom motif must begin with `{expected}`"
        )))
    }
}

fn next_command(
    tokens: &[PathToken],
    index: &mut usize,
) -> Result<char, NativeRecipeOperationError> {
    let Some(token) = tokens.get(*index) else {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif path ended unexpectedly",
        ));
    };
    *index += 1;
    match token {
        PathToken::Command(command) => Ok(*command),
        PathToken::Number(_) => Err(NativeRecipeOperationError::new(
            "Shapes custom motif requires explicit path commands",
        )),
    }
}

fn next_number(tokens: &[PathToken], index: &mut usize) -> Result<f64, NativeRecipeOperationError> {
    let Some(token) = tokens.get(*index) else {
        return Err(NativeRecipeOperationError::new(
            "Shapes custom motif path ended unexpectedly",
        ));
    };
    *index += 1;
    match token {
        PathToken::Number(value) if value.is_finite() => Ok(*value),
        PathToken::Number(_) => Err(NativeRecipeOperationError::new(
            "Shapes custom motif contains a non-finite coordinate",
        )),
        PathToken::Command(_) => Err(NativeRecipeOperationError::new(
            "Shapes custom motif command is missing a coordinate",
        )),
    }
}

fn wrap_signed_grid_offset(offset: f64, spacing: f64) -> f64 {
    if !offset.is_finite() || !spacing.is_finite() || spacing <= 0.0 {
        0.0
    } else {
        positive_modulo(offset + spacing / 2.0, spacing) - spacing / 2.0
    }
}

fn positive_modulo(value: f64, modulus: f64) -> f64 {
    ((value % modulus) + modulus) % modulus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancellationToken;
    use crate::artwork_pipeline::{
        ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
        OutputModel, SourceAlphaPolicy,
    };
    use crate::model::{ClosedShapePath, ShapeAnchor, ShapePoint, WebShape, WebShapeSettings};
    use crate::render::{
        Channel, generate_web_shape_marks_for_pipeline, legacy_pipeline_from_facade,
    };
    use crate::shapes_recipe::adapt_shapes_settings_to_recipe;
    use crate::{OutputMode, ValueMode};
    use image::{Rgba, RgbaImage};
    use std::cell::Cell;
    use std::collections::BTreeMap;

    fn settings() -> WebShapeSettings {
        let mut settings = WebShapeSettings {
            output_width: 32,
            output_height: 16,
            long_edge_cells: 4.0,
            grid_scale: 100.0,
            min_mark: 0.0,
            max_mark: 100.0,
            ..WebShapeSettings::default()
        };
        settings.channels.r.color = "#123456".into();
        settings.channels.r.opacity = 0.5;
        settings
    }

    fn context<'a>(
        field: Option<&'a DistributionField>,
        provider: Option<&'a dyn crate::RecipeSourceFieldProvider>,
        token: &'a CancellationToken,
    ) -> RecipeExecutionContext<'a> {
        RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 32,
                height: 16,
            },
            output_channel: Some(OutputChannelId::RgbRed),
            source_field_provider: provider,
            source_field: field,
            source_generation: 1,
            resolved_field_generation: 1,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: token,
        }
    }

    fn recipe_output(
        configured: &WebShapeSettings,
        field: &DistributionField,
    ) -> CanonicalPatternOutput {
        let adaptation = adapt_shapes_settings_to_recipe(configured).unwrap();
        adaptation
            .definition
            .execute_recipe(
                &adaptation.instance,
                &context(Some(field), None, &CancellationToken::new()),
                &SHAPES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap()
    }

    fn assert_oracle_case(
        label: &str,
        source: &RgbaImage,
        configured: &WebShapeSettings,
        pipeline: &ArtworkPipelineSettings,
    ) {
        let prepared = PreparedSource::from_rgba_image(source, 100);
        let token = CancellationToken::new();
        let live =
            generate_web_shape_marks_for_pipeline(&prepared, configured, pipeline, &token).unwrap();
        let expected = adapt_legacy_shapes(PatternId::COMPATIBILITY_SHAPES_V1, live).unwrap();
        let first =
            execute_bundled_shapes_recipe_cancellable(&prepared, configured, pipeline, &token)
                .unwrap();
        let second =
            execute_bundled_shapes_recipe_cancellable(&prepared, configured, pipeline, &token)
                .unwrap();
        assert_eq!(first, expected, "oracle case: {label}");
        assert_eq!(second, first, "deterministic repeat: {label}");
    }

    fn cubic_path(points: [(f64, f64); 3], handle_offset: f64) -> ClosedShapePath {
        let anchors = points.map(|(x, y)| ShapeAnchor {
            point: ShapePoint { x, y },
            incoming: ShapePoint {
                x: x - handle_offset,
                y: y + handle_offset,
            },
            outgoing: ShapePoint {
                x: x + handle_offset,
                y: y - handle_offset,
            },
        });
        ClosedShapePath {
            anchors: anchors.into(),
        }
    }

    #[test]
    fn orchestrator_matches_live_compatibility_marks_without_switching_dispatch() {
        let mut configured = settings();
        configured.value_mode = ValueMode::Cmyk;
        configured.channels.c.enabled = true;
        configured.channels.m.enabled = true;
        configured.channels.y.enabled = true;
        configured.channels.k.enabled = true;
        configured.channels.r.enabled = false;
        let source = RgbaImage::from_fn(32, 16, |x, y| {
            Rgba([(x * 7) as u8, (y * 13) as u8, 255 - (x * 3) as u8, 255])
        });
        let prepared = PreparedSource::from_rgba_image(&source, 7);
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            configured.single_channel,
        );
        let token = CancellationToken::new();
        let live = generate_web_shape_marks_for_pipeline(&prepared, &configured, &pipeline, &token)
            .unwrap();
        let expected = adapt_legacy_shapes(PatternId::COMPATIBILITY_SHAPES_V1, live).unwrap();
        reset_shapes_recipe_orchestration_instrumentation();
        let actual =
            execute_bundled_shapes_recipe_cancellable(&prepared, &configured, &pipeline, &token)
                .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(provider_cache_misses(), 1);
    }

    #[test]
    fn provider_cache_resolves_distinct_lattice_dimensions_separately() {
        let mut configured = settings();
        configured.channels.c.enabled = true;
        configured.channels.m.enabled = true;
        configured.channels.y.enabled = true;
        configured.channels.k.enabled = true;
        configured.channels.c.resolution_scale = 1.0;
        configured.channels.m.resolution_scale = 2.0;
        configured.channels.y.resolution_scale = 1.0;
        configured.channels.k.resolution_scale = 2.0;
        let source = RgbaImage::from_pixel(32, 16, Rgba([10, 20, 30, 255]));
        let prepared = PreparedSource::from_rgba_image(&source, 9);
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            configured.single_channel,
        );
        reset_shapes_recipe_orchestration_instrumentation();
        execute_bundled_shapes_recipe_cancellable(
            &prepared,
            &configured,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(provider_cache_misses(), 2);
    }

    #[test]
    fn canonical_oracle_matrix_exercises_pipeline_shapes_paths_and_channel_settings() {
        let opaque = RgbaImage::from_fn(32, 16, |x, y| {
            Rgba([(x * 7) as u8, (y * 13) as u8, 255 - (x * 3) as u8, 255])
        });
        let translucent = RgbaImage::from_fn(32, 16, |x, y| {
            Rgba([
                x as u8 * 4,
                y as u8 * 8,
                200,
                if (x + y) % 3 == 0 { 0 } else { 128 },
            ])
        });
        let automatic_rgb = ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            ),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            active_channel: None,
            ..ArtworkPipelineSettings::default()
        };
        let automatic_cmyk = ArtworkPipelineSettings {
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
            ),
            alpha_policy: SourceAlphaPolicy::LegacyCurrentV1,
            active_channel: None,
            ..ArtworkPipelineSettings::default()
        };
        let luminance_all = ArtworkPipelineSettings {
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::AllChannels,
            source: ArtworkSource::Value,
            active_channel: None,
            ..ArtworkPipelineSettings::default()
        };
        let active_red = ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::ActiveChannel,
            source: ArtworkSource::Value,
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        };

        let mut rgb = settings();
        rgb.channels.c.enabled = false;
        rgb.channels.m.enabled = false;
        rgb.channels.y.enabled = false;
        rgb.channels.k.enabled = false;
        rgb.channels.r.enabled = true;
        rgb.channels.g.enabled = true;
        rgb.channels.b.enabled = false;
        rgb.use_shared_mark = false;
        rgb.channels.r.shape = WebShape::Rectangle;
        rgb.channels.g.shape = WebShape::Hexagon;
        rgb.channels.r.rotation = 17.0;
        rgb.channels.g.grid_rotation = -31.0;
        rgb.channels.g.grid_pivot_x = 3.5;
        rgb.channels.g.grid_pivot_y = -1.5;
        rgb.channels.g.offset_x = 2.0;
        rgb.channels.g.offset_y = -3.0;
        rgb.channels.g.resolution_scale = 2.0;
        rgb.channels.r.threshold = 0.2;
        rgb.min_mark = 10.0;
        rgb.max_mark = 90.0;
        rgb.channels.r.max_size = 55.0;
        rgb.channels.r.scale = 1.2;
        rgb.channels.r.width_scale = 1.4;
        rgb.channels.r.height_scale = 0.7;
        rgb.channels.r.color = "#abcdef".into();
        rgb.channels.r.opacity = 0.4;
        assert_oracle_case(
            "automatic RGB, mixed disabled, transforms, threshold/min/max-size/color/opacity",
            &opaque,
            &rgb,
            &automatic_rgb,
        );

        let mut cmyk = settings();
        cmyk.channels.c.enabled = true;
        cmyk.channels.m.enabled = true;
        cmyk.channels.y.enabled = true;
        cmyk.channels.k.enabled = true;
        for (shape, channel) in [
            (WebShape::Circle, &mut cmyk.channels.c),
            (WebShape::RegularPolygon, &mut cmyk.channels.m),
            (WebShape::Triangle, &mut cmyk.channels.y),
            (WebShape::Pentagon, &mut cmyk.channels.k),
        ] {
            channel.shape = shape;
        }
        cmyk.use_shared_mark = false;
        cmyk.channels.m.polygon_sides = 5;
        assert_oracle_case(
            "automatic CMYK opaque, circle/regular-polygon/triangle/pentagon",
            &opaque,
            &cmyk,
            &automatic_cmyk,
        );

        let mut shared = settings();
        shared.channels.c.enabled = true;
        shared.channels.m.enabled = true;
        shared.channels.y.enabled = true;
        shared.channels.k.enabled = true;
        shared.shared_shape = WebShape::Hexagon;
        assert_oracle_case(
            "luminance AllChannels translucent shared hexagon",
            &translucent,
            &shared,
            &luminance_all,
        );

        let shared_cubic_path = cubic_path([(-0.42, -0.38), (0.47, -0.12), (-0.04, 0.48)], 0.13);
        let mut shared_cubic = shared.clone();
        shared_cubic.shared_shape = WebShape::UserDefined;
        shared_cubic.custom_shape_path = Some(shared_cubic_path);
        assert_oracle_case(
            "luminance AllChannels translucent shared user-defined cubic path",
            &translucent,
            &shared_cubic,
            &luminance_all,
        );
        let shared_cubic_output = execute_bundled_shapes_recipe_cancellable(
            &PreparedSource::from_rgba_image(&translucent, 101),
            &shared_cubic,
            &luminance_all,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Marks(shared_cubic_output) = shared_cubic_output else {
            unreachable!("Shapes recipe emits marks")
        };
        assert!(
            shared_cubic_output
                .geometry
                .marks
                .iter()
                .all(|mark| matches!(
                    mark.geometry,
                    MarkGeometry::WebShape(ResolvedWebShape::Cubic { .. })
                ))
        );

        let cyan_cubic_path = cubic_path([(-0.48, -0.36), (0.38, -0.28), (0.12, 0.44)], 0.09);
        let magenta_cubic_path = cubic_path([(-0.32, -0.46), (0.49, 0.04), (-0.28, 0.39)], 0.17);
        let mut independent_cubics = cmyk.clone();
        independent_cubics.channels.y.enabled = false;
        independent_cubics.channels.k.enabled = false;
        independent_cubics.channels.c.shape = WebShape::UserDefined;
        independent_cubics.channels.c.custom_shape_path = Some(cyan_cubic_path);
        independent_cubics.channels.m.shape = WebShape::UserDefined;
        independent_cubics.channels.m.custom_shape_path = Some(magenta_cubic_path);
        assert_oracle_case(
            "automatic CMYK independent user-defined cubic paths",
            &opaque,
            &independent_cubics,
            &automatic_cmyk,
        );
        let independent_cubics_output = execute_bundled_shapes_recipe_cancellable(
            &PreparedSource::from_rgba_image(&opaque, 102),
            &independent_cubics,
            &automatic_cmyk,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Marks(independent_cubics_output) = independent_cubics_output
        else {
            unreachable!("Shapes recipe emits marks")
        };
        let first_cyan_start = independent_cubics_output
            .geometry
            .marks
            .iter()
            .find_map(|mark| match (&mark.channel, &mark.geometry) {
                (Channel::Cyan, MarkGeometry::WebShape(ResolvedWebShape::Cubic { start, .. })) => {
                    Some(*start)
                }
                _ => None,
            })
            .expect("cyan cubic asset is projected into the recipe output");
        let first_magenta_start = independent_cubics_output
            .geometry
            .marks
            .iter()
            .find_map(|mark| match (&mark.channel, &mark.geometry) {
                (
                    Channel::Magenta,
                    MarkGeometry::WebShape(ResolvedWebShape::Cubic { start, .. }),
                ) => Some(*start),
                _ => None,
            })
            .expect("magenta cubic asset is projected into the recipe output");
        assert_ne!(first_cyan_start, first_magenta_start);

        let mut active = rgb.clone();
        active.channels.g.enabled = false;
        assert_oracle_case(
            "ActiveChannel single semantic red translucent",
            &translucent,
            &active,
            &active_red,
        );

        let mut crosshatch = shared.clone();
        crosshatch.value_mode = ValueMode::CrosshatchLuminance;
        crosshatch.crosshatch_color = "#345678".into();
        let crosshatch_pipeline = legacy_pipeline_from_facade(
            ValueMode::CrosshatchLuminance,
            OutputMode::CmykInks,
            crosshatch.single_channel,
        );
        assert_oracle_case(
            "Crosshatch legacy compatibility external single color",
            &opaque,
            &crosshatch,
            &crosshatch_pipeline,
        );
        let CanonicalPatternOutput::Marks(crosshatch_output) =
            execute_bundled_shapes_recipe_cancellable(
                &PreparedSource::from_rgba_image(&opaque, 103),
                &crosshatch,
                &crosshatch_pipeline,
                &CancellationToken::new(),
            )
            .unwrap()
        else {
            unreachable!("Shapes recipe emits marks")
        };
        assert!(
            crosshatch_output
                .geometry
                .layers
                .iter()
                .filter(|layer| layer.enabled)
                .all(|layer| layer.color == (0x34, 0x56, 0x78))
        );
        assert!(
            !std::str::from_utf8(crate::SHAPES_BUNDLED_BYTES)
                .unwrap()
                .contains("crosshatch-color")
        );
    }

    #[test]
    fn orchestrator_skips_disabled_channels_without_native_work_and_keeps_layers() {
        let mut configured = settings();
        for channel in [
            &mut configured.channels.c,
            &mut configured.channels.m,
            &mut configured.channels.y,
            &mut configured.channels.k,
        ] {
            channel.enabled = false;
        }
        let source = RgbaImage::from_pixel(32, 16, Rgba([20, 40, 60, 255]));
        let prepared = PreparedSource::from_rgba_image(&source, 8);
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            configured.single_channel,
        );
        reset_native_node_invocations();
        reset_shapes_recipe_orchestration_instrumentation();
        let output = execute_bundled_shapes_recipe_cancellable(
            &prepared,
            &configured,
            &pipeline,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Marks(output) = output else {
            unreachable!()
        };
        assert!(output.geometry.marks.is_empty());
        assert_eq!(output.geometry.layers.len(), 4);
        assert!(output.geometry.layers.iter().all(|layer| !layer.enabled));
        assert_eq!(native_node_invocations(), 0);
        assert_eq!(provider_cache_misses(), 0);
    }

    #[test]
    fn cancellation_stops_before_or_during_recipe_execution_without_output_installation() {
        let configured = settings();
        let source = RgbaImage::from_pixel(32, 16, Rgba([20, 40, 60, 255]));
        let prepared = PreparedSource::from_rgba_image(&source, 104);
        let pipeline = legacy_pipeline_from_facade(
            ValueMode::Cmyk,
            OutputMode::CmykInks,
            configured.single_channel,
        );
        let pre_cancelled = CancellationToken::new();
        pre_cancelled.cancel();
        let error = execute_bundled_shapes_recipe_cancellable(
            &prepared,
            &configured,
            &pipeline,
            &pre_cancelled,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));

        struct CancellingProvider<'a> {
            token: &'a CancellationToken,
            called: Cell<bool>,
        }
        impl crate::RecipeSourceFieldProvider for CancellingProvider<'_> {
            fn resolve_source_field(
                &self,
                _: OutputChannelId,
                columns: u32,
                rows: u32,
                _: &CancellationToken,
            ) -> Result<DistributionField, NativeRecipeOperationError> {
                self.called.set(true);
                self.token.cancel();
                DistributionField::new(columns, rows, vec![1.0; (columns * rows) as usize])
                    .map_err(native_error)
            }
        }
        let token = CancellationToken::new();
        let provider = CancellingProvider {
            token: &token,
            called: Cell::new(false),
        };
        let adaptation = adapt_shapes_settings_to_recipe(&configured).unwrap();
        let error = adaptation
            .definition
            .execute_recipe(
                &adaptation.instance,
                &context(None, Some(&provider), &token),
                &SHAPES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap_err();
        assert!(provider.called.get());
        assert!(error.to_string().contains("cancelled before node"));
    }

    #[test]
    fn native_recipe_executes_typed_shapes_flow_and_emits_semantic_marks() {
        let configured = settings();
        let field = DistributionField::new(4, 2, vec![1.0; 8]).unwrap();
        let output = recipe_output(&configured, &field);
        let CanonicalPatternOutput::Marks(output) = output else {
            panic!("Shapes recipe must emit marks");
        };
        assert_eq!(output.geometry.width, 32);
        assert_eq!(output.geometry.height, 16);
        assert_eq!(output.geometry.layers.len(), 1);
        assert_eq!(output.geometry.layers[0].channel, Channel::Red);
        assert_eq!(output.geometry.layers[0].color, (18, 52, 86));
        assert_eq!(output.geometry.layers[0].opacity, 0.5);
        assert!(output.geometry.layers[0].enabled);
        assert_eq!(output.geometry.marks.len(), 8);
        assert!(output.geometry.marks.iter().all(|mark| matches!(
            mark.geometry,
            MarkGeometry::WebShape(ResolvedWebShape::Circle)
        )));
    }

    #[test]
    fn source_sample_requests_declared_lattice_dimensions_from_context_provider() {
        struct Provider(Cell<Option<(OutputChannelId, u32, u32)>>);
        impl crate::RecipeSourceFieldProvider for Provider {
            fn resolve_source_field(
                &self,
                channel: OutputChannelId,
                columns: u32,
                rows: u32,
                _: &CancellationToken,
            ) -> Result<DistributionField, NativeRecipeOperationError> {
                self.0.set(Some((channel, columns, rows)));
                DistributionField::new(columns, rows, vec![1.0; (columns * rows) as usize])
                    .map_err(native_error)
            }
        }
        let configured = settings();
        let adaptation = adapt_shapes_settings_to_recipe(&configured).unwrap();
        let provider = Provider(Cell::new(None));
        let output = adaptation
            .definition
            .execute_recipe(
                &adaptation.instance,
                &context(None, Some(&provider), &CancellationToken::new()),
                &SHAPES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap();
        assert!(matches!(output, CanonicalPatternOutput::Marks(_)));
        assert_eq!(provider.0.get(), Some((OutputChannelId::RgbRed, 4, 2)));
    }

    #[test]
    fn operations_reject_missing_inputs_parameters_cancellation_and_resource_overflow() {
        let token = CancellationToken::new();
        let empty_inputs = BTreeMap::new();
        let empty_parameters = BTreeMap::new();
        let direct = context(None, None, &token);
        assert!(shapes_lattice_placement(&direct, &empty_inputs, &empty_parameters).is_err());
        assert!(shapes_source_sample(&direct, &empty_inputs, &empty_parameters).is_err());
        assert!(shapes_mark_map(&direct, &empty_inputs, &empty_parameters).is_err());
        assert!(shapes_primitive_selection(&direct, &empty_inputs, &empty_parameters).is_err());
        assert!(shapes_transforms(&direct, &empty_inputs, &empty_parameters).is_err());
        assert!(shapes_emit_marks(&direct, &empty_inputs, &empty_parameters).is_err());

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(
            shapes_lattice_placement(
                &context(None, None, &cancelled),
                &empty_inputs,
                &empty_parameters,
            )
            .is_err()
        );

        let large_context = RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 100_000,
                height: 100_000,
            },
            output_channel: Some(OutputChannelId::RgbRed),
            source_field_provider: None,
            source_field: None,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: &token,
        };
        let width = LiteralValue::Integer(100_000);
        let height = LiteralValue::Integer(100_000);
        let density = LiteralValue::Number(10_000.0);
        let resolution = LiteralValue::Number(100.0);
        let zero = LiteralValue::Number(0.0);
        let parameters = BTreeMap::from([
            ("output-width", &width),
            ("output-height", &height),
            ("long-edge-cells", &density),
            ("resolution-scale", &resolution),
            ("grid-rotation", &zero),
            ("grid-pivot-x", &zero),
            ("grid-pivot-y", &zero),
            ("offset-x", &zero),
            ("offset-y", &zero),
        ]);
        assert!(
            shapes_lattice_placement(&large_context, &empty_inputs, &parameters)
                .unwrap_err()
                .to_string()
                .contains("bounded source-sample grid")
        );

        let lattice = RecipeRuntimeValue::ShapesLattice(ShapesLattice {
            artboard: large_context.artboard,
            grid: calculate_web_grid(100_000, 100_000, 2.0),
            grid_rotation: 45.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::Grid,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        });
        let RecipeRuntimeValue::ShapesLattice(lattice_value) = &lattice else {
            unreachable!();
        };
        let primitive = RecipeRuntimeValue::ShapesPrimitive(ShapesSelectedPrimitive {
            mapped_values: ShapesMappedValues {
                lattice: lattice_value.clone(),
                values: vec![1_000.0; 4],
                maximum_extent_factor: 1_000.0,
                uniform_extent_factor: 1_000.0,
                placements: None,
            },
            shape: ResolvedWebShape::Circle,
        });
        let rotation = LiteralValue::Number(0.0);
        let scale = LiteralValue::Number(100.0);
        let width_scale = LiteralValue::Number(100.0);
        let height_scale = LiteralValue::Number(100.0);
        let grid_scale = LiteralValue::Number(1_000.0);
        let transform_parameters = BTreeMap::from([
            ("rotation", &rotation),
            ("scale", &scale),
            ("width-scale", &width_scale),
            ("height-scale", &height_scale),
            ("grid-scale", &grid_scale),
        ]);
        assert!(
            shapes_transforms(
                &large_context,
                &BTreeMap::from([("lattice", &lattice), ("primitive", &primitive)]),
                &transform_parameters,
            )
            .unwrap_err()
            .to_string()
            .contains("bounded candidate count")
        );
    }

    #[test]
    fn mapping_transforms_rotated_offsets_and_primitive_kinds_are_bounded_and_typed() {
        let mut configured = settings();
        configured.channels.r.grid_rotation = 45.0;
        configured.channels.r.offset_x = 3.5;
        configured.channels.r.offset_y = -2.0;
        configured.channels.r.max_size = 10_000.0;
        let field = DistributionField::new(4, 2, vec![0.5; 8]).unwrap();
        let output = recipe_output(&configured, &field);
        let CanonicalPatternOutput::Marks(output) = output else {
            panic!("Shapes recipe must emit marks");
        };
        assert!(
            output
                .geometry
                .marks
                .iter()
                .any(|mark| { mark.x < 0.0 || mark.x > 32.0 || mark.y < 0.0 || mark.y > 16.0 })
        );
        let min_x = output
            .geometry
            .marks
            .iter()
            .map(|mark| mark.x)
            .fold(f32::INFINITY, f32::min);
        let max_x = output
            .geometry
            .marks
            .iter()
            .map(|mark| mark.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = output
            .geometry
            .marks
            .iter()
            .map(|mark| mark.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = output
            .geometry
            .marks
            .iter()
            .map(|mark| mark.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(min_x < 0.0 && max_x > 32.0);
        assert!(min_y < 0.0 && max_y > 16.0);

        let adaptation = adapt_shapes_settings_to_recipe(&configured).unwrap();
        let assets = &adaptation.definition.assets;
        for (shape, sides) in [
            ("circle", 4),
            ("regular-polygon", 3),
            ("rectangle", 4),
            ("triangle", 3),
            ("pentagon", 5),
            ("hexagon", 6),
        ] {
            assert!(resolve_shape(shape, sides, "unused", assets).is_ok());
        }
        let mut custom = configured;
        custom.shared_shape = WebShape::UserDefined;
        let custom_assets = adapt_shapes_settings_to_recipe(&custom).unwrap();
        let output = custom_assets
            .definition
            .execute_recipe(
                &custom_assets.instance,
                &context(Some(&field), None, &CancellationToken::new()),
                &SHAPES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap();
        assert!(matches!(
            output,
            CanonicalPatternOutput::Marks(MarkPatternOutput { geometry: MarkSet { marks, .. } })
                if matches!(marks[0].geometry, MarkGeometry::WebShape(ResolvedWebShape::Cubic { .. }))
        ));
    }

    #[test]
    fn custom_motif_requires_matching_digest_and_supported_editable_subset() {
        let mut asset = EmbeddedSvgAsset {
            digest: "sha256:placeholder".into(),
            svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"M 0 0 Q 1 0 1 1 Z\"/></svg>".into(),
        };
        asset.digest = format!("sha256:{:x}", Sha256::digest(asset.svg.as_bytes()));
        assert!(
            parse_supported_custom_motif(&asset.digest, &[asset.clone()])
                .unwrap_err()
                .to_string()
                .contains("unsupported")
        );
        asset.digest =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".into();
        let digest = asset.digest.clone();
        assert!(
            parse_supported_custom_motif(&digest, &[asset])
                .unwrap_err()
                .to_string()
                .contains("digest")
        );

        let mut configured = settings();
        configured.shared_shape = WebShape::UserDefined;
        let mut adaptation = adapt_shapes_settings_to_recipe(&configured).unwrap();
        let global = adaptation
            .instance
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "global-custom-motif")
            .unwrap();
        let LiteralValue::SvgAsset(original_digest) = &global.value else {
            panic!("custom motif must be asset-backed");
        };
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-0.5 -0.5 1 1\"><path d=\"M 0 0 Q 1 0 1 1 Z\"/></svg>";
        let digest = format!("sha256:{:x}", Sha256::digest(svg.as_bytes()));
        assert_ne!(&digest, original_digest);
        adaptation.definition.assets.push(EmbeddedSvgAsset {
            digest: digest.clone(),
            svg: svg.into(),
        });
        global.value = LiteralValue::SvgAsset(digest);
        let field = DistributionField::new(4, 2, vec![1.0; 8]).unwrap();
        reset_native_node_invocations();
        let error = adaptation
            .definition
            .execute_recipe(
                &adaptation.instance,
                &context(Some(&field), None, &CancellationToken::new()),
                &SHAPES_NATIVE_OPERATION_REGISTRY,
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("native recipe preflight failed"),
            "unexpected preflight error: {error}"
        );
        assert_eq!(native_node_invocations(), 0);
    }

    #[test]
    fn editor_random_sampling_reuses_distribution_without_the_legacy_site_cap() {
        let token = CancellationToken::new();
        let field = DistributionField::new(32, 16, vec![0.5; 512]).unwrap();
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 1_000,
                height: 1_000,
            },
            grid: calculate_web_grid(1_000, 1_000, 100.0),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::Random,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Uniform,
            seed: 123,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        };
        let placements =
            build_explicit_placements(&lattice, &field, &context(Some(&field), None, &token))
                .unwrap();
        assert_eq!(placements.len(), 10_000);
    }

    #[test]
    fn weighted_random_sites_remain_clustered_after_pattern_dispersion() {
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 200,
                height: 100,
            },
            grid: calculate_web_grid(200, 100, 20.0),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::Random,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Weighted,
            seed: 73,
            weight_influence: 8.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        };
        let mut values = Vec::with_capacity(20 * 10);
        for _row in 0..10 {
            values.extend(std::iter::repeat_n(1.0, 10));
            values.extend(std::iter::repeat_n(0.01, 10));
        }
        let field = DistributionField::new(20, 10, values).unwrap();
        let placements = build_explicit_placements(
            &lattice,
            &field,
            &context(Some(&field), None, &CancellationToken::new()),
        )
        .unwrap();
        let left = placements.iter().filter(|point| point.x < 100.0).count();
        let right = placements.len() - left;
        assert!(
            left > right * 3,
            "weighted random sites should follow the high-value half: left={left} right={right}"
        );
    }

    #[test]
    fn editor_random_sampling_detail_changes_site_population() {
        let token = CancellationToken::new();
        let context = RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 320,
                height: 220,
            },
            output_channel: Some(OutputChannelId::RgbRed),
            source_field_provider: None,
            source_field: None,
            source_generation: 1,
            resolved_field_generation: 1,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: &token,
        };
        let site_count = |resolution: f64, placement_name: &str| {
            let output_width = LiteralValue::Integer(320);
            let output_height = LiteralValue::Integer(220);
            let density = LiteralValue::Number(24.0);
            let resolution = LiteralValue::Number(resolution);
            let zero = LiteralValue::Number(0.0);
            let spacing = LiteralValue::Number(0.0);
            let straight = LiteralValue::Choice("straight".into());
            let sine = LiteralValue::Choice("sine".into());
            let curve_spacing = LiteralValue::Number(20.0);
            let placement = LiteralValue::Choice(placement_name.into());
            let dispersion = LiteralValue::Choice("uniform".into());
            let point_definition = LiteralValue::Choice("intersections".into());
            let sampler = LiteralValue::Choice("grid".into());
            let seed = LiteralValue::Integer(19);
            let weight = LiteralValue::Number(1.0);
            let jitter = LiteralValue::Number(0.0);
            let parameters = BTreeMap::from([
                ("output-width", &output_width),
                ("output-height", &output_height),
                ("long-edge-cells", &density),
                ("resolution-scale", &resolution),
                ("grid-rotation", &zero),
                ("grid-pivot-x", &zero),
                ("grid-pivot-y", &zero),
                ("offset-x", &zero),
                ("offset-y", &zero),
                ("x-grid-spacing", &spacing),
                ("y-grid-spacing", &spacing),
                ("x-grid-mode", &straight),
                ("y-grid-mode", &straight),
                ("x-grid-curve", &zero),
                ("y-grid-curve", &zero),
                ("curve-function", &sine),
                ("placement-strategy", &placement),
                ("curve-spacing", &curve_spacing),
                ("random-dispersion", &dispersion),
                ("point-definition", &point_definition),
                ("point-sampler", &sampler),
                ("channel-seed", &seed),
                ("channel-weight-influence", &weight),
                ("random-size-response", &weight),
                ("jitter-factor", &jitter),
            ]);
            let RecipeRuntimeValue::ShapesLattice(lattice) =
                shapes_lattice_placement_editor(&context, &BTreeMap::new(), &parameters).unwrap()
            else {
                unreachable!();
            };
            (
                u64::from(lattice.grid.cols) * u64::from(lattice.grid.rows),
                lattice.sampler,
            )
        };
        let (sparse, sparse_sampler) = site_count(0.5, "random");
        let (dense, dense_sampler) = site_count(2.0, "random");
        assert!(dense > sparse, "sampling detail must increase random sites");
        assert_eq!(sparse_sampler, ShapesPointSampler::Weighted);
        assert_eq!(dense_sampler, ShapesPointSampler::Weighted);
        let (triangular_sparse, _) = site_count(0.5, "triangular-grid");
        let (triangular_dense, _) = site_count(2.0, "triangular-grid");
        assert!(
            triangular_dense > triangular_sparse,
            "sampling detail must increase triangular-grid sites"
        );
    }

    #[test]
    fn random_shape_size_response_blends_to_a_uniform_mark_extent() {
        let field =
            DistributionField::new(4, 2, vec![0.1, 0.8, 0.1, 0.8, 0.1, 0.8, 0.1, 0.8]).unwrap();
        let extents = |random_size_response: f64| {
            let lattice = ShapesLattice {
                artboard: ArtboardSpace {
                    width: 32,
                    height: 16,
                },
                grid: calculate_web_grid(32, 16, 4.0),
                grid_rotation: 0.0,
                grid_pivot_x: 0.0,
                grid_pivot_y: 0.0,
                offset_x: 0.0,
                offset_y: 0.0,
                x_grid_curved: false,
                y_grid_curved: false,
                x_grid_curve: 0.0,
                y_grid_curve: 0.0,
                curve_function: ShapesCurveFunction::Sine,
                placement_strategy: ShapesPlacementStrategy::Random,
                random_dispersion: ShapesRandomDispersion::Uniform,
                point_definition: ShapesPointDefinition::Intersections,
                sampler: ShapesPointSampler::Uniform,
                seed: 1,
                weight_influence: 1.0,
                random_size_response,
                jitter_factor: 0.0,
                curve_spacing: 4.0,
                wrap_sites: false,
            };
            let mapped_values = ShapesMappedValues {
                lattice: lattice.clone(),
                values: vec![0.1, 0.8],
                maximum_extent_factor: 0.8,
                uniform_extent_factor: 0.45,
                placements: Some(vec![
                    ShapesPlacement {
                        x: 8.0,
                        y: 8.0,
                        visible: true,
                        sample_index: 0,
                    },
                    ShapesPlacement {
                        x: 24.0,
                        y: 8.0,
                        visible: true,
                        sample_index: 1,
                    },
                ]),
            };
            let primitive = RecipeRuntimeValue::ShapesPrimitive(ShapesSelectedPrimitive {
                mapped_values,
                shape: ResolvedWebShape::Circle,
            });
            let rotation = LiteralValue::Number(0.0);
            let scale = LiteralValue::Number(100.0);
            let width_scale = LiteralValue::Number(100.0);
            let height_scale = LiteralValue::Number(100.0);
            let grid_scale = LiteralValue::Number(100.0);
            let parameters = BTreeMap::from([
                ("rotation", &rotation),
                ("scale", &scale),
                ("width-scale", &width_scale),
                ("height-scale", &height_scale),
                ("grid-scale", &grid_scale),
            ]);
            let lattice = match &primitive {
                RecipeRuntimeValue::ShapesPrimitive(primitive) => {
                    RecipeRuntimeValue::ShapesLattice(primitive.mapped_values.lattice.clone())
                }
                _ => unreachable!(),
            };
            let RecipeRuntimeValue::ShapesTransformedMarks(transformed) = shapes_transforms(
                &context(Some(&field), None, &CancellationToken::new()),
                &BTreeMap::from([("lattice", &lattice), ("primitive", &primitive)]),
                &parameters,
            )
            .unwrap() else {
                unreachable!();
            };
            transformed
                .marks
                .into_iter()
                .map(|mark| mark.extent)
                .collect::<Vec<_>>()
        };
        let uniform = extents(0.0);
        let source_responsive = extents(1.0);
        assert_eq!(uniform.len(), 2);
        assert!((uniform[0] - uniform[1]).abs() < f32::EPSILON);
        assert!((source_responsive[0] - source_responsive[1]).abs() > f32::EPSILON);
    }

    #[test]
    fn triangular_grid_uses_sixty_degree_staggered_axes() {
        let field = DistributionField::new(8, 8, vec![0.5; 64]).unwrap();
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 120,
                height: 120,
            },
            grid: editor_grid(120, 120, &calculate_web_grid(120, 120, 6.0), 30.0, 30.0).unwrap(),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::TriangularGrid,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        };
        let placements = build_explicit_placements(
            &lattice,
            &field,
            &context(Some(&field), None, &CancellationToken::new()),
        )
        .unwrap();
        assert_eq!(placements.len(), 16);
        let row_width = lattice.grid.cols as usize;
        let first = placements[0].x;
        let next_row = placements[row_width].x;
        assert!((next_row - first - lattice.grid.cell_width * 0.5).abs() < 1e-9);
        assert!(placements[row_width].y > placements[0].y);
        let angle = (placements[row_width].y - placements[0].y)
            .atan2(placements[row_width].x - placements[0].x)
            .to_degrees();
        assert!((angle - 60.0).abs() < 1e-9);
    }

    #[test]
    fn site_density_changes_editor_grid_population_even_with_explicit_spacing() {
        let sparse = calculate_web_grid(320, 220, 12.0);
        let dense = calculate_web_grid(320, 220, 48.0);
        let sparse_editor = density_adjusted_editor_grid(320, 220, &sparse, 24.0, 20.0).unwrap();
        let dense_editor = density_adjusted_editor_grid(320, 220, &dense, 24.0, 20.0).unwrap();
        assert!(
            u64::from(dense_editor.cols) * u64::from(dense_editor.rows)
                > u64::from(sparse_editor.cols) * u64::from(sparse_editor.rows)
        );
    }

    #[test]
    fn math_function_presets_produce_distinct_deterministic_warps() {
        let mut values = Vec::new();
        for function in [
            ShapesCurveFunction::Sine,
            ShapesCurveFunction::Square,
            ShapesCurveFunction::Spiral,
            ShapesCurveFunction::Sawtooth,
        ] {
            let lattice = ShapesLattice {
                artboard: ArtboardSpace {
                    width: 100,
                    height: 100,
                },
                grid: calculate_web_grid(100, 100, 10.0),
                grid_rotation: 0.0,
                grid_pivot_x: 0.0,
                grid_pivot_y: 0.0,
                offset_x: 0.0,
                offset_y: 0.0,
                x_grid_curved: true,
                y_grid_curved: true,
                x_grid_curve: 20.0,
                y_grid_curve: 20.0,
                curve_function: function,
                placement_strategy: ShapesPlacementStrategy::MathFunction,
                random_dispersion: ShapesRandomDispersion::Uniform,
                point_definition: ShapesPointDefinition::Intersections,
                sampler: ShapesPointSampler::Grid,
                seed: 0,
                weight_influence: 1.0,
                random_size_response: 1.0,
                jitter_factor: 0.0,
                curve_spacing: 20.0,
                wrap_sites: false,
            };
            values.push(lattice.warp_point(37.0, 61.0));
        }
        assert!(values.windows(2).any(|pair| pair[0] != pair[1]));
        assert!(values.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
    }

    #[test]
    fn square_wave_warp_expands_symmetrically_about_artboard_center() {
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 100,
                height: 100,
            },
            grid: calculate_web_grid(100, 100, 10.0),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: true,
            y_grid_curved: false,
            x_grid_curve: 20.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Square,
            placement_strategy: ShapesPlacementStrategy::MathFunction,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        };
        let left = lattice.warp_point(20.0, 25.0).0;
        let right = lattice.warp_point(80.0, 25.0).0;
        assert!((left - 8.0).abs() < 1e-9);
        assert!((right - 92.0).abs() < 1e-9);
        assert!((left + right - 100.0).abs() < 1e-9);
    }

    #[test]
    fn editor_rotated_grid_sites_wrap_into_the_artboard() {
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 100,
                height: 80,
            },
            grid: calculate_web_grid(100, 80, 10.0),
            grid_rotation: 37.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::Grid,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: true,
        };
        for col in -8..=8 {
            for row in -8..=8 {
                let placement = lattice.placement(col, row, 0.0);
                assert!((0.0..100.0).contains(&placement.x));
                assert!((0.0..80.0).contains(&placement.y));
            }
        }
    }

    #[test]
    fn spiral_math_function_emits_one_ordered_outward_path() {
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 200,
                height: 200,
            },
            grid: calculate_web_grid(200, 200, 24.0),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: true,
            y_grid_curved: true,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Spiral,
            placement_strategy: ShapesPlacementStrategy::MathFunction,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::FullCurves,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 24.0,
            wrap_sites: false,
        };
        let points = spiral_sample_points(&lattice, true);
        assert!(points.len() > 100);
        assert!((points[0].x - 100.0).abs() < 1e-9);
        assert!((points[0].y - 100.0).abs() < 1e-9);
        let mut previous_radius = 0.0;
        for point in &points {
            let radius = (point.x - 100.0).hypot(point.y - 100.0);
            assert!(radius + 1e-9 >= previous_radius);
            previous_radius = radius;
        }
        assert!(previous_radius > 95.0);
        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(min_x <= 0.0 && max_x >= 200.0);
        assert!(min_y <= 0.0 && max_y >= 200.0);

        let field = DistributionField::new(24, 24, vec![1.0; 576]).unwrap();
        let placements = build_explicit_placements(
            &lattice,
            &field,
            &context(Some(&field), None, &CancellationToken::new()),
        )
        .unwrap();
        assert_eq!(placements.len(), points.len());
        assert!((placements[0].x - 100.0).abs() < 1e-9);
        assert!((placements[0].y - 100.0).abs() < 1e-9);
        assert!(
            placements
                .windows(2)
                .all(|pair| { (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y) > 0.0 })
        );
        let distances: Vec<f64> = points
            .windows(2)
            .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
            .collect();
        let minimum = distances.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(minimum > 0.0);
        let pitch = lattice.curve_spacing.max(0.01);
        let corner_radius = 100.0_f64.hypot(100.0);
        let theta_max = std::f64::consts::TAU * (corner_radius / pitch).clamp(1.0, 64.0);
        let radial_scale = (corner_radius + pitch) / theta_max;
        let total_length = archimedean_spiral_arc_length(radial_scale, theta_max);
        let expected_arc_step = total_length / (points.len() - 1) as f64;
        let mut previous_length = 0.0;
        for index in 0..=points.len() - 1 {
            let target = total_length * index as f64 / (points.len() - 1) as f64;
            let angle = archimedean_spiral_theta_at_length(radial_scale, target, theta_max);
            let length = archimedean_spiral_arc_length(radial_scale, angle);
            if index > 0 {
                assert!((length - previous_length - expected_arc_step).abs() < 1e-8);
            }
            previous_length = length;
        }
    }

    #[test]
    fn network_emission_splits_at_thresholded_samples_instead_of_bridging_gaps() {
        let mark = |x: f32| Mark {
            channel: Channel::Red,
            x,
            y: 8.0,
            extent: 2.0,
            thickness: 2.0,
            angle: 0.0,
            treatment: Treatment::Dots,
            geometry: MarkGeometry::WebShape(ResolvedWebShape::Circle),
        };
        let mut middle = mark(12.0);
        middle.thickness = 6.0;
        let transformed = RecipeRuntimeValue::ShapesTransformedMarks(ShapesTransformedMarks {
            artboard: ArtboardSpace {
                width: 32,
                height: 16,
            },
            marks: vec![mark(4.0), middle, mark(24.0)],
            // The third point follows a zero/thresholded sample, so it starts
            // a new authored path rather than reconnecting to the second.
            continuation: vec![false, true, false],
        });
        let enabled = LiteralValue::Boolean(true);
        let color = LiteralValue::Text("#123456".into());
        let opacity = LiteralValue::Number(1.0);
        let connection_mode = LiteralValue::Choice("linear".into());
        let inputs = BTreeMap::from([("marks", &transformed)]);
        let parameters = BTreeMap::from([
            ("enabled", &enabled),
            ("color", &color),
            ("opacity", &opacity),
            ("connection-mode", &connection_mode),
        ]);
        let output = shapes_emit_network(
            &context(None, None, &CancellationToken::new()),
            &inputs,
            &parameters,
        )
        .unwrap();
        let RecipeRuntimeValue::CanonicalOutput(CanonicalPatternOutput::Network(output)) = output
        else {
            panic!("Shapes network operation must emit a network");
        };
        assert_eq!(output.nodes.len(), 3);
        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].start, NetworkNodeId(1));
        assert_eq!(output.edges[0].end, NetworkNodeId(2));
        assert_eq!(output.strokes.len(), 1);
        assert_eq!(output.strokes[0].centerline.len(), 2);
        assert!(output.strokes[0].widths[0] < output.strokes[0].widths[1]);
        assert!(output.strokes[0].outline.commands.iter().all(|command| {
            matches!(
                command,
                crate::curve_render::CurveCommand::Move(_)
                    | crate::curve_render::CurveCommand::Cubic { .. }
                    | crate::curve_render::CurveCommand::Close
            )
        }));
        let svg = crate::svg_export::canonical_pattern_svg_bytes(
            &CanonicalPatternOutput::Network(output.clone()),
            "connected",
        )
        .unwrap();
        let svg = String::from_utf8(svg).unwrap();
        assert!(svg.contains("-stroke-1"));
        assert!(!svg.contains("stroke-width="));
        assert_eq!(output.layers[0].blend_mode, CanonicalBlendMode::Screen);
    }

    #[test]
    fn threshold_and_size_mapping_follow_declared_stages() {
        let lattice = ShapesLattice {
            artboard: ArtboardSpace {
                width: 32,
                height: 16,
            },
            grid: calculate_web_grid(32, 16, 4.0),
            grid_rotation: 0.0,
            grid_pivot_x: 0.0,
            grid_pivot_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            x_grid_curved: false,
            y_grid_curved: false,
            x_grid_curve: 0.0,
            y_grid_curve: 0.0,
            curve_function: ShapesCurveFunction::Sine,
            placement_strategy: ShapesPlacementStrategy::Grid,
            random_dispersion: ShapesRandomDispersion::Uniform,
            point_definition: ShapesPointDefinition::Intersections,
            sampler: ShapesPointSampler::Grid,
            seed: 0,
            weight_influence: 1.0,
            random_size_response: 1.0,
            jitter_factor: 0.0,
            curve_spacing: 20.0,
            wrap_sites: false,
        };
        let samples = RecipeRuntimeValue::ShapesSamples(ShapesSamples {
            lattice,
            field: DistributionField::new(4, 2, vec![0.25, 0.5, 0.75, 1.0, 0.0, 0.5, 0.75, 1.0])
                .unwrap(),
            placements: None,
        });
        let threshold = LiteralValue::Number(0.5);
        let minimum = LiteralValue::Number(10.0);
        let maximum = LiteralValue::Number(90.0);
        let max_size = LiteralValue::Number(50.0);
        let parameters = BTreeMap::from([
            ("threshold", &threshold),
            ("min-mark", &minimum),
            ("max-mark", &maximum),
            ("max-size", &max_size),
        ]);
        let token = CancellationToken::new();
        let mapped = shapes_mark_map(
            &context(None, None, &token),
            &BTreeMap::from([("samples", &samples)]),
            &parameters,
        )
        .unwrap();
        let RecipeRuntimeValue::ShapesMappedValues(mapped) = mapped else {
            panic!("mark map must preserve typed mapped values");
        };
        assert_eq!(mapped.values[0], 0.0);
        assert_eq!(mapped.values[1], 0.0);
        assert!((mapped.values[2] - 0.275).abs() < 1e-12);
        assert!((mapped.values[3] - 0.45).abs() < 1e-12);
    }
}
