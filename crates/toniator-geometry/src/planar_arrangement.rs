//! Deterministic finite half-edge embedding shared by guide-face producers.

use crate::{CurveError, CurveSegment, Point2, Vector2};

/// One normalized finite endpoint key in the fixed arrangement lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct VertexKey(pub i64, pub i64);

/// Quantizes a finite point onto the stable arrangement lattice.
///
/// # Errors
///
/// Returns a curve numeric error for non-finite or unrepresentable coordinates.
pub(crate) fn vertex_key(point: Point2) -> Result<VertexKey, CurveError> {
    if !point.is_finite() {
        return Err(CurveError::new(
            "region.guide_faces.geometry.point",
            "arrangement points must be finite",
        ));
    }
    let scale = 1_000_000_000.0;
    let x = (point.x * scale).round();
    let y = (point.y * scale).round();
    if !x.is_finite()
        || !y.is_finite()
        || x < i64::MIN as f64
        || x > i64::MAX as f64
        || y < i64::MIN as f64
        || y > i64::MAX as f64
    {
        return Err(CurveError::new(
            "region.guide_faces.geometry.point",
            "arrangement lattice coordinate is unrepresentable",
        ));
    }
    Ok(VertexKey(x as i64, y as i64))
}

/// Returns a finite lattice-normalized point.
pub(crate) fn point_for_key(key: VertexKey) -> Point2 {
    Point2::new(
        key.0 as f64 / 1_000_000_000.0,
        key.1 as f64 / 1_000_000_000.0,
    )
}

/// One reversible geometric piece held by the arrangement embedding.
#[derive(Clone, Debug)]
pub(crate) struct ArrangementPiece<T> {
    pub segment: CurveSegment,
    pub start: VertexKey,
    pub end: VertexKey,
    pub payload: T,
}

/// One directed half-edge referring to a reversible arrangement piece.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HalfEdge {
    pub piece: usize,
    pub forward: bool,
    pub to: VertexKey,
}

/// Ordered outgoing half-edge indices keyed by a sorted lattice vertex vector.
///
/// Unlike `BTreeMap`, this material arrangement product reserves every vector
/// fallibly before publication. The small lookup is binary-search based.
pub(crate) type OutgoingHalfEdges = Vec<(VertexKey, Vec<usize>)>;

/// Constructs an ordered half-edge embedding without creating any topology from a canvas.
///
/// # Errors
///
/// Returns curve failures for stationary tangents; callers retain source-specific diagnostics.
pub(crate) fn embed<T>(
    pieces: &[ArrangementPiece<T>],
    mut cancelled: impl FnMut() -> bool,
) -> Result<(Vec<HalfEdge>, OutgoingHalfEdges), CurveError> {
    let edge_capacity = pieces.len().checked_mul(2).ok_or(CurveError::new(
        "region.guide_faces.allocation.half_edges",
        "arrangement half-edge allocation size overflowed",
    ))?;
    let mut edges = Vec::new();
    edges.try_reserve(edge_capacity).map_err(|_| {
        CurveError::new(
            "region.guide_faces.allocation.half_edges",
            "arrangement half-edge allocation failed",
        )
    })?;
    let mut entries = Vec::<(VertexKey, usize)>::new();
    entries.try_reserve(edge_capacity).map_err(|_| {
        CurveError::new(
            "region.guide_faces.allocation.outgoing",
            "arrangement outgoing-entry allocation failed",
        )
    })?;
    for (piece, value) in pieces.iter().enumerate() {
        if cancelled() {
            return Err(CurveError::new(
                "evaluation.cancelled",
                "guide-face evaluation was cancelled",
            ));
        }
        let first = edges.len();
        edges.push(HalfEdge {
            piece,
            forward: true,
            to: value.end,
        });
        entries.push((value.start, first));
        let second = edges.len();
        edges.push(HalfEdge {
            piece,
            forward: false,
            to: value.start,
        });
        entries.push((value.end, second));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let mut outgoing = Vec::<(VertexKey, Vec<usize>)>::new();
    outgoing.try_reserve(entries.len()).map_err(|_| {
        CurveError::new(
            "region.guide_faces.allocation.outgoing",
            "arrangement outgoing-group allocation failed",
        )
    })?;
    let mut entry_index = 0usize;
    while entry_index < entries.len() {
        if cancelled() {
            return Err(CurveError::new(
                "evaluation.cancelled",
                "guide-face evaluation was cancelled",
            ));
        }
        let vertex = entries[entry_index].0;
        let end = entries[entry_index..]
            .iter()
            .take_while(|entry| entry.0 == vertex)
            .count()
            .checked_add(entry_index)
            .ok_or(CurveError::new(
                "region.guide_faces.allocation.outgoing",
                "arrangement outgoing-group range overflowed",
            ))?;
        let mut values = Vec::new();
        values.try_reserve(end - entry_index).map_err(|_| {
            CurveError::new(
                "region.guide_faces.allocation.outgoing",
                "arrangement outgoing-edge allocation failed",
            )
        })?;
        values.extend(entries[entry_index..end].iter().map(|entry| entry.1));
        values.sort_by(|left, right| {
            edge_angle(*left, &edges, pieces)
                .unwrap_or(f64::NAN)
                .total_cmp(&edge_angle(*right, &edges, pieces).unwrap_or(f64::NAN))
                .then_with(|| left.cmp(right))
        });
        if values.windows(2).any(|pair| {
            let first = edge_angle(pair[0], &edges, pieces).unwrap_or(f64::NAN);
            let second = edge_angle(pair[1], &edges, pieces).unwrap_or(f64::NAN);
            (first - second).abs() <= 1e-12
        }) {
            return Err(CurveError::new(
                "region.guide_faces.geometry.vertex",
                "arrangement has ambiguous collinear incident directions",
            ));
        }
        outgoing.push((vertex, values));
        entry_index = end;
    }
    Ok((edges, outgoing))
}

/// Returns the outgoing tangent angle of one directed edge.
///
/// # Errors
///
/// Propagates stationary-tangent errors from the exact curve segment.
fn edge_angle<T>(
    index: usize,
    edges: &[HalfEdge],
    pieces: &[ArrangementPiece<T>],
) -> Result<f64, CurveError> {
    let edge = edges[index];
    let tangent = if edge.forward {
        pieces[edge.piece].segment.unit_tangent_at(0.0)?
    } else {
        let tangent = pieces[edge.piece].segment.unit_tangent_at(1.0)?;
        Vector2::new(-tangent.x, -tangent.y)
    };
    Ok(tangent.y.atan2(tangent.x))
}

/// Returns the deterministic predecessor-of-reverse successor used for positive face walks.
pub(crate) fn successor(
    edge_index: usize,
    edges: &[HalfEdge],
    outgoing: &OutgoingHalfEdges,
) -> Option<usize> {
    let edge = edges[edge_index];
    let reverse = edge_index ^ 1;
    let position = outgoing
        .binary_search_by_key(&edge.to, |entry| entry.0)
        .ok()?;
    let choices = &outgoing[position].1;
    let index = choices.iter().position(|candidate| *candidate == reverse)?;
    Some(choices[predecessor_of_reverse_index(index, choices.len())])
}

/// Returns the cyclic predecessor index used by the shared positive-face walk.
///
/// The caller supplies the position of a reverse half-edge in a nonempty
/// counter-clockwise outgoing-edge ordering. Both Guide Faces and maze walls
/// use this exact successor convention so their existing face orientation is
/// preserved while sharing the topology rule.
///
/// # Panics
///
/// Panics when `len` is zero because a half-edge cannot have an empty outgoing
/// ordering.
pub(crate) fn predecessor_of_reverse_index(reverse_index: usize, len: usize) -> usize {
    assert!(len > 0, "a face successor requires an outgoing half-edge");
    (reverse_index + len - 1) % len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSegment;

    /// Proves embedding polls cancellation before it publishes half-edge or outgoing-order state.
    #[test]
    fn embedding_cancellation_is_atomic() {
        let segment = CurveSegment::Line(
            LineSegment::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).expect("finite line"),
        );
        let pieces = vec![ArrangementPiece {
            segment,
            start: vertex_key(segment.start()).expect("finite start"),
            end: vertex_key(segment.end()).expect("finite end"),
            payload: (),
        }];
        assert_eq!(
            embed(&pieces, || true)
                .expect_err("cancelled embedding rejects")
                .path(),
            "evaluation.cancelled",
        );
    }
}
