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

    /// Converts immutable construction geometry into an ID-free authoritative replacement payload.
    ///
    /// The caller supplies the resource identity through `ReplaceAuthoredStructure`; this geometry
    /// boundary never allocates IDs or mutates a document.
    pub fn to_authored_structure_draft(
        &self,
    ) -> Result<toniator_domain::AuthoredStructureDraft, CurveError> {
        let kind = match self.closure() {
            PathClosure::Open => AuthoredStructureKind::OpenPath,
            PathClosure::Closed => AuthoredStructureKind::ClosedShape,
        };
        let segments = self
            .segments()
            .iter()
            .map(|segment| match segment {
                CurveSegment::Line(line) => AuthoredCurveSegment::Line {
                    start: domain_point(line.start()),
                    end: domain_point(line.end()),
                },
                CurveSegment::CubicBezier(cubic) => AuthoredCurveSegment::CubicBezier {
                    start: domain_point(cubic.start()),
                    control_1: domain_point(cubic.control_1()),
                    control_2: domain_point(cubic.control_2()),
                    end: domain_point(cubic.end()),
                },
            })
            .collect();
        toniator_domain::AuthoredStructureDraft::new(kind, segments).map_err(|_| {
            CurveError::new(
                "curve.path.authored",
                "edited path must satisfy authored structure validation",
            )
        })
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

/// Reinterprets finite construction geometry as a document-space authored point.
fn domain_point(point: Point2) -> toniator_domain::AuthoredPoint2 {
    toniator_domain::AuthoredPoint2 {
        x: point.x,
        y: point.y,
    }
}
