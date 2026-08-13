use toniator_domain::{AuthoredPoint2, AuthoredStructure, GuidePrototype};

use crate::{CubicBezierSegment, CurveError, CurvePath, CurveSegment, PathClosure, Point2};

/// Resolves one document-aware or procedural generic guide prototype into finite curve geometry.
///
/// # Errors
///
/// Returns the stable curve constructor diagnostic; authored resource lookup and kind validation
/// remain the document/pattern boundary rather than being inferred by geometry.
pub fn resolve_guide_prototype(
    prototype: &GuidePrototype,
    authored: Option<&AuthoredStructure>,
) -> Result<CurvePath, CurveError> {
    match prototype {
        GuidePrototype::AuthoredOpenPath { .. } => authored
            .ok_or(CurveError::new(
                "curve.guide.prototype.reference",
                "guide prototype requires a resolved authored open path",
            ))
            .and_then(CurvePath::from_authored_structure),
        GuidePrototype::CircularArc {
            center,
            radius,
            start_angle_degrees,
            sweep_angle_degrees,
        } => construct_circular_arc(*center, *radius, *start_angle_degrees, *sweep_angle_degrees),
    }
}

/// Constructs the fixed one-to-four-cubic Stage 20D circular-arc approximation as an open path.
///
/// # Errors
///
/// Returns the stable arc diagnostic when any input or intermediate arithmetic is non-finite;
/// no partial path is exposed.
pub fn construct_circular_arc(
    center: AuthoredPoint2,
    radius: f64,
    start_angle_degrees: f64,
    sweep_angle_degrees: f64,
) -> Result<CurvePath, CurveError> {
    if !center.x.is_finite()
        || !center.y.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !start_angle_degrees.is_finite()
        || !sweep_angle_degrees.is_finite()
        || sweep_angle_degrees == 0.0
        || sweep_angle_degrees.abs() > 360.0
    {
        return Err(CurveError::new(
            "curve.guide.arc",
            "circular-arc construction requires finite valid inputs",
        ));
    }
    let count = (sweep_angle_degrees.abs() / 90.0).ceil() as usize;
    let start = start_angle_degrees.to_radians();
    let span = sweep_angle_degrees.to_radians() / count as f64;
    let mut segments = Vec::with_capacity(count);
    let mut prior = point(center, radius, start)?;
    for index in 0..count {
        let first = start + span * index as f64;
        let second = first + span;
        let factor = 4.0 / 3.0 * (span / 4.0).tan();
        let next = point(center, radius, second)?;
        let first_tangent = Point2::new(-radius * first.sin(), radius * first.cos());
        let second_tangent = Point2::new(-radius * second.sin(), radius * second.cos());
        let control_1 = Point2::new(
            prior.x + factor * first_tangent.x,
            prior.y + factor * first_tangent.y,
        );
        let control_2 = Point2::new(
            next.x - factor * second_tangent.x,
            next.y - factor * second_tangent.y,
        );
        if !control_1.is_finite() || !control_2.is_finite() {
            return Err(CurveError::new(
                "curve.guide.arc",
                "circular-arc construction requires finite valid inputs",
            ));
        }
        segments.push(CurveSegment::CubicBezier(CubicBezierSegment::new(
            prior, control_1, control_2, next,
        )?));
        prior = next;
    }
    CurvePath::new(segments, PathClosure::Open)
}

/// Computes one finite analytical arc endpoint without rounding through a separate representation.
fn point(center: AuthoredPoint2, radius: f64, angle: f64) -> Result<Point2, CurveError> {
    let point = Point2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    );
    point.is_finite().then_some(point).ok_or(CurveError::new(
        "curve.guide.arc",
        "circular-arc construction requires finite valid inputs",
    ))
}
