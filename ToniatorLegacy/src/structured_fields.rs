//! Reusable native operations for deterministic Structured Fields.
//!
//! A structured field emits finite continuous paths in local field space. The
//! generic emitter owns canonical path conversion; individual field operations
//! do not select documents, mutate GTK state, or branch render/export paths.

use crate::cancel::CancellationToken;
use crate::curve_render::{
    CurveGeometry, CurveInkLayer, VariablePoint, outline_from_variable_points,
};
use crate::model::parse_hex_color;
use crate::parametric_paths::ParametricPathPoint;
use crate::pattern::{ArtboardSpace, CanonicalPatternOutput, PathPatternOutput};
use crate::pattern_definition::{
    LiteralValue, NativeRecipeOperationError, NativeRecipeOperationRegistry, PatternDefinition,
    PatternInstanceParameters, REGISTERED_OPERATIONS, RecipeExecutionContext, RecipeExecutionError,
    RecipeOperationInputs, RecipeOperationParameters, RecipeRuntimeValue,
    RegisteredNativeRecipeOperation,
};
use crate::render::{Channel, InkLayer};

pub const WAVE_LINE_FIELD_OPERATION_ID: &str = "structured-fields.wave-line-field";
pub const STRUCTURED_FIELD_SOURCE_WIDTH_OPERATION_ID: &str = "structured-fields.source-width";
pub const STRUCTURED_FIELD_EMIT_PATHS_OPERATION_ID: &str = "structured-fields.emit-paths";
const MAX_DOCUMENT_DISTANCE: f64 = 100_000.0;
const MAX_PATHS: usize = 20_000;
const MAX_POINTS: usize = 1_000_000;
const FIXED_SAMPLE_DISTANCE: f64 = 2.0;

/// Native-only collection of distinct continuous paths sharing one artboard.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredFieldPaths {
    pub artboard: ArtboardSpace,
    pub paths: Vec<Vec<ParametricPathPoint>>,
    pub widths: Vec<Vec<f64>>,
}

/// Local parallel lines displaced sinusoidally. `orientation_degrees` is the
/// authored bearing of the lines; a second rotation is omitted because it
/// would be mathematically identical duplicate authority for this field.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveLineFieldParameters {
    pub line_spacing: f64,
    pub orientation_degrees: f64,
    pub amplitude: f64,
    pub wavelength: f64,
    pub phase_degrees: f64,
    pub edge_overscan: f64,
}

impl Default for WaveLineFieldParameters {
    fn default() -> Self {
        Self {
            line_spacing: 24.0,
            orientation_degrees: 0.0,
            amplitude: 6.0,
            wavelength: 96.0,
            phase_degrees: 0.0,
            edge_overscan: 24.0,
        }
    }
}

impl WaveLineFieldParameters {
    fn validate(&self) -> Result<(), NativeRecipeOperationError> {
        let values = [
            self.line_spacing,
            self.orientation_degrees,
            self.amplitude,
            self.wavelength,
            self.phase_degrees,
            self.edge_overscan,
        ];
        if !values.into_iter().all(f64::is_finite)
            || !(4.0..=MAX_DOCUMENT_DISTANCE).contains(&self.line_spacing)
            || !(0.0..=MAX_DOCUMENT_DISTANCE).contains(&self.amplitude)
            || !(4.0..=MAX_DOCUMENT_DISTANCE).contains(&self.wavelength)
            || !(-360.0..=360.0).contains(&self.orientation_degrees)
            || !(-360.0..=360.0).contains(&self.phase_degrees)
            || !(0.0..=MAX_DOCUMENT_DISTANCE).contains(&self.edge_overscan)
        {
            return Err(NativeRecipeOperationError::new(
                "wave line field parameter is outside its declared bounds",
            ));
        }
        if self.amplitude * 2.0 >= self.line_spacing {
            return Err(NativeRecipeOperationError::new(
                "wave amplitude must remain less than half the line spacing to preserve distinct paths",
            ));
        }
        Ok(())
    }

    fn from_operation_parameters(
        values: &RecipeOperationParameters<'_>,
    ) -> Result<Self, NativeRecipeOperationError> {
        let number = |key| required_number("wave line field", values, key);
        let parameters = Self {
            line_spacing: number("line-spacing")?,
            orientation_degrees: number("orientation-degrees")?,
            amplitude: number("amplitude")?,
            wavelength: number("wavelength")?,
            phase_degrees: number("phase-degrees")?,
            edge_overscan: number("edge-overscan")?,
        };
        parameters.validate()?;
        Ok(parameters)
    }
}

/// Generates full-corner coverage using local lines from the artboard's
/// circumcircle. Explicit overscan adds coverage margin rather than relying on
/// later crop behavior. Sampling is fixed at two document units;
/// it is intentionally not exposed as a raw creator constant.
pub fn generate_wave_line_field(
    parameters: &WaveLineFieldParameters,
    artboard: ArtboardSpace,
    cancellation: &CancellationToken,
) -> Result<StructuredFieldPaths, NativeRecipeOperationError> {
    parameters.validate()?;
    artboard
        .validate()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    cancellation
        .checkpoint()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    let half_diagonal = (f64::from(artboard.width) * 0.5).hypot(f64::from(artboard.height) * 0.5);
    let radius = half_diagonal + parameters.edge_overscan + parameters.amplitude;
    let count = ((radius * 2.0) / parameters.line_spacing).ceil() as usize + 1;
    if count > MAX_PATHS {
        return Err(NativeRecipeOperationError::new(
            "wave line field exceeds the bounded path limit",
        ));
    }
    let samples = ((radius * 2.0) / FIXED_SAMPLE_DISTANCE).ceil() as usize + 1;
    if count.saturating_mul(samples) > MAX_POINTS {
        return Err(NativeRecipeOperationError::new(
            "wave line field exceeds the bounded sample limit",
        ));
    }
    let angle = parameters.orientation_degrees.to_radians();
    let (cosine, sine) = (angle.cos(), angle.sin());
    let phase = parameters.phase_degrees.to_radians();
    let center_x = f64::from(artboard.width) * 0.5;
    let center_y = f64::from(artboard.height) * 0.5;
    let mut paths = Vec::with_capacity(count);
    let mut widths = Vec::with_capacity(count);
    for line in 0..count {
        if line % 32 == 0 {
            cancellation
                .checkpoint()
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
        }
        let base_y = -radius + line as f64 * parameters.line_spacing;
        let mut points = Vec::with_capacity(samples);
        for sample in 0..samples {
            if sample % 256 == 0 {
                cancellation
                    .checkpoint()
                    .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            }
            let x = -radius + (radius * 2.0) * sample as f64 / (samples - 1) as f64;
            let y = base_y
                + parameters.amplitude
                    * ((std::f64::consts::TAU * x / parameters.wavelength) + phase).sin();
            points.push(ParametricPathPoint {
                x: center_x + x * cosine - y * sine,
                y: center_y + x * sine + y * cosine,
            });
        }
        widths.push(vec![1.0; points.len()]);
        paths.push(points);
    }
    Ok(StructuredFieldPaths {
        artboard,
        paths,
        widths,
    })
}

pub static STRUCTURED_FIELDS_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 3] = [
    RegisteredNativeRecipeOperation {
        id: WAVE_LINE_FIELD_OPERATION_ID,
        version: 1,
        execute: wave_line_field_operation,
    },
    RegisteredNativeRecipeOperation {
        id: STRUCTURED_FIELD_SOURCE_WIDTH_OPERATION_ID,
        version: 1,
        execute: structured_field_source_width_operation,
    },
    RegisteredNativeRecipeOperation {
        id: STRUCTURED_FIELD_EMIT_PATHS_OPERATION_ID,
        version: 1,
        execute: structured_field_emit_paths_operation,
    },
];

pub static STRUCTURED_FIELDS_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::new(
        REGISTERED_OPERATIONS.entries(),
        &STRUCTURED_FIELDS_NATIVE_OPERATIONS,
    );

pub fn execute_structured_fields_definition_cancellable(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    context: &RecipeExecutionContext<'_>,
) -> Result<CanonicalPatternOutput, RecipeExecutionError> {
    definition.execute_recipe(
        instance,
        context,
        &STRUCTURED_FIELDS_NATIVE_OPERATION_REGISTRY,
    )
}

fn wave_line_field_operation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    values: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    if !inputs.is_empty() {
        return Err(NativeRecipeOperationError::new(
            "wave line field accepts no runtime inputs",
        ));
    }
    Ok(RecipeRuntimeValue::StructuredFieldPaths(
        generate_wave_line_field(
            &WaveLineFieldParameters::from_operation_parameters(values)?,
            context.artboard,
            context.cancellation,
        )?,
    ))
}

fn structured_field_emit_paths_operation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    values: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    let paths = match inputs.get("paths") {
        Some(RecipeRuntimeValue::StructuredFieldPaths(paths)) => paths,
        _ => {
            return Err(NativeRecipeOperationError::new(
                "structured field emitter requires structured field paths",
            ));
        }
    };
    if inputs.len() != 1 || paths.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "structured field emitter input does not match execution context",
        ));
    }
    let enabled = required_boolean("structured field emitter", values, "enabled")?;
    let color = match required_literal("structured field emitter", values, "color")? {
        LiteralValue::Text(value) => parse_hex_color(value).ok_or_else(|| {
            NativeRecipeOperationError::new(
                "structured field emitter color must be a six-digit hex color",
            )
        })?,
        _ => {
            return Err(NativeRecipeOperationError::new(
                "structured field emitter color must be text",
            ));
        }
    };
    let opacity = required_number("structured field emitter", values, "opacity")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(NativeRecipeOperationError::new(
            "structured field emitter opacity must be between zero and one",
        ));
    }
    let channel = context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new("structured field emitter requires an output channel")
    })?;
    let mut outlines = Vec::with_capacity(paths.paths.len());
    if enabled {
        for (index, path) in paths.paths.iter().enumerate() {
            if index % 32 == 0 {
                context
                    .cancellation
                    .checkpoint()
                    .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            }
            let widths = paths.widths.get(index).ok_or_else(|| {
                NativeRecipeOperationError::new("structured field widths do not match paths")
            })?;
            if widths.len() != path.len() {
                return Err(NativeRecipeOperationError::new(
                    "structured field widths do not match path points",
                ));
            }
            let points = path
                .iter()
                .zip(widths)
                .map(|(point, width)| VariablePoint {
                    x: point.x,
                    y: point.y,
                    width: *width,
                })
                .collect::<Vec<_>>();
            outlines.push(outline_from_variable_points(&points, false).ok_or_else(|| {
                NativeRecipeOperationError::new(
                    "structured field path requires at least two distinct points",
                )
            })?);
        }
    }
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Paths(PathPatternOutput {
            geometry: CurveGeometry {
                width: paths.artboard.width,
                height: paths.artboard.height,
                layers: vec![CurveInkLayer {
                    layer: InkLayer {
                        channel: Channel::from(channel.to_legacy_ink()),
                        enabled,
                        color,
                        opacity: opacity as f32,
                    },
                    outlines,
                }],
            },
        }),
    ))
}

fn structured_field_source_width_operation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    values: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    let paths = match inputs.get("paths") {
        Some(RecipeRuntimeValue::StructuredFieldPaths(paths)) => paths,
        _ => {
            return Err(NativeRecipeOperationError::new(
                "structured field source width requires structured field paths",
            ));
        }
    };
    let minimum = required_number("structured field source width", values, "line-width-min")?;
    let maximum = required_number("structured field source width", values, "line-width-max")?;
    let influence = required_number(
        "structured field source width",
        values,
        "source-width-influence",
    )?;
    let detail = required_integer(
        "structured field source width",
        values,
        "source-sampling-detail",
    )? as u32;
    if !(0.1..=1_000.0).contains(&minimum)
        || !(minimum..=1_000.0).contains(&maximum)
        || !(0.0..=1.0).contains(&influence)
        || !(16..=512).contains(&detail)
    {
        return Err(NativeRecipeOperationError::new(
            "structured field source width parameters are outside declared bounds",
        ));
    }
    let channel = context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new("structured field source width requires an output channel")
    })?;
    let field = match context.source_field_provider {
        Some(provider) => {
            provider.resolve_source_field(channel, detail, detail, context.cancellation)?
        }
        None => context.source_field.cloned().ok_or_else(|| {
            NativeRecipeOperationError::new(
                "structured field source width requires a source provider or neutral field",
            )
        })?,
    };
    let (cols, rows) = field.dimensions();
    let mut widths = Vec::with_capacity(paths.paths.len());
    for (path_index, path) in paths.paths.iter().enumerate() {
        if path_index % 32 == 0 {
            context
                .cancellation
                .checkpoint()
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
        }
        let mut path_widths = Vec::with_capacity(path.len());
        for point in path {
            let x = (point.x / f64::from(paths.artboard.width) * f64::from(cols))
                .floor()
                .clamp(0.0, f64::from(cols - 1)) as u32;
            let y = (point.y / f64::from(paths.artboard.height) * f64::from(rows))
                .floor()
                .clamp(0.0, f64::from(rows - 1)) as u32;
            let sampled = field.values()[(y * cols + x) as usize].clamp(0.0, 1.0);
            let response = 0.5 * (1.0 - influence) + sampled * influence;
            path_widths.push(minimum + (maximum - minimum) * response);
        }
        widths.push(path_widths);
    }
    Ok(RecipeRuntimeValue::StructuredFieldPaths(
        StructuredFieldPaths {
            artboard: paths.artboard,
            paths: paths.paths.clone(),
            widths,
        },
    ))
}

fn required_literal<'a>(
    operation: &str,
    values: &'a RecipeOperationParameters<'_>,
    key: &str,
) -> Result<&'a LiteralValue, NativeRecipeOperationError> {
    values
        .get(key)
        .copied()
        .ok_or_else(|| NativeRecipeOperationError::new(format!("{operation} is missing `{key}`")))
}
fn required_number(
    operation: &str,
    values: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<f64, NativeRecipeOperationError> {
    match required_literal(operation, values, key)? {
        LiteralValue::Number(value) if value.is_finite() => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "{operation} `{key}` must be finite number"
        ))),
    }
}
fn required_boolean(
    operation: &str,
    values: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<bool, NativeRecipeOperationError> {
    match required_literal(operation, values, key)? {
        LiteralValue::Boolean(value) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "{operation} `{key}` must be boolean"
        ))),
    }
}
fn required_integer(
    operation: &str,
    values: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<u64, NativeRecipeOperationError> {
    match required_literal(operation, values, key)? {
        LiteralValue::Integer(value) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "{operation} `{key}` must be integer"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtworkPipelineSettings, CanonicalPatternOutput, DistributionField, OutputChannelId,
        execute_resolved_definition_cancellable, load_bundled_wave_line_field_definition,
    };
    use std::collections::BTreeMap;

    struct TestSourceProvider;

    impl crate::RecipeSourceFieldProvider for TestSourceProvider {
        fn resolve_source_field(
            &self,
            channel: OutputChannelId,
            columns: u32,
            rows: u32,
            cancellation: &CancellationToken,
        ) -> Result<DistributionField, NativeRecipeOperationError> {
            cancellation
                .checkpoint()
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            let value = match channel {
                OutputChannelId::RgbRed => 0.0,
                OutputChannelId::RgbGreen => 1.0,
                OutputChannelId::CmykCyan => {
                    return DistributionField::new(
                        columns,
                        rows,
                        (0..columns * rows)
                            .map(|index| (index % columns) as f64 / f64::from(columns - 1))
                            .collect(),
                    )
                    .map_err(|error| NativeRecipeOperationError::new(error.to_string()));
                }
                _ => 0.5,
            };
            DistributionField::new(columns, rows, vec![value; (columns * rows) as usize])
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))
        }
    }

    fn source_width_context<'a>(
        channel: OutputChannelId,
        provider: &'a dyn crate::RecipeSourceFieldProvider,
        cancellation: &'a CancellationToken,
    ) -> RecipeExecutionContext<'a> {
        RecipeExecutionContext {
            artboard: ArtboardSpace {
                width: 160,
                height: 120,
            },
            output_channel: Some(channel),
            source_field_provider: Some(provider),
            source_field: None,
            source_generation: 1,
            resolved_field_generation: 1,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation,
        }
    }

    fn source_width_parameters(
        minimum: f64,
        maximum: f64,
        influence: f64,
    ) -> BTreeMap<&'static str, LiteralValue> {
        BTreeMap::from([
            ("line-width-min", LiteralValue::Number(minimum)),
            ("line-width-max", LiteralValue::Number(maximum)),
            ("source-width-influence", LiteralValue::Number(influence)),
            ("source-sampling-detail", LiteralValue::Integer(16)),
        ])
    }

    fn resolved_widths(value: RecipeRuntimeValue) -> StructuredFieldPaths {
        let RecipeRuntimeValue::StructuredFieldPaths(paths) = value else {
            panic!("source width operation emits structured field paths");
        };
        paths
    }
    #[test]
    fn wave_line_field_is_deterministic_continuous_and_covers_corners() {
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 160,
            height: 120,
        };
        let first = generate_wave_line_field(&WaveLineFieldParameters::default(), artboard, &token)
            .unwrap();
        assert_eq!(
            first,
            generate_wave_line_field(&WaveLineFieldParameters::default(), artboard, &token)
                .unwrap()
        );
        assert!(first.paths.iter().all(|path| path.len() > 2));
        assert!(
            first
                .paths
                .iter()
                .any(|path| path.iter().any(|point| point.x < 0.0 && point.y < 0.0))
        );
    }
    #[test]
    fn wave_line_field_structural_parameters_are_ordinary_sensitive_values() {
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 160,
            height: 120,
        };
        let base = WaveLineFieldParameters::default();
        for adjust in [
            ("orientation", 90.0),
            ("phase", 45.0),
            ("amplitude", 8.0),
            ("wavelength", 72.0),
            ("spacing", 28.0),
        ] {
            let mut changed = base.clone();
            match adjust.0 {
                "orientation" => changed.orientation_degrees = adjust.1,
                "phase" => changed.phase_degrees = adjust.1,
                "amplitude" => changed.amplitude = adjust.1,
                "wavelength" => changed.wavelength = adjust.1,
                _ => changed.line_spacing = adjust.1,
            };
            assert_ne!(
                generate_wave_line_field(&base, artboard, &token).unwrap(),
                generate_wave_line_field(&changed, artboard, &token).unwrap()
            );
        }
    }

    #[test]
    fn source_width_operation_is_provider_aware_and_preserves_field_geometry() {
        let token = CancellationToken::new();
        let provider = TestSourceProvider;
        let paths = generate_wave_line_field(
            &WaveLineFieldParameters::default(),
            ArtboardSpace {
                width: 160,
                height: 120,
            },
            &token,
        )
        .unwrap();
        let runtime = RecipeRuntimeValue::StructuredFieldPaths(paths.clone());
        let inputs = BTreeMap::from([("paths", &runtime)]);
        let execute = |channel, influence| {
            let values = source_width_parameters(0.6, 1.4, influence);
            let values = values
                .iter()
                .map(|(key, value)| (*key, value))
                .collect::<RecipeOperationParameters<'_>>();
            resolved_widths(
                structured_field_source_width_operation(
                    &source_width_context(channel, &provider, &token),
                    &inputs,
                    &values,
                )
                .unwrap(),
            )
        };

        let uniform = execute(OutputChannelId::RgbRed, 0.0);
        let dark = execute(OutputChannelId::RgbRed, 1.0);
        let light = execute(OutputChannelId::RgbGreen, 1.0);
        let neutral = execute(OutputChannelId::RgbBlue, 1.0);
        let gradient = execute(OutputChannelId::CmykCyan, 1.0);

        for result in [&uniform, &dark, &light, &neutral, &gradient] {
            assert_eq!(result.artboard, paths.artboard);
            assert_eq!(result.paths, paths.paths);
            assert_eq!(result.widths.len(), result.paths.len());
        }
        assert!(uniform.widths.iter().flatten().all(|width| *width == 1.0));
        assert!(dark.widths.iter().flatten().all(|width| *width == 0.6));
        assert!(light.widths.iter().flatten().all(|width| *width == 1.4));
        assert!(neutral.widths.iter().flatten().all(|width| *width == 1.0));
        assert_ne!(dark.widths, light.widths);
        assert!(gradient.widths.iter().flatten().any(
            |width| (*width - 0.6).abs() > f64::EPSILON && (*width - 1.4).abs() > f64::EPSILON
        ));

        let invalid_values = source_width_parameters(1.4, 0.6, 1.0);
        let invalid = invalid_values
            .iter()
            .map(|(key, value)| (*key, value))
            .collect::<RecipeOperationParameters<'_>>();
        assert!(
            structured_field_source_width_operation(
                &source_width_context(OutputChannelId::RgbRed, &provider, &token),
                &inputs,
                &invalid,
            )
            .is_err()
        );

        for values in [
            source_width_parameters(0.6, 1.4, 1.1),
            BTreeMap::from([
                ("line-width-min", LiteralValue::Number(0.6)),
                ("line-width-max", LiteralValue::Number(1.4)),
                ("source-width-influence", LiteralValue::Number(1.0)),
                ("source-sampling-detail", LiteralValue::Integer(8)),
            ]),
        ] {
            let values = values
                .iter()
                .map(|(key, value)| (*key, value))
                .collect::<RecipeOperationParameters<'_>>();
            assert!(
                structured_field_source_width_operation(
                    &source_width_context(OutputChannelId::RgbRed, &provider, &token),
                    &inputs,
                    &values,
                )
                .is_err()
            );
        }

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_values = source_width_parameters(0.6, 1.4, 1.0);
        let values = cancelled_values
            .iter()
            .map(|(key, value)| (*key, value))
            .collect::<RecipeOperationParameters<'_>>();
        assert!(
            structured_field_source_width_operation(
                &source_width_context(OutputChannelId::RgbRed, &provider, &cancelled),
                &inputs,
                &values,
            )
            .is_err()
        );
    }

    #[test]
    fn bundled_wave_line_field_executes_through_the_generic_resolved_runtime() {
        let definition = load_bundled_wave_line_field_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let token = CancellationToken::new();
        let output = execute_resolved_definition_cancellable(
            &definition,
            &instance,
            &ArtworkPipelineSettings::default(),
            ArtboardSpace {
                width: 160,
                height: 120,
            },
            &token,
        )
        .unwrap();
        let CanonicalPatternOutput::Paths(paths) = output else {
            panic!("wave field emits paths");
        };
        assert_eq!(paths.geometry.layers.len(), OutputChannelId::CMYK.len());
        assert!(
            paths
                .geometry
                .layers
                .iter()
                .all(|layer| !layer.outlines.is_empty())
        );
    }
}
