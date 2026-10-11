use toniator_domain::{CurveWinding, ParametricCurve, SpiralCurve, SpiralShape};

use crate::{Point2, Vector2};

use super::{CubicBezierSegment, CurveError, CurvePath, CurveSegment, PathClosure};

/// Maximum analytic-to-cubic positional residual in document units.
const ROUND_SPIRAL_RESIDUAL_LIMIT: f64 = 1.0 / 64.0;
/// Maximum dyadic refinements from an initially quarter-turn-or-smaller span.
const ROUND_SPIRAL_MAX_SUBDIVISION_DEPTH: u32 = 24;
/// Inclusive structural limit shared with finite path consumers.
const PARAMETRIC_CURVE_MAX_SEGMENTS: usize = 4_096;

/// Converts validated finite parametric intent into one canonical open CurvePath at `origin`.
/// The caller chooses presentation placement; this construction never consults canvas bounds.
///
/// # Errors
///
/// Returns a stable numeric, segment-limit, path, or cancellation failure without partial geometry.
pub fn construct_parametric_curve_path_cancellable(
    curve: &ParametricCurve,
    origin: Point2,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CurvePath, CurveError> {
    match curve {
        ParametricCurve::Spiral(spiral) => {
            construct_spiral_curve_path(spiral, origin, is_cancelled)
        }
    }
}

/// Converts one finite round/square spiral to deterministic cubic or exact-polyline geometry.
///
/// # Errors
///
/// Returns one bounded construction failure before a path is made visible to consumers.
fn construct_spiral_curve_path(
    spiral: &SpiralCurve,
    origin: Point2,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CurvePath, CurveError> {
    let sign = match spiral.winding {
        CurveWinding::Clockwise => -1.0,
        CurveWinding::CounterClockwise => 1.0,
    };
    let phase = spiral.phase_degrees.to_radians();
    match spiral.shape {
        SpiralShape::Round => {
            let total = core::f64::consts::TAU * spiral.turns;
            let minimum_count = (total / core::f64::consts::FRAC_PI_2).ceil() as usize;
            if minimum_count == 0 || minimum_count > PARAMETRIC_CURVE_MAX_SEGMENTS {
                return Err(CurveError::new(
                    "curve.parametric.segment_limit",
                    "spiral construction exceeds the CurvePath segment limit",
                ));
            }
            let mut segments = Vec::with_capacity(minimum_count);
            let mut block_start = 0.0;
            let mut previous_full_turn_count = 4_usize;
            let whole_turns = spiral.turns.floor() as usize;
            for block_index in 0..spiral.turns.ceil() as usize {
                if is_cancelled() {
                    return Err(CurveError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let block_end = (core::f64::consts::TAU * (block_index + 1) as f64).min(total);
                let span = block_end - block_start;
                if span <= 0.0 {
                    return Err(CurveError::new(
                        "curve.parametric.residual_limit",
                        "spiral parameter spans must remain positive",
                    ));
                }
                let minimum = (span / core::f64::consts::FRAC_PI_2).ceil() as usize;
                let full_turn = block_index < whole_turns;
                let mut count = if full_turn {
                    previous_full_turn_count.max(minimum)
                } else {
                    minimum
                };
                loop {
                    if count > PARAMETRIC_CURVE_MAX_SEGMENTS - segments.len() {
                        return Err(CurveError::new(
                            "curve.parametric.segment_limit",
                            "spiral construction exceeds the CurvePath segment limit",
                        ));
                    }
                    if round_spiral_block_fits(
                        spiral,
                        origin,
                        phase,
                        sign,
                        block_start,
                        block_end,
                        count,
                        is_cancelled,
                    )? {
                        break;
                    }
                    count += 1;
                }
                let mut start_theta = block_start;
                for index in 0..count {
                    let end_theta = if index + 1 == count {
                        block_end
                    } else {
                        block_start + span * (index + 1) as f64 / count as f64
                    };
                    append_adaptive_round_spiral_span(
                        &mut segments,
                        spiral,
                        origin,
                        phase,
                        sign,
                        start_theta,
                        end_theta,
                        0,
                        is_cancelled,
                    )?;
                    start_theta = end_theta;
                }
                if full_turn {
                    previous_full_turn_count = count;
                }
                block_start = block_end;
            }
            CurvePath::new(segments, PathClosure::Open)
        }
        SpiralShape::Square => {
            let quarter_turns = spiral.turns * 4.0;
            let count = quarter_turns.ceil() as usize;
            if count == 0 || count > PARAMETRIC_CURVE_MAX_SEGMENTS {
                return Err(CurveError::new(
                    "curve.parametric.segment_limit",
                    "spiral construction exceeds the CurvePath segment limit",
                ));
            }
            let mut vertices = Vec::with_capacity(count + 1);
            let mut point = origin;
            vertices.push(point);
            let (sin, cos) = phase.sin_cos();
            let mut direction = Vector2::new(cos, sin);
            for index in 0..count {
                if is_cancelled() {
                    return Err(CurveError::new(
                        "evaluation.cancelled",
                        "evaluation was cancelled",
                    ));
                }
                let fraction = (quarter_turns - index as f64).clamp(0.0, 1.0);
                let length = spiral.radial_spacing * (index / 2 + 1) as f64 * fraction;
                point = Point2::new(
                    point.x + length * direction.x,
                    point.y + length * direction.y,
                );
                vertices.push(point);
                direction = if sign > 0.0 {
                    Vector2::new(-direction.y, direction.x)
                } else {
                    Vector2::new(direction.y, -direction.x)
                };
            }
            CurvePath::polyline(vertices, PathClosure::Open)
        }
    }
}

/// Checks whether equal initial Hermite spans across one revolution satisfy the canonical
/// three-witness residual, retaining the adaptive builder as the publication boundary.
///
/// # Errors
///
/// Returns cancellation, finite-coordinate, or collapsed-parameter diagnostics before allocation.
#[allow(clippy::too_many_arguments)]
fn round_spiral_block_fits(
    spiral: &SpiralCurve,
    origin: Point2,
    phase: f64,
    sign: f64,
    block_start: f64,
    block_end: f64,
    count: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<bool, CurveError> {
    let span = block_end - block_start;
    let mut start_theta = block_start;
    for index in 0..count {
        if is_cancelled() {
            return Err(CurveError::new(
                "evaluation.cancelled",
                "evaluation was cancelled",
            ));
        }
        let end_theta = if index + 1 == count {
            block_end
        } else {
            block_start + span * (index + 1) as f64 / count as f64
        };
        if end_theta <= start_theta {
            return Err(CurveError::new(
                "curve.parametric.residual_limit",
                "spiral parameter spans must remain positive",
            ));
        }
        let candidate =
            round_spiral_hermite_segment(spiral, origin, phase, sign, start_theta, end_theta)?;
        if !round_spiral_residual_is_within_limit(
            &candidate,
            spiral,
            origin,
            phase,
            sign,
            start_theta,
            end_theta,
        )? {
            return Ok(false);
        }
        start_theta = end_theta;
    }
    Ok(true)
}

/// Appends one dyadically refined Hermite cubic span after deterministic residual witnesses.
///
/// # Errors
///
/// Returns cancellation, numeric, residual-depth, or segment-limit diagnostics without appending
/// an unverified segment.
#[allow(clippy::too_many_arguments)]
fn append_adaptive_round_spiral_span(
    segments: &mut Vec<CurveSegment>,
    spiral: &SpiralCurve,
    origin: Point2,
    phase: f64,
    sign: f64,
    start_theta: f64,
    end_theta: f64,
    depth: u32,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), CurveError> {
    if is_cancelled() {
        return Err(CurveError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ));
    }
    let segment =
        round_spiral_hermite_segment(spiral, origin, phase, sign, start_theta, end_theta)?;
    if round_spiral_residual_is_within_limit(
        &segment,
        spiral,
        origin,
        phase,
        sign,
        start_theta,
        end_theta,
    )? {
        if segments.len() >= PARAMETRIC_CURVE_MAX_SEGMENTS {
            return Err(CurveError::new(
                "curve.parametric.segment_limit",
                "spiral construction exceeds the CurvePath segment limit",
            ));
        }
        segments.push(CurveSegment::CubicBezier(segment));
        return Ok(());
    }
    if depth >= ROUND_SPIRAL_MAX_SUBDIVISION_DEPTH {
        return Err(CurveError::new(
            "curve.parametric.residual_limit",
            "spiral cubic approximation exceeds the bounded residual tolerance",
        ));
    }
    let midpoint = (start_theta + end_theta) * 0.5;
    append_adaptive_round_spiral_span(
        segments,
        spiral,
        origin,
        phase,
        sign,
        start_theta,
        midpoint,
        depth + 1,
        is_cancelled,
    )?;
    append_adaptive_round_spiral_span(
        segments,
        spiral,
        origin,
        phase,
        sign,
        midpoint,
        end_theta,
        depth + 1,
        is_cancelled,
    )
}

/// Builds the exact-endpoint cubic Hermite approximation for one finite round-spiral span.
///
/// # Errors
///
/// Returns the cubic constructor's finite-coordinate failure.
fn round_spiral_hermite_segment(
    spiral: &SpiralCurve,
    origin: Point2,
    phase: f64,
    sign: f64,
    start_theta: f64,
    end_theta: f64,
) -> Result<CubicBezierSegment, CurveError> {
    let (start, first) =
        round_spiral_point_and_derivative(spiral, origin, phase, sign, start_theta);
    let (end, last) = round_spiral_point_and_derivative(spiral, origin, phase, sign, end_theta);
    let span = end_theta - start_theta;
    CubicBezierSegment::new(
        start,
        Point2::new(
            start.x + first.x * span / 3.0,
            start.y + first.y * span / 3.0,
        ),
        Point2::new(end.x - last.x * span / 3.0, end.y - last.y * span / 3.0),
        end,
    )
}

/// Evaluates the finite analytic round spiral and its derivative at one nonnegative parameter.
fn round_spiral_point_and_derivative(
    spiral: &SpiralCurve,
    origin: Point2,
    phase: f64,
    sign: f64,
    theta: f64,
) -> (Point2, Vector2) {
    let radial_rate = spiral.radial_spacing / core::f64::consts::TAU;
    let radius = radial_rate * theta;
    let angle = phase + sign * theta;
    let (sin, cos) = angle.sin_cos();
    (
        Point2::new(origin.x + radius * cos, origin.y + radius * sin),
        Vector2::new(
            radial_rate * cos - sign * radius * sin,
            radial_rate * sin + sign * radius * cos,
        ),
    )
}

/// Tests the required quarter, half, and three-quarter residual witnesses for one cubic span.
///
/// # Errors
///
/// Returns a stable path-point failure only if a constructor-validated cubic cannot be sampled.
#[allow(clippy::too_many_arguments)]
fn round_spiral_residual_is_within_limit(
    segment: &CubicBezierSegment,
    spiral: &SpiralCurve,
    origin: Point2,
    phase: f64,
    sign: f64,
    start_theta: f64,
    end_theta: f64,
) -> Result<bool, CurveError> {
    for parameter in [0.25, 0.5, 0.75] {
        let actual = round_spiral_point_and_derivative(
            spiral,
            origin,
            phase,
            sign,
            start_theta + (end_theta - start_theta) * parameter,
        )
        .0;
        let cubic = CurveSegment::CubicBezier(*segment).point_at(parameter)?;
        if ((actual.x - cubic.x).powi(2) + (actual.y - cubic.y).powi(2)).sqrt()
            > ROUND_SPIRAL_RESIDUAL_LIMIT
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds one finite spiral fixture without involving document validation or canvas placement.
    fn spiral(shape: SpiralShape, turns: f64, winding: CurveWinding) -> ParametricCurve {
        ParametricCurve::Spiral(SpiralCurve {
            shape,
            turns,
            radial_spacing: 8.0,
            phase_degrees: 0.0,
            winding,
        })
    }

    /// Proves every published round cubic meets the required three-point analytic residual policy.
    #[test]
    fn round_spiral_cubics_meet_dyadic_residual_witnesses() {
        let curve = spiral(SpiralShape::Round, 4.0, CurveWinding::CounterClockwise);
        let path =
            construct_parametric_curve_path_cancellable(&curve, Point2::new(0.0, 0.0), &|| false)
                .expect("finite round spiral");
        let ParametricCurve::Spiral(spiral) = curve;
        let radial_rate = spiral.radial_spacing / core::f64::consts::TAU;
        for segment in path.segments() {
            let CurveSegment::CubicBezier(cubic) = segment else {
                panic!("round output is cubic");
            };
            let start = cubic.start().x.hypot(cubic.start().y) / radial_rate;
            let end = cubic.end().x.hypot(cubic.end().y) / radial_rate;
            assert!(
                round_spiral_residual_is_within_limit(
                    cubic,
                    &spiral,
                    Point2::new(0.0, 0.0),
                    0.0,
                    1.0,
                    start,
                    end
                )
                .expect("samples")
            );
        }
    }

    /// Keeps concrete full-width default and fine spirals within the finite cubic budget for
    /// either winding and phase while preserving analytic endpoints, tangents, and dense residuals.
    ///
    /// # Panics
    /// Panics if a published span exceeds the fixed geometric residual or loses analytic joins.
    #[test]
    fn full_width_round_spirals_fit_with_exact_hermite_joins() {
        for (turns, spacing, maximum_segments) in [(47.0, 16.0, 900), (188.0, 4.0, 3_500)] {
            for winding in [CurveWinding::Clockwise, CurveWinding::CounterClockwise] {
                for phase_degrees in [0.0, 37.0] {
                    let spiral = SpiralCurve {
                        shape: SpiralShape::Round,
                        turns,
                        radial_spacing: spacing,
                        phase_degrees,
                        winding,
                    };
                    let origin = Point2::new(512.0, 512.0);
                    let path = construct_parametric_curve_path_cancellable(
                        &ParametricCurve::Spiral(spiral.clone()),
                        origin,
                        &|| false,
                    )
                    .expect("full-width spiral fits");
                    assert!(path.segments().len() <= maximum_segments);
                    let sign = if winding == CurveWinding::Clockwise {
                        -1.0
                    } else {
                        1.0
                    };
                    let phase = phase_degrees.to_radians();
                    let radial_rate = spacing / core::f64::consts::TAU;
                    let mut previous_end = origin;
                    for segment in path.segments() {
                        let CurveSegment::CubicBezier(cubic) = segment else {
                            panic!("round spiral uses cubics");
                        };
                        assert_eq!(cubic.start(), previous_end);
                        let start_theta = (cubic.start().x - origin.x)
                            .hypot(cubic.start().y - origin.y)
                            / radial_rate;
                        let end_theta = (cubic.end().x - origin.x).hypot(cubic.end().y - origin.y)
                            / radial_rate;
                        let span = end_theta - start_theta;
                        let (expected_start, derivative_start) = round_spiral_point_and_derivative(
                            &spiral,
                            origin,
                            phase,
                            sign,
                            start_theta,
                        );
                        let (expected_end, derivative_end) = round_spiral_point_and_derivative(
                            &spiral, origin, phase, sign, end_theta,
                        );
                        assert!(
                            (cubic.start().x - expected_start.x)
                                .hypot(cubic.start().y - expected_start.y)
                                < 1.0e-8
                        );
                        assert!(
                            (cubic.end().x - expected_end.x).hypot(cubic.end().y - expected_end.y)
                                < 1.0e-8
                        );
                        let start_handle = cubic.control_1();
                        let end_handle = cubic.control_2();
                        assert!(
                            (start_handle.x - cubic.start().x - derivative_start.x * span / 3.0)
                                .hypot(
                                    start_handle.y
                                        - cubic.start().y
                                        - derivative_start.y * span / 3.0
                                )
                                < 1.0e-7
                        );
                        assert!(
                            (cubic.end().x - end_handle.x - derivative_end.x * span / 3.0).hypot(
                                cubic.end().y - end_handle.y - derivative_end.y * span / 3.0
                            ) < 1.0e-7
                        );
                        for sample in 1..16 {
                            let fraction = sample as f64 / 16.0;
                            let expected = round_spiral_point_and_derivative(
                                &spiral,
                                origin,
                                phase,
                                sign,
                                start_theta + span * fraction,
                            )
                            .0;
                            let actual = CurveSegment::CubicBezier(*cubic)
                                .point_at(fraction)
                                .expect("finite cubic sample");
                            assert!(
                                (actual.x - expected.x).hypot(actual.y - expected.y)
                                    <= ROUND_SPIRAL_RESIDUAL_LIMIT + 1.0e-9
                            );
                        }
                        previous_end = cubic.end();
                    }
                    let expected_end = round_spiral_point_and_derivative(
                        &spiral,
                        origin,
                        phase,
                        sign,
                        core::f64::consts::TAU * turns,
                    )
                    .0;
                    assert_eq!(path.start(), origin);
                    assert_eq!(path.end(), expected_end);
                }
            }
        }
    }

    /// Proves a five-turn 1024-square spiral supports bounded equal-arc measurement and inversion.
    #[test]
    fn five_turn_artboard_spiral_supports_equal_arc_sampling() {
        let curve = ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 5.0,
            radial_spacing: 1024.0_f64.hypot(1024.0) * 0.5 / 4.0,
            phase_degrees: 0.0,
            winding: CurveWinding::CounterClockwise,
        });
        let path =
            construct_parametric_curve_path_cancellable(&curve, Point2::new(512.0, 512.0), &|| {
                false
            })
            .expect("five-turn artboard spiral constructs");
        let measured = path
            .measure_arc_length()
            .expect("five-turn artboard spiral measures");
        for ordinal in 0..=256 {
            measured
                .location_at_length(measured.total_length() * f64::from(ordinal) / 256.0)
                .expect("five-turn artboard spiral length inverts");
        }
    }

    /// Preserves exact square phase-basis corners for fractional turns and both winding directions.
    #[test]
    fn square_spiral_uses_signed_perpendicular_corner_steps() {
        let clockwise = construct_parametric_curve_path_cancellable(
            &spiral(SpiralShape::Square, 0.3, CurveWinding::Clockwise),
            Point2::new(0.0, 0.0),
            &|| false,
        )
        .expect("clockwise square");
        let counterclockwise = construct_parametric_curve_path_cancellable(
            &spiral(SpiralShape::Square, 0.3, CurveWinding::CounterClockwise),
            Point2::new(0.0, 0.0),
            &|| false,
        )
        .expect("counterclockwise square");
        assert!((clockwise.end().x - 8.0).abs() <= 1.0e-12);
        assert!((clockwise.end().y + 1.6).abs() <= 1.0e-12);
        assert!((counterclockwise.end().x - 8.0).abs() <= 1.0e-12);
        assert!((counterclockwise.end().y - 1.6).abs() <= 1.0e-12);
    }

    /// Rejects cancellation and segment-limit violations before publishing a partial parametric path.
    #[test]
    fn parametric_construction_bounds_cancellation_and_segment_count() {
        assert_eq!(
            construct_parametric_curve_path_cancellable(
                &spiral(SpiralShape::Round, 1.0, CurveWinding::Clockwise),
                Point2::new(0.0, 0.0),
                &|| true
            )
            .unwrap_err()
            .path(),
            "evaluation.cancelled"
        );
        assert_eq!(
            construct_parametric_curve_path_cancellable(
                &spiral(SpiralShape::Square, 1025.0, CurveWinding::Clockwise),
                Point2::new(0.0, 0.0),
                &|| false
            )
            .unwrap_err()
            .path(),
            "curve.parametric.segment_limit"
        );
    }
}
