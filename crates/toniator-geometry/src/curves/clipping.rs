use crate::{Bounds, Point2};

use super::{
    CubicBezierSegment, CurveError, CurvePath, CurveSegment, LineSegment, MAX_CLIPPING_FRAGMENTS,
    MAX_SUBDIVISION_DEPTH, MAX_WORK_ITEMS,
    segment::{cubic_range, split_cubic},
};

/// Clips one path into source-ordered open fragments without adding boundary or seam geometry.
pub(crate) fn clip_path(path: &CurvePath, bounds: Bounds) -> Result<Vec<CurvePath>, CurveError> {
    validate_bounds(bounds)?;
    let mut fully_contained = true;
    for segment in path.segments() {
        fully_contained &= segment_contained(*segment, bounds)?;
    }
    if fully_contained {
        return Ok(vec![path.clone()]);
    }
    let mut groups: Vec<Vec<CurveSegment>> = Vec::new();
    let mut root_budget = RootBudget::new(MAX_WORK_ITEMS);
    for segment in path.segments().iter().copied() {
        let pieces = clip_segment(segment, bounds, &mut root_budget)?;
        for piece in pieces {
            if let Some(previous) = groups
                .last_mut()
                .filter(|group| group.last().is_some_and(|last| last.end() == piece.start()))
            {
                previous.push(piece);
            } else {
                groups.push(vec![piece]);
            }
        }
    }
    if groups.len() > MAX_CLIPPING_FRAGMENTS {
        return Err(CurveError::new(
            "curve.path.clipping.fragment_limit",
            "clipping fragment limit exceeded",
        ));
    }
    CurvePath::fragments_from_segment_groups(groups)
}

/// Validates public bounds despite public legacy fields allowing post-construction mutation.
fn validate_bounds(bounds: Bounds) -> Result<(), CurveError> {
    (bounds.min.is_finite()
        && bounds.max.is_finite()
        && bounds.min.x <= bounds.max.x
        && bounds.min.y <= bounds.max.y)
        .then_some(())
        .ok_or(CurveError::new(
            "curve.path.clipping.bounds",
            "clipping bounds must be finite and ordered",
        ))
}

/// Determines whether existing segment geometry is fully contained, preserving its exact clone.
fn segment_contained(segment: CurveSegment, bounds: Bounds) -> Result<bool, CurveError> {
    let curve_bounds = segment.bounds()?;
    let tolerance = super::tolerance([curve_bounds.min, curve_bounds.max, bounds.min, bounds.max])?;
    Ok(curve_bounds.min.x >= bounds.min.x - tolerance
        && curve_bounds.max.x <= bounds.max.x + tolerance
        && curve_bounds.min.y >= bounds.min.y - tolerance
        && curve_bounds.max.y <= bounds.max.y + tolerance)
}

/// Clips one source segment while preserving its authored kind and direction.
fn clip_segment(
    segment: CurveSegment,
    bounds: Bounds,
    root_budget: &mut RootBudget,
) -> Result<Vec<CurveSegment>, CurveError> {
    match segment {
        CurveSegment::Line(line) => {
            clip_line(line, bounds).map(|piece| piece.into_iter().collect())
        }
        CurveSegment::CubicBezier(cubic) => clip_cubic(cubic, bounds, root_budget),
    }
}

/// Implements Liang-Barsky clipping for one finite source line.
fn clip_line(line: LineSegment, bounds: Bounds) -> Result<Option<CurveSegment>, CurveError> {
    let dx = line.end().x - line.start().x;
    let dy = line.end().y - line.start().y;
    let mut start = 0.0_f64;
    let mut end = 1.0_f64;
    for (p, q) in [
        (-dx, line.start().x - bounds.min.x),
        (dx, bounds.max.x - line.start().x),
        (-dy, line.start().y - bounds.min.y),
        (dy, bounds.max.y - line.start().y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return Ok(None);
            }
        } else {
            let ratio = q / p;
            if !ratio.is_finite() {
                return Err(CurveError::new(
                    "curve.path.numeric_overflow",
                    "curve-path arithmetic must remain finite",
                ));
            }
            if p < 0.0 {
                start = start.max(ratio);
            } else {
                end = end.min(ratio);
            }
        }
    }
    if start > end || end < 0.0 || start > 1.0 {
        return Ok(None);
    }
    start = start.clamp(0.0, 1.0);
    end = end.clamp(0.0, 1.0);
    let start_point = snap_to_bounds(
        Point2::new(line.start().x + dx * start, line.start().y + dy * start),
        bounds,
    );
    let end_point = snap_to_bounds(
        Point2::new(line.start().x + dx * end, line.start().y + dy * end),
        bounds,
    );
    if start == end && (line.start() != line.end() || !bounds.contains(line.start())) {
        return Ok(None);
    }
    Ok(Some(CurveSegment::Line(LineSegment::new(
        start_point,
        end_point,
    )?)))
}

/// Isolates cubic intervals with nonempty strict interior inside the finite clipping rectangle.
fn clip_cubic(
    cubic: CubicBezierSegment,
    bounds: Bounds,
    root_budget: &mut RootBudget,
) -> Result<Vec<CurveSegment>, CurveError> {
    let ranges =
        cubic_inside_ranges_with_budget(cubic, bounds, MAX_SUBDIVISION_DEPTH, root_budget)?;
    ranges
        .into_iter()
        .map(|(start, end)| {
            let fragment = cubic_range(cubic, start, end)?;
            let start_point = snap_to_bounds(fragment.start(), bounds);
            let end_point = snap_to_bounds(fragment.end(), bounds);
            Ok(CurveSegment::CubicBezier(CubicBezierSegment::new(
                start_point,
                fragment.control_1(),
                fragment.control_2(),
                end_point,
            )?))
        })
        .collect()
}

/// Finds ordered cubic parameter ranges whose conservative construction boxes lie inside bounds.
#[cfg(test)]
fn cubic_inside_ranges(
    cubic: CubicBezierSegment,
    bounds: Bounds,
    maximum_depth: u8,
    maximum_work_items: usize,
) -> Result<Vec<(f64, f64)>, CurveError> {
    if maximum_depth == 0 || maximum_work_items < 2 {
        return Err(CurveError::new(
            "curve.path.clipping.subdivision_limit",
            "clipping subdivision work limit exceeded",
        ));
    }
    let mut budget = RootBudget::new(maximum_work_items);
    cubic_inside_ranges_with_budget(cubic, bounds, maximum_depth, &mut budget)
}

/// Finds cubic clipping intervals while charging a caller-owned path-wide root-isolation budget.
fn cubic_inside_ranges_with_budget(
    cubic: CubicBezierSegment,
    bounds: Bounds,
    maximum_depth: u8,
    budget: &mut RootBudget,
) -> Result<Vec<(f64, f64)>, CurveError> {
    let mut parameters = vec![0.0, 1.0];
    for (coordinate, boundary) in [
        (Axis::X, bounds.min.x),
        (Axis::X, bounds.max.x),
        (Axis::Y, bounds.min.y),
        (Axis::Y, bounds.max.y),
    ] {
        parameters.extend(axis_roots(
            cubic,
            coordinate,
            boundary,
            bounds,
            maximum_depth,
            budget,
        )?);
    }
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|first, second| (*first - *second).abs() <= super::PARAMETER_TOLERANCE);
    let mut ranges = Vec::new();
    for pair in parameters.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if end - start <= super::PARAMETER_TOLERANCE {
            continue;
        }
        let midpoint = CurveSegment::CubicBezier(cubic).point_at((start + end) * 0.5)?;
        if bounds.contains(midpoint) {
            append_range(&mut ranges, start, end);
        }
    }
    Ok(ranges)
}

/// Selects the one coordinate axis used to isolate cubic intersections with a clipping edge.
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

/// Finds all cubic crossings and tangencies with one finite clipping coordinate by control-value isolation.
fn axis_roots(
    cubic: CubicBezierSegment,
    axis: Axis,
    boundary: f64,
    bounds: Bounds,
    maximum_depth: u8,
    budget: &mut RootBudget,
) -> Result<Vec<f64>, CurveError> {
    let mut stack = vec![(cubic, 0.0, 1.0, 0_u8)];
    let mut roots = Vec::new();
    while let Some((node, start, end, depth)) = stack.pop() {
        budget.consume()?;
        let values = [
            axis_value(node.start(), axis) - boundary,
            axis_value(node.control_1(), axis) - boundary,
            axis_value(node.control_2(), axis) - boundary,
            axis_value(node.end(), axis) - boundary,
        ];
        let tolerance = super::tolerance([
            cubic.start(),
            cubic.control_1(),
            cubic.control_2(),
            cubic.end(),
            bounds.min,
            bounds.max,
        ])?;
        let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if minimum > tolerance || maximum < -tolerance {
            continue;
        }
        if minimum >= -tolerance && maximum <= tolerance {
            roots.push(start);
            roots.push(end);
            continue;
        }
        if end - start <= super::PARAMETER_TOLERANCE && maximum - minimum <= tolerance {
            roots.push((start + end) * 0.5);
        } else if depth >= maximum_depth {
            return Err(CurveError::new(
                "curve.path.clipping.subdivision_limit",
                "clipping root isolation depth limit exceeded",
            ));
        } else {
            let (left, right) = split_cubic(node, 0.5)?;
            let middle = (start + end) * 0.5;
            stack.push((right, middle, end, depth + 1));
            stack.push((left, start, middle, depth + 1));
        }
    }
    Ok(roots)
}

/// Extracts a finite coordinate without changing construction geometry.
fn axis_value(point: Point2, axis: Axis) -> f64 {
    match axis {
        Axis::X => point.x,
        Axis::Y => point.y,
    }
}

/// Tracks shared root-isolation work across all four clipping boundaries for one public operation.
struct RootBudget {
    work_items: usize,
    maximum_work_items: usize,
}

impl RootBudget {
    /// Creates a private aggregate clipping budget without making limits caller-adjustable.
    const fn new(maximum_work_items: usize) -> Self {
        Self {
            work_items: 0,
            maximum_work_items,
        }
    }

    /// Charges one deterministic subdivision node and fails before partial ranges are published.
    fn consume(&mut self) -> Result<(), CurveError> {
        self.work_items = self.work_items.checked_add(1).ok_or(CurveError::new(
            "curve.path.clipping.subdivision_limit",
            "clipping subdivision work limit exceeded",
        ))?;
        (self.work_items <= self.maximum_work_items)
            .then_some(())
            .ok_or(CurveError::new(
                "curve.path.clipping.subdivision_limit",
                "clipping subdivision work limit exceeded",
            ))
    }
}

/// Appends a source-ordered interval, coalescing only exact adjacent parameter ranges.
fn append_range(ranges: &mut Vec<(f64, f64)>, start: f64, end: f64) {
    if let Some(previous) = ranges.last_mut().filter(|previous| previous.1 == start) {
        previous.1 = end;
    } else {
        ranges.push((start, end));
    }
}

/// Snaps only coordinates lying at a clipping edge onto that exact finite bound.
fn snap_to_bounds(point: Point2, bounds: Bounds) -> Point2 {
    let tolerance = super::tolerance([point, bounds.min, bounds.max]).unwrap_or(0.0);
    let x = if (point.x - bounds.min.x).abs() <= tolerance {
        bounds.min.x
    } else if (point.x - bounds.max.x).abs() <= tolerance {
        bounds.max.x
    } else {
        point.x
    };
    let y = if (point.y - bounds.min.y).abs() <= tolerance {
        bounds.min.y
    } else if (point.y - bounds.max.y).abs() <= tolerance {
        bounds.max.y
    } else {
        point.y
    };
    Point2::new(x, y)
}

#[cfg(test)]
mod tests {
    use crate::{CubicBezierSegment, Point2};

    use super::*;

    /// Proves private reduced clipping work budgets reject before emitting a partial interval list.
    #[test]
    fn reduced_clipping_work_budget_is_atomic() {
        let cubic = CubicBezierSegment::new(
            Point2::new(-2.0, 0.0),
            Point2::new(-1.0, 4.0),
            Point2::new(1.0, -4.0),
            Point2::new(2.0, 0.0),
        )
        .expect("finite cubic");
        let bounds = Bounds::new(Point2::new(-0.5, -0.5), Point2::new(0.5, 0.5))
            .expect("finite ordered bounds");
        assert_eq!(
            cubic_inside_ranges(cubic, bounds, 48, 1)
                .expect_err("one work item cannot classify this cubic")
                .path(),
            "curve.path.clipping.subdivision_limit"
        );
    }

    /// Proves unresolved non-dyadic control-value roots fail the reduced depth budget without fragments.
    #[test]
    fn reduced_clipping_depth_budget_is_atomic() {
        let cubic = CubicBezierSegment::new(
            Point2::new(0.0, -1.0),
            Point2::new(0.2, 3.0),
            Point2::new(0.8, -2.0),
            Point2::new(1.0, 1.0),
        )
        .expect("finite cubic");
        let bounds = Bounds::new(Point2::new(0.0, 0.0), Point2::new(1.0, 2.0))
            .expect("finite ordered bounds");
        assert_eq!(
            cubic_inside_ranges(cubic, bounds, 1, 128)
                .expect_err("one split cannot isolate nondyadic roots")
                .path(),
            "curve.path.clipping.subdivision_limit"
        );
    }
}
