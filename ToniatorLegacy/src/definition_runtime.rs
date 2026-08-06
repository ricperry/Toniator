//! Generic execution of a resolved declarative definition.
//!
//! This boundary deliberately resolves no display name, selector index, or
//! pattern family. It receives already-resolved definition/instance authority
//! and executes the graph through the complete finite native-operation set.

use crate::artwork_pipeline::{
    ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
    OutputChannelId, PreparedSource, resolve_channel_fields_cancellable,
};
use crate::cancel::CancellationToken;
use crate::curve_render::CurveGeometry;
use crate::parametric_paths::PARAMETRIC_PATHS_NATIVE_OPERATIONS;
use crate::pattern::{ArtboardSpace, CanonicalPatternOutput, PathPatternOutput};
use crate::pattern_definition::{
    NativeRecipeOperationError, NativeRecipeOperationRegistry, PatternDefinition,
    PatternInstanceParameters, REGISTERED_OPERATIONS, RecipeExecutionContext,
    RecipeSourceFieldProvider, RegisteredNativeRecipeOperation,
};
use crate::site_distribution::DistributionField;
use crate::structured_fields::STRUCTURED_FIELDS_NATIVE_OPERATIONS;
use crate::{
    CURVES_NATIVE_OPERATIONS, SHAPES_NATIVE_OPERATIONS, WEIGHTED_VORONOI_NATIVE_OPERATIONS,
};
use std::cell::RefCell;
use std::collections::HashMap;

/// Every currently registered native operation, exposed as one bounded
/// registry for resolved definition execution. Individual compatibility
/// adapters retain their existing specialized orchestration/preflight seams.
pub static RESOLVED_DEFINITION_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 26] = [
    SHAPES_NATIVE_OPERATIONS[0],
    SHAPES_NATIVE_OPERATIONS[1],
    SHAPES_NATIVE_OPERATIONS[2],
    SHAPES_NATIVE_OPERATIONS[3],
    SHAPES_NATIVE_OPERATIONS[4],
    SHAPES_NATIVE_OPERATIONS[5],
    SHAPES_NATIVE_OPERATIONS[6],
    SHAPES_NATIVE_OPERATIONS[7],
    SHAPES_NATIVE_OPERATIONS[8],
    CURVES_NATIVE_OPERATIONS[0],
    CURVES_NATIVE_OPERATIONS[1],
    CURVES_NATIVE_OPERATIONS[2],
    CURVES_NATIVE_OPERATIONS[3],
    CURVES_NATIVE_OPERATIONS[4],
    CURVES_NATIVE_OPERATIONS[5],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[0],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[1],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[2],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[3],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[4],
    WEIGHTED_VORONOI_NATIVE_OPERATIONS[5],
    PARAMETRIC_PATHS_NATIVE_OPERATIONS[0],
    PARAMETRIC_PATHS_NATIVE_OPERATIONS[1],
    STRUCTURED_FIELDS_NATIVE_OPERATIONS[0],
    STRUCTURED_FIELDS_NATIVE_OPERATIONS[1],
    STRUCTURED_FIELDS_NATIVE_OPERATIONS[2],
];

pub static RESOLVED_DEFINITION_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::new(
        REGISTERED_OPERATIONS.entries(),
        &RESOLVED_DEFINITION_NATIVE_OPERATIONS,
    );

/// Executes an already-resolved definition once for every semantic output
/// channel and combines same-kind canonical path layers. The definition's
/// graph, not a pattern identity or family, chooses its native operations.
pub fn execute_resolved_definition_cancellable(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    pipeline: &ArtworkPipelineSettings,
    artboard: ArtboardSpace,
    cancellation: &CancellationToken,
) -> anyhow::Result<CanonicalPatternOutput> {
    execute_resolved_definition_with_source_cancellable(
        definition,
        instance,
        pipeline,
        artboard,
        None,
        cancellation,
    )
}

/// Generic resolved execution with an optional prepared artwork source. A
/// missing source gets a deterministic neutral field, so source-aware graphs
/// still execute their declared operation during local draft preview.
pub fn execute_resolved_definition_with_source_cancellable(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    pipeline: &ArtworkPipelineSettings,
    artboard: ArtboardSpace,
    prepared: Option<&PreparedSource>,
    cancellation: &CancellationToken,
) -> anyhow::Result<CanonicalPatternOutput> {
    let provider = ResolvedDefinitionSourceProvider {
        prepared,
        pipeline,
        cache: RefCell::new(HashMap::new()),
    };
    let channels = pipeline.output_model.channels();
    let mut outputs = Vec::with_capacity(channels.len());
    for (index, channel) in channels.iter().enumerate() {
        cancellation.checkpoint()?;
        let context = RecipeExecutionContext {
            artboard,
            output_channel: Some(*channel),
            source_field_provider: Some(&provider),
            source_field: None,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: index as u32,
            enabled_layer_index: index as u32,
            definition_assets: &definition.assets,
            cancellation,
        };
        outputs.push(definition.execute_recipe(
            instance,
            &context,
            &RESOLVED_DEFINITION_NATIVE_OPERATION_REGISTRY,
        )?);
    }
    combine_canonical_outputs(outputs)
}

struct ResolvedDefinitionSourceProvider<'a> {
    prepared: Option<&'a PreparedSource>,
    pipeline: &'a ArtworkPipelineSettings,
    cache: RefCell<HashMap<(OutputChannelId, u32, u32, u64), DistributionField>>,
}

impl RecipeSourceFieldProvider for ResolvedDefinitionSourceProvider<'_> {
    fn resolve_source_field(
        &self,
        channel: OutputChannelId,
        columns: u32,
        rows: u32,
        cancellation: &CancellationToken,
    ) -> Result<DistributionField, NativeRecipeOperationError> {
        let generation = self.prepared.map(|source| source.generation).unwrap_or(0);
        let key = (channel, columns, rows, generation);
        if let Some(field) = self.cache.borrow().get(&key) {
            cancellation
                .checkpoint()
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            return Ok(field.clone());
        }
        let field = if let Some(prepared) = self.prepared {
            // Recipe output channels are semantic. Resolve the requested channel
            // independently of the UI's current assignment so a definition graph
            // cannot accidentally inherit display-routing behavior. Full-color
            // artwork must retain the output-model's corresponding separation.
            let mut pipeline = self.pipeline.clone();
            pipeline.assignment = match pipeline.source {
                ArtworkSource::FullColor => {
                    let strategy = match pipeline.output_model {
                        crate::OutputModel::CmykPrint => {
                            AutomaticSeparationStrategy::CmykEncodedRgbMaxBlackV1
                        }
                        crate::OutputModel::RgbScreen => {
                            AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1
                        }
                    };
                    ChannelAssignment::automatic(strategy)
                }
                _ => ChannelAssignment::AllChannels,
            };
            let resolved = resolve_channel_fields_cancellable(
                prepared,
                &pipeline,
                columns,
                rows,
                generation,
                &[channel],
                cancellation,
            )
            .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            let field = resolved.field(channel).ok_or_else(|| {
                NativeRecipeOperationError::new(
                    "resolved source field is unavailable for semantic channel",
                )
            })?;
            DistributionField::new(
                columns,
                rows,
                field
                    .values()
                    .iter()
                    .zip(field.coverage())
                    .map(|(value, coverage)| f64::from(*value * *coverage))
                    .collect(),
            )
            .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?
        } else {
            DistributionField::new(
                columns,
                rows,
                vec![0.5; (columns as usize) * (rows as usize)],
            )
            .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?
        };
        self.cache.borrow_mut().insert(key, field.clone());
        Ok(field)
    }
}

fn combine_canonical_outputs(
    outputs: Vec<CanonicalPatternOutput>,
) -> anyhow::Result<CanonicalPatternOutput> {
    let mut outputs = outputs.into_iter();
    let Some(CanonicalPatternOutput::Paths(first)) = outputs.next() else {
        anyhow::bail!("resolved definition did not emit canonical paths");
    };
    let mut geometry = first.geometry;
    for output in outputs {
        let CanonicalPatternOutput::Paths(next) = output else {
            anyhow::bail!("resolved definition emitted inconsistent canonical output kinds");
        };
        anyhow::ensure!(
            geometry.width == next.geometry.width && geometry.height == next.geometry.height,
            "resolved definition emitted inconsistent canonical path artboards"
        );
        geometry.layers.extend(next.geometry.layers);
    }
    Ok(CanonicalPatternOutput::Paths(PathPatternOutput {
        geometry: CurveGeometry {
            width: geometry.width,
            height: geometry.height,
            layers: geometry.layers,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CancellationToken, OutputChannelId, PatternId,
        load_bundled_quadratic_radial_spiral_definition,
    };
    use image::{Rgba, RgbaImage};

    #[test]
    fn resolved_definition_execution_is_stable_for_every_declared_channel() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let token = CancellationToken::new();
        let pipeline = ArtworkPipelineSettings::default();
        let first = execute_resolved_definition_cancellable(
            &definition,
            &instance,
            &pipeline,
            ArtboardSpace {
                width: 64,
                height: 48,
            },
            &token,
        )
        .unwrap();
        let second = execute_resolved_definition_cancellable(
            &definition,
            &instance,
            &pipeline,
            ArtboardSpace {
                width: 64,
                height: 48,
            },
            &token,
        )
        .unwrap();
        assert_eq!(first, second);
        let CanonicalPatternOutput::Paths(paths) = first else {
            panic!("Spiral definition must retain its declared output kind");
        };
        assert_eq!(paths.geometry.layers.len(), OutputChannelId::CMYK.len());
        assert!(
            paths
                .geometry
                .layers
                .iter()
                .all(|layer| layer.layer.enabled)
        );
        assert_eq!(definition.id, PatternId::QUADRATIC_RADIAL_SPIRAL_V1);
    }

    #[test]
    fn source_provider_caches_by_semantic_channel_dimensions_and_generation() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let prepared = PreparedSource::from_rgba_image(&image, 7);
        let pipeline = ArtworkPipelineSettings {
            output_model: crate::OutputModel::RgbScreen,
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        };
        let provider = ResolvedDefinitionSourceProvider {
            prepared: Some(&prepared),
            pipeline: &pipeline,
            cache: RefCell::new(HashMap::new()),
        };
        let token = CancellationToken::new();
        let first = provider
            .resolve_source_field(OutputChannelId::RgbRed, 16, 16, &token)
            .unwrap();
        let second = provider
            .resolve_source_field(OutputChannelId::RgbRed, 16, 16, &token)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(provider.cache.borrow().len(), 1);
        provider
            .resolve_source_field(OutputChannelId::RgbGreen, 16, 16, &token)
            .unwrap();
        provider
            .resolve_source_field(OutputChannelId::RgbRed, 32, 16, &token)
            .unwrap();
        assert_eq!(provider.cache.borrow().len(), 3);
    }

    #[test]
    fn prepared_source_provider_preserves_non_source_spiral_output() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let token = CancellationToken::new();
        let pipeline = ArtworkPipelineSettings::default();
        let artboard = ArtboardSpace {
            width: 64,
            height: 48,
        };
        let neutral = execute_resolved_definition_cancellable(
            &definition,
            &instance,
            &pipeline,
            artboard,
            &token,
        )
        .unwrap();
        let source = PreparedSource::from_rgba_image(
            &RgbaImage::from_pixel(2, 2, Rgba([17, 91, 231, 255])),
            3,
        );
        let live = execute_resolved_definition_with_source_cancellable(
            &definition,
            &instance,
            &pipeline,
            artboard,
            Some(&source),
            &token,
        )
        .unwrap();
        assert_eq!(live, neutral);
    }
}
