use super::{
    CurveError, CurvePath, CurveSegment, MAX_ARC_LENGTH_LEAVES, MAX_SUBDIVISION_DEPTH,
    MAX_WORK_ITEMS, PARAMETER_TOLERANCE, PathLocation,
    segment::{ArcLengthLeaf, cubic_range},
};

/// Immutable ordered segment-length table for one validated curve path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathArcLength {
    segments: Vec<SegmentArcLength>,
    total_length: f64,
}

impl PathArcLength {
    /// Measures every segment in stored order with compensated path summation.
    ///
    /// # Errors
    ///
    /// Propagates bounded segment measurement failures without exposing a partial table.
    pub(crate) fn measure(path: &CurvePath) -> Result<Self, CurveError> {
        let mut segments = Vec::with_capacity(path.segments().len());
        let mut budget = InverseBudget::new(MAX_WORK_ITEMS, MAX_ARC_LENGTH_LEAVES);
        let mut total = 0.0;
        let mut compensation = 0.0;
        for segment in path.segments() {
            let profile = segment.arc_length_profile_with_counts(
                MAX_SUBDIVISION_DEPTH,
                budget.remaining_work()?,
                budget.remaining_leaves()?,
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
            segments.push(SegmentArcLength {
                segment: *segment,
                leaves,
                length,
            });
        }
        Ok(Self {
            segments,
            total_length: total,
        })
    }

    /// Returns the finite complete measured length.
    pub const fn total_length(&self) -> f64 {
        self.total_length
    }

    /// Inverts a finite distance monotonically to the earliest matching path location.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.arc_length.distance` when distance is non-finite or outside `[0, total]`.
    pub fn location_at_length(&self, distance: f64) -> Result<PathLocation, CurveError> {
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
        let mut prefix = 0.0;
        for (index, measured) in self.segments.iter().enumerate() {
            let end = prefix + measured.length;
            if distance <= end {
                let parameter = measured.parameter_at_length(distance - prefix)?;
                return PathLocation::new(index, parameter);
            }
            prefix = end;
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
    fn parameter_at_length(&self, distance: f64) -> Result<f64, CurveError> {
        self.parameter_at_length_with_limits(
            distance,
            MAX_SUBDIVISION_DEPTH,
            MAX_WORK_ITEMS,
            MAX_ARC_LENGTH_LEAVES,
        )
    }

    /// Inverts a segment distance with private reduced limits for atomic inverse-budget witnesses.
    fn parameter_at_length_with_limits(
        &self,
        distance: f64,
        maximum_depth: u8,
        maximum_work_items: usize,
        maximum_leaves: usize,
    ) -> Result<f64, CurveError> {
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
    ) -> Result<f64, CurveError> {
        let CurveSegment::CubicBezier(cubic) = self.segment else {
            unreachable!("only cubic leaves require nonlinear inverse refinement")
        };
        let mut budget = InverseBudget::new(limits.maximum_work_items, limits.maximum_leaves);
        for _ in 0..MAX_SUBDIVISION_DEPTH {
            let middle = (low + high) * 0.5;
            let partial = cubic_range(cubic, leaf_start, middle)?;
            let profile = CurveSegment::CubicBezier(partial).arc_length_profile_with_counts(
                limits.maximum_depth,
                budget.remaining_work()?,
                budget.remaining_leaves()?,
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
    use crate::{CubicBezierSegment, CurvePath, CurveSegment, PathClosure, Point2};

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
