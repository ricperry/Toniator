use crate::{AffineTransform2D, Bounds, Point2, Vector2};

use super::{
    ABSOLUTE_TOLERANCE, CurveError, MAX_ARC_LENGTH_LEAVES, MAX_SUBDIVISION_DEPTH, MAX_WORK_ITEMS,
    SegmentIntersection, distance, intersections, parameter, tolerance,
};

/// One finite line segment with explicit construction endpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegment {
    start: Point2,
    end: Point2,
}

impl LineSegment {
    /// Constructs finite line geometry without imposing a nonzero-length invariant.
    ///
    /// # Errors
    ///
    /// Returns `curve.segment.coordinates` when either endpoint is non-finite.
    pub fn new(start: Point2, end: Point2) -> Result<Self, CurveError> {
        finite_points([start, end])?;
        Ok(Self { start, end })
    }

    /// Returns the immutable authored start endpoint.
    pub const fn start(&self) -> Point2 {
        self.start
    }

    /// Returns the immutable authored end endpoint.
    pub const fn end(&self) -> Point2 {
        self.end
    }
}

/// One finite cubic Bézier segment with explicit authored control points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezierSegment {
    start: Point2,
    control_1: Point2,
    control_2: Point2,
    end: Point2,
}

impl CubicBezierSegment {
    /// Constructs finite cubic construction geometry without imposing motion or smoothness.
    ///
    /// # Errors
    ///
    /// Returns `curve.segment.coordinates` when an endpoint or control point is non-finite.
    pub fn new(
        start: Point2,
        control_1: Point2,
        control_2: Point2,
        end: Point2,
    ) -> Result<Self, CurveError> {
        finite_points([start, control_1, control_2, end])?;
        Ok(Self {
            start,
            control_1,
            control_2,
            end,
        })
    }

    /// Returns the immutable authored start endpoint.
    pub const fn start(&self) -> Point2 {
        self.start
    }

    /// Returns the immutable first construction control point.
    pub const fn control_1(&self) -> Point2 {
        self.control_1
    }

    /// Returns the immutable second construction control point.
    pub const fn control_2(&self) -> Point2 {
        self.control_2
    }

    /// Returns the immutable authored end endpoint.
    pub const fn end(&self) -> Point2 {
        self.end
    }
}

/// One finite curve segment, retaining its authored line or cubic kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveSegment {
    Line(LineSegment),
    CubicBezier(CubicBezierSegment),
}

impl CurveSegment {
    /// Returns the immutable segment start without changing construction kind.
    pub const fn start(&self) -> Point2 {
        match self {
            Self::Line(line) => line.start(),
            Self::CubicBezier(cubic) => cubic.start(),
        }
    }

    /// Returns the immutable segment end without changing construction kind.
    pub const fn end(&self) -> Point2 {
        match self {
            Self::Line(line) => line.end(),
            Self::CubicBezier(cubic) => cubic.end(),
        }
    }

    /// Evaluates an exact segment-local Bernstein point for a finite parameter in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a stable parameter or numeric-overflow failure; it never extrapolates.
    pub fn point_at(&self, parameter_value: f64) -> Result<Point2, CurveError> {
        let t = parameter(parameter_value)?;
        let point = match self {
            Self::Line(line) => lerp(line.start, line.end, t)?,
            Self::CubicBezier(cubic) => cubic_point(*cubic, t)?,
        };
        point.is_finite().then_some(point).ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
    }

    /// Returns the normalized analytic derivative at a segment-local parameter.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.tangent.stationary` for a derivative within its local
    /// control-polygon threshold, and otherwise returns parameter or overflow failures.
    pub fn unit_tangent_at(&self, parameter_value: f64) -> Result<Vector2, CurveError> {
        let t = parameter(parameter_value)?;
        let derivative = match self {
            Self::Line(line) => vector(line.start, line.end)?,
            Self::CubicBezier(cubic) => cubic_derivative(*cubic, t)?,
        };
        normalize(derivative, self.control_polygon_length()?)
    }

    /// Returns the left-hand unit normal derived from the segment-local tangent.
    ///
    /// # Errors
    ///
    /// Propagates the exact parameter, stationary-tangent, and overflow failures from tangent evaluation.
    pub fn unit_normal_at(&self, parameter_value: f64) -> Result<Vector2, CurveError> {
        Ok(self.unit_tangent_at(parameter_value)?.perpendicular())
    }

    /// Returns analytical line bounds or conservative cubic extrema bounds.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.numeric_overflow` when required finite arithmetic cannot be represented.
    pub fn bounds(&self) -> Result<Bounds, CurveError> {
        match self {
            Self::Line(line) => Bounds::from_points([line.start, line.end]).ok_or(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            )),
            Self::CubicBezier(cubic) => cubic_bounds(*cubic),
        }
    }

    /// Measures this segment under the fixed bounded adaptive arc-length policy.
    ///
    /// # Errors
    ///
    /// Returns a stable subdivision or numeric-overflow failure without a partial length.
    pub fn arc_length(&self) -> Result<f64, CurveError> {
        self.arc_length_with_limits(MAX_SUBDIVISION_DEPTH, MAX_WORK_ITEMS, MAX_ARC_LENGTH_LEAVES)
    }

    /// Intersects two finite segments with deterministic ordering and bounded refinement.
    ///
    /// # Errors
    ///
    /// Returns overlap, subdivision, result-limit, or numeric failures without partial results.
    pub fn intersections(&self, other: &Self) -> Result<Vec<SegmentIntersection>, CurveError> {
        intersections::segment_intersections(*self, *other)
    }

    /// Applies the existing affine-transform authority while preserving segment kind.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.transform.non_finite` if transformed construction geometry is non-finite.
    pub fn transformed(&self, transform: AffineTransform2D) -> Result<Self, CurveError> {
        let transform_point = |point| {
            let transformed = transform.apply_point(point);
            transformed
                .is_finite()
                .then_some(transformed)
                .ok_or(CurveError::new(
                    "curve.path.transform.non_finite",
                    "affine transformation must retain finite curve coordinates",
                ))
        };
        match self {
            Self::Line(line) => Ok(Self::Line(LineSegment::new(
                transform_point(line.start)?,
                transform_point(line.end)?,
            )?)),
            Self::CubicBezier(cubic) => Ok(Self::CubicBezier(CubicBezierSegment::new(
                transform_point(cubic.start)?,
                transform_point(cubic.control_1)?,
                transform_point(cubic.control_2)?,
                transform_point(cubic.end)?,
            )?)),
        }
    }

    /// Measures one segment with private reduced limits for bounded internal witnesses.
    pub(crate) fn arc_length_with_limits(
        &self,
        maximum_depth: u8,
        maximum_work_items: usize,
        maximum_leaves: usize,
    ) -> Result<f64, CurveError> {
        if let Self::Line(line) = self {
            return distance(line.start, line.end);
        }
        let Self::CubicBezier(cubic) = self else {
            unreachable!("line segments returned before cubic subdivision")
        };
        let mut stack = vec![(*cubic, 0_u8)];
        let mut work_items = 0_usize;
        let mut leaves = 0_usize;
        let mut sum = 0.0;
        let mut compensation = 0.0;
        while let Some((node, depth)) = stack.pop() {
            work_items += 1;
            if work_items > maximum_work_items {
                return Err(CurveError::new(
                    "curve.path.arc_length.subdivision_limit",
                    "arc-length subdivision work limit exceeded",
                ));
            }
            let chord = distance(node.start, node.end)?;
            let polygon = cubic_polygon_length(node)?;
            if polygon - chord <= tolerance([node.start, node.control_1, node.control_2, node.end])?
            {
                leaves += 1;
                if leaves > maximum_leaves {
                    return Err(CurveError::new(
                        "curve.path.arc_length.result_limit",
                        "arc-length leaf limit exceeded",
                    ));
                }
                let value = (polygon + chord) * 0.5;
                if !value.is_finite() {
                    return Err(CurveError::new(
                        "curve.path.numeric_overflow",
                        "curve-path arithmetic must remain finite",
                    ));
                }
                let adjusted = value - compensation;
                let next = sum + adjusted;
                compensation = (next - sum) - adjusted;
                sum = next;
            } else if depth >= maximum_depth {
                return Err(CurveError::new(
                    "curve.path.arc_length.subdivision_limit",
                    "arc-length subdivision depth limit exceeded",
                ));
            } else {
                let (left, right) = split_cubic(node, 0.5)?;
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
        }
        sum.is_finite().then_some(sum).ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
    }

    /// Produces adaptive leaves together with exact consumed work for aggregate inverse-lookup budgets.
    pub(crate) fn arc_length_profile_with_counts(
        &self,
        maximum_depth: u8,
        maximum_work_items: usize,
        maximum_leaves: usize,
    ) -> Result<ArcLengthProfile, CurveError> {
        if let Self::Line(line) = self {
            return Ok(ArcLengthProfile {
                leaves: vec![ArcLengthLeaf {
                    start: 0.0,
                    end: 1.0,
                    length: distance(line.start, line.end)?,
                }],
                work_items: 1,
            });
        }
        let Self::CubicBezier(cubic) = self else {
            unreachable!("line segments returned before cubic subdivision")
        };
        let mut stack = vec![(*cubic, 0.0, 1.0, 0_u8)];
        let mut work_items = 0_usize;
        let mut leaves = Vec::new();
        while let Some((node, start, end, depth)) = stack.pop() {
            work_items += 1;
            if work_items > maximum_work_items {
                return Err(CurveError::new(
                    "curve.path.arc_length.subdivision_limit",
                    "arc-length subdivision work limit exceeded",
                ));
            }
            let chord = distance(node.start, node.end)?;
            let polygon = cubic_polygon_length(node)?;
            if polygon - chord <= tolerance([node.start, node.control_1, node.control_2, node.end])?
            {
                if leaves.len() >= maximum_leaves {
                    return Err(CurveError::new(
                        "curve.path.arc_length.result_limit",
                        "arc-length leaf limit exceeded",
                    ));
                }
                leaves.push(ArcLengthLeaf {
                    start,
                    end,
                    length: (polygon + chord) * 0.5,
                });
            } else if depth >= maximum_depth {
                return Err(CurveError::new(
                    "curve.path.arc_length.subdivision_limit",
                    "arc-length subdivision depth limit exceeded",
                ));
            } else {
                let (left, right) = split_cubic(node, 0.5)?;
                let middle = (start + end) * 0.5;
                stack.push((right, middle, end, depth + 1));
                stack.push((left, start, middle, depth + 1));
            }
        }
        Ok(ArcLengthProfile { leaves, work_items })
    }

    /// Returns the analytic non-normalized derivative for bounded intersection refinement.
    pub(crate) fn derivative_at(&self, parameter_value: f64) -> Result<Vector2, CurveError> {
        let t = parameter(parameter_value)?;
        match self {
            Self::Line(line) => vector(line.start, line.end),
            Self::CubicBezier(cubic) => cubic_derivative(*cubic, t),
        }
    }

    /// Returns the local construction-polygon scale used for stationary classification.
    pub(crate) fn control_polygon_length(&self) -> Result<f64, CurveError> {
        match self {
            Self::Line(line) => distance(line.start, line.end),
            Self::CubicBezier(cubic) => cubic_polygon_length(*cubic),
        }
    }
}

/// One immutable adaptive segment-local length interval used by a measured path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ArcLengthLeaf {
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) length: f64,
}

/// Ordered adaptive leaves with the finite work count consumed to establish them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArcLengthProfile {
    pub(crate) leaves: Vec<ArcLengthLeaf>,
    pub(crate) work_items: usize,
}

/// Validates construction points without accepting non-finite segment coordinates.
pub(crate) fn finite_points(points: impl IntoIterator<Item = Point2>) -> Result<(), CurveError> {
    points
        .into_iter()
        .all(Point2::is_finite)
        .then_some(())
        .ok_or(CurveError::new(
            "curve.segment.coordinates",
            "segment coordinates and control points must be finite",
        ))
}

/// Interpolates two finite points and rejects any non-finite intermediate result.
pub(crate) fn lerp(first: Point2, second: Point2, t: f64) -> Result<Point2, CurveError> {
    let x = first.x + (second.x - first.x) * t;
    let y = first.y + (second.y - first.y) * t;
    let point = Point2::new(x, y);
    point.is_finite().then_some(point).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Returns a finite vector from the first point to the second point.
pub(crate) fn vector(first: Point2, second: Point2) -> Result<Vector2, CurveError> {
    let x = second.x - first.x;
    let y = second.y - first.y;
    (x.is_finite() && y.is_finite())
        .then_some(Vector2::new(x, y))
        .ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
}

/// Normalizes a derivative against a local construction-scale stationary threshold.
pub(crate) fn normalize(vector_value: Vector2, scale: f64) -> Result<Vector2, CurveError> {
    let length = vector_value.x.hypot(vector_value.y);
    let threshold = ABSOLUTE_TOLERANCE + super::RELATIVE_TOLERANCE * scale.max(1.0);
    if !length.is_finite() {
        return Err(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ));
    }
    if length <= threshold {
        return Err(CurveError::new(
            "curve.path.tangent.stationary",
            "curve segment derivative is stationary at this parameter",
        ));
    }
    Ok(Vector2::new(
        vector_value.x / length,
        vector_value.y / length,
    ))
}

/// Evaluates one cubic Bézier point with its Bernstein form.
pub(crate) fn cubic_point(cubic: CubicBezierSegment, t: f64) -> Result<Point2, CurveError> {
    let inverse = 1.0 - t;
    let first = inverse * inverse * inverse;
    let second = 3.0 * inverse * inverse * t;
    let third = 3.0 * inverse * t * t;
    let fourth = t * t * t;
    let point = Point2::new(
        first * cubic.start.x
            + second * cubic.control_1.x
            + third * cubic.control_2.x
            + fourth * cubic.end.x,
        first * cubic.start.y
            + second * cubic.control_1.y
            + third * cubic.control_2.y
            + fourth * cubic.end.y,
    );
    point.is_finite().then_some(point).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Evaluates one analytic cubic derivative at a valid segment-local parameter.
pub(crate) fn cubic_derivative(cubic: CubicBezierSegment, t: f64) -> Result<Vector2, CurveError> {
    let inverse = 1.0 - t;
    let first = vector(cubic.start, cubic.control_1)?;
    let second = vector(cubic.control_1, cubic.control_2)?;
    let third = vector(cubic.control_2, cubic.end)?;
    let x = 3.0 * (inverse * inverse * first.x + 2.0 * inverse * t * second.x + t * t * third.x);
    let y = 3.0 * (inverse * inverse * first.y + 2.0 * inverse * t * second.y + t * t * third.y);
    (x.is_finite() && y.is_finite())
        .then_some(Vector2::new(x, y))
        .ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
}

/// Splits a cubic exactly with de Casteljau construction at a valid local parameter.
pub(crate) fn split_cubic(
    cubic: CubicBezierSegment,
    t: f64,
) -> Result<(CubicBezierSegment, CubicBezierSegment), CurveError> {
    let first = lerp(cubic.start, cubic.control_1, t)?;
    let second = lerp(cubic.control_1, cubic.control_2, t)?;
    let third = lerp(cubic.control_2, cubic.end, t)?;
    let fourth = lerp(first, second, t)?;
    let fifth = lerp(second, third, t)?;
    let middle = lerp(fourth, fifth, t)?;
    Ok((
        CubicBezierSegment::new(cubic.start, first, fourth, middle)?,
        CubicBezierSegment::new(middle, fifth, third, cubic.end)?,
    ))
}

/// Returns an exact sub-cubic preserving source direction over the supplied parameter interval.
pub(crate) fn cubic_range(
    cubic: CubicBezierSegment,
    start: f64,
    end: f64,
) -> Result<CubicBezierSegment, CurveError> {
    if start == 0.0 && end == 1.0 {
        return Ok(cubic);
    }
    let (left, _) = split_cubic(cubic, end)?;
    let ratio = if end == 0.0 { 0.0 } else { start / end };
    let (_, selected) = split_cubic(left, ratio)?;
    Ok(selected)
}

/// Returns the finite control-polygon length of one cubic.
pub(crate) fn cubic_polygon_length(cubic: CubicBezierSegment) -> Result<f64, CurveError> {
    let length = distance(cubic.start, cubic.control_1)?
        + distance(cubic.control_1, cubic.control_2)?
        + distance(cubic.control_2, cubic.end)?;
    length.is_finite().then_some(length).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Returns conservative cubic bounds by solving each derivative quadratic in `(0, 1)`.
pub(crate) fn cubic_bounds(cubic: CubicBezierSegment) -> Result<Bounds, CurveError> {
    let mut points = vec![cubic.start, cubic.end];
    for t in derivative_roots(
        cubic.start.x,
        cubic.control_1.x,
        cubic.control_2.x,
        cubic.end.x,
    )? {
        points.push(cubic_point(cubic, t)?);
    }
    for t in derivative_roots(
        cubic.start.y,
        cubic.control_1.y,
        cubic.control_2.y,
        cubic.end.y,
    )? {
        points.push(cubic_point(cubic, t)?);
    }
    let bounds = Bounds::from_points(points).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))?;
    let expansion = tolerance([cubic.start, cubic.control_1, cubic.control_2, cubic.end])?;
    bounds.expanded(expansion).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Solves one cubic-coordinate derivative quadratic and retains strict unit-interval extrema.
fn derivative_roots(
    first: f64,
    second: f64,
    third: f64,
    fourth: f64,
) -> Result<Vec<f64>, CurveError> {
    let a = -first + 3.0 * second - 3.0 * third + fourth;
    let b = 2.0 * (first - 2.0 * second + third);
    let c = second - first;
    if !(a.is_finite() && b.is_finite() && c.is_finite()) {
        return Err(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ));
    }
    let coordinate_scale = first
        .abs()
        .max(second.abs())
        .max(third.abs())
        .max(fourth.abs())
        .max(1.0);
    let coefficient_tolerance =
        ABSOLUTE_TOLERANCE + super::RELATIVE_TOLERANCE * coordinate_scale * 8.0;
    let mut roots = Vec::new();
    if a.abs() <= coefficient_tolerance {
        if b.abs() > coefficient_tolerance {
            roots.push(-c / b);
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if !discriminant.is_finite() {
            return Err(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            ));
        }
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            roots.push((-b + root) / (2.0 * a));
            roots.push((-b - root) / (2.0 * a));
        }
    }
    roots.retain(|root| root.is_finite() && *root > 0.0 && *root < 1.0);
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|first_root, second_root| {
        (*first_root - *second_root).abs() <= super::PARAMETER_TOLERANCE
    });
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves private reduced arc-length leaf budgets reject before exposing a partial measurement.
    #[test]
    fn reduced_arc_length_leaf_budget_is_atomic() {
        let segment = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 100.0),
                Point2::new(100.0, -100.0),
                Point2::new(100.0, 0.0),
            )
            .expect("finite cubic"),
        );
        assert_eq!(
            segment
                .arc_length_with_limits(48, 262_144, 1)
                .expect_err("one leaf cannot measure this cubic")
                .path(),
            "curve.path.arc_length.result_limit"
        );
    }
}
