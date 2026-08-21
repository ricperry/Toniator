use crate::{AffineTransform2D, Bounds, Point2, Vector2};

use super::{
    CubicBezierSegment, CurveError, CurveSegment, LineSegment, MAX_CLIPPED_SEGMENTS,
    PathIntersection, arc_length::PathArcLength, clipping, intersections, parameter,
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

    /// Moves one authored anchor and retains the adjacent cubic handle vectors relative to it.
    ///
    /// The operation returns a complete replacement and never mutates this immutable path.
    ///
    /// # Errors
    ///
    /// Returns stable index or finite-coordinate failures without exposing a partial path.
    pub fn move_anchor(&self, node_index: usize, point: Point2) -> Result<Self, CurveError> {
        if !point.is_finite() {
            return Err(CurveError::new(
                "curve.path.edit.coordinates",
                "edited anchors must be finite",
            ));
        }
        let count = self.node_count();
        if node_index >= count {
            return Err(CurveError::new(
                "curve.path.edit.node",
                "node index is outside this path",
            ));
        }
        let old = self.node_point(node_index)?;
        let dx = point.x - old.x;
        let dy = point.y - old.y;
        let translate = |value: Point2| Point2::new(value.x + dx, value.y + dy);
        let segments = self
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let start_node = index;
                let end_node = if self.closure == PathClosure::Closed {
                    (index + 1) % count
                } else {
                    index + 1
                };
                match *segment {
                    CurveSegment::Line(line) => Ok(CurveSegment::Line(LineSegment::new(
                        if start_node == node_index {
                            point
                        } else {
                            line.start()
                        },
                        if end_node == node_index {
                            point
                        } else {
                            line.end()
                        },
                    )?)),
                    CurveSegment::CubicBezier(cubic) => {
                        Ok(CurveSegment::CubicBezier(CubicBezierSegment::new(
                            if start_node == node_index {
                                point
                            } else {
                                cubic.start()
                            },
                            if start_node == node_index {
                                translate(cubic.control_1())
                            } else {
                                cubic.control_1()
                            },
                            if end_node == node_index {
                                translate(cubic.control_2())
                            } else {
                                cubic.control_2()
                            },
                            if end_node == node_index {
                                point
                            } else {
                                cubic.end()
                            },
                        )?))
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(segments, self.closure)
    }

    /// Moves one addressed cubic control point while retaining all other construction geometry.
    ///
    /// # Errors
    ///
    /// Returns stable segment, control-kind, or finite-coordinate failures without a partial path.
    pub fn move_cubic_control(
        &self,
        segment_index: usize,
        first: bool,
        point: Point2,
    ) -> Result<Self, CurveError> {
        if !point.is_finite() {
            return Err(CurveError::new(
                "curve.path.edit.coordinates",
                "edited controls must be finite",
            ));
        }
        let segment = self.segments.get(segment_index).ok_or(CurveError::new(
            "curve.path.edit.segment",
            "segment index is outside this path",
        ))?;
        let CurveSegment::CubicBezier(cubic) = segment else {
            return Err(CurveError::new(
                "curve.path.edit.control",
                "only cubic segments have editable controls",
            ));
        };
        let mut segments = self.segments.clone();
        segments[segment_index] = CurveSegment::CubicBezier(CubicBezierSegment::new(
            cubic.start(),
            if first { point } else { cubic.control_1() },
            if first { cubic.control_2() } else { point },
            cubic.end(),
        )?);
        Self::new(segments, self.closure)
    }

    /// Converts one line to a cubic with exact one-third-chord controls, or one cubic to its chord.
    ///
    /// # Errors
    ///
    /// Returns a stable segment-index failure without changing this path.
    pub fn toggle_segment_kind(&self, segment_index: usize) -> Result<Self, CurveError> {
        let source = *self.segments.get(segment_index).ok_or(CurveError::new(
            "curve.path.edit.segment",
            "segment index is outside this path",
        ))?;
        let replacement = match source {
            CurveSegment::Line(line) => {
                let third = Vector2::new(
                    (line.end().x - line.start().x) / 3.0,
                    (line.end().y - line.start().y) / 3.0,
                );
                CurveSegment::CubicBezier(CubicBezierSegment::new(
                    line.start(),
                    Point2::new(line.start().x + third.x, line.start().y + third.y),
                    Point2::new(line.end().x - third.x, line.end().y - third.y),
                    line.end(),
                )?)
            }
            CurveSegment::CubicBezier(cubic) => {
                CurveSegment::Line(LineSegment::new(cubic.start(), cubic.end())?)
            }
        };
        let mut segments = self.segments.clone();
        segments[segment_index] = replacement;
        Self::new(segments, self.closure)
    }

    /// Inserts one anchor by exact line interpolation or De Casteljau cubic subdivision.
    ///
    /// # Errors
    ///
    /// Returns stable segment, parameter, or numeric failures without a partial replacement.
    pub fn insert_node(&self, location: PathLocation) -> Result<Self, CurveError> {
        let index = location.segment_index();
        let segment = *self.segment_for(location)?;
        let (left, right) = split_segment(segment, location.parameter())?;
        let mut segments = self.segments.clone();
        segments.splice(index..=index, [left, right]);
        Self::new(segments, self.closure)
    }

    /// Deletes one node while retaining at least two nodes and reconnecting the remaining path.
    ///
    /// Open endpoint deletion removes its adjacent segment. Interior and closed deletion reconnect
    /// retained neighbours with a deterministic cubic whose handles retain the outer chord direction.
    ///
    /// # Errors
    ///
    /// Returns stable node-count or index failures without mutating this path or exposing a partial result.
    pub fn delete_node(&self, node_index: usize) -> Result<Self, CurveError> {
        let nodes = self.node_count();
        if nodes <= 2 {
            return Err(CurveError::new(
                "curve.path.edit.node_minimum",
                "paths retain at least two nodes",
            ));
        }
        if node_index >= nodes {
            return Err(CurveError::new(
                "curve.path.edit.node",
                "node index is outside this path",
            ));
        }
        let mut segments = self.segments.clone();
        if self.closure == PathClosure::Open && node_index == 0 {
            segments.remove(0);
            return Self::new(segments, self.closure);
        }
        if self.closure == PathClosure::Open && node_index + 1 == nodes {
            segments.pop();
            return Self::new(segments, self.closure);
        }
        let previous = if node_index == 0 {
            self.segments.len() - 1
        } else {
            node_index - 1
        };
        let next = if self.closure == PathClosure::Closed {
            node_index % self.segments.len()
        } else {
            node_index
        };
        let before = self.segments[previous];
        let after = self.segments[next];
        let replacement = reconnect(before, after)?;
        if self.closure == PathClosure::Closed && previous > next {
            segments.remove(previous);
            segments[0] = replacement;
        } else {
            segments.splice(previous..=next, [replacement]);
        }
        Self::new(segments, self.closure)
    }

    /// Returns the number of explicit anchors represented by this path topology.
    fn node_count(&self) -> usize {
        if self.closure == PathClosure::Closed {
            self.segments.len()
        } else {
            self.segments.len() + 1
        }
    }

    /// Resolves one anchor in stored path order.
    fn node_point(&self, node_index: usize) -> Result<Point2, CurveError> {
        if node_index < self.segments.len() {
            Ok(self.segments[node_index].start())
        } else if self.closure == PathClosure::Open && node_index == self.segments.len() {
            Ok(self.segments.last().expect("validated nonempty path").end())
        } else {
            Err(CurveError::new(
                "curve.path.edit.node",
                "node index is outside this path",
            ))
        }
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

/// Splits one segment exactly with the same Bernstein construction used by the path evaluator.
fn split_segment(
    segment: CurveSegment,
    parameter: f64,
) -> Result<(CurveSegment, CurveSegment), CurveError> {
    match segment {
        CurveSegment::Line(line) => {
            let middle = Point2::new(
                line.start().x + (line.end().x - line.start().x) * parameter,
                line.start().y + (line.end().y - line.start().y) * parameter,
            );
            Ok((
                CurveSegment::Line(LineSegment::new(line.start(), middle)?),
                CurveSegment::Line(LineSegment::new(middle, line.end())?),
            ))
        }
        CurveSegment::CubicBezier(cubic) => {
            let t = parameter;
            let lerp =
                |a: Point2, b: Point2| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let a = lerp(cubic.start(), cubic.control_1());
            let b = lerp(cubic.control_1(), cubic.control_2());
            let c = lerp(cubic.control_2(), cubic.end());
            let d = lerp(a, b);
            let e = lerp(b, c);
            let f = lerp(d, e);
            Ok((
                CurveSegment::CubicBezier(CubicBezierSegment::new(cubic.start(), a, d, f)?),
                CurveSegment::CubicBezier(CubicBezierSegment::new(f, e, c, cubic.end())?),
            ))
        }
    }
}

/// Builds the deterministic bounded reconnection used by node deletion.
///
/// The fit samples the retained outer segments at equal approximate arc lengths, preserves their
/// endpoint directions through nonnegative handle magnitudes, and compares the bounded cubic to
/// a direct chord before returning. It performs no partial path mutation.
///
/// # Errors
///
/// Returns finite arithmetic, segment evaluation, or constructor failures before exposing a
/// reconnection candidate.
fn reconnect(before: CurveSegment, after: CurveSegment) -> Result<CurveSegment, CurveError> {
    let start = before.start();
    let end = after.end();
    let incoming = match before {
        CurveSegment::Line(line) => {
            Vector2::new(line.end().x - line.start().x, line.end().y - line.start().y)
        }
        CurveSegment::CubicBezier(cubic) => Vector2::new(
            cubic.end().x - cubic.control_2().x,
            cubic.end().y - cubic.control_2().y,
        ),
    };
    let outgoing = match after {
        CurveSegment::Line(line) => {
            Vector2::new(line.end().x - line.start().x, line.end().y - line.start().y)
        }
        CurveSegment::CubicBezier(cubic) => Vector2::new(
            cubic.control_1().x - cubic.start().x,
            cubic.control_1().y - cubic.start().y,
        ),
    };
    let chord = Vector2::new(end.x - start.x, end.y - start.y);
    let chord_length = chord.x.hypot(chord.y);
    if !chord_length.is_finite() {
        return Err(CurveError::new(
            "curve.path.numeric_overflow",
            "curve-path arithmetic must remain finite",
        ));
    }
    let normalize = |vector: Vector2| {
        let length = vector.x.hypot(vector.y);
        if length.is_finite() && length > 1.0e-12 {
            Vector2::new(vector.x / length, vector.y / length)
        } else if chord_length > 1.0e-12 {
            Vector2::new(chord.x / chord_length, chord.y / chord_length)
        } else {
            Vector2::new(0.0, 0.0)
        }
    };
    let a = normalize(incoming);
    let b = normalize(outgoing);
    const SAMPLES: usize = 24;
    let targets = equal_arc_targets(before, after, SAMPLES)?;
    let mut normal = [[0.0; 2]; 2];
    let mut rhs = [0.0; 2];
    for (sample, target) in targets.iter().enumerate() {
        let t = (sample + 1) as f64 / (SAMPLES + 1) as f64;
        let one = 1.0 - t;
        let base = Point2::new(
            one.powi(3) * start.x + t.powi(3) * end.x,
            one.powi(3) * start.y + t.powi(3) * end.y,
        );
        let column_1 = Vector2::new(3.0 * one * one * t * a.x, 3.0 * one * one * t * a.y);
        let column_2 = Vector2::new(-3.0 * one * t * t * b.x, -3.0 * one * t * t * b.y);
        let target = Vector2::new(target.x - base.x, target.y - base.y);
        normal[0][0] += column_1.dot(column_1);
        normal[0][1] += column_1.dot(column_2);
        normal[1][0] += column_2.dot(column_1);
        normal[1][1] += column_2.dot(column_2);
        rhs[0] += column_1.dot(target);
        rhs[1] += column_2.dot(target);
        if !normal.iter().flatten().all(|value| value.is_finite())
            || !rhs.iter().all(|value| value.is_finite())
        {
            return Err(CurveError::new(
                "curve.path.edit.fit",
                "reconnection fit arithmetic must remain finite",
            ));
        }
    }
    let fallback = chord_length / 3.0;
    let (first, second) = nonnegative_least_squares(normal, rhs, fallback)?;
    if !first.is_finite() || !second.is_finite() {
        return Err(CurveError::new(
            "curve.path.edit.fit",
            "reconnection fit must remain finite",
        ));
    }
    let cubic = CurveSegment::CubicBezier(CubicBezierSegment::new(
        start,
        Point2::new(start.x + a.x * first, start.y + a.y * first),
        Point2::new(end.x - b.x * second, end.y - b.y * second),
        end,
    )?);
    let line = CurveSegment::Line(LineSegment::new(start, end)?);
    let residual = |candidate: CurveSegment| -> Result<f64, CurveError> {
        let mut value = 0.0;
        for (sample, target) in targets.iter().enumerate() {
            let t = (sample + 1) as f64 / (SAMPLES + 1) as f64;
            let point = candidate.point_at(t)?;
            value += (point.x - target.x).powi(2) + (point.y - target.y).powi(2);
            if !value.is_finite() {
                return Err(CurveError::new(
                    "curve.path.edit.fit",
                    "reconnection residual must remain finite",
                ));
            }
        }
        Ok(value)
    };
    if residual(cubic)? <= residual(line)? {
        Ok(cubic)
    } else {
        Ok(line)
    }
}

/// Samples the retained outer segments at fixed-count equal approximate arc-length positions.
///
/// # Errors
///
/// Returns segment evaluation or finite arithmetic failures before exposing partial targets.
fn equal_arc_targets(
    before: CurveSegment,
    after: CurveSegment,
    count: usize,
) -> Result<Vec<Point2>, CurveError> {
    const ARC_STEPS: usize = 32;
    const MAX_TARGETS: usize = 64;
    if count == 0 || count > MAX_TARGETS {
        return Err(CurveError::new(
            "curve.path.edit.fit.limit",
            "reconnection target count is outside the fixed work limit",
        ));
    }
    let before_length = approximate_segment_length(before, ARC_STEPS)?;
    let after_length = approximate_segment_length(after, ARC_STEPS)?;
    let total = before_length + after_length;
    if !total.is_finite() {
        return Err(CurveError::new(
            "curve.path.edit.fit",
            "reconnection arc lengths must remain finite",
        ));
    }
    let mut targets = Vec::with_capacity(count);
    for index in 1..=count {
        let distance = total * index as f64 / (count + 1) as f64;
        let (segment, local_distance, length) = if distance <= before_length {
            (before, distance, before_length)
        } else {
            (after, distance - before_length, after_length)
        };
        let parameter = approximate_arc_parameter(segment, local_distance, length, ARC_STEPS)?;
        let point = segment.point_at(parameter)?;
        if !point.is_finite() {
            return Err(CurveError::new(
                "curve.path.edit.fit",
                "reconnection targets must remain finite",
            ));
        }
        targets.push(point);
    }
    Ok(targets)
}

/// Approximates one segment length with a fixed bounded chord partition.
///
/// # Errors
///
/// Returns evaluation or finite arithmetic failures without retaining partial length state.
fn approximate_segment_length(segment: CurveSegment, steps: usize) -> Result<f64, CurveError> {
    let mut previous = segment.point_at(0.0)?;
    let mut length = 0.0;
    for index in 1..=steps {
        let point = segment.point_at(index as f64 / steps as f64)?;
        length += (point.x - previous.x).hypot(point.y - previous.y);
        if !length.is_finite() {
            return Err(CurveError::new(
                "curve.path.edit.fit",
                "reconnection arc-length approximation must remain finite",
            ));
        }
        previous = point;
    }
    Ok(length)
}

/// Inverts one fixed chord partition to a deterministic approximate arc-length parameter.
///
/// # Errors
///
/// Returns segment evaluation or finite arithmetic failures without manufacturing a parameter.
fn approximate_arc_parameter(
    segment: CurveSegment,
    distance: f64,
    length: f64,
    steps: usize,
) -> Result<f64, CurveError> {
    if !distance.is_finite() || !length.is_finite() {
        return Err(CurveError::new(
            "curve.path.edit.fit",
            "reconnection arc parameter inputs must remain finite",
        ));
    }
    if length <= 1.0e-12 {
        return Ok(0.5);
    }
    let target = distance.clamp(0.0, length);
    let mut previous = segment.point_at(0.0)?;
    let mut accumulated = 0.0;
    for index in 1..=steps {
        let parameter = index as f64 / steps as f64;
        let point = segment.point_at(parameter)?;
        let chord = (point.x - previous.x).hypot(point.y - previous.y);
        if accumulated + chord >= target && chord > 1.0e-12 {
            let fraction = ((target - accumulated) / chord).clamp(0.0, 1.0);
            return Ok(((index - 1) as f64 + fraction) / steps as f64);
        }
        accumulated += chord;
        previous = point;
    }
    Ok(1.0)
}

/// Solves the two-variable nonnegative least-squares normal system with every boundary candidate.
///
/// # Errors
///
/// Returns only finite arithmetic failures; a singular normal system deterministically includes
/// the chord-length fallback candidate rather than exposing an unconstrained partial solution.
fn nonnegative_least_squares(
    normal: [[f64; 2]; 2],
    rhs: [f64; 2],
    fallback: f64,
) -> Result<(f64, f64), CurveError> {
    if !normal.iter().flatten().all(|value| value.is_finite())
        || !rhs.iter().all(|value| value.is_finite())
        || !fallback.is_finite()
    {
        return Err(CurveError::new(
            "curve.path.edit.fit",
            "nonnegative least-squares inputs must remain finite",
        ));
    }
    let mut candidates = vec![(0.0, 0.0), (fallback.max(0.0), fallback.max(0.0))];
    let determinant = normal[0][0] * normal[1][1] - normal[0][1] * normal[1][0];
    if determinant.is_finite() && determinant.abs() > 1.0e-12 {
        let first = (rhs[0] * normal[1][1] - normal[0][1] * rhs[1]) / determinant;
        let second = (normal[0][0] * rhs[1] - rhs[0] * normal[1][0]) / determinant;
        if first >= 0.0 && second >= 0.0 && first.is_finite() && second.is_finite() {
            candidates.push((first, second));
        }
    }
    if normal[0][0] > 1.0e-12 {
        candidates.push(((rhs[0] / normal[0][0]).max(0.0), 0.0));
    }
    if normal[1][1] > 1.0e-12 {
        candidates.push((0.0, (rhs[1] / normal[1][1]).max(0.0)));
    }
    candidates
        .into_iter()
        .filter(|(first, second)| first.is_finite() && second.is_finite())
        .min_by(|left, right| {
            let residual = |(first, second): &(f64, f64)| {
                let residual_first = normal[0][0] * first + normal[0][1] * second - rhs[0];
                let residual_second = normal[1][0] * first + normal[1][1] * second - rhs[1];
                residual_first.mul_add(residual_first, residual_second * residual_second)
            };
            residual(left).total_cmp(&residual(right))
        })
        .ok_or(CurveError::new(
            "curve.path.edit.fit",
            "nonnegative least-squares has no finite candidate",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves equal-arc sampling distributes fixed targets across unequal retained chords.
    #[test]
    fn equal_arc_targets_follow_combined_retained_length() {
        let before = CurveSegment::Line(
            LineSegment::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).expect("line"),
        );
        let after = CurveSegment::Line(
            LineSegment::new(Point2::new(1.0, 0.0), Point2::new(4.0, 0.0)).expect("line"),
        );
        assert_eq!(
            equal_arc_targets(before, after, 3).expect("fixed targets"),
            vec![
                Point2::new(1.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(3.0, 0.0),
            ]
        );
    }

    /// Proves singular normal systems retain a deterministic finite nonnegative fallback candidate.
    #[test]
    fn nonnegative_fit_handles_singular_systems_without_negative_handles() {
        let result = nonnegative_least_squares([[0.0, 0.0], [0.0, 0.0]], [1.0, -1.0], 2.0)
            .expect("singular fallback");
        assert_eq!(result, (0.0, 0.0));
        assert!(result.0 >= 0.0 && result.1 >= 0.0);
    }

    /// Proves candidate ranking evaluates both normal-equation residuals against original handles.
    #[test]
    fn nonnegative_fit_ranks_the_positive_interior_candidate_before_a_boundary() {
        let result = nonnegative_least_squares([[1.0, 0.9], [0.9, 1.0]], [10.9, 10.0], 10.0)
            .expect("positive interior fit");
        assert!((result.0 - 10.0).abs() < 1.0e-12);
        assert!((result.1 - 1.0).abs() < 1.0e-12);
    }

    /// Proves the fitted reconnect never has a larger sampled residual than its direct chord.
    #[test]
    fn reconnect_prefers_only_residuals_no_worse_than_the_chord() {
        let before = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(3.0, 3.0),
                Point2::new(4.0, 4.0),
            )
            .expect("before"),
        );
        let after = CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(4.0, 4.0),
                Point2::new(5.0, 5.0),
                Point2::new(7.0, 1.0),
                Point2::new(8.0, 0.0),
            )
            .expect("after"),
        );
        let targets = equal_arc_targets(before, after, 24).expect("targets");
        let fitted = reconnect(before, after).expect("reconnect");
        let chord =
            CurveSegment::Line(LineSegment::new(before.start(), after.end()).expect("chord"));
        let residual = |candidate: CurveSegment| {
            targets
                .iter()
                .enumerate()
                .map(|(index, target)| {
                    let point = candidate
                        .point_at((index + 1) as f64 / 25.0)
                        .expect("sample");
                    (point.x - target.x).powi(2) + (point.y - target.y).powi(2)
                })
                .sum::<f64>()
        };
        assert!(residual(fitted) <= residual(chord));
    }
}
