use crate::{AffineTransform2D, Bounds, Point2, Vector2};

use super::{
    CurveError, CurveSegment, LineSegment, MAX_CLIPPED_SEGMENTS, PathIntersection,
    arc_length::PathArcLength, clipping, intersections, parameter,
};

/// Authored topological closure of a connected path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathClosure {
    /// The path has no closure requirement, even when its endpoints happen to coincide.
    Open,
    /// The supplied final endpoint equals the supplied initial endpoint exactly.
    Closed,
}

/// A segment-local path coordinate rather than a global normalized parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathLocation {
    segment_index: usize,
    parameter: f64,
}

impl PathLocation {
    /// Creates a segment-local coordinate after validating only its finite unit parameter.
    ///
    /// Segment-index validity belongs to the receiving `CurvePath` because paths own their extent.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.location.parameter` for a non-finite or out-of-range parameter.
    pub fn new(segment_index: usize, parameter_value: f64) -> Result<Self, CurveError> {
        Ok(Self {
            segment_index,
            parameter: parameter(parameter_value)?,
        })
    }

    /// Returns the immutable segment index in the receiving path's stored order.
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// Returns the immutable segment-local parameter in `[0, 1]`.
    pub const fn parameter(&self) -> f64 {
        self.parameter
    }
}

/// Validated connected finite construction geometry with explicit closure semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct CurvePath {
    segments: Vec<CurveSegment>,
    closure: PathClosure,
}

impl CurvePath {
    /// Builds a connected finite path without manufacturing a closure segment or smoothing joins.
    ///
    /// # Errors
    ///
    /// Returns a stable empty, limit, continuity, or closure failure before exposing a path.
    pub fn new(segments: Vec<CurveSegment>, closure: PathClosure) -> Result<Self, CurveError> {
        if segments.is_empty() {
            return Err(CurveError::new(
                "curve.path.segments.empty",
                "curve paths require at least one segment",
            ));
        }
        if segments.len() > 4_096 {
            return Err(CurveError::new(
                "curve.path.segments.limit",
                "curve paths support at most 4096 segments",
            ));
        }
        for pair in segments.windows(2) {
            if pair[0].end() != pair[1].start() {
                return Err(CurveError::new(
                    "curve.path.continuity",
                    "adjacent segment endpoints must be exactly equal",
                ));
            }
        }
        if closure == PathClosure::Closed
            && segments.last().expect("nonempty path").end() != segments[0].start()
        {
            return Err(CurveError::new(
                "curve.path.closure",
                "closed paths require the final endpoint to equal the initial start",
            ));
        }
        Ok(Self { segments, closure })
    }

    /// Builds the smallest explicit open line path from finite endpoints.
    ///
    /// # Errors
    ///
    /// Propagates finite-coordinate validation from the line segment constructor.
    pub fn line(start: Point2, end: Point2) -> Result<Self, CurveError> {
        Self::new(
            vec![CurveSegment::Line(LineSegment::new(start, end)?)],
            PathClosure::Open,
        )
    }

    /// Builds an explicit line path from vertices, adding only the authored closed convenience edge.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.polyline.vertices` for fewer than two or non-finite vertices, or a
    /// stable general path failure if the bounded constructed segment list is invalid.
    pub fn polyline(vertices: Vec<Point2>, closure: PathClosure) -> Result<Self, CurveError> {
        if vertices.len() < 2 || vertices.iter().any(|point| !point.is_finite()) {
            return Err(CurveError::new(
                "curve.path.polyline.vertices",
                "polylines require at least two finite vertices",
            ));
        }
        let mut segments = vertices
            .windows(2)
            .map(|pair| LineSegment::new(pair[0], pair[1]).map(CurveSegment::Line))
            .collect::<Result<Vec<_>, _>>()?;
        if closure == PathClosure::Closed && vertices.last() != vertices.first() {
            segments.push(CurveSegment::Line(LineSegment::new(
                *vertices.last().expect("two vertices"),
                vertices[0],
            )?));
        }
        Self::new(segments, closure)
    }

    /// Returns source segments in immutable authored order.
    pub fn segments(&self) -> &[CurveSegment] {
        &self.segments
    }

    /// Returns the explicit authored closure without inferring it from endpoint equality.
    pub const fn closure(&self) -> PathClosure {
        self.closure
    }

    /// Returns the first authored segment start.
    pub fn start(&self) -> Point2 {
        self.segments[0].start()
    }

    /// Returns the last authored segment end.
    pub fn end(&self) -> Point2 {
        self.segments.last().expect("validated nonempty path").end()
    }

    /// Evaluates a path location on exactly its stored segment, never averaging join behavior.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.location.segment` when the location does not address this path.
    pub fn point_at(&self, location: PathLocation) -> Result<Point2, CurveError> {
        self.segment_for(location)?.point_at(location.parameter())
    }

    /// Evaluates the normalized tangent on exactly its stored segment, never averaging joins.
    ///
    /// # Errors
    ///
    /// Returns location, stationary-tangent, or numeric failures from the addressed segment.
    pub fn unit_tangent_at(&self, location: PathLocation) -> Result<Vector2, CurveError> {
        self.segment_for(location)?
            .unit_tangent_at(location.parameter())
    }

    /// Evaluates the left-hand normal on exactly its stored segment, never averaging joins.
    ///
    /// # Errors
    ///
    /// Returns location, stationary-tangent, or numeric failures from the addressed segment.
    pub fn unit_normal_at(&self, location: PathLocation) -> Result<Vector2, CurveError> {
        self.segment_for(location)?
            .unit_normal_at(location.parameter())
    }

    /// Unions segment bounds in stored order without creating topology.
    ///
    /// # Errors
    ///
    /// Returns a numeric-overflow failure when a finite union cannot be represented.
    pub fn bounds(&self) -> Result<Bounds, CurveError> {
        let mut bounds = self.segments[0].bounds()?;
        for segment in &self.segments[1..] {
            let next = segment.bounds()?;
            bounds = Bounds::new(
                Point2::new(bounds.min.x.min(next.min.x), bounds.min.y.min(next.min.y)),
                Point2::new(bounds.max.x.max(next.max.x), bounds.max.y.max(next.max.y)),
            )
            .ok_or(CurveError::new(
                "curve.path.numeric_overflow",
                "curve-path arithmetic must remain finite",
            ))?;
        }
        Ok(bounds)
    }

    /// Builds an immutable ordered arc-length table under the fixed bounded policy.
    ///
    /// # Errors
    ///
    /// Returns arc-length limits or numeric failures without exposing a partial table.
    pub fn measure_arc_length(&self) -> Result<PathArcLength, CurveError> {
        PathArcLength::measure(self)
    }

    /// Intersects two paths in deterministic lexicographic location order.
    ///
    /// # Errors
    ///
    /// Returns pair, subdivision, overlap, result-limit, or numeric failures without partial output.
    pub fn intersections(&self, other: &Self) -> Result<Vec<PathIntersection>, CurveError> {
        intersections::path_intersections(self, other)
    }

    /// Clips the path into ordered open fragments without manufacturing boundary topology.
    ///
    /// # Errors
    ///
    /// Returns clipping limits or numeric failures without partial fragment output.
    pub fn clip_to_bounds(&self, bounds: Bounds) -> Result<Vec<Self>, CurveError> {
        clipping::clip_path(self, bounds)
    }

    /// Applies the existing affine authority to all construction points while retaining topology.
    ///
    /// # Errors
    ///
    /// Returns `curve.path.transform.non_finite` without a partial transformed path.
    pub fn transformed(&self, transform: AffineTransform2D) -> Result<Self, CurveError> {
        let segments = self
            .segments
            .iter()
            .map(|segment| segment.transformed(transform))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(segments, self.closure)
    }

    /// Returns an addressed segment or the stable path-local index failure.
    pub(crate) fn segment_for(&self, location: PathLocation) -> Result<&CurveSegment, CurveError> {
        self.segments
            .get(location.segment_index())
            .ok_or(CurveError::new(
                "curve.path.location.segment",
                "path location segment index is outside this path",
            ))
    }

    /// Builds clipping output only after enforcing the path-wide output-segment limit.
    pub(crate) fn fragments_from_segment_groups(
        groups: Vec<Vec<CurveSegment>>,
    ) -> Result<Vec<Self>, CurveError> {
        let count = groups.iter().map(Vec::len).sum::<usize>();
        if count > MAX_CLIPPED_SEGMENTS {
            return Err(CurveError::new(
                "curve.path.clipping.segment_limit",
                "clipped output segment limit exceeded",
            ));
        }
        groups
            .into_iter()
            .map(|segments| Self::new(segments, PathClosure::Open))
            .collect()
    }
}
