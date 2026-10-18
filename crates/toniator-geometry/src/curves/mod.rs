//! Finite, deterministic construction geometry for connected curve paths.

mod arc_length;
mod authored;
mod clipping;
mod intersections;
mod parametric;
mod path;
mod segment;

pub use arc_length::PathArcLength;
pub use intersections::{IntersectionKind, PathIntersection, SegmentIntersection};
pub use parametric::{
    construct_parametric_curve_path_cancellable,
    construct_parametric_curve_path_with_limits_cancellable,
};
pub use path::{CurvePath, PathClosure, PathLocation};
pub use segment::{CubicBezierSegment, CurveSegment, LineSegment};

use std::{error::Error, fmt};

use crate::Point2;

pub(crate) const ABSOLUTE_TOLERANCE: f64 = 1.0e-9;
pub(crate) const RELATIVE_TOLERANCE: f64 = 64.0 * f64::EPSILON;
pub(crate) const PARAMETER_TOLERANCE: f64 = 1.0e-12;
pub(crate) const MAX_SUBDIVISION_DEPTH: u8 = 48;
pub(crate) const MAX_WORK_ITEMS: usize = 262_144;
pub(crate) const MAX_ARC_LENGTH_LEAVES: usize = 65_536;
pub(crate) const MAX_SEGMENT_PAIRS: usize = 262_144;
pub(crate) const MAX_INTERSECTIONS: usize = 4_096;

/// Geometry-owned storage/work policy for canonical construction and clipping only.
/// Numerical depth/tolerances and unrelated intersection/arc-length policies remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CurveWorkLimits {
    maximum_segments: usize,
    maximum_clipping_work: usize,
    maximum_clipping_fragments: usize,
    maximum_clipped_segments: usize,
}

impl CurveWorkLimits {
    /// Returns the explicit maximum segments policy.
    pub const fn maximum_segments(self) -> usize {
        self.maximum_segments
    }

    /// Returns the explicit maximum clipping work policy.
    pub const fn maximum_clipping_work(self) -> usize {
        self.maximum_clipping_work
    }

    /// Returns the explicit maximum clipping fragments policy.
    pub const fn maximum_clipping_fragments(self) -> usize {
        self.maximum_clipping_fragments
    }

    /// Returns the explicit maximum clipped segments policy.
    pub const fn maximum_clipped_segments(self) -> usize {
        self.maximum_clipped_segments
    }

    /// Validates explicit nonzero finite caller bounds without creating creative defaults.
    /// # Errors
    /// Returns curve.path.limits if any caller disables an operation entirely.
    pub fn new(
        segments: usize,
        work: usize,
        fragments: usize,
        clipped: usize,
    ) -> Result<Self, CurveError> {
        if [segments, work, fragments, clipped].contains(&0) {
            return Err(CurveError::new(
                "curve.path.limits",
                "curve work limits must be nonzero",
            ));
        }
        Ok(Self {
            maximum_segments: segments,
            maximum_clipping_work: work,
            maximum_clipping_fragments: fragments,
            maximum_clipped_segments: clipped,
        })
    }
}
impl Default for CurveWorkLimits {
    /// Supplies machine-addressable creative work while keeping algorithmic numerical bounds.
    fn default() -> Self {
        Self {
            maximum_segments: usize::MAX,
            maximum_clipping_work: usize::MAX,
            maximum_clipping_fragments: usize::MAX,
            maximum_clipped_segments: usize::MAX,
        }
    }
}

/// Rejects cancellation before checked work or fallible storage operations.
/// # Errors
/// Returns evaluation.cancelled without exposing partial geometry.
pub(crate) fn check_curve_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), CurveError> {
    if cancelled() {
        Err(CurveError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Converts an already rounded nonnegative finite count before Rust's saturating float cast.
/// # Errors
/// Rejects machine representation overflow, including usize::MAX rounded upward as f64.
pub(crate) fn curve_count(value: f64) -> Result<usize, CurveError> {
    if !value.is_finite() || value < 0.0 || value >= usize::MAX as f64 {
        return Err(CurveError::new(
            "curve.path.count_overflow",
            "curve count exceeds machine representation",
        ));
    }
    Ok(value as usize)
}

#[cfg(test)]
std::thread_local! { static FAIL_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

/// Rejects a proven unaddressable element count before the first allocation.
/// # Errors
/// Returns curve.path.count_overflow when the exact byte extent exceeds machine addressability.
pub(crate) fn curve_storage_count<T>(count: usize) -> Result<usize, CurveError> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .map(|_| count)
        .ok_or(CurveError::new(
            "curve.path.count_overflow",
            "curve storage exceeds machine addressability",
        ))
}

/// Reserves only incremental reached storage and reports allocator failure atomically.
/// # Errors
/// Returns checked address/count overflow or curve.path.allocation before any push.
pub(crate) fn reserve_curve<T>(storage: &mut Vec<T>, additional: usize) -> Result<(), CurveError> {
    let count = storage
        .len()
        .checked_add(additional)
        .filter(|n| {
            n.checked_mul(std::mem::size_of::<T>())
                .is_some_and(|bytes| bytes <= isize::MAX as usize)
        })
        .ok_or(CurveError::new(
            "curve.path.count_overflow",
            "curve storage exceeds machine addressability",
        ))?;
    if count <= storage.capacity() {
        return Ok(());
    }
    #[cfg(test)]
    if FAIL_RESERVATION.with(|flag| flag.replace(false)) {
        return Err(CurveError::new(
            "curve.path.allocation",
            "curve storage allocation failed",
        ));
    }
    storage
        .try_reserve(additional)
        .map_err(|_| CurveError::new("curve.path.allocation", "curve storage allocation failed"))
}

#[cfg(test)]
mod capacity_tests {
    use super::*;
    /// Exercises reported allocation failure through the production constructor reserve helper.
    /// # Panics
    /// Panics if the injected allocator report is lost or a later ordinary request cannot recover.
    #[test]
    fn production_constructor_reports_reserve_failure_atomically() {
        let curve = toniator_domain::ParametricCurve::Spiral(toniator_domain::SpiralCurve {
            shape: toniator_domain::SpiralShape::Round,
            turns: 1.0,
            radial_spacing: 16.0,
            phase_degrees: 0.0,
            winding: toniator_domain::CurveWinding::Clockwise,
        });
        FAIL_RESERVATION.with(|flag| flag.set(true));
        assert_eq!(
            construct_parametric_curve_path_cancellable(&curve, Point2::new(0.0, 0.0), &|| false)
                .unwrap_err()
                .path(),
            "curve.path.allocation"
        );
        assert!(
            construct_parametric_curve_path_cancellable(&curve, Point2::new(0.0, 0.0), &|| false)
                .is_ok()
        );
    }

    /// Reports real reserve-helper failures during clipping and derived outline storage without publication.
    /// # Panics
    /// Panics if the production helpers ignore the injected allocation report or fail recovery.
    #[test]
    fn production_clip_and_outline_report_reserve_failure_atomically() {
        let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)).unwrap();
        let bounds = crate::Bounds::new(Point2::new(-1.0, -1.0), Point2::new(11.0, 1.0)).unwrap();
        FAIL_RESERVATION.with(|flag| flag.set(true));
        assert_eq!(
            path.clip_to_bounds(bounds).unwrap_err().path(),
            "curve.path.allocation"
        );
        assert_eq!(path.clip_to_bounds(bounds).unwrap(), vec![path.clone()]);
        let samples = [0.0, 0.5, 1.0].map(|t| crate::VariableWidthPathSample {
            location: PathLocation::new(0, t).unwrap(),
            width: 2.0,
        });
        let build = || {
            crate::build_variable_width_outline_cancellable(
                &path,
                &samples,
                toniator_domain::PathStrokeStyle::default(),
                0.0,
                0.125,
                crate::VariableWidthOutlineLimits::new(100).unwrap(),
                &|| false,
            )
        };
        FAIL_RESERVATION.with(|flag| flag.set(true));
        assert_eq!(build().unwrap_err().path(), "curve.path.allocation");
        assert!(!build().unwrap().contours.is_empty());
    }
}

/// Stable failure reported by the canonical curve-path boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurveError {
    path: &'static str,
    message: &'static str,
}

impl CurveError {
    /// Creates a stable curve-path diagnostic without exposing mutable geometry state.
    pub(crate) const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    /// Returns the stable diagnostic path owned by the curve-path contract.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the fixed human-readable diagnostic message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for CurveError {
    /// Formats the stable failure without adding caller-specific context.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for CurveError {}

/// Returns a scale-aware tolerance for finite points under the fixed policy.
pub(crate) fn tolerance(points: impl IntoIterator<Item = Point2>) -> Result<f64, CurveError> {
    let mut scale = 1.0_f64;
    for point in points {
        if !point.is_finite() {
            return Err(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            ));
        }
        scale = scale.max(point.x.abs()).max(point.y.abs());
    }
    let value = ABSOLUTE_TOLERANCE + RELATIVE_TOLERANCE * scale;
    value.is_finite().then_some(value).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Returns a finite point displacement while rejecting arithmetic overflow.
pub(crate) fn subtract(first: Point2, second: Point2) -> Result<(f64, f64), CurveError> {
    let x = first.x - second.x;
    let y = first.y - second.y;
    (x.is_finite() && y.is_finite())
        .then_some((x, y))
        .ok_or(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ))
}

/// Returns a finite Euclidean distance while rejecting arithmetic overflow.
pub(crate) fn distance(first: Point2, second: Point2) -> Result<f64, CurveError> {
    let (x, y) = subtract(first, second)?;
    let length = x.hypot(y);
    length.is_finite().then_some(length).ok_or(CurveError::new(
        "curve.path.numeric_overflow",
        "curve-path arithmetic must remain finite",
    ))
}

/// Snaps an in-range parameter onto a contract endpoint when it is sufficiently close.
pub(crate) fn snap_parameter(parameter: f64) -> f64 {
    if parameter.abs() <= PARAMETER_TOLERANCE {
        0.0
    } else if (1.0 - parameter).abs() <= PARAMETER_TOLERANCE {
        1.0
    } else {
        parameter
    }
}

/// Validates one segment-local parameter without inventing a global path parameter.
pub(crate) fn parameter(parameter: f64) -> Result<f64, CurveError> {
    if !parameter.is_finite() || !(0.0..=1.0).contains(&parameter) {
        return Err(CurveError::new(
            "curve.path.location.parameter",
            "path parameters must be finite values in [0, 1]",
        ));
    }
    Ok(snap_parameter(parameter))
}

/// Tests finite points for scale-aware geometric coincidence.
pub(crate) fn coincident(first: Point2, second: Point2) -> Result<bool, CurveError> {
    Ok(distance(first, second)? <= tolerance([first, second])?)
}
