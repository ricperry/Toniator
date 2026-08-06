//! Thin Weighted Voronoi adapter over neutral placement and geometry services.

use crate::CancellationToken;
use crate::artwork_pipeline::{OutputChannelId, ResolvedChannelField, ResolvedChannelFields};
use crate::bundled_pattern_definitions::load_bundled_weighted_voronoi_definition;
use crate::model::{
    Document, WeightedVoronoiArrangementPolicy, WeightedVoronoiDensityPolarity,
    WeightedVoronoiPlacementMode, WeightedVoronoiSettings,
};
use crate::pattern::{
    AffineTransform, ArtboardSpace, CanonicalBlendMode, CanonicalColor, CanonicalLayer,
    CanonicalLayerId, CanonicalPatternOutput, CanonicalPoint, CompositePatternOutput, FillRule,
    FilledRegion, GeometryPolarity, PolygonRing, RegionId, RegionPatternOutput, RingWinding,
};
use crate::pattern_definition::{
    LiteralValue, NativeRecipeOperationError, NativeRecipeOperationRegistry,
    PatternInstanceParameters, PatternInstanceValue, REGISTERED_OPERATIONS, RecipeExecutionContext,
    RecipeOperationInputs, RecipeOperationParameters, RecipeRuntimeValue, RecipeVoronoiDiagram,
    RegisteredNativeRecipeOperation,
};
#[cfg(test)]
use crate::site_distribution::DistributionFingerprint;
use crate::site_distribution::{
    ArrangementPolicy, DistributionField, DistributionIdentity, DistributionLimits,
    DistributionMode, DistributionPolarity, DistributionRequest, DistributionRequestMetadata,
    DomainBounds, generate_site_distribution_cancellable,
};
use crate::voronoi_geometry::{
    GeometryLimits, build_voronoi_diagram_cancellable, inset_clipped_cell_for_response,
};
use anyhow::{Result, ensure};

/// Long edge of the bounded resolved source field used by this adapter.
pub const WEIGHTED_VORONOI_MAX_FIELD_EDGE: u32 = 256;

/// Per-channel cache provenance. This is metadata only; the adapter owns no
/// global cache and callers decide whether a completed result can be reused.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedVoronoiCacheMetadata {
    pub channel: OutputChannelId,
    pub source_generation: u64,
    pub resolved_field_generation: u64,
    pub distribution_fingerprint: DistributionFingerprint,
    pub geometry_fingerprint: u64,
    pub view_key: &'static str,
}

/// Maps a semantic channel/site pair to its visible final canonical region.
///
/// The raw clipped cell and its response inset are producer intermediates;
/// only the inset polygon is canonical artwork.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedVoronoiCellRegion {
    pub channel: OutputChannelId,
    pub site_index: usize,
    pub visible_region: RegionId,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedVoronoiGeneratedOutput {
    pub output: CanonicalPatternOutput,
    pub cache_metadata: Vec<WeightedVoronoiCacheMetadata>,
    pub cell_regions: Vec<WeightedVoronoiCellRegion>,
}

pub fn weighted_voronoi_field_dimensions(domain: DomainBounds) -> Result<(u32, u32)> {
    domain.validate()?;
    let longest = domain.width.max(domain.height);
    let scale = (f64::from(WEIGHTED_VORONOI_MAX_FIELD_EDGE) / f64::from(longest)).min(1.0);
    Ok((
        (f64::from(domain.width) * scale).round().max(1.0) as u32,
        (f64::from(domain.height) * scale).round().max(1.0) as u32,
    ))
}

/// Converts validated semantic fields into canonical final visible regions.
/// Uniform placement still resolves fields
/// for interior response, but never reads them for distribution.
/// Test-only oracle retained through the 3C dispatch transition. Production
/// rendering enters the bundled recipe executor below; remove this duplicate
/// after later migration evidence no longer needs output equivalence.
#[cfg(test)]
fn generate_weighted_voronoi_cancellable(
    domain: DomainBounds,
    settings: &WeightedVoronoiSettings,
    fields: &ResolvedChannelFields,
    token: &CancellationToken,
) -> Result<WeightedVoronoiGeneratedOutput> {
    record_weighted_oracle_generation();
    token.checkpoint()?;
    domain.validate()?;
    settings.validate()?;
    ensure!(
        fields.bounds.width > 0
            && fields.bounds.height > 0
            && fields.bounds.width <= WEIGHTED_VORONOI_MAX_FIELD_EDGE
            && fields.bounds.height <= WEIGHTED_VORONOI_MAX_FIELD_EDGE,
        "Weighted Voronoi resolved fields exceed the bounded field grid"
    );
    let artboard = ArtboardSpace {
        width: domain.width,
        height: domain.height,
    };
    let mut layers = Vec::new();
    let mut regions = Vec::new();
    let mut metadata = Vec::new();
    let mut cell_regions = Vec::new();

    for (channel_index, field) in fields.fields().iter().enumerate() {
        token.checkpoint()?;
        let channel_settings = settings.channel_settings(field.channel)?;
        if !channel_settings.enabled {
            continue;
        }
        validate_field(field, fields)?;
        let response_field = distribution_field(field)?;
        let distribution = generate_distribution_for_settings(
            domain,
            &response_field,
            field.channel,
            channel_settings,
            token,
        )?;
        let diagram = build_voronoi_diagram_cancellable(
            domain,
            &distribution.points,
            GeometryLimits {
                max_sites: DistributionLimits::default().max_sites,
            },
            token,
        )?;
        let layer_id = CanonicalLayerId(layers.len() as u32 + 1);
        layers.push(channel_layer(field.channel, layer_id, layers.len() as u32));
        let geometry_fingerprint = fingerprint_geometry(
            &diagram
                .cells
                .iter()
                .flat_map(|cell| &cell.vertices)
                .copied()
                .collect::<Vec<_>>(),
        );
        metadata.push(WeightedVoronoiCacheMetadata {
            channel: field.channel,
            source_generation: fields.generation,
            resolved_field_generation: field.generation,
            distribution_fingerprint: distribution.fingerprint,
            geometry_fingerprint,
            // Preview/PNG/SVG must not influence source, field, distribution,
            // geometry, or channel cache keys; they consume this same output.
            view_key: "canonical-output-v1",
        });

        for cell in &diagram.cells {
            if cell.site_index % 64 == 0 {
                token.checkpoint()?;
            }
            let response = response_at(
                &response_field,
                distribution.points[cell.site_index],
                domain,
            );
            let scale = if channel_settings.response_strength == 0.0 {
                1.0
            } else {
                channel_settings.minimum_cell_scale
                    + (1.0 - channel_settings.minimum_cell_scale)
                        * response.powf(channel_settings.response_strength)
            };
            if scale <= 1.0e-6 {
                continue;
            }
            let inset = inset_clipped_cell_for_response(
                domain,
                cell,
                distribution.points[cell.site_index],
                scale,
                channel_settings.boundary_gap,
            )?;
            let region_offset = (channel_index as u32)
                .saturating_mul(DistributionLimits::default().max_sites as u32)
                .saturating_add(cell.site_index as u32)
                .saturating_add(1);
            let visible_region = RegionId(region_offset);
            regions.push(FilledRegion {
                id: visible_region,
                layer_id,
                order: cell.site_index as u32,
                rings: vec![ring(&inset)],
                fill_rule: FillRule::NonZero,
                polarity: GeometryPolarity::Positive,
                transform: AffineTransform::IDENTITY,
            });
            cell_regions.push(WeightedVoronoiCellRegion {
                channel: field.channel,
                site_index: cell.site_index,
                visible_region,
            });
        }
    }
    let output = CanonicalPatternOutput::Composite(CompositePatternOutput {
        artboard,
        regions: Some(RegionPatternOutput {
            artboard,
            layers,
            regions,
        }),
        network: None,
    });
    output.validate().map_err(anyhow::Error::new)?;
    Ok(WeightedVoronoiGeneratedOutput {
        output,
        cache_metadata: metadata,
        cell_regions,
    })
}

#[cfg(test)]
fn generate_distribution(
    domain: DomainBounds,
    field: &ResolvedChannelField,
    settings: &crate::model::WeightedVoronoiChannelSettings,
    token: &CancellationToken,
) -> Result<crate::site_distribution::SiteDistribution> {
    let response_field = distribution_field(field)?;
    generate_distribution_for_settings(domain, &response_field, field.channel, settings, token)
}

#[cfg(test)]
fn generate_distribution_for_settings(
    domain: DomainBounds,
    field: &DistributionField,
    channel: OutputChannelId,
    settings: &crate::model::WeightedVoronoiChannelSettings,
    token: &CancellationToken,
) -> Result<crate::site_distribution::SiteDistribution> {
    let metadata = DistributionRequestMetadata {
        seed: settings.seed,
        identity: DistributionIdentity(channel_identity(channel)),
        arrangement: match settings.arrangement {
            WeightedVoronoiArrangementPolicy::Shared => ArrangementPolicy::Shared,
            WeightedVoronoiArrangementPolicy::Independent => ArrangementPolicy::Independent,
        },
        mode: match settings.placement {
            WeightedVoronoiPlacementMode::Uniform => DistributionMode::Uniform,
            WeightedVoronoiPlacementMode::SourceWeighted => DistributionMode::SourceWeighted,
        },
        polarity: match settings.density_polarity {
            WeightedVoronoiDensityPolarity::DarkerMoreDense => {
                DistributionPolarity::HigherValuesMoreDense
            }
            WeightedVoronoiDensityPolarity::LighterMoreDense => {
                DistributionPolarity::LowerValuesMoreDense
            }
        },
        strength_milli: (settings.density_strength * 1_000.0).round() as u32,
    };
    generate_distribution_with_metadata(
        domain,
        field,
        settings.cell_count as usize,
        metadata,
        token,
    )
}

fn generate_distribution_with_metadata(
    domain: DomainBounds,
    field: &DistributionField,
    count: usize,
    metadata: DistributionRequestMetadata,
    token: &CancellationToken,
) -> Result<crate::site_distribution::SiteDistribution> {
    generate_site_distribution_cancellable(
        DistributionRequest {
            domain,
            count,
            metadata,
            field: Some(field),
            limits: DistributionLimits::default(),
        },
        token,
    )
}

fn distribution_field(field: &ResolvedChannelField) -> Result<DistributionField> {
    record_recipe_field_conversion();
    let values = field
        .values()
        .iter()
        .enumerate()
        .map(|(index, _)| field.value_at(index))
        .collect();
    DistributionField::new(field.bounds.width, field.bounds.height, values)
}

fn validate_field(field: &ResolvedChannelField, fields: &ResolvedChannelFields) -> Result<()> {
    ensure!(
        field.bounds == fields.bounds,
        "Weighted Voronoi fields have inconsistent bounds"
    );
    ensure!(
        field.values().len()
            == (field.bounds.width as usize).saturating_mul(field.bounds.height as usize),
        "Weighted Voronoi field dimensions do not match values"
    );
    ensure!(
        field
            .values()
            .iter()
            .chain(field.coverage())
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
        "Weighted Voronoi fields must be finite normalized values"
    );
    Ok(())
}

fn response_at(
    field: &DistributionField,
    point: crate::site_distribution::OrderedPoint,
    domain: DomainBounds,
) -> f64 {
    let (width, height) = field.dimensions();
    let x = ((point.x / f64::from(domain.width)) * f64::from(width))
        .floor()
        .clamp(0.0, f64::from(width - 1)) as usize;
    let y = ((point.y / f64::from(domain.height)) * f64::from(height))
        .floor()
        .clamp(0.0, f64::from(height - 1)) as usize;
    field.values()[y * width as usize + x].clamp(0.0, 1.0)
}

fn ring(points: &[crate::site_distribution::OrderedPoint]) -> PolygonRing {
    PolygonRing {
        vertices: points
            .iter()
            .map(|point| CanonicalPoint {
                x: point.x as f32,
                y: point.y as f32,
            })
            .collect(),
        winding: RingWinding::Clockwise,
    }
}

fn channel_layer(channel: OutputChannelId, id: CanonicalLayerId, order: u32) -> CanonicalLayer {
    let (color, blend_mode) = match channel {
        OutputChannelId::CmykCyan => (
            CanonicalColor {
                red: 0,
                green: 174,
                blue: 239,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykMagenta => (
            CanonicalColor {
                red: 236,
                green: 0,
                blue: 140,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykYellow => (
            CanonicalColor {
                red: 255,
                green: 242,
                blue: 0,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykBlack => (
            CanonicalColor {
                red: 17,
                green: 17,
                blue: 17,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::RgbRed => (
            CanonicalColor {
                red: 255,
                green: 0,
                blue: 0,
            },
            CanonicalBlendMode::Screen,
        ),
        OutputChannelId::RgbGreen => (
            CanonicalColor {
                red: 0,
                green: 255,
                blue: 0,
            },
            CanonicalBlendMode::Screen,
        ),
        OutputChannelId::RgbBlue => (
            CanonicalColor {
                red: 0,
                green: 0,
                blue: 255,
            },
            CanonicalBlendMode::Screen,
        ),
    };
    CanonicalLayer {
        id,
        channel: Some(channel),
        label: format!("Weighted Voronoi {}", channel.stable_id()),
        order,
        color,
        opacity: 1.0,
        blend_mode,
    }
}

fn channel_identity(channel: OutputChannelId) -> u64 {
    channel
        .stable_id()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3)
        })
}

#[cfg(test)]
fn fingerprint_geometry(points: &[crate::site_distribution::OrderedPoint]) -> u64 {
    points.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, point| {
        hash ^ point.x.to_bits().wrapping_mul(31) ^ point.y.to_bits().rotate_left(17)
    })
}

/// The fixed, production-only native implementations for the bundled Weighted
/// Voronoi v1 recipe. Declarative recipe bytes select only these exact bodies.
pub static WEIGHTED_VORONOI_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 6] = [
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.source-sample",
        version: 1,
        execute: weighted_voronoi_source_sample,
    },
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.response-map",
        version: 1,
        execute: weighted_voronoi_response_map,
    },
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.site-distribution",
        version: 1,
        execute: weighted_voronoi_site_distribution,
    },
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.construct-voronoi",
        version: 1,
        execute: weighted_voronoi_construct_voronoi,
    },
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.response-inset",
        version: 1,
        execute: weighted_voronoi_response_inset,
    },
    RegisteredNativeRecipeOperation {
        id: "weighted-voronoi.emit-regions",
        version: 1,
        execute: weighted_voronoi_emit_regions,
    },
];

/// Static production registry for the six bundled Weighted Voronoi stages.
pub static WEIGHTED_VORONOI_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::new(
        REGISTERED_OPERATIONS.entries(),
        &WEIGHTED_VORONOI_NATIVE_OPERATIONS,
    );

fn native_error(error: impl std::fmt::Display) -> NativeRecipeOperationError {
    NativeRecipeOperationError::new(error.to_string())
}

#[cfg(test)]
thread_local! {
    static RECIPE_FIELD_CONVERSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECIPE_NATIVE_OPERATION_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECIPE_LIVE_EXECUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WEIGHTED_ORACLE_GENERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_recipe_field_conversion() {
    RECIPE_FIELD_CONVERSIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_recipe_field_conversion() {}

#[cfg(test)]
fn record_recipe_native_operation() {
    RECIPE_NATIVE_OPERATION_INVOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_recipe_native_operation() {}

#[cfg(test)]
fn record_recipe_live_execution() {
    RECIPE_LIVE_EXECUTIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
fn record_recipe_live_execution() {}

#[cfg(test)]
fn record_weighted_oracle_generation() {
    WEIGHTED_ORACLE_GENERATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn reset_recipe_execution_instrumentation() {
    RECIPE_FIELD_CONVERSIONS.with(|count| count.set(0));
    RECIPE_NATIVE_OPERATION_INVOCATIONS.with(|count| count.set(0));
}

#[cfg(test)]
fn recipe_execution_instrumentation() -> (usize, usize) {
    let field_conversions = RECIPE_FIELD_CONVERSIONS.with(|count| count.get());
    let native_operations = RECIPE_NATIVE_OPERATION_INVOCATIONS.with(|count| count.get());
    (field_conversions, native_operations)
}

/// Test seam for the live render branch. Thread-local storage keeps parallel
/// unit tests independent while proving the production dispatch cannot fall
/// back to the retained oracle.
#[cfg(test)]
pub(crate) fn reset_weighted_dispatch_instrumentation() {
    RECIPE_LIVE_EXECUTIONS.with(|count| count.set(0));
    WEIGHTED_ORACLE_GENERATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn weighted_dispatch_instrumentation() -> (usize, usize) {
    let recipe_executions = RECIPE_LIVE_EXECUTIONS.with(|count| count.get());
    let oracle_generations = WEIGHTED_ORACLE_GENERATIONS.with(|count| count.get());
    (recipe_executions, oracle_generations)
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

fn required_channel(
    context: &RecipeExecutionContext<'_>,
) -> Result<OutputChannelId, NativeRecipeOperationError> {
    context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new(
            "Weighted Voronoi operation requires a semantic output channel",
        )
    })
}

fn weighted_voronoi_source_sample(
    context: &RecipeExecutionContext<'_>,
    _: &RecipeOperationInputs<'_>,
    _: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let field = context.source_field.ok_or_else(|| {
        NativeRecipeOperationError::new("Weighted Voronoi source-sample requires a source field")
    })?;
    Ok(RecipeRuntimeValue::Samples(field.clone()))
}

fn weighted_voronoi_response_map(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    _: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::Samples(field) = required_input(inputs, "samples")? else {
        return Err(NativeRecipeOperationError::new(
            "input `samples` must be sampled source values",
        ));
    };
    Ok(RecipeRuntimeValue::MappedField(field.clone()))
}

fn weighted_voronoi_site_distribution(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::MappedField(field) = required_input(inputs, "response-field")? else {
        return Err(NativeRecipeOperationError::new(
            "input `response-field` must be a mapped source field",
        ));
    };
    let channel = required_channel(context)?;
    let count =
        usize::try_from(required_integer(parameters, "cell-count")?).map_err(native_error)?;
    let arrangement = match required_choice(parameters, "arrangement")? {
        "shared" => ArrangementPolicy::Shared,
        "independent" => ArrangementPolicy::Independent,
        value => {
            return Err(NativeRecipeOperationError::new(format!(
                "unsupported arrangement `{value}`"
            )));
        }
    };
    let mode = match required_choice(parameters, "placement")? {
        "uniform" => DistributionMode::Uniform,
        "source-weighted" => DistributionMode::SourceWeighted,
        value => {
            return Err(NativeRecipeOperationError::new(format!(
                "unsupported placement `{value}`"
            )));
        }
    };
    let polarity = match required_choice(parameters, "density-polarity")? {
        "darker-more-dense" => DistributionPolarity::HigherValuesMoreDense,
        "lighter-more-dense" => DistributionPolarity::LowerValuesMoreDense,
        value => {
            return Err(NativeRecipeOperationError::new(format!(
                "unsupported density polarity `{value}`"
            )));
        }
    };
    let strength = required_number(parameters, "density-strength")?;
    if !(0.001..=16.0).contains(&strength) {
        return Err(NativeRecipeOperationError::new(
            "density-strength is outside the supported range",
        ));
    }
    let distribution = generate_distribution_with_metadata(
        DomainBounds {
            width: context.artboard.width,
            height: context.artboard.height,
        },
        field,
        count,
        DistributionRequestMetadata {
            seed: required_integer(parameters, "seed")?,
            identity: DistributionIdentity(channel_identity(channel)),
            arrangement,
            mode,
            polarity,
            strength_milli: (strength * 1_000.0).round() as u32,
        },
        context.cancellation,
    )
    .map_err(native_error)?;
    Ok(RecipeRuntimeValue::Placement(distribution))
}

fn weighted_voronoi_construct_voronoi(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    _: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::Placement(distribution) = required_input(inputs, "sites")? else {
        return Err(NativeRecipeOperationError::new(
            "input `sites` must be a site distribution",
        ));
    };
    let domain = DomainBounds {
        width: context.artboard.width,
        height: context.artboard.height,
    };
    if distribution.domain != domain {
        return Err(NativeRecipeOperationError::new(
            "site distribution domain does not match the recipe artboard",
        ));
    }
    let diagram = build_voronoi_diagram_cancellable(
        domain,
        &distribution.points,
        GeometryLimits {
            max_sites: DistributionLimits::default().max_sites,
        },
        context.cancellation,
    )
    .map_err(native_error)?;
    Ok(RecipeRuntimeValue::VoronoiDiagram(RecipeVoronoiDiagram {
        diagram,
        sites: distribution.points.clone(),
    }))
}

fn weighted_voronoi_response_inset(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let RecipeRuntimeValue::VoronoiDiagram(diagram) = required_input(inputs, "diagram")? else {
        return Err(NativeRecipeOperationError::new(
            "input `diagram` must be a Voronoi diagram",
        ));
    };
    let RecipeRuntimeValue::MappedField(field) = required_input(inputs, "response-field")? else {
        return Err(NativeRecipeOperationError::new(
            "input `response-field` must be a mapped source field",
        ));
    };
    let domain = DomainBounds {
        width: context.artboard.width,
        height: context.artboard.height,
    };
    if diagram.diagram.domain != domain {
        return Err(NativeRecipeOperationError::new(
            "Voronoi diagram domain does not match the recipe artboard",
        ));
    }
    let response_strength = required_number(parameters, "response-strength")?;
    let minimum_cell_scale = required_number(parameters, "minimum-cell-scale")?;
    let boundary_gap = required_number(parameters, "boundary-gap")?;
    if !(0.0..=16.0).contains(&response_strength)
        || !(0.0..=1.0).contains(&minimum_cell_scale)
        || !(0.0..=64.0).contains(&boundary_gap)
    {
        return Err(NativeRecipeOperationError::new(
            "response inset parameters are outside their supported ranges",
        ));
    }
    let channel = required_channel(context)?;
    let layer_id = CanonicalLayerId(context.enabled_layer_index.saturating_add(1));
    let mut regions = Vec::new();
    for cell in &diagram.diagram.cells {
        if cell.site_index % 64 == 0 {
            context.cancellation.checkpoint().map_err(native_error)?;
        }
        let site = *diagram.sites.get(cell.site_index).ok_or_else(|| {
            NativeRecipeOperationError::new("Voronoi diagram cell references a missing site")
        })?;
        let response = response_at(field, site, domain);
        let scale = if response_strength == 0.0 {
            1.0
        } else {
            minimum_cell_scale + (1.0 - minimum_cell_scale) * response.powf(response_strength)
        };
        if scale <= 1.0e-6 {
            continue;
        }
        let inset = inset_clipped_cell_for_response(domain, cell, site, scale, boundary_gap)
            .map_err(native_error)?;
        let region_offset = context
            .semantic_channel_index
            .saturating_mul(DistributionLimits::default().max_sites as u32)
            .saturating_add(cell.site_index as u32)
            .saturating_add(1);
        regions.push(FilledRegion {
            id: RegionId(region_offset),
            layer_id,
            order: cell.site_index as u32,
            rings: vec![ring(&inset)],
            fill_rule: FillRule::NonZero,
            polarity: GeometryPolarity::Positive,
            transform: AffineTransform::IDENTITY,
        });
    }
    Ok(RecipeRuntimeValue::BoundaryDerivedRegionCells(
        RegionPatternOutput {
            artboard: context.artboard,
            layers: vec![channel_layer(
                channel,
                layer_id,
                context.enabled_layer_index,
            )],
            regions,
        },
    ))
}

fn weighted_voronoi_emit_regions(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    record_recipe_native_operation();
    context.cancellation.checkpoint().map_err(native_error)?;
    let enabled = required_boolean(parameters, "enabled")?;
    let regions = if enabled {
        let RecipeRuntimeValue::BoundaryDerivedRegionCells(regions) =
            required_input(inputs, "response-insets")?
        else {
            return Err(NativeRecipeOperationError::new(
                "input `response-insets` must be boundary-derived region cells",
            ));
        };
        if regions.artboard != context.artboard {
            return Err(NativeRecipeOperationError::new(
                "response insets artboard does not match the recipe context",
            ));
        }
        regions.clone()
    } else {
        RegionPatternOutput {
            artboard: context.artboard,
            layers: Vec::new(),
            regions: Vec::new(),
        }
    };
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Composite(CompositePatternOutput {
            artboard: context.artboard,
            regions: Some(regions),
            network: None,
        }),
    ))
}

/// Deterministically adapts the current typed Weighted settings into the
/// strict, data-only bundled recipe instance. This is one-way: it neither
/// reads nor writes renderer state or persisted recipe authority.
pub fn weighted_voronoi_recipe_instance_from_settings(
    settings: &WeightedVoronoiSettings,
) -> Result<PatternInstanceParameters> {
    settings.validate()?;
    let definition = load_bundled_weighted_voronoi_definition().map_err(native_error)?;
    let mut instance = definition.default_instance_parameters(
        OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB),
    )?;
    for values in &mut instance.output_channel_values {
        let channel = values
            .channel
            .parse::<OutputChannelId>()
            .map_err(|error| anyhow::anyhow!(error))?;
        let settings = settings.channel_settings(channel)?;
        values.values = weighted_voronoi_instance_values(settings);
    }
    definition.validate_instance_parameters(&instance)?;
    Ok(instance)
}

/// Reads the current document's persisted `pattern_state` settings and adapts
/// them one-way to a strict recipe instance for equivalence execution.
pub fn weighted_voronoi_recipe_instance_from_document(
    document: &Document,
) -> Result<PatternInstanceParameters> {
    weighted_voronoi_recipe_instance_from_settings(
        &document.pattern_state.weighted_voronoi_settings()?,
    )
}

fn weighted_voronoi_instance_values(
    settings: &crate::model::WeightedVoronoiChannelSettings,
) -> Vec<PatternInstanceValue> {
    vec![
        instance_value("enabled", LiteralValue::Boolean(settings.enabled)),
        instance_value(
            "arrangement",
            LiteralValue::Choice(
                match settings.arrangement {
                    WeightedVoronoiArrangementPolicy::Shared => "shared",
                    WeightedVoronoiArrangementPolicy::Independent => "independent",
                }
                .into(),
            ),
        ),
        instance_value(
            "cell-count",
            LiteralValue::Integer(u64::from(settings.cell_count)),
        ),
        instance_value("seed", LiteralValue::Integer(settings.seed)),
        instance_value("boundary-gap", LiteralValue::Number(settings.boundary_gap)),
        instance_value(
            "placement",
            LiteralValue::Choice(
                match settings.placement {
                    WeightedVoronoiPlacementMode::Uniform => "uniform",
                    WeightedVoronoiPlacementMode::SourceWeighted => "source-weighted",
                }
                .into(),
            ),
        ),
        instance_value(
            "density-polarity",
            LiteralValue::Choice(
                match settings.density_polarity {
                    WeightedVoronoiDensityPolarity::DarkerMoreDense => "darker-more-dense",
                    WeightedVoronoiDensityPolarity::LighterMoreDense => "lighter-more-dense",
                }
                .into(),
            ),
        ),
        instance_value(
            "density-strength",
            LiteralValue::Number(settings.density_strength),
        ),
        instance_value(
            "response-strength",
            LiteralValue::Number(settings.response_strength),
        ),
        instance_value(
            "minimum-cell-scale",
            LiteralValue::Number(settings.minimum_cell_scale),
        ),
    ]
}

fn instance_value(key: &str, value: LiteralValue) -> PatternInstanceValue {
    PatternInstanceValue {
        key: key.to_owned(),
        value,
    }
}

/// Executes the bundled recipe once per selected semantic field and assembles
/// its canonical channel outputs. This is the live Weighted renderer authority;
/// preview, PNG, and SVG consume this canonical result through shared paths.
pub fn execute_bundled_weighted_voronoi_recipe_cancellable(
    domain: DomainBounds,
    settings: &WeightedVoronoiSettings,
    fields: &ResolvedChannelFields,
    token: &CancellationToken,
) -> Result<CanonicalPatternOutput> {
    record_recipe_live_execution();
    token.checkpoint()?;
    domain.validate()?;
    settings.validate()?;
    ensure!(
        fields.bounds.width > 0
            && fields.bounds.height > 0
            && fields.bounds.width <= WEIGHTED_VORONOI_MAX_FIELD_EDGE
            && fields.bounds.height <= WEIGHTED_VORONOI_MAX_FIELD_EDGE,
        "Weighted Voronoi resolved fields exceed the bounded field grid"
    );
    let definition = load_bundled_weighted_voronoi_definition().map_err(native_error)?;
    let instance = weighted_voronoi_recipe_instance_from_settings(settings)?;
    let artboard = ArtboardSpace {
        width: domain.width,
        height: domain.height,
    };
    let mut layers = Vec::new();
    let mut regions = Vec::new();
    let mut enabled_layer_index = 0u32;
    for (semantic_channel_index, resolved_field) in fields.fields().iter().enumerate() {
        token.checkpoint()?;
        let enabled = settings.channel_settings(resolved_field.channel)?.enabled;
        // Channel enablement is orchestration/output assignment, not geometry:
        // disabled semantic channels never enter the bundled recipe graph.
        // This deliberately matches the shipping adapter's early skip, so a
        // disabled stale/malformed field cannot create work or cancellation.
        if !enabled {
            continue;
        }
        validate_field(resolved_field, fields)?;
        let source_field = distribution_field(resolved_field)?;
        let context = RecipeExecutionContext {
            artboard,
            output_channel: Some(resolved_field.channel),
            source_field_provider: None,
            source_field: Some(&source_field),
            source_generation: fields.generation,
            resolved_field_generation: resolved_field.generation,
            semantic_channel_index: semantic_channel_index as u32,
            enabled_layer_index,
            definition_assets: &[],
            cancellation: token,
        };
        let output = definition.execute_recipe(
            &instance,
            &context,
            &WEIGHTED_VORONOI_NATIVE_OPERATION_REGISTRY,
        )?;
        let CanonicalPatternOutput::Composite(output) = output else {
            unreachable!("the bundled Weighted recipe declares region output")
        };
        let channel_regions = output
            .regions
            .expect("the bundled Weighted recipe emits region capability");
        layers.extend(channel_regions.layers);
        regions.extend(channel_regions.regions);
        enabled_layer_index = enabled_layer_index.saturating_add(1);
    }
    let output = CanonicalPatternOutput::Composite(CompositePatternOutput {
        artboard,
        regions: Some(RegionPatternOutput {
            artboard,
            layers,
            regions,
        }),
        network: None,
    });
    output.validate().map_err(anyhow::Error::new)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artwork_pipeline::{
        ArtworkPipelineSettings, AutomaticSeparationStrategy, ChannelAssignment, OutputModel,
        PreparedSource, SourceAlphaPolicy, resolve_channel_fields,
    };
    use crate::model::{
        Document, DocumentEditor, OutputMode, SourceArtwork, WeightedVoronoiArrangementPolicy,
        WeightedVoronoiPlacementMode,
    };
    use crate::render::{
        generate_document_pattern_output, render_canonical_pattern_output_cancellable,
    };
    use crate::{
        canonical_pattern_png_bytes, canonical_pattern_svg_bytes, load_document,
        save_document_atomic,
    };
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::sync::Arc;

    fn rgb_pipeline() -> ArtworkPipelineSettings {
        ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            ),
            alpha_policy: SourceAlphaPolicy::Ignore,
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        }
    }

    fn cmyk_pipeline() -> ArtworkPipelineSettings {
        ArtworkPipelineSettings {
            output_model: OutputModel::CmykPrint,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1,
            ),
            alpha_policy: SourceAlphaPolicy::Ignore,
            active_channel: Some(OutputChannelId::CmykCyan),
            ..ArtworkPipelineSettings::default()
        }
    }

    fn fields(source: &RgbaImage) -> ResolvedChannelFields {
        fields_for(source, &rgb_pipeline(), &OutputChannelId::RGB)
    }

    fn cmyk_fields(source: &RgbaImage) -> ResolvedChannelFields {
        fields_for(source, &cmyk_pipeline(), &OutputChannelId::CMYK)
    }

    fn fields_for(
        source: &RgbaImage,
        pipeline: &ArtworkPipelineSettings,
        channels: &[OutputChannelId],
    ) -> ResolvedChannelFields {
        let prepared = PreparedSource::from_rgba_image(source, 41);
        resolve_channel_fields(&prepared, pipeline, 32, 16, 41, channels).unwrap()
    }

    fn settings(count: u32) -> WeightedVoronoiSettings {
        let mut settings = WeightedVoronoiSettings::default();
        for channel in OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB)
        {
            let channel_settings = settings.channel_settings_mut(channel).unwrap();
            channel_settings.enabled = channel.belongs_to(OutputModel::RgbScreen);
            channel_settings.cell_count = count;
            channel_settings.seed = 77;
            channel_settings.boundary_gap = 0.5;
        }
        settings
    }

    fn cmyk_settings(count: u32) -> WeightedVoronoiSettings {
        let mut settings = WeightedVoronoiSettings::default();
        for channel in OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB)
        {
            let channel_settings = settings.channel_settings_mut(channel).unwrap();
            channel_settings.enabled = channel.belongs_to(OutputModel::CmykPrint);
            channel_settings.cell_count = count;
            channel_settings.seed = 0xfedc_ba98_7654_3210;
            channel_settings.boundary_gap = 1.5;
            channel_settings.arrangement = WeightedVoronoiArrangementPolicy::Independent;
            channel_settings.density_polarity =
                crate::model::WeightedVoronoiDensityPolarity::LighterMoreDense;
            channel_settings.density_strength = 2.5;
            channel_settings.response_strength = 1.75;
            channel_settings.minimum_cell_scale = 0.2;
        }
        settings
    }

    fn assert_recipe_equivalence(
        domain: DomainBounds,
        settings: &WeightedVoronoiSettings,
        fields: &ResolvedChannelFields,
    ) {
        let authoritative = generate_weighted_voronoi_cancellable(
            domain,
            settings,
            fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let assembled = execute_bundled_weighted_voronoi_recipe_cancellable(
            domain,
            settings,
            fields,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(assembled, authoritative.output);
        assert!(
            authoritative
                .cache_metadata
                .iter()
                .all(|entry| entry.distribution_fingerprint.0 != 0)
        );
    }

    #[test]
    fn bundled_recipe_matches_authoritative_weighted_output_for_rgb_and_cmyk() {
        let source = RgbaImage::from_fn(64, 32, |x, y| {
            Rgba([
                ((x * 3 + y * 7) % 256) as u8,
                ((x * 11 + y * 5) % 256) as u8,
                ((255 - x * 2 + y * 3) % 256) as u8,
                255,
            ])
        });
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };

        let mut rgb_shared = settings(20);
        for channel in OutputChannelId::RGB {
            let configured = rgb_shared.channel_settings_mut(channel).unwrap();
            configured.seed = u64::MAX;
            configured.boundary_gap = 0.0;
            configured.density_strength = 0.5;
        }
        assert_recipe_equivalence(domain, &rgb_shared, &fields(&source));

        let mut rgb_uniform = settings(20);
        for channel in OutputChannelId::RGB {
            let configured = rgb_uniform.channel_settings_mut(channel).unwrap();
            configured.arrangement = WeightedVoronoiArrangementPolicy::Independent;
            configured.placement = WeightedVoronoiPlacementMode::Uniform;
            configured.density_polarity =
                crate::model::WeightedVoronoiDensityPolarity::LighterMoreDense;
            configured.density_strength = 4.0;
            configured.response_strength = 1.5;
            configured.minimum_cell_scale = 0.25;
            configured.boundary_gap = 2.0;
        }
        assert_recipe_equivalence(domain, &rgb_uniform, &fields(&source));

        assert_recipe_equivalence(domain, &cmyk_settings(18), &cmyk_fields(&source));
    }

    #[test]
    fn bundled_recipe_preserves_disabled_channel_empty_output_behavior() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 255, 255])
            }
        });
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };
        let mut configured = settings(16);
        for channel in OutputChannelId::RGB {
            configured.channel_settings_mut(channel).unwrap().enabled =
                channel == OutputChannelId::RgbGreen;
        }
        assert_recipe_equivalence(domain, &configured, &fields(&source));

        let output = execute_bundled_weighted_voronoi_recipe_cancellable(
            domain,
            &configured,
            &fields(&source),
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Composite(output) = output else {
            panic!("bundled Weighted recipe must emit a composite output");
        };
        let regions = output.regions.unwrap();
        assert_eq!(regions.layers.len(), 1);
        assert_eq!(regions.layers[0].channel, Some(OutputChannelId::RgbGreen));

        reset_recipe_execution_instrumentation();
        execute_bundled_weighted_voronoi_recipe_cancellable(
            domain,
            &configured,
            &fields(&source),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(recipe_execution_instrumentation(), (1, 6));
    }

    #[test]
    fn disabled_channels_are_filtered_before_field_or_native_recipe_work() {
        let source = RgbaImage::from_pixel(64, 32, Rgba([128, 64, 32, 255]));
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };
        let mut configured = settings(16);
        for channel in OutputChannelId::RGB {
            configured.channel_settings_mut(channel).unwrap().enabled = false;
        }
        reset_recipe_execution_instrumentation();
        let output = execute_bundled_weighted_voronoi_recipe_cancellable(
            domain,
            &configured,
            &fields(&source),
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Composite(output) = output else {
            panic!("bundled Weighted recipe must emit a composite output");
        };
        let regions = output.regions.unwrap();
        assert!(regions.layers.is_empty());
        assert!(regions.regions.is_empty());
        assert_eq!(recipe_execution_instrumentation(), (0, 0));
    }

    #[test]
    fn bundled_native_operations_report_missing_source_channel_and_cancellation() {
        let token = CancellationToken::new();
        let empty_inputs = std::collections::BTreeMap::new();
        let empty_parameters = std::collections::BTreeMap::new();
        let missing_source = RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 64,
                height: 32,
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
        assert!(
            weighted_voronoi_source_sample(&missing_source, &empty_inputs, &empty_parameters)
                .unwrap_err()
                .to_string()
                .contains("requires a source field")
        );

        let field = DistributionField::new(2, 2, vec![0.0, 0.25, 0.5, 1.0]).unwrap();
        let mapped = RecipeRuntimeValue::MappedField(field);
        let inputs = std::collections::BTreeMap::from([("response-field", &mapped)]);
        let cell_count = LiteralValue::Integer(2);
        let seed = LiteralValue::Integer(9);
        let arrangement = LiteralValue::Choice("shared".into());
        let placement = LiteralValue::Choice("source-weighted".into());
        let density_polarity = LiteralValue::Choice("darker-more-dense".into());
        let density_strength = LiteralValue::Number(1.0);
        let parameters = std::collections::BTreeMap::from([
            ("cell-count", &cell_count),
            ("seed", &seed),
            ("arrangement", &arrangement),
            ("placement", &placement),
            ("density-polarity", &density_polarity),
            ("density-strength", &density_strength),
        ]);
        let missing_channel = RecipeExecutionContext {
            output_channel: None,
            source_field: None,
            ..missing_source
        };
        assert!(
            weighted_voronoi_site_distribution(&missing_channel, &inputs, &parameters)
                .unwrap_err()
                .to_string()
                .contains("requires a semantic output channel")
        );

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_context = RecipeExecutionContext {
            cancellation: &cancelled,
            ..missing_source
        };
        assert!(
            weighted_voronoi_source_sample(&cancelled_context, &empty_inputs, &empty_parameters)
                .unwrap_err()
                .to_string()
                .contains("operation cancelled")
        );
    }

    #[test]
    fn settings_adapter_is_strict_deterministic_and_reads_document_pattern_state_one_way() {
        let mut configured = settings(24);
        let red = configured
            .channel_settings_mut(OutputChannelId::RgbRed)
            .unwrap();
        red.seed = u64::MAX;
        red.enabled = false;
        let first = weighted_voronoi_recipe_instance_from_settings(&configured).unwrap();
        let second = weighted_voronoi_recipe_instance_from_settings(&configured).unwrap();
        assert_eq!(first, second);
        let red = first
            .output_channel_values
            .iter()
            .find(|values| values.channel == OutputChannelId::RgbRed.stable_id())
            .unwrap();
        assert!(red.values.contains(&PatternInstanceValue {
            key: "seed".into(),
            value: LiteralValue::Integer(u64::MAX),
        }));
        assert!(red.values.contains(&PatternInstanceValue {
            key: "enabled".into(),
            value: LiteralValue::Boolean(false),
        }));

        let document = Document::new(SourceArtwork {
            name: "adapter.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from([0u8; 1]),
        });
        let mut editor = DocumentEditor::new(document);
        assert!(editor.select_pattern(crate::pattern::PatternId::WEIGHTED_VORONOI_V1));
        let document = editor.document();
        let from_document = weighted_voronoi_recipe_instance_from_document(document).unwrap();
        let from_settings = weighted_voronoi_recipe_instance_from_settings(
            &document.pattern_state.weighted_voronoi_settings().unwrap(),
        )
        .unwrap();
        assert_eq!(from_document, from_settings);
    }

    #[test]
    fn semantic_fields_and_weighted_channels_remain_distinct() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let fields = fields(&source);
        assert_ne!(
            fields.field(OutputChannelId::RgbRed).unwrap().values(),
            fields.field(OutputChannelId::RgbBlue).unwrap().values()
        );
        let generated = generate_weighted_voronoi_cancellable(
            DomainBounds {
                width: 64,
                height: 32,
            },
            &settings(24),
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let red = generated
            .cache_metadata
            .iter()
            .find(|entry| entry.channel == OutputChannelId::RgbRed)
            .unwrap();
        let blue = generated
            .cache_metadata
            .iter()
            .find(|entry| entry.channel == OutputChannelId::RgbBlue)
            .unwrap();
        assert_ne!(red.geometry_fingerprint, blue.geometry_fingerprint);
        let CanonicalPatternOutput::Composite(composite) = &generated.output else {
            panic!("Weighted Voronoi must use canonical composite output");
        };
        let regions = composite.regions.as_ref().unwrap();
        for cell_region in &generated.cell_regions {
            assert!(
                regions
                    .regions
                    .iter()
                    .any(|region| region.id == cell_region.visible_region)
            );
        }
    }

    #[test]
    fn final_cells_are_positive_boundary_derived_insets_without_construction_masks() {
        let source = RgbaImage::from_pixel(64, 32, Rgba([255, 255, 255, 255]));
        let fields = fields(&source);
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };
        let mut configured = settings(8);
        for channel in OutputChannelId::RGB {
            let channel_settings = configured.channel_settings_mut(channel).unwrap();
            channel_settings.enabled = channel == OutputChannelId::RgbRed;
            channel_settings.placement = WeightedVoronoiPlacementMode::Uniform;
            channel_settings.response_strength = 0.0;
            channel_settings.boundary_gap = 2.0;
        }

        let generated = generate_weighted_voronoi_cancellable(
            domain,
            &configured,
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let CanonicalPatternOutput::Composite(composite) = &generated.output else {
            panic!("Weighted Voronoi must use canonical composite output");
        };
        let regions = composite.regions.as_ref().unwrap();
        assert_eq!(regions.regions.len(), 8);
        assert_eq!(generated.cell_regions.len(), 8);
        assert!(
            regions
                .regions
                .iter()
                .all(|region| region.polarity == GeometryPolarity::Positive)
        );
        assert!(
            regions
                .regions
                .iter()
                .all(|region| { region.fill_rule == FillRule::NonZero && region.rings.len() == 1 })
        );
        assert_eq!(
            generated
                .cell_regions
                .iter()
                .map(|cell_region| (cell_region.channel, cell_region.site_index))
                .collect::<Vec<_>>(),
            (0..8)
                .map(|site_index| (OutputChannelId::RgbRed, site_index))
                .collect::<Vec<_>>()
        );

        let field = fields.field(OutputChannelId::RgbRed).unwrap();
        let channel_settings = configured
            .channel_settings(OutputChannelId::RgbRed)
            .unwrap();
        let distribution =
            generate_distribution(domain, field, channel_settings, &CancellationToken::new())
                .unwrap();
        let diagram = build_voronoi_diagram_cancellable(
            domain,
            &distribution.points,
            GeometryLimits {
                max_sites: DistributionLimits::default().max_sites,
            },
            &CancellationToken::new(),
        )
        .unwrap();
        for cell_region in &generated.cell_regions {
            let cell = &diagram.cells[cell_region.site_index];
            let expected_inset = inset_clipped_cell_for_response(
                domain,
                cell,
                distribution.points[cell.site_index],
                1.0,
                channel_settings.boundary_gap,
            )
            .unwrap();
            let visible_region = regions
                .regions
                .iter()
                .find(|region| region.id == cell_region.visible_region)
                .unwrap();
            assert_eq!(visible_region.rings, vec![ring(&expected_inset)]);
            assert_ne!(visible_region.rings, vec![ring(&cell.vertices)]);
        }
    }

    #[test]
    fn uniform_is_source_independent_while_shared_and_independent_are_explicit() {
        let left = RgbaImage::from_pixel(64, 32, Rgba([255, 0, 0, 255]));
        let right = RgbaImage::from_pixel(64, 32, Rgba([0, 0, 255, 255]));
        let first = fields(&left);
        let second = fields(&right);
        let mut configured = settings(24);
        let red = configured
            .channel_settings_mut(OutputChannelId::RgbRed)
            .unwrap();
        red.placement = WeightedVoronoiPlacementMode::Uniform;
        let first_distribution = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbRed).unwrap(),
            red,
            &CancellationToken::new(),
        )
        .unwrap();
        let second_distribution = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            second.field(OutputChannelId::RgbRed).unwrap(),
            red,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(first_distribution.points, second_distribution.points);
        let green = configured
            .channel_settings_mut(OutputChannelId::RgbGreen)
            .unwrap();
        green.placement = WeightedVoronoiPlacementMode::Uniform;
        green.arrangement = WeightedVoronoiArrangementPolicy::Shared;
        let shared = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbGreen).unwrap(),
            green,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(first_distribution.points, shared.points);
        green.arrangement = WeightedVoronoiArrangementPolicy::Independent;
        let independent = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbGreen).unwrap(),
            green,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_ne!(first_distribution.points, independent.points);
    }

    #[test]
    fn geometry_only_controls_preserve_distribution_fingerprints() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let fields = fields(&source);
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };
        let first = generate_weighted_voronoi_cancellable(
            domain,
            &settings(24),
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let mut geometry_only = settings(24);
        for channel in OutputChannelId::RGB {
            geometry_only
                .channel_settings_mut(channel)
                .unwrap()
                .boundary_gap = 8.0;
        }
        let second = generate_weighted_voronoi_cancellable(
            domain,
            &geometry_only,
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        for channel in OutputChannelId::RGB {
            let first_metadata = first
                .cache_metadata
                .iter()
                .find(|entry| entry.channel == channel)
                .unwrap();
            let second_metadata = second
                .cache_metadata
                .iter()
                .find(|entry| entry.channel == channel)
                .unwrap();
            assert_eq!(
                first_metadata.distribution_fingerprint, second_metadata.distribution_fingerprint,
                "boundary-gap-only changes must not alter site distribution"
            );
        }
    }

    #[test]
    fn canonical_preview_png_svg_share_cells_without_a_perimeter_border() {
        let source = SourceArtwork {
            name: "weighted.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from({
                let image = RgbaImage::from_fn(40, 24, |x, _| Rgba([(x * 6) as u8, 0, 255, 255]));
                let mut bytes = Cursor::new(Vec::new());
                image.write_to(&mut bytes, ImageFormat::Png).unwrap();
                bytes.into_inner()
            }),
        };
        let mut editor = DocumentEditor::new(Document::new(source));
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert!(editor.select_pattern(crate::pattern::PatternId::WEIGHTED_VORONOI_V1));
        let mut configured = settings(16);
        configured
            .channel_settings_mut(OutputChannelId::RgbRed)
            .unwrap()
            .enabled = true;
        assert!(editor.set_artwork_pipeline(rgb_pipeline()));
        assert!(editor.set_weighted_voronoi_settings(configured));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("weighted.toniator");
        save_document_atomic(&path, editor.document()).unwrap();
        let reopened = load_document(&path).unwrap();
        assert_eq!(
            reopened.pattern_state.selected_pattern_id(),
            Some(crate::pattern::PatternId::WEIGHTED_VORONOI_V1)
        );
        let output = generate_document_pattern_output(editor.document()).unwrap();
        let preview = render_canonical_pattern_output_cancellable(
            &output,
            40,
            24,
            false,
            None,
            &CancellationToken::new(),
        )
        .unwrap();
        let png = canonical_pattern_png_bytes(&output, 40, 24, false, None).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(preview, decoded);
        let svg = String::from_utf8(canonical_pattern_svg_bytes(&output, "weighted.png").unwrap())
            .unwrap();
        assert!(!svg.contains("fill-rule=\"evenodd\""));
        assert!(!svg.contains("-subtract\""));
        assert!(!svg.contains("stroke-width=\"0.5\""));
    }

    #[test]
    fn live_cmyk_recipe_output_is_deterministic_across_preview_png_and_svg() {
        let source = SourceArtwork {
            name: "weighted-cmyk.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from({
                let image = RgbaImage::from_fn(40, 24, |x, y| {
                    Rgba([(x * 6) as u8, (y * 9) as u8, 255 - (x * 4) as u8, 255])
                });
                let mut bytes = Cursor::new(Vec::new());
                image.write_to(&mut bytes, ImageFormat::Png).unwrap();
                bytes.into_inner()
            }),
        };
        let mut editor = DocumentEditor::new(Document::new(source));
        assert!(editor.select_pattern(crate::pattern::PatternId::WEIGHTED_VORONOI_V1));
        let mut configured = editor
            .document()
            .pattern_state
            .weighted_voronoi_settings()
            .unwrap();
        for channel in OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB)
        {
            let channel_settings = configured.channel_settings_mut(channel).unwrap();
            channel_settings.enabled = channel.belongs_to(OutputModel::CmykPrint);
            channel_settings.cell_count = 12;
            channel_settings.seed = 0x1234_5678_9abc_def0;
            channel_settings.placement = WeightedVoronoiPlacementMode::Uniform;
            channel_settings.response_strength = 0.0;
            channel_settings.boundary_gap = 1.0;
        }
        assert!(editor.set_weighted_voronoi_settings(configured));

        let output = generate_document_pattern_output(editor.document()).unwrap();
        assert_eq!(
            output,
            generate_document_pattern_output(editor.document()).unwrap(),
            "live CMYK recipe output must be deterministic"
        );
        let preview = render_canonical_pattern_output_cancellable(
            &output,
            40,
            24,
            false,
            None,
            &CancellationToken::new(),
        )
        .unwrap();
        let png = canonical_pattern_png_bytes(&output, 40, 24, false, None).unwrap();
        assert_eq!(
            preview,
            image::load_from_memory(&png).unwrap().to_rgba8(),
            "PNG must consume the same live CMYK canonical output"
        );
        let svg =
            String::from_utf8(canonical_pattern_svg_bytes(&output, "weighted-cmyk.png").unwrap())
                .unwrap();
        assert_eq!(svg.matches("mix-blend-mode:multiply").count(), 4);
        assert!(!svg.contains("<mask "));
        assert!(!svg.contains("fill-rule=\"evenodd\""));
    }
}
