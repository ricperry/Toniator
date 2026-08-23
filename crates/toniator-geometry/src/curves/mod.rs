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
pub use parametric::construct_parametric_curve_path_cancellable;
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
pub(crate) const MAX_CLIPPING_FRAGMENTS: usize = 4_096;
pub(crate) const MAX_CLIPPED_SEGMENTS: usize = 65_536;

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
