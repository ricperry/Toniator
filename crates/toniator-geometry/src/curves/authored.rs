use toniator_domain::{AuthoredCurveSegment, AuthoredStructure, AuthoredStructureKind};

use crate::Point2;

use super::{CubicBezierSegment, CurveError, CurvePath, CurveSegment, LineSegment, PathClosure};

impl CurvePath {
    /// Converts one validated document-owned authored structure into Stage 20B construction geometry.
    ///
    /// The conversion preserves exact coordinate bits, segment variants, authored order, and declared
    /// closure. It introduces no path identity, render, guide, cache, or canonical-output authority.
    ///
    /// # Errors
    ///
    /// Propagates the accepted Stage 20B constructor diagnostics if the domain structure cannot be
    /// represented as finite connected construction geometry.
    pub fn from_authored_structure(structure: &AuthoredStructure) -> Result<Self, CurveError> {
        let segments = structure
            .segments()
            .iter()
            .map(authored_segment_to_curve)
            .collect::<Result<Vec<_>, _>>()?;
        let closure = match structure.kind() {
            AuthoredStructureKind::OpenPath => PathClosure::Open,
            AuthoredStructureKind::ClosedShape => PathClosure::Closed,
        };
        Self::new(segments, closure)
    }
}

/// Maps one domain-owned authored segment to the corresponding Stage 20B construction segment.
///
/// # Errors
///
/// Propagates only Stage 20B finite-construction validation failures without changing coordinate bits.
fn authored_segment_to_curve(segment: &AuthoredCurveSegment) -> Result<CurveSegment, CurveError> {
    match segment {
        AuthoredCurveSegment::Line { start, end } => Ok(CurveSegment::Line(LineSegment::new(
            authored_point(*start),
            authored_point(*end),
        )?)),
        AuthoredCurveSegment::CubicBezier {
            start,
            control_1,
            control_2,
            end,
        } => Ok(CurveSegment::CubicBezier(CubicBezierSegment::new(
            authored_point(*start),
            authored_point(*control_1),
            authored_point(*control_2),
            authored_point(*end),
        )?)),
    }
}

/// Reinterprets one validated authored coordinate pair as the identical geometry point.
fn authored_point(point: toniator_domain::AuthoredPoint2) -> Point2 {
    Point2::new(point.x, point.y)
}
