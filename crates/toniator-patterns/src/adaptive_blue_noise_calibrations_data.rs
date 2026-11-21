//! Genuine blue-noise response; regenerate with adaptive_square_companion OUTPUT_DIRECTORY blue-noise.
use super::MeasuredData;
use toniator_domain::CalibrationKnot;
/// Returns the frozen complete binding and unchanged reference observations.
pub(super) fn data(id: &str) -> Option<MeasuredData<'static>> {
    if id != "adaptive-shape-marks" {
        return None;
    }
    Some((
        "CalibrationBinding { recipe: PatternDefinitionRecipe { structure: OrderedOutputs { definition: GeneralizedStraightGuides { name: \"\", coverage: CoveragePolicy { guard_steps: 2, additional_margin: 0.0 }, dimensions: [GuideDimensionDraft { baseline_angle_degrees: 0.0, phase: 0.0, spacing_multiplier: 1.0 }, GuideDimensionDraft { baseline_angle_degrees: 90.0, phase: 0.0, spacing_multiplier: 1.0 }], product: Intersections { dimension_indices: [0, 1], merge_epsilon: 1e-8 }, orientation: Fixed }, outputs: [AdaptiveSquareBlueNoiseMarks] }, output_settings: [PatternOutputSettingsRecipe { source_filter: All, response: Marks(MarkGeometryResponse { minimum_fill: 0.0, maximum_fill: 1.0 }) }] }, density_aspect: 1.0, shape_rotation_degrees: 0.0 }",
        "adaptive-square-reference-canvas1024-spacings32-nearest-area-v1+cell-area-overlap-q16-mask256-v1+blue-noise64-area-mixing-v1",
        &[
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 0.2222222222222222,
                coverage: 0.2223707879529097,
            },
            CalibrationKnot {
                input: 0.4444444444444444,
                coverage: 0.44511007050076007,
            },
            CalibrationKnot {
                input: 0.6666666666666666,
                coverage: 0.665868931801615,
            },
            CalibrationKnot {
                input: 0.8888888888888888,
                coverage: 0.8886908054434071,
            },
            CalibrationKnot {
                input: 1.1111111111111112,
                coverage: 1.0,
            },
            CalibrationKnot {
                input: 1.3333333333333333,
                coverage: 1.0,
            },
            CalibrationKnot {
                input: 1.5555555555555556,
                coverage: 1.0,
            },
            CalibrationKnot {
                input: 1.7777777777777777,
                coverage: 1.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 1.0,
            },
        ],
        None,
        None,
    ))
}
