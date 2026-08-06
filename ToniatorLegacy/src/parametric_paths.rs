//! Reusable native constructors for authored parametric paths.
//!
//! Gate 3B adds one generic typed-centerline-to-canonical-path emitter and an
//! immutable bundled recipe. This module still has no selection, UI, preview,
//! PNG, SVG, or family-specific consumer branch.

use crate::cancel::CancellationToken;
use crate::curve_render::{
    CurveGeometry, CurveInkLayer, VariablePoint, outline_from_variable_points,
};
use crate::model::parse_hex_color;
use crate::pattern::ArtboardSpace;
use crate::pattern::{CanonicalPatternOutput, PathPatternOutput};
use crate::pattern_definition::{
    AuthoringLayout, AuthoringSection, CreatorParameterCategory, CreatorParameterIncrement,
    CreatorParameterMetadata, CreatorParameterUnit, DefinitionParameterScope, LiteralValue,
    NativeRecipeOperationError, NativeRecipeOperationRegistry, ParameterApplicability,
    ParameterAuthoring, ParameterInvalidationScope, ParameterOwnership,
    ParameterSerializationBehavior, ParameterValidationBehavior, PatternDefinition,
    PatternInstanceParameters, PatternParameterConstraints, PatternParameterDefinition,
    REGISTERED_OPERATIONS, RecipeExecutionContext, RecipeExecutionError, RecipeOperationInputs,
    RecipeOperationParameters, RecipeRuntimeValue, RecipeValueType,
    RegisteredNativeRecipeOperation, TwoDimensionalAxis, TwoDimensionalRelation,
};
use crate::render::{Channel, InkLayer};

/// Stable operation identity for the first Parametric Paths generator.
pub const QUADRATIC_RADIAL_SPIRAL_OPERATION_ID: &str = "parametric-paths.quadratic-radial-spiral";
pub const QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION: u32 = 1;
/// Generic canonical emission for a typed parametric centerline.
pub const PARAMETRIC_PATH_EMIT_PATHS_OPERATION_ID: &str = "parametric-paths.emit-paths";
pub const PARAMETRIC_PATH_EMIT_PATHS_OPERATION_VERSION: u32 = 1;
/// Bounded before allocating the native-only path value.
pub const PARAMETRIC_PATHS_MAX_SAMPLES: usize = 1_000_000;
const PARAMETRIC_PATH_STROKE_WIDTH: f64 = 1.0;
const MAX_TURNS: f64 = 64.0;
const MAX_DOCUMENT_DISTANCE: f64 = 100_000.0;

/// One generated centerline point in canonical artboard coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParametricPathPoint {
    pub x: f64,
    pub y: f64,
}

/// Native-only authored centerline produced before a later recipe stage turns
/// it into canonical path, mark, or network geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ParametricPath {
    pub artboard: ArtboardSpace,
    pub points: Vec<ParametricPathPoint>,
    /// The requested authored extent, measured in revolutions.
    pub base_turns: f64,
    /// The actual generated extent after optional explicit overscan.
    pub generated_turns: f64,
    pub edge_extension: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadraticRadialSpiralDirection {
    Clockwise,
    Counterclockwise,
}

impl QuadraticRadialSpiralDirection {
    fn angular_sign(self) -> f64 {
        // Artboard Y increases downward, so increasing mathematical angle is
        // visually clockwise in Toniator's canonical coordinate system.
        match self {
            Self::Clockwise => 1.0,
            Self::Counterclockwise => -1.0,
        }
    }

    fn from_literal(value: &LiteralValue) -> Result<Self, NativeRecipeOperationError> {
        match value {
            LiteralValue::Choice(value) if value == "clockwise" => Ok(Self::Clockwise),
            LiteralValue::Choice(value) if value == "counterclockwise" => {
                Ok(Self::Counterclockwise)
            }
            _ => Err(NativeRecipeOperationError::new(
                "direction must be clockwise or counterclockwise",
            )),
        }
    }
}

/// Parameters for a generalized quadratic-radial spiral, using revolutions as
/// `u`. When `spacing_growth_per_revolution` is zero, this is the
/// Archimedean specialization.
///
/// `r(u) = r0 + s*u + 0.5*g*u^2`
/// `theta(u) = theta0 + d*2*pi*u`
///
/// where `d` is +1 for clockwise and -1 for counterclockwise in canonical
/// artboard coordinates. `s + g*u` is constrained non-negative over the full
/// generated interval, so radius never folds back over an earlier turn.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadraticRadialSpiralParameters {
    pub turns: f64,
    pub starting_radius: f64,
    pub radial_growth_per_revolution: f64,
    pub spacing_growth_per_revolution: f64,
    pub starting_angle_degrees: f64,
    pub direction: QuadraticRadialSpiralDirection,
    /// Offsets from the execution-context artboard center, in canonical
    /// document/artboard units.
    pub center_x: f64,
    pub center_y: f64,
    /// Maximum centerline distance between adjacent generated points, in
    /// canonical document/artboard units.
    pub maximum_sample_distance: f64,
    /// Append enough path beyond `turns` to reach `edge_overscan` additional
    /// radial distance. Later canonical consumers own clipping.
    pub edge_extension: bool,
    pub edge_overscan: f64,
}

impl Default for QuadraticRadialSpiralParameters {
    fn default() -> Self {
        Self {
            // On the checkpoint's common 640 x 480 artboard this reproduces
            // the former 20-unit pitch and reaches the 400-unit corner radius.
            turns: 20.0,
            starting_radius: 0.0,
            radial_growth_per_revolution: 20.0,
            spacing_growth_per_revolution: 0.0,
            starting_angle_degrees: 0.0,
            direction: QuadraticRadialSpiralDirection::Clockwise,
            center_x: 0.0,
            center_y: 0.0,
            maximum_sample_distance: 4.0,
            edge_extension: true,
            edge_overscan: 20.0,
        }
    }
}

impl QuadraticRadialSpiralParameters {
    pub fn validate(&self) -> Result<(), NativeRecipeOperationError> {
        let finite = [
            self.turns,
            self.starting_radius,
            self.radial_growth_per_revolution,
            self.spacing_growth_per_revolution,
            self.starting_angle_degrees,
            self.center_x,
            self.center_y,
            self.maximum_sample_distance,
            self.edge_overscan,
        ]
        .into_iter()
        .all(f64::is_finite);
        if !finite {
            return Err(NativeRecipeOperationError::new(
                "quadratic-radial spiral parameters must be finite",
            ));
        }
        if !(0.25..=MAX_TURNS).contains(&self.turns)
            || !(0.0..=MAX_DOCUMENT_DISTANCE).contains(&self.starting_radius)
            || !(0.0..=MAX_DOCUMENT_DISTANCE).contains(&self.radial_growth_per_revolution)
            || !(-MAX_DOCUMENT_DISTANCE..=MAX_DOCUMENT_DISTANCE)
                .contains(&self.spacing_growth_per_revolution)
            || !(-360.0..=360.0).contains(&self.starting_angle_degrees)
            || !(-MAX_DOCUMENT_DISTANCE..=MAX_DOCUMENT_DISTANCE).contains(&self.center_x)
            || !(-MAX_DOCUMENT_DISTANCE..=MAX_DOCUMENT_DISTANCE).contains(&self.center_y)
            || !(0.01..=MAX_DOCUMENT_DISTANCE).contains(&self.maximum_sample_distance)
            || !(0.0..=MAX_DOCUMENT_DISTANCE).contains(&self.edge_overscan)
        {
            return Err(NativeRecipeOperationError::new(
                "quadratic-radial spiral parameter is outside the declared bounds",
            ));
        }
        let end_growth =
            self.radial_growth_per_revolution + self.spacing_growth_per_revolution * self.turns;
        if end_growth < 0.0
            || (self.radial_growth_per_revolution == 0.0
                && self.spacing_growth_per_revolution <= 0.0)
        {
            return Err(NativeRecipeOperationError::new(
                "quadratic-radial spiral growth must remain positive over its turns",
            ));
        }
        Ok(())
    }

    fn radius_at(&self, turns: f64) -> f64 {
        self.starting_radius
            + self.radial_growth_per_revolution * turns
            + 0.5 * self.spacing_growth_per_revolution * turns * turns
    }

    fn radial_derivative_at(&self, turns: f64) -> f64 {
        self.radial_growth_per_revolution + self.spacing_growth_per_revolution * turns
    }

    fn generated_turns(&self, artboard: ArtboardSpace) -> Result<f64, NativeRecipeOperationError> {
        if !self.edge_extension {
            return Ok(self.turns);
        }
        let center_x = f64::from(artboard.width) * 0.5 + self.center_x;
        let center_y = f64::from(artboard.height) * 0.5 + self.center_y;
        let farthest_corner_radius = [
            (0.0, 0.0),
            (f64::from(artboard.width), 0.0),
            (0.0, f64::from(artboard.height)),
            (f64::from(artboard.width), f64::from(artboard.height)),
        ]
        .into_iter()
        .map(|(x, y)| (x - center_x).hypot(y - center_y))
        .fold(0.0, f64::max);
        let target_radius = self
            .radius_at(self.turns)
            .max(farthest_corner_radius + self.edge_overscan);
        let maximum_valid_turns = if self.spacing_growth_per_revolution < 0.0 {
            self.radial_growth_per_revolution / -self.spacing_growth_per_revolution
        } else {
            MAX_TURNS * 4.0
        };
        if target_radius > self.radius_at(maximum_valid_turns) {
            return Err(NativeRecipeOperationError::new(
                "quadratic-radial spiral edge extension would reverse radial growth",
            ));
        }
        let mut low = self.turns;
        let mut high = self.turns.max(1.0).min(maximum_valid_turns);
        while self.radius_at(high) < target_radius {
            high = (high * 2.0).min(maximum_valid_turns);
            if high == maximum_valid_turns && self.radius_at(high) < target_radius {
                return Err(NativeRecipeOperationError::new(
                    "quadratic-radial spiral edge overscan exceeds the bounded path extent",
                ));
            }
        }
        for _ in 0..64 {
            let middle = (low + high) * 0.5;
            if self.radius_at(middle) < target_radius {
                low = middle;
            } else {
                high = middle;
            }
        }
        Ok((low + high) * 0.5)
    }

    fn from_operation_parameters(
        parameters: &RecipeOperationParameters<'_>,
    ) -> Result<Self, NativeRecipeOperationError> {
        Ok(Self {
            turns: required_number(parameters, "turns")?,
            starting_radius: required_number(parameters, "starting-radius")?,
            radial_growth_per_revolution: required_number(
                parameters,
                "radial-growth-per-revolution",
            )?,
            spacing_growth_per_revolution: required_number(
                parameters,
                "spacing-growth-per-revolution",
            )?,
            starting_angle_degrees: required_number(parameters, "starting-angle-degrees")?,
            direction: QuadraticRadialSpiralDirection::from_literal(required_literal(
                parameters,
                "direction",
            )?)?,
            center_x: required_number(parameters, "center-x")?,
            center_y: required_number(parameters, "center-y")?,
            maximum_sample_distance: required_number(parameters, "maximum-sample-distance")?,
            edge_extension: required_boolean(parameters, "edge-extension")?,
            edge_overscan: required_number(parameters, "edge-overscan")?,
        })
    }
}

/// Creator-facing Gate 2 metadata for a future immutable Parametric Paths
/// recipe. This creates no definition or consumer in Gate 3A.
pub fn quadratic_radial_spiral_parameter_definitions() -> Vec<PatternParameterDefinition> {
    let creator = |category: CreatorParameterCategory,
                   unit: CreatorParameterUnit,
                   increment: CreatorParameterIncrement,
                   precision: u8,
                   group: &str,
                   display_order: u32,
                   applicability: ParameterApplicability,
                   two_dimensional: Option<TwoDimensionalRelation>| {
        ParameterAuthoring::Creator(CreatorParameterMetadata {
            category,
            unit,
            increment,
            precision,
            group: group.into(),
            display_order,
            ownership: ParameterOwnership::PatternDefinition,
            required: true,
            applicability,
            validation: ParameterValidationBehavior::Strict,
            serialization: ParameterSerializationBehavior::Always,
            invalidation: ParameterInvalidationScope::Geometry,
            two_dimensional,
        })
    };
    let number =
        |key: &str,
         label: &str,
         help: &str,
         default: f64,
         minimum: f64,
         maximum: f64,
         step: f64,
         category: CreatorParameterCategory,
         unit: CreatorParameterUnit,
         precision: u8,
         group: &str,
         display_order: u32,
         applicability: ParameterApplicability,
         two_dimensional: Option<TwoDimensionalRelation>| PatternParameterDefinition {
            key: key.into(),
            label: label.into(),
            help: help.into(),
            scope: DefinitionParameterScope::Pattern,
            value_type: RecipeValueType::Number,
            default: LiteralValue::Number(default),
            constraints: PatternParameterConstraints::Number {
                minimum,
                maximum,
                step,
            },
            choices: vec![],
            authoring: creator(
                category,
                unit,
                CreatorParameterIncrement::Number(step),
                precision,
                group,
                display_order,
                applicability,
                two_dimensional,
            ),
        };
    let defaults = QuadraticRadialSpiralParameters::default();
    vec![
        number(
            "turns",
            "Turns",
            "Base path extent in complete revolutions.",
            defaults.turns,
            0.25,
            MAX_TURNS,
            0.25,
            CreatorParameterCategory::BoundedNumber,
            CreatorParameterUnit::Unitless,
            2,
            "geometry",
            0,
            ParameterApplicability::Always,
            None,
        ),
        number(
            "starting-radius",
            "Starting Radius",
            "Radius at the center before the first revolution.",
            defaults.starting_radius,
            0.0,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::DocumentRelativeDistance,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "geometry",
            1,
            ParameterApplicability::Always,
            None,
        ),
        number(
            "radial-growth-per-revolution",
            "Radial Growth per Revolution",
            "Base winding spacing added for each revolution.",
            defaults.radial_growth_per_revolution,
            0.0,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::DocumentRelativeDistance,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "geometry",
            2,
            ParameterApplicability::Always,
            None,
        ),
        number(
            "spacing-growth-per-revolution",
            "Spacing Growth per Revolution",
            "Additional radial growth per revolution; it contributes 0.5 times this value times turns squared.",
            defaults.spacing_growth_per_revolution,
            -MAX_DOCUMENT_DISTANCE,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::DocumentRelativeDistance,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "geometry",
            3,
            ParameterApplicability::Always,
            None,
        ),
        number(
            "starting-angle-degrees",
            "Starting Angle",
            "Angle at the first path point, in canonical artboard degrees.",
            defaults.starting_angle_degrees,
            -360.0,
            360.0,
            1.0,
            CreatorParameterCategory::Angle,
            CreatorParameterUnit::Degrees,
            0,
            "orientation",
            0,
            ParameterApplicability::Always,
            None,
        ),
        PatternParameterDefinition {
            key: "direction".into(),
            label: "Direction".into(),
            help: "Wind clockwise or counterclockwise in artboard coordinates.".into(),
            scope: DefinitionParameterScope::Pattern,
            value_type: RecipeValueType::Choice,
            default: LiteralValue::Choice("clockwise".into()),
            constraints: PatternParameterConstraints::Choice,
            choices: vec!["clockwise".into(), "counterclockwise".into()],
            authoring: creator(
                CreatorParameterCategory::Enumeration,
                CreatorParameterUnit::None,
                CreatorParameterIncrement::None,
                0,
                "orientation",
                1,
                ParameterApplicability::Always,
                None,
            ),
        },
        number(
            "center-x",
            "Center X",
            "Horizontal offset from the execution artboard center.",
            defaults.center_x,
            -MAX_DOCUMENT_DISTANCE,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::TwoDimensionalOffset,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "center",
            0,
            ParameterApplicability::Always,
            Some(TwoDimensionalRelation {
                pair_id: "center".into(),
                axis: TwoDimensionalAxis::X,
            }),
        ),
        number(
            "center-y",
            "Center Y",
            "Vertical offset from the execution artboard center.",
            defaults.center_y,
            -MAX_DOCUMENT_DISTANCE,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::TwoDimensionalOffset,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "center",
            1,
            ParameterApplicability::Always,
            Some(TwoDimensionalRelation {
                pair_id: "center".into(),
                axis: TwoDimensionalAxis::Y,
            }),
        ),
        number(
            "maximum-sample-distance",
            "Maximum Sample Distance",
            "Maximum centerline distance between generated points in canonical artboard units.",
            defaults.maximum_sample_distance,
            0.01,
            MAX_DOCUMENT_DISTANCE,
            0.01,
            CreatorParameterCategory::DocumentRelativeDistance,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "sampling",
            0,
            ParameterApplicability::Always,
            None,
        ),
        PatternParameterDefinition {
            key: "edge-extension".into(),
            label: "Extend Beyond Edge".into(),
            help: "Append path outside the requested turns so a later clipped consumer can retain edge coverage.".into(),
            scope: DefinitionParameterScope::Pattern,
            value_type: RecipeValueType::Boolean,
            default: LiteralValue::Boolean(defaults.edge_extension),
            constraints: PatternParameterConstraints::Boolean,
            choices: vec![],
            authoring: creator(
                CreatorParameterCategory::Boolean,
                CreatorParameterUnit::None,
                CreatorParameterIncrement::None,
                0,
                "coverage",
                0,
                ParameterApplicability::Always,
                None,
            ),
        },
        number(
            "edge-overscan",
            "Edge Overscan",
            "Additional radial distance appended when edge extension is enabled.",
            defaults.edge_overscan,
            0.0,
            MAX_DOCUMENT_DISTANCE,
            0.25,
            CreatorParameterCategory::DocumentRelativeDistance,
            CreatorParameterUnit::DocumentRelativeDistance,
            2,
            "coverage",
            1,
            ParameterApplicability::WhenParameterEquals {
                parameter: "edge-extension".into(),
                value: LiteralValue::Boolean(true),
            },
            None,
        ),
    ]
}

pub fn quadratic_radial_spiral_authoring_layout() -> AuthoringLayout {
    AuthoringLayout {
        sections: vec![
            AuthoringSection {
                id: "geometry".into(),
                label: "Geometry".into(),
                parameters: vec![
                    "turns".into(),
                    "starting-radius".into(),
                    "radial-growth-per-revolution".into(),
                    "spacing-growth-per-revolution".into(),
                ],
            },
            AuthoringSection {
                id: "orientation".into(),
                label: "Orientation".into(),
                parameters: vec!["starting-angle-degrees".into(), "direction".into()],
            },
            AuthoringSection {
                id: "center".into(),
                label: "Center".into(),
                parameters: vec!["center-x".into(), "center-y".into()],
            },
            AuthoringSection {
                id: "sampling".into(),
                label: "Sampling".into(),
                parameters: vec!["maximum-sample-distance".into()],
            },
            AuthoringSection {
                id: "coverage".into(),
                label: "Coverage".into(),
                parameters: vec!["edge-extension".into(), "edge-overscan".into()],
            },
        ],
        node_positions: Default::default(),
    }
}

pub static PARAMETRIC_PATHS_NATIVE_OPERATIONS: [RegisteredNativeRecipeOperation; 2] = [
    RegisteredNativeRecipeOperation {
        id: QUADRATIC_RADIAL_SPIRAL_OPERATION_ID,
        version: QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION,
        execute: quadratic_radial_spiral_operation,
    },
    RegisteredNativeRecipeOperation {
        id: PARAMETRIC_PATH_EMIT_PATHS_OPERATION_ID,
        version: PARAMETRIC_PATH_EMIT_PATHS_OPERATION_VERSION,
        execute: parametric_path_emit_paths_operation,
    },
];

pub static PARAMETRIC_PATHS_NATIVE_OPERATION_REGISTRY: NativeRecipeOperationRegistry<'static> =
    NativeRecipeOperationRegistry::new(
        REGISTERED_OPERATIONS.entries(),
        &PARAMETRIC_PATHS_NATIVE_OPERATIONS,
    );

/// Runs any validated Parametric Paths definition solely through the typed
/// operation registry. It deliberately has no bundled-id, family-display, UI,
/// renderer, or exporter branch.
pub fn execute_parametric_paths_definition_cancellable(
    definition: &PatternDefinition,
    instance: &PatternInstanceParameters,
    context: &RecipeExecutionContext<'_>,
) -> Result<CanonicalPatternOutput, RecipeExecutionError> {
    definition.execute_recipe(
        instance,
        context,
        &PARAMETRIC_PATHS_NATIVE_OPERATION_REGISTRY,
    )
}

pub fn generate_quadratic_radial_spiral(
    parameters: &QuadraticRadialSpiralParameters,
    artboard: ArtboardSpace,
    cancellation: &CancellationToken,
) -> Result<ParametricPath, NativeRecipeOperationError> {
    parameters.validate()?;
    artboard
        .validate()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    cancellation
        .checkpoint()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    let generated_turns = parameters.generated_turns(artboard)?;
    if parameters.radial_derivative_at(generated_turns) < 0.0 {
        return Err(NativeRecipeOperationError::new(
            "quadratic-radial spiral growth reverses before its generated extent",
        ));
    }
    let max_radius = parameters.radius_at(generated_turns);
    let max_radial_derivative = parameters
        .radial_derivative_at(0.0)
        .abs()
        .max(parameters.radial_derivative_at(generated_turns).abs());
    let angular_speed = std::f64::consts::TAU * max_radius;
    let speed_bound = max_radial_derivative.hypot(angular_speed);
    let segments = (generated_turns * speed_bound / parameters.maximum_sample_distance)
        .ceil()
        .max(1.0) as usize;
    if segments >= PARAMETRIC_PATHS_MAX_SAMPLES {
        return Err(NativeRecipeOperationError::new(
            "quadratic-radial spiral exceeds the bounded sample limit",
        ));
    }
    let center_x = f64::from(artboard.width) * 0.5 + parameters.center_x;
    let center_y = f64::from(artboard.height) * 0.5 + parameters.center_y;
    let start_angle = parameters.starting_angle_degrees.to_radians();
    let mut points = Vec::with_capacity(segments + 1);
    for index in 0..=segments {
        if index % 256 == 0 {
            cancellation
                .checkpoint()
                .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
        }
        let turns = generated_turns * index as f64 / segments as f64;
        let radius = parameters.radius_at(turns);
        let theta =
            start_angle + parameters.direction.angular_sign() * std::f64::consts::TAU * turns;
        points.push(ParametricPathPoint {
            x: center_x + radius * theta.cos(),
            y: center_y + radius * theta.sin(),
        });
    }
    Ok(ParametricPath {
        artboard,
        points,
        base_turns: parameters.turns,
        generated_turns,
        edge_extension: parameters.edge_extension,
    })
}

fn quadratic_radial_spiral_operation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    if !inputs.is_empty() {
        return Err(NativeRecipeOperationError::new(
            "quadratic-radial spiral accepts no runtime inputs",
        ));
    }
    let parameters = QuadraticRadialSpiralParameters::from_operation_parameters(parameters)?;
    Ok(RecipeRuntimeValue::ParametricPath(
        generate_quadratic_radial_spiral(&parameters, context.artboard, context.cancellation)?,
    ))
}

/// Converts one typed centerline into Toniator's established canonical curve
/// geometry. The graph supplies channel-instance values; this operation never
/// stores or edits them.
fn parametric_path_emit_paths_operation(
    context: &RecipeExecutionContext<'_>,
    inputs: &RecipeOperationInputs<'_>,
    parameters: &RecipeOperationParameters<'_>,
) -> Result<RecipeRuntimeValue, NativeRecipeOperationError> {
    let path = match inputs.get("path") {
        Some(RecipeRuntimeValue::ParametricPath(path)) => path,
        Some(_) => {
            return Err(NativeRecipeOperationError::new(
                "parametric path emitter input `path` has the wrong runtime type",
            ));
        }
        None => {
            return Err(NativeRecipeOperationError::new(
                "parametric path emitter is missing input `path`",
            ));
        }
    };
    if inputs.len() != 1 {
        return Err(NativeRecipeOperationError::new(
            "parametric path emitter accepts exactly one runtime input",
        ));
    }
    if path.artboard != context.artboard {
        return Err(NativeRecipeOperationError::new(
            "parametric path artboard must match execution context",
        ));
    }
    let enabled = required_boolean_for("parametric path emitter", parameters, "enabled")?;
    let color = match required_literal_for("parametric path emitter", parameters, "color")? {
        LiteralValue::Text(color) => parse_hex_color(color).ok_or_else(|| {
            NativeRecipeOperationError::new(
                "parametric path emitter color must be a six-digit hex color",
            )
        })?,
        _ => {
            return Err(NativeRecipeOperationError::new(
                "parametric path emitter `color` must be text",
            ));
        }
    };
    let opacity = required_number_for("parametric path emitter", parameters, "opacity")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(NativeRecipeOperationError::new(
            "parametric path emitter opacity must be between zero and one",
        ));
    }
    let output_channel = context.output_channel.ok_or_else(|| {
        NativeRecipeOperationError::new("parametric path emitter requires an output channel")
    })?;
    context
        .cancellation
        .checkpoint()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    let points = path
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            if index % 256 == 0 {
                context
                    .cancellation
                    .checkpoint()
                    .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
            }
            Ok(VariablePoint {
                x: point.x,
                y: point.y,
                width: PARAMETRIC_PATH_STROKE_WIDTH,
            })
        })
        .collect::<Result<Vec<_>, NativeRecipeOperationError>>()?;
    let outlines = if enabled {
        vec![outline_from_variable_points(&points, false).ok_or_else(|| {
            NativeRecipeOperationError::new(
                "parametric path emitter requires at least two distinct points",
            )
        })?]
    } else {
        Vec::new()
    };
    context
        .cancellation
        .checkpoint()
        .map_err(|error| NativeRecipeOperationError::new(error.to_string()))?;
    Ok(RecipeRuntimeValue::CanonicalOutput(
        CanonicalPatternOutput::Paths(PathPatternOutput {
            geometry: CurveGeometry {
                width: path.artboard.width,
                height: path.artboard.height,
                layers: vec![CurveInkLayer {
                    layer: InkLayer {
                        channel: Channel::from(output_channel.to_legacy_ink()),
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

fn required_literal<'a>(
    parameters: &'a RecipeOperationParameters<'_>,
    key: &str,
) -> Result<&'a LiteralValue, NativeRecipeOperationError> {
    parameters.get(key).copied().ok_or_else(|| {
        NativeRecipeOperationError::new(format!("quadratic-radial spiral is missing `{key}`"))
    })
}

fn required_literal_for<'a>(
    operation: &str,
    parameters: &'a RecipeOperationParameters<'_>,
    key: &str,
) -> Result<&'a LiteralValue, NativeRecipeOperationError> {
    parameters
        .get(key)
        .copied()
        .ok_or_else(|| NativeRecipeOperationError::new(format!("{operation} is missing `{key}`")))
}

fn required_number_for(
    operation: &str,
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<f64, NativeRecipeOperationError> {
    match required_literal_for(operation, parameters, key)? {
        LiteralValue::Number(value) if value.is_finite() => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "{operation} `{key}` must be a finite number"
        ))),
    }
}

fn required_boolean_for(
    operation: &str,
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<bool, NativeRecipeOperationError> {
    match required_literal_for(operation, parameters, key)? {
        LiteralValue::Boolean(value) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "{operation} `{key}` must be boolean"
        ))),
    }
}

fn required_number(
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<f64, NativeRecipeOperationError> {
    match required_literal(parameters, key)? {
        LiteralValue::Number(value) if value.is_finite() => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "quadratic-radial spiral `{key}` must be a finite number"
        ))),
    }
}

fn required_boolean(
    parameters: &RecipeOperationParameters<'_>,
    key: &str,
) -> Result<bool, NativeRecipeOperationError> {
    match required_literal(parameters, key)? {
        LiteralValue::Boolean(value) => Ok(*value),
        _ => Err(NativeRecipeOperationError::new(format!(
            "quadratic-radial spiral `{key}` must be boolean"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundled_pattern_definitions::load_bundled_quadratic_radial_spiral_definition;
    use crate::pattern::{CanonicalPatternOutput, PatternFamily, PatternId, PatternOutputKind};
    use crate::pattern_definition::{
        OperationPortDescriptor, OperationReference, OperationRegistry, PatternDefinition,
        PatternDisplayMetadata, RecipeArgument, RecipeEdge, RecipeGraph, RecipeNode,
        RecipePortType, RegisteredOperationDescriptor,
    };
    use std::collections::BTreeMap;

    fn artboard() -> ArtboardSpace {
        ArtboardSpace {
            width: 640,
            height: 480,
        }
    }

    fn parameters() -> BTreeMap<&'static str, LiteralValue> {
        let defaults = QuadraticRadialSpiralParameters::default();
        BTreeMap::from([
            ("turns", LiteralValue::Number(defaults.turns)),
            (
                "starting-radius",
                LiteralValue::Number(defaults.starting_radius),
            ),
            (
                "radial-growth-per-revolution",
                LiteralValue::Number(defaults.radial_growth_per_revolution),
            ),
            (
                "spacing-growth-per-revolution",
                LiteralValue::Number(defaults.spacing_growth_per_revolution),
            ),
            (
                "starting-angle-degrees",
                LiteralValue::Number(defaults.starting_angle_degrees),
            ),
            ("direction", LiteralValue::Choice("clockwise".into())),
            ("center-x", LiteralValue::Number(defaults.center_x)),
            ("center-y", LiteralValue::Number(defaults.center_y)),
            (
                "maximum-sample-distance",
                LiteralValue::Number(defaults.maximum_sample_distance),
            ),
            (
                "edge-extension",
                LiteralValue::Boolean(defaults.edge_extension),
            ),
            (
                "edge-overscan",
                LiteralValue::Number(defaults.edge_overscan),
            ),
        ])
    }

    fn generate(values: &BTreeMap<&'static str, LiteralValue>) -> ParametricPath {
        let token = CancellationToken::new();
        let parameters = values
            .iter()
            .map(|(key, value)| (*key, value))
            .collect::<RecipeOperationParameters<'_>>();
        let parameters =
            QuadraticRadialSpiralParameters::from_operation_parameters(&parameters).unwrap();
        generate_quadratic_radial_spiral(&parameters, artboard(), &token).unwrap()
    }

    fn execution_context<'a>(token: &'a CancellationToken) -> RecipeExecutionContext<'a> {
        RecipeExecutionContext {
            artboard: artboard(),
            output_channel: Some(crate::OutputChannelId::RgbRed),
            source_field_provider: None,
            source_field: None,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: token,
        }
    }

    #[test]
    fn bundled_spiral_executes_through_the_generic_typed_registry_seam() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        assert_eq!(definition.id, PatternId::QUADRATIC_RADIAL_SPIRAL_V1);
        definition
            .validate_with_registry(&REGISTERED_OPERATIONS)
            .unwrap();
        let instance = definition
            .default_instance_parameters([crate::OutputChannelId::RgbRed])
            .unwrap();
        let token = CancellationToken::new();
        let first = execute_parametric_paths_definition_cancellable(
            &definition,
            &instance,
            &execution_context(&token),
        )
        .unwrap();
        let second = execute_parametric_paths_definition_cancellable(
            &definition,
            &instance,
            &execution_context(&token),
        )
        .unwrap();
        assert_eq!(first, second);
        let CanonicalPatternOutput::Paths(paths) = first else {
            panic!("bundled parametric recipe must emit paths");
        };
        assert_eq!(paths.geometry.width, 640);
        assert_eq!(paths.geometry.height, 480);
        assert_eq!(paths.geometry.layers.len(), 1);
        assert!(paths.geometry.layers[0].layer.enabled);
        assert_eq!(paths.geometry.layers[0].outlines.len(), 1);
    }

    #[test]
    fn bundled_spiral_execution_propagates_cancellation_and_sample_limits() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters([crate::OutputChannelId::RgbRed])
            .unwrap();
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(
            execute_parametric_paths_definition_cancellable(
                &definition,
                &instance,
                &execution_context(&cancelled),
            )
            .is_err()
        );

        let mut beyond_limit = instance;
        beyond_limit
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "turns")
            .unwrap()
            .value = LiteralValue::Number(64.0);
        beyond_limit
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "radial-growth-per-revolution")
            .unwrap()
            .value = LiteralValue::Number(100_000.0);
        beyond_limit
            .pattern_values
            .iter_mut()
            .find(|value| value.key == "maximum-sample-distance")
            .unwrap()
            .value = LiteralValue::Number(0.01);
        let token = CancellationToken::new();
        let error = execute_parametric_paths_definition_cancellable(
            &definition,
            &beyond_limit,
            &execution_context(&token),
        )
        .unwrap_err();
        assert!(error.to_string().contains("bounded sample limit"));
    }

    #[test]
    fn quadratic_radial_spiral_contract_is_typed_creator_owned_and_registered() {
        let descriptor = REGISTERED_OPERATIONS
            .get(
                QUADRATIC_RADIAL_SPIRAL_OPERATION_ID,
                QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION,
            )
            .unwrap();
        assert_eq!(descriptor.inputs, &[]);
        assert_eq!(
            descriptor.output,
            OperationPortDescriptor {
                name: "path",
                kind: RecipePortType::ParametricPath,
            }
        );
        assert_eq!(descriptor.parameters.len(), 11);
        assert!(
            PARAMETRIC_PATHS_NATIVE_OPERATION_REGISTRY
                .get(
                    QUADRATIC_RADIAL_SPIRAL_OPERATION_ID,
                    QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION
                )
                .is_some()
        );

        let parameters = quadratic_radial_spiral_parameter_definitions();
        assert_eq!(parameters.len(), 11);
        assert!(parameters.iter().all(|parameter| {
            matches!(
                parameter.authoring,
                ParameterAuthoring::Creator(CreatorParameterMetadata {
                    ownership: ParameterOwnership::PatternDefinition,
                    invalidation: ParameterInvalidationScope::Geometry,
                    ..
                })
            )
        }));
        let overscan = parameters
            .iter()
            .find(|parameter| parameter.key == "edge-overscan")
            .unwrap();
        assert!(matches!(
            overscan.authoring,
            ParameterAuthoring::Creator(CreatorParameterMetadata {
                applicability: ParameterApplicability::WhenParameterEquals { .. },
                ..
            })
        ));
        let layout = quadratic_radial_spiral_authoring_layout();
        assert_eq!(layout.sections.len(), 5);
        assert_eq!(
            layout.sections[4].parameters,
            ["edge-extension", "edge-overscan"]
        );
    }

    #[test]
    fn quadratic_radial_creator_contract_passes_full_definition_schema_validation() {
        static PATH_INPUT: [OperationPortDescriptor; 1] = [OperationPortDescriptor {
            name: "path",
            kind: RecipePortType::ParametricPath,
        }];
        static NO_PARAMETERS: [crate::OperationParameterDescriptor; 0] = [];
        static PATH_OUTPUT: [PatternOutputKind; 1] = [PatternOutputKind::Paths];
        let mut descriptors = REGISTERED_OPERATIONS.entries().to_vec();
        descriptors.push(RegisteredOperationDescriptor {
            id: "test-parametric.emit-path",
            version: 1,
            inputs: &PATH_INPUT,
            output: OperationPortDescriptor {
                name: "geometry",
                kind: RecipePortType::CanonicalGeometry,
            },
            parameters: &NO_PARAMETERS,
            canonical_output_kinds: &PATH_OUTPUT,
        });
        let registry = OperationRegistry::new(Box::leak(descriptors.into_boxed_slice()));
        let parameters = quadratic_radial_spiral_parameter_definitions();
        let spiral_parameters = parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.key.clone(),
                    RecipeArgument::Parameter(parameter.key.clone()),
                )
            })
            .collect();
        let definition = PatternDefinition {
            format_version: crate::TNPATTERN_FORMAT_VERSION,
            recipe_version: crate::TNPATTERN_RECIPE_VERSION,
            id: PatternId::new("test.quadratic-radial-spiral.v1").unwrap(),
            display: PatternDisplayMetadata {
                name: "Quadratic Radial Spiral Test".into(),
                summary: "Schema-only test fixture; no native canonical emitter.".into(),
            },
            family: PatternFamily::ParametricPaths,
            outputs: vec![PatternOutputKind::Paths],
            parameters,
            quick_controls: vec![],
            layout: quadratic_radial_spiral_authoring_layout(),
            recipe: RecipeGraph {
                nodes: vec![
                    RecipeNode {
                        id: "spiral".into(),
                        operation: OperationReference {
                            id: QUADRATIC_RADIAL_SPIRAL_OPERATION_ID.into(),
                            version: QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION,
                        },
                        parameters: spiral_parameters,
                    },
                    RecipeNode {
                        id: "emit".into(),
                        operation: OperationReference {
                            id: "test-parametric.emit-path".into(),
                            version: 1,
                        },
                        parameters: Default::default(),
                    },
                ],
                edges: vec![RecipeEdge {
                    from: crate::PortReference {
                        node: "spiral".into(),
                        port: "path".into(),
                    },
                    to: crate::PortReference {
                        node: "emit".into(),
                        port: "path".into(),
                    },
                }],
                output: crate::PortReference {
                    node: "emit".into(),
                    port: "geometry".into(),
                },
            },
            assets: vec![],
        };
        definition.validate_with_registry(&registry).unwrap();
    }

    #[test]
    fn default_spiral_is_deterministic_and_matches_checkpoint_geometry_metrics() {
        let values = parameters();
        let first = generate(&values);
        let second = generate(&values);
        assert_eq!(first, second);
        assert_eq!(first.base_turns, 20.0);
        assert!(first.generated_turns > first.base_turns);
        assert_eq!(first.points.len(), 13_856);
        assert_eq!(
            first.points.first().unwrap(),
            &ParametricPathPoint { x: 320.0, y: 240.0 }
        );
        let end = first.points.last().unwrap();
        assert!((end.x - 740.0).abs() < 0.001);
        assert!(end.y.abs() < 0.001 || (end.y - 240.0).abs() < 0.001);
        assert!(first.points.iter().any(|point| point.x > 640.0));
        assert!(first.points.iter().any(|point| point.y > 480.0));
    }

    #[test]
    fn every_exposed_parameter_changes_the_generated_path() {
        for (key, replacement, disable_edge_extension) in [
            ("turns", LiteralValue::Number(12.0), true),
            ("starting-radius", LiteralValue::Number(8.0), false),
            (
                "radial-growth-per-revolution",
                LiteralValue::Number(12.0),
                false,
            ),
            (
                "spacing-growth-per-revolution",
                LiteralValue::Number(0.5),
                false,
            ),
            ("starting-angle-degrees", LiteralValue::Number(45.0), false),
            (
                "direction",
                LiteralValue::Choice("counterclockwise".into()),
                false,
            ),
            ("center-x", LiteralValue::Number(16.0), false),
            ("center-y", LiteralValue::Number(-12.0), false),
            ("maximum-sample-distance", LiteralValue::Number(8.0), false),
            ("edge-extension", LiteralValue::Boolean(false), false),
            ("edge-overscan", LiteralValue::Number(40.0), false),
        ] {
            let mut baseline_values = parameters();
            if disable_edge_extension {
                baseline_values.insert("edge-extension", LiteralValue::Boolean(false));
            }
            let baseline = generate(&baseline_values);
            let mut values = parameters();
            if disable_edge_extension {
                values.insert("edge-extension", LiteralValue::Boolean(false));
            }
            values.insert(key, replacement);
            assert_ne!(
                generate(&values).points,
                baseline.points,
                "{key} did not affect generated points"
            );
        }
    }

    #[test]
    fn edge_extension_is_explicit_and_supports_downstream_clipping_coverage() {
        let mut values = parameters();
        values.insert("edge-extension", LiteralValue::Boolean(false));
        let clipped_extent = generate(&values);
        values.insert("edge-extension", LiteralValue::Boolean(true));
        values.insert("edge-overscan", LiteralValue::Number(40.0));
        let extended = generate(&values);
        assert_eq!(clipped_extent.generated_turns, clipped_extent.base_turns);
        assert!(extended.generated_turns > extended.base_turns);
        assert!(extended.points.len() > clipped_extent.points.len());
        assert!(extended.points.iter().any(|point| point.x > 640.0));
    }

    #[test]
    fn edge_extension_reaches_wide_tall_and_offset_artboard_corners() {
        for (artboard, center_x, center_y) in [
            (
                ArtboardSpace {
                    width: 1_200,
                    height: 240,
                },
                80.0,
                -30.0,
            ),
            (
                ArtboardSpace {
                    width: 240,
                    height: 1_000,
                },
                -50.0,
                70.0,
            ),
        ] {
            let parameters = QuadraticRadialSpiralParameters {
                center_x,
                center_y,
                edge_overscan: 12.0,
                ..Default::default()
            };
            let path =
                generate_quadratic_radial_spiral(&parameters, artboard, &CancellationToken::new())
                    .unwrap();
            let center = (
                f64::from(artboard.width) * 0.5 + center_x,
                f64::from(artboard.height) * 0.5 + center_y,
            );
            let furthest_corner = [
                (0.0, 0.0),
                (f64::from(artboard.width), 0.0),
                (0.0, f64::from(artboard.height)),
                (f64::from(artboard.width), f64::from(artboard.height)),
            ]
            .into_iter()
            .map(|(x, y)| (x - center.0).hypot(y - center.1))
            .fold(0.0, f64::max);
            let endpoint = path.points.last().unwrap();
            assert!(
                (endpoint.x - center.0).hypot(endpoint.y - center.1)
                    >= furthest_corner + parameters.edge_overscan - 0.001
            );
            assert!(parameters.radial_derivative_at(path.generated_turns) >= 0.0);
        }
    }

    #[test]
    fn sampling_is_bounded_smooth_and_cancellable() {
        let values = parameters();
        let path = generate(&values);
        assert!(path.points.windows(2).all(|points| {
            let dx = points[1].x - points[0].x;
            let dy = points[1].y - points[0].y;
            dx.hypot(dy) <= QuadraticRadialSpiralParameters::default().maximum_sample_distance
        }));
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(
            generate_quadratic_radial_spiral(
                &QuadraticRadialSpiralParameters::default(),
                artboard(),
                &token
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_growth_limits_and_operation_inputs_are_rejected() {
        let invalid = QuadraticRadialSpiralParameters {
            spacing_growth_per_revolution: -2.0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
        let invalid = QuadraticRadialSpiralParameters {
            maximum_sample_distance: 0.0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
        let invalid = QuadraticRadialSpiralParameters {
            edge_overscan: MAX_DOCUMENT_DISTANCE,
            ..Default::default()
        };
        assert!(
            generate_quadratic_radial_spiral(&invalid, artboard(), &CancellationToken::new())
                .is_err()
        );
        let invalid = QuadraticRadialSpiralParameters {
            turns: MAX_TURNS,
            radial_growth_per_revolution: MAX_DOCUMENT_DISTANCE,
            maximum_sample_distance: 0.01,
            ..Default::default()
        };
        assert!(
            generate_quadratic_radial_spiral(&invalid, artboard(), &CancellationToken::new())
                .is_err()
        );

        let values = parameters();
        let parameters = values
            .iter()
            .map(|(key, value)| (*key, value))
            .collect::<RecipeOperationParameters<'_>>();
        let token = CancellationToken::new();
        let context = RecipeExecutionContext {
            artboard: artboard(),
            output_channel: None,
            source_field_provider: None,
            source_field: None,
            source_generation: 0,
            resolved_field_generation: 0,
            semantic_channel_index: 0,
            enabled_layer_index: 0,
            definition_assets: &[],
            cancellation: &token,
        };
        let unexpected =
            RecipeRuntimeValue::Samples(crate::DistributionField::new(1, 1, vec![1.0]).unwrap());
        let inputs = BTreeMap::from([("unexpected", &unexpected)]);
        assert!(quadratic_radial_spiral_operation(&context, &inputs, &parameters).is_err());
    }
}
