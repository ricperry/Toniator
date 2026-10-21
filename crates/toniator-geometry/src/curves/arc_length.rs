use super::{
    CurveError, CurvePath, CurveSegment, MAX_ARC_LENGTH_LEAVES, MAX_SUBDIVISION_DEPTH,
    MAX_WORK_ITEMS, PARAMETER_TOLERANCE, PathLocation,
    segment::{ArcLengthLeaf, cubic_range},
};

/// Explicit aggregate work/leaf admission for reached artistic arc-length preparation.
/// Numerical depth and error certificates remain fixed; normal creative admission is machine-addressable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcLengthWorkLimits { pub maximum_work_items: usize, pub maximum_leaves: usize }
impl Default for ArcLengthWorkLimits {
    /// Supplies machine-addressable aggregate admission without changing numerical convergence.
    fn default() -> Self { Self { maximum_work_items: usize::MAX, maximum_leaves: usize::MAX } }
}

/// Immutable ordered segment-length table for one validated curve path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathArcLength {
    segments: Vec<SegmentArcLength>,
    segment_ends: Vec<f64>,
    total_length: f64,
}

impl PathArcLength {
    /// Measures every segment in stored order with compensated path summation.
    ///
    /// # Errors
    ///
    /// Propagates bounded segment measurement failures without exposing a partial table.
    pub(crate) fn measure(path: &CurvePath) -> Result<Self, CurveError> {
        Self::measure_cancellable(path, ArcLengthWorkLimits { maximum_work_items: MAX_WORK_ITEMS, maximum_leaves: MAX_ARC_LENGTH_LEAVES }, &|| false)
    }

    /// Prepares the full ordered metric using fallible incremental storage and explicit aggregate policy.
    /// # Errors
    /// Returns cancellation, allocation, count, numeric or unchanged convergence diagnostics atomically.
    pub(crate) fn measure_cancellable(
        path: &CurvePath,
        limits: ArcLengthWorkLimits,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurveError> {
        let mut segments = Vec::new();
        let mut segment_ends = Vec::new();
        let mut budget = InverseBudget::new(limits.maximum_work_items, limits.maximum_leaves);
        let mut total = 0.0;
        let mut compensation = 0.0;
        let mut ordinary_prefix = 0.0;
        for segment in path.segments() {
            super::check_curve_cancelled(cancelled)?;
            super::reserve_curve(&mut segments, 1)?;
            super::reserve_curve(&mut segment_ends, 1)?;
            let profile = segment.arc_length_profile_with_counts_cancellable(
                MAX_SUBDIVISION_DEPTH,
                budget.remaining_work()?,
                budget.remaining_leaves()?,
                cancelled,
            )?;
            budget.consume(profile.work_items, profile.leaves.len())?;
            let leaves = profile.leaves;
            let length = compensated_leaf_sum(&leaves)?;
            let adjusted = length - compensation;
            let next = total + adjusted;
            compensation = (next - total) - adjusted;
            total = next;
            if !total.is_finite() {
                return Err(CurveError::new(
                    "curve.path.numeric_overflow",
                    "curve-path arithmetic must remain finite",
                ));
            }
            ordinary_prefix += length;
            segment_ends.push(ordinary_prefix);
            segments.push(SegmentArcLength {
                segment: *segment,
                leaves,
                length,
            });
        }
        Ok(Self {
            segments,
            segment_ends,
            total_length: total,
        })
    }

    /// Returns the finite complete measured length.
    pub const fn total_length(&self) -> f64 {
        self.total_length
    }

    /// Returns global arc distance for one exact addressed path location under the same measurement policy.
    /// # Errors
    /// Returns invalid-location, cancellation or unchanged bounded quadrature/convergence diagnostics.
    pub fn length_at_location_cancellable(
        &self,
        location: PathLocation,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<f64, CurveError> {
        super::check_curve_cancelled(cancelled)?;
        let measured = self
            .segments
            .get(location.segment_index())
            .ok_or(CurveError::new(
                "curve.path.arc_length.location",
                "arc location must address the measured path",
            ))?;
        let prefix = if location.segment_index() == 0 {
            0.0
        } else {
            self.segment_ends[location.segment_index() - 1]
        };
        let t = location.parameter();
        if t == 0.0 {
            return Ok(prefix);
        }
        if t == 1.0 {
            return Ok(prefix + measured.length);
        }
        let local = match measured.segment {
            CurveSegment::Line(_) => measured.length * t,
            CurveSegment::CubicBezier(cubic) => {
                let partial = CurveSegment::CubicBezier(cubic_range(cubic, 0.0, t)?);
                let profile = partial.arc_length_profile_with_counts_cancellable(
                    MAX_SUBDIVISION_DEPTH,
                    MAX_WORK_ITEMS,
                    MAX_ARC_LENGTH_LEAVES,
                    cancelled,
                )?;
                compensated_leaf_sum(&profile.leaves)?
            }
        };
        let value = prefix + local;
        value.is_finite().then_some(value).ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "arc distance must remain finite",
        ))
    }

    /// Inverts a finite distance monotonically to the earliest matching path location.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.arc_length.distance` when distance is non-finite or outside `[0, total]`.
    pub fn location_at_length(&self, distance: f64) -> Result<PathLocation, CurveError> {
        self.location_at_length_cancellable(distance, &|| false)
    }

    /// Inverts one finite distance while polling cancellation through every numeric refinement.
    /// # Errors
    /// Returns invalid distance, cancellation or unchanged bounded numerical convergence diagnostics.
    pub fn location_at_length_cancellable(
        &self,
        distance: f64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PathLocation, CurveError> {
        super::check_curve_cancelled(cancelled)?;
        if !distance.is_finite() || distance < 0.0 || distance > self.total_length {
            return Err(CurveError::new(
                "curve.path.arc_length.distance",
                "arc-length distance must be finite and within the measured path",
            ));
        }
        if self.total_length == 0.0 {
            return (distance == 0.0)
                .then(|| PathLocation::new(0, 0.0))
                .transpose()?
                .ok_or(CurveError::new(
                    "curve.path.arc_length.distance",
                    "only zero is valid for a zero-length path",
                ));
        }
        if distance == self.total_length {
            return PathLocation::new(self.segments.len() - 1, 1.0);
        }
        let index = self.segment_ends.partition_point(|end| *end < distance);
        if let Some(measured) = self.segments.get(index) {
            let prefix = if index == 0 {
                0.0
            } else {
                self.segment_ends[index - 1]
            };
            let parameter = measured.parameter_at_length_with_limits_cancellable(
                distance - prefix,
                MAX_SUBDIVISION_DEPTH,
                MAX_WORK_ITEMS,
                MAX_ARC_LENGTH_LEAVES,
                cancelled,
            )?;
            return PathLocation::new(index, parameter);
        }
        PathLocation::new(self.segments.len() - 1, 1.0)
    }
}

/// Immutable adaptive measurement retained for one source segment in a path table.
#[derive(Clone, Debug, PartialEq)]
struct SegmentArcLength {
    segment: CurveSegment,
    leaves: Vec<ArcLengthLeaf>,
    length: f64,
}

impl SegmentArcLength {
    /// Inverts within adaptive leaves then refines cubic partial length under one aggregate bounded budget.
    #[cfg(test)]
    fn parameter_at_length(&self, distance: f64) -> Result<f64, CurveError> {
        self.parameter_at_length_with_limits(
            distance,
            MAX_SUBDIVISION_DEPTH,
            MAX_WORK_ITEMS,
            MAX_ARC_LENGTH_LEAVES,
        )
    }

    /// Inverts a segment distance with private reduced limits for atomic inverse-budget witnesses.
    #[cfg(test)]
    fn parameter_at_length_with_limits(
        &self,
        distance: f64,
        maximum_depth: u8,
        maximum_work_items: usize,
        maximum_leaves: usize,
    ) -> Result<f64, CurveError> {
        self.parameter_at_length_with_limits_cancellable(distance, maximum_depth, maximum_work_items, maximum_leaves, &|| false)
    }

    /// Resolves one inverse metric under fixed numeric bounds and a caller cancellation token.
    /// # Errors
    /// Returns cancellation or the established inverse numerical diagnostics.
    fn parameter_at_length_with_limits_cancellable(
        &self,
        distance: f64,
        maximum_depth: u8,
        maximum_work_items: usize,
        maximum_leaves: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<f64, CurveError> {
        super::check_curve_cancelled(cancelled)?;
        if self.length == 0.0 {
            return Ok(0.0);
        }
        if distance <= 0.0 {
            return Ok(0.0);
        }
        if distance >= self.length {
            return Ok(1.0);
        }
        let mut prefix = 0.0;
        for leaf in &self.leaves {
            super::check_curve_cancelled(cancelled)?;
            let end = prefix + leaf.length;
            if distance <= end {
                if leaf.length == 0.0 {
                    return Ok(leaf.start);
                }
                if matches!(self.segment, CurveSegment::Line(_)) {
                    let fraction = ((distance - prefix) / leaf.length).clamp(0.0, 1.0);
                    return Ok(leaf.start + (leaf.end - leaf.start) * fraction);
                }
                return self.refine_cubic_parameter(
                    distance - prefix,
                    leaf.start,
                    leaf.start,
                    leaf.end,
                    InverseLimits {
                        maximum_depth,
                        maximum_work_items,
                        maximum_leaves,
                    },
                    cancelled,
                );
            }
            prefix = end;
        }
        Ok(1.0)
    }

    /// Bisects one selected cubic leaf until its measured partial-length residual meets the fixed policy.
    fn refine_cubic_parameter(
        &self,
        target: f64,
        leaf_start: f64,
        mut low: f64,
        mut high: f64,
        limits: InverseLimits,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<f64, CurveError> {
        let CurveSegment::CubicBezier(cubic) = self.segment else {
            unreachable!("only cubic leaves require nonlinear inverse refinement")
        };
        let mut budget = InverseBudget::new(limits.maximum_work_items, limits.maximum_leaves);
        for _ in 0..MAX_SUBDIVISION_DEPTH {
            super::check_curve_cancelled(cancelled)?;
            let middle = (low + high) * 0.5;
            let partial = cubic_range(cubic, leaf_start, middle)?;
            let profile = CurveSegment::CubicBezier(partial).arc_length_profile_with_counts_cancellable(
                limits.maximum_depth,
                budget.remaining_work()?,
                budget.remaining_leaves()?,
                cancelled,
            )?;
            budget.consume(profile.work_items, profile.leaves.len())?;
            let length = compensated_leaf_sum(&profile.leaves)?;
            let residual = length - target;
            if residual.abs()
                <= super::tolerance([
                    cubic.start(),
                    cubic.control_1(),
                    cubic.control_2(),
                    cubic.end(),
                    partial.start(),
                    partial.control_1(),
                    partial.control_2(),
                    partial.end(),
                ])?
                || high - low <= PARAMETER_TOLERANCE
            {
                return Ok(middle);
            }
            if residual < 0.0 {
                low = middle;
            } else {
                high = middle;
            }
        }
        Err(CurveError::new(
            "curve.path.arc_length.subdivision_limit",
            "arc-length inverse refinement depth limit exceeded",
        ))
    }
}

/// Fixed private limits carried together through one nonlinear inverse refinement.
#[derive(Clone, Copy)]
struct InverseLimits {
    maximum_depth: u8,
    maximum_work_items: usize,
    maximum_leaves: usize,
}

/// Tracks aggregate adaptive subdivision resources consumed by one cubic inverse request.
struct InverseBudget {
    work_items: usize,
    leaves: usize,
    maximum_work_items: usize,
    maximum_leaves: usize,
}

impl InverseBudget {
    /// Creates the private aggregate budget without exposing caller-adjustable public limits.
    const fn new(maximum_work_items: usize, maximum_leaves: usize) -> Self {
        Self {
            work_items: 0,
            leaves: 0,
            maximum_work_items,
            maximum_leaves,
        }
    }

    /// Returns remaining subdivision work or the stable atomic inverse limit failure.
    fn remaining_work(&self) -> Result<usize, CurveError> {
        self.maximum_work_items
            .checked_sub(self.work_items)
            .ok_or(CurveError::new(
                "curve.path.arc_length.subdivision_limit",
                "arc-length inverse subdivision work limit exceeded",
            ))
    }

    /// Returns remaining adaptive leaves or the stable atomic inverse leaf-limit failure.
    fn remaining_leaves(&self) -> Result<usize, CurveError> {
        self.maximum_leaves
            .checked_sub(self.leaves)
            .ok_or(CurveError::new(
                "curve.path.arc_length.result_limit",
                "arc-length inverse leaf limit exceeded",
            ))
    }

    /// Charges one completed partial measurement and rejects resource exhaustion before publication.
    fn consume(&mut self, work_items: usize, leaves: usize) -> Result<(), CurveError> {
        self.work_items = self
            .work_items
            .checked_add(work_items)
            .ok_or(CurveError::new(
                "curve.path.arc_length.subdivision_limit",
                "arc-length inverse subdivision work limit exceeded",
            ))?;
        self.leaves = self.leaves.checked_add(leaves).ok_or(CurveError::new(
            "curve.path.arc_length.result_limit",
            "arc-length inverse leaf limit exceeded",
        ))?;
        self.remaining_work()?;
        self.remaining_leaves()?;
        Ok(())
    }
}

/// Sums finite adaptive leaf lengths with compensation before publishing a measured table.
fn compensated_leaf_sum(leaves: &[ArcLengthLeaf]) -> Result<f64, CurveError> {
    let mut total = 0.0;
    let mut compensation = 0.0;
    for leaf in leaves {
        let adjusted = leaf.length - compensation;
        let next = total + adjusted;
        compensation = (next - total) - adjusted;
        total = next;
    }
    total.is_finite().then_some(total).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

#[cfg(test)]
mod tests {
    use crate::{CubicBezierSegment, CurvePath, CurveSegment, PathClosure, PathLocation, Point2};

    /// Keeps earliest exact joins, repeated zero-length prefixes, and the final endpoint stable
    /// when the path table selects a segment by binary search.
    ///
    /// # Panics
    /// Panics if distance inversion skips an earlier join or loses the last path endpoint.
    #[test]
    fn prefix_lookup_preserves_joins_and_zero_length_segments() {
        let path = CurvePath::polyline(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(3.0, 0.0),
                Point2::new(3.0, 0.0),
                Point2::new(3.0, 4.0),
                Point2::new(3.0, 4.0),
            ],
            PathClosure::Open,
        )
        .expect("finite path with degenerate segments");
        let measured = path.measure_arc_length().expect("finite length table");
        assert_eq!(measured.total_length(), 7.0);
        assert_eq!(
            measured.location_at_length(0.0).unwrap(),
            PathLocation::new(0, 0.0).unwrap()
        );
        assert_eq!(
            measured.location_at_length(3.0).unwrap(),
            PathLocation::new(1, 1.0).unwrap()
        );
        assert_eq!(
            measured.location_at_length(7.0).unwrap(),
            PathLocation::new(4, 1.0).unwrap()
        );
        assert_eq!(measured.location_at_length(3.5).unwrap().segment_index(), 3);
    }

    /// Proves the zero-length inverse contract rejects every positive distance atomically.
    #[test]
    fn zero_length_inverse_rejects_positive_distance() {
        let path = CurvePath::line(Point2::new(2.0, 3.0), Point2::new(2.0, 3.0))
            .expect("finite degenerate line");
        assert_eq!(
            path.measure_arc_length()
                .expect("zero length is measurable")
                .location_at_length(1.0)
                .expect_err("positive distance invalid")
                .path(),
            "curve.path.arc_length.distance"
        );
    }

    /// Proves reduced aggregate inverse resources fail atomically before publishing a cubic parameter.
    #[test]
    fn reduced_inverse_budget_is_atomic() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(0.0, 8.0),
                    Point2::new(10.0, 8.0),
                    Point2::new(10.0, 0.0),
                )
                .expect("finite cubic"),
            )],
            PathClosure::Open,
        )
        .expect("finite path");
        let measured = path.measure_arc_length().expect("measured cubic");
        assert_eq!(
            measured.segments[0]
                .parameter_at_length_with_limits(measured.total_length * 0.25, 48, 1, 1)
                .expect_err("one aggregate work item cannot refine inverse")
                .path(),
            "curve.path.arc_length.subdivision_limit"
        );
    }
}
