// Toniator local patch (2026-10-13): opt-in checked Boolean work; see PROVENANCE.md.
use crate::core::checked::{SplitControl, Unchecked};
use crate::core::edge_data::{EdgeDataSplit, OverlayEdgeData};
use crate::core::integer::OverlayInt;
use crate::core::solver::Solver;
use crate::geom::x_segment::XSegment;
use crate::segm::merge::ShapeSegmentsMerge;
use crate::segm::segment::Segment;
use crate::segm::sort::ShapeSegmentsSort;
use crate::segm::winding::WindingCount;
use crate::split::cross_solver::{CrossSolver, CrossType, EndMask};
use crate::split::line_mark::{LineMark, SortMarkByIndexAndPoint};
use alloc::vec::Vec;
use i_float::int::number::{int::IntNumber, wide_int::WideIntNumber};

pub(crate) struct SplitSolver<I: IntNumber> {
    pub(super) marks: Vec<LineMark<I>>,
}

impl<I: IntNumber> SplitSolver<I> {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self { marks: Vec::new() }
    }
}

impl<I> SplitSolver<I>
where
    I: OverlayInt,
{
    #[inline]
    pub(crate) fn split_segments<C: WindingCount, D: OverlayEdgeData<C>>(
        &mut self,
        segments: &mut Vec<Segment<C, I, D>>,
        solver: &Solver,
    ) -> bool {
        let mut store = D::Store::default();
        self.split_segments_with_store(segments, solver, &mut store)
    }

    #[inline]
    /// Runs the original split strategy with optional checked work and cancellation authority.
    /// Checked failure leaves local geometry for the checked caller to discard.
    pub(crate) fn split_segments_with_store<C: WindingCount, D: OverlayEdgeData<C>>(
        &mut self,
        segments: &mut Vec<Segment<C, I, D>>,
        solver: &Solver,
        store: &mut D::Store,
    ) -> bool {
        self.split_segments_with_control(segments, solver, store, &mut Unchecked)
    }

    /// Splits the existing ordered segments under optional work and mutation authority.
    /// A failed control leaves local incomplete geometry which checked callers discard.
    pub(crate) fn split_segments_with_control<C: WindingCount, D: OverlayEdgeData<C>>(
        &mut self,
        segments: &mut Vec<Segment<C, I, D>>,
        solver: &Solver,
        store: &mut D::Store,
        control: &mut impl SplitControl<I>,
    ) -> bool {
        if !control.sort(segments.len()) || segments.is_empty() {
            return false;
        }

        segments.sort_by_ab(solver.is_parallel_sort_allowed());
        if !control.charge(segments.len()) {
            return false;
        }
        let any_merged = segments.merge_if_needed_with_control(store, control);
        if segments.is_empty() {
            return true;
        }

        let any_intersection = self.split(segments, solver, store, control);
        any_merged | any_intersection
    }

    #[inline]
    /// Dispatches checked or ordinary splitting through the original strategy selection.
    fn split<C: WindingCount, D: OverlayEdgeData<C>>(
        &mut self,
        segments: &mut Vec<Segment<C, I, D>>,
        solver: &Solver,
        store: &mut D::Store,
        control: &mut impl SplitControl<I>,
    ) -> bool {
        let is_list = solver.is_list_split(segments);
        let snap_radius = solver.snap_radius();
        if is_list {
            return self.list_split(snap_radius, segments, solver, store, control);
        }

        let is_fragmentation = solver.is_fragmentation_required(segments);

        if is_fragmentation {
            self.fragment_split(snap_radius, segments, solver, store, control)
        } else {
            self.tree_split(snap_radius, segments, solver, store, control)
        }
    }

    pub(super) fn cross(
        i: usize,
        j: usize,
        ei: &XSegment<I>,
        ej: &XSegment<I>,
        marks: &mut Vec<LineMark<I>>,
        radius_squared: I::WideUInt,
    ) -> bool {
        let cross = if let Some(cross) = CrossSolver::<I>::cross(ei, ej, radius_squared) {
            cross
        } else {
            return false;
        };

        match cross.cross_type {
            CrossType::Pure => {
                marks.push(LineMark {
                    index: i,
                    point: cross.point,
                });
                marks.push(LineMark {
                    index: j,
                    point: cross.point,
                });
            }
            CrossType::TargetEnd => {
                marks.push(LineMark {
                    index: j,
                    point: cross.point,
                });
            }
            CrossType::OtherEnd => {
                marks.push(LineMark {
                    index: i,
                    point: cross.point,
                });
            }
            CrossType::Overlay => {
                let mask = CrossSolver::<I>::collinear(ei, ej);
                if mask == 0 {
                    return false;
                }

                if mask.is_target_a() {
                    marks.push(LineMark {
                        index: j,
                        point: ei.a,
                    });
                }

                if mask.is_target_b() {
                    marks.push(LineMark {
                        index: j,
                        point: ei.b,
                    });
                }

                if mask.is_other_a() {
                    marks.push(LineMark {
                        index: i,
                        point: ej.a,
                    });
                }

                if mask.is_other_b() {
                    marks.push(LineMark {
                        index: i,
                        point: ej.b,
                    });
                }
            }
        }

        cross.is_round
    }

    /// Applies every sorted mark with a sequential directed-edge triangle certificate.
    pub(super) fn apply<C: WindingCount, D: OverlayEdgeData<C>>(
        &mut self,
        segments: &mut Vec<Segment<C, I, D>>,
        reusable_buffer: &mut Vec<LineMark<I>>,
        solver: &Solver,
        store: &mut D::Store,
        control: &mut impl SplitControl<I>,
    ) {
        if !control.sort(self.marks.len()) {
            return;
        }
        self.marks
            .sort_by_index_and_point(solver.is_parallel_sort_allowed(), reusable_buffer);
        if !control.charge(self.marks.len()) {
            return;
        }
        // Same stable adjacent deduplication, with a cancellation boundary for every mark.
        let mut retained = 0;
        for read in 0..self.marks.len() {
            if !control.charge(1) {
                return;
            }
            if retained == 0 || self.marks[read] != self.marks[retained - 1] {
                self.marks[retained] = self.marks[read];
                retained += 1;
            }
        }
        self.marks.truncate(retained);

        if !control.charge(self.marks.len().saturating_mul(2)) {
            return;
        }
        segments.reserve(self.marks.len());

        // split segments

        let mut i = 0;
        while i < self.marks.len() {
            if !control.charge(1) {
                return;
            }
            let start = i;
            let m0 = self.marks[i];

            i += 1;
            while i < self.marks.len() && self.marks[i].index == m0.index {
                if !control.charge(1) {
                    return;
                }
                i += 1;
            }

            let s0 = unsafe {
                // SAFETY: m0.index < segments.len() (marks are built from valid segment indices).
                // We take at most one &mut to that element per group. We drop the &mut before any push,
                // so no aliasing or reallocation invalidation can occur.
                segments.get_unchecked_mut(m0.index)
            };

            let count = s0.count;
            let data = s0.data;
            let x_seg = s0.x_segment;

            if start + 1 == i {
                // single split
                if !control.certify(x_seg.a, m0.point, x_seg.b) {
                    return;
                }
                let (d0, d1) = data.split(
                    EdgeDataSplit {
                        a: x_seg.a,
                        p: m0.point,
                        b: x_seg.b,
                    },
                    store,
                );
                *s0 = Segment::create_and_validate_with_data(x_seg.a, m0.point, count, d0, store);
                let s1 =
                    Segment::create_and_validate_with_data(m0.point, x_seg.b, count, d1, store);
                segments.push(s1);

                continue;
            }

            // we have several points
            let sub_marks = &mut self.marks[start..i];
            if !control.sort(sub_marks.len()) {
                return;
            }
            Self::sort_sub_marks(sub_marks, x_seg, control);
            if !control.charge(0) {
                return;
            }

            let m0 = sub_marks[0];
            if !control.certify(x_seg.a, m0.point, x_seg.b) {
                return;
            }
            let (d0, mut rest_data) = data.split(
                EdgeDataSplit {
                    a: x_seg.a,
                    p: m0.point,
                    b: x_seg.b,
                },
                store,
            );
            *s0 = Segment::create_and_validate_with_data(x_seg.a, m0.point, count, d0, store);

            let mut p0 = m0.point;

            for mi in sub_marks.iter().skip(1) {
                if !control.certify(p0, mi.point, x_seg.b) {
                    return;
                }
                let (di, next_data) = rest_data.split(
                    EdgeDataSplit {
                        a: p0,
                        p: mi.point,
                        b: x_seg.b,
                    },
                    store,
                );
                segments.push(Segment::create_and_validate_with_data(
                    p0, mi.point, count, di, store,
                ));
                rest_data = next_data;
                p0 = mi.point;
            }

            segments.push(Segment::create_and_validate_with_data(
                p0, x_seg.b, count, rest_data, store,
            ));
        }

        if !control.sort(segments.len()) {
            return;
        }
        segments.sort_by_ab(solver.is_parallel_sort_allowed());
        if !control.charge(segments.len()) {
            return;
        }
        segments.merge_if_needed_with_control(store, control);
    }

    #[inline]
    /// Orders snapped marks along directed Y on steep edges and retains shallow-edge ordering.
    /// Integer differences widen before subtraction, and equal-Y marks retain ascending X ties.
    /// The caller precharges sorting and polls afterward; coordinates, snapping, certificates and
    /// shared work/cancellation authority stay unchanged. Existing grouped scans also poll control.
    /// The caller supplies one nonempty same-edge mark group with at least two points.
    /// # Panics
    /// Panics for an empty group on the preserved shallow-edge path, violating that caller invariant.
    fn sort_sub_marks(
        marks: &mut [LineMark<I>],
        x_seg: XSegment<I>,
        control: &mut impl SplitControl<I>,
    ) {
        let dx = (x_seg.b.x.to_wide() - x_seg.a.x.to_wide()).unsigned_abs();
        let dy = (x_seg.b.y.to_wide() - x_seg.a.y.to_wide()).unsigned_abs();
        if dy > dx {
            let ascending = x_seg.a.y <= x_seg.b.y;
            marks.sort_unstable_by(|first, second| {
                let y_order = if ascending {
                    first.point.y.cmp(&second.point.y)
                } else {
                    second.point.y.cmp(&first.point.y)
                };
                y_order.then(first.point.x.cmp(&second.point.x))
            });
            return;
        }
        let mut j0 = 0;
        let mut j = 1;

        let m0 = marks[0];
        let mut x0 = m0.point.x;
        while j < marks.len() {
            if !control.charge(1) {
                return;
            }
            let xi = marks[j].point.x;
            if x0 == xi {
                j += 1;
                continue;
            }

            if j0 + 1 < j {
                let (y0, y1) = Self::y_range(j0, j, x_seg, marks);
                Self::sort_sub_marks_by_y(y0, y1, &mut marks[j0..j], control);
            }

            x0 = xi;
            j0 = j;
            j += 1;
        }

        if j0 + 1 < j {
            let (y0, y1) = Self::y_range(j0, j, x_seg, marks);
            Self::sort_sub_marks_by_y(y0, y1, &mut marks[j0..j], control);
        }
    }

    #[inline]
    fn y_range(j0: usize, j1: usize, s: XSegment<I>, marks: &[LineMark<I>]) -> (I, I) {
        let y0 = if j0 == 0 {
            s.a.y
        } else {
            marks[j0 - 1].point.y
        };
        let y1 = if j1 == marks.len() {
            s.b.y
        } else {
            marks[j1].point.y
        };
        (y0, y1)
    }

    #[inline]
    /// Reverses a descending equal-x mark group with bounded checks and unchanged ordering.
    fn sort_sub_marks_by_y(
        y0: I,
        y1: I,
        marks: &mut [LineMark<I>],
        control: &mut impl SplitControl<I>,
    ) {
        // The x-coordinate is the same for every point
        // By default, the range should be sorted in ascending order by the y-coordinate.
        if y0 > y1 {
            // reverse the order to sort the range in descending order by the y-coordinate.
            for i in 0..marks.len() / 2 {
                if !control.charge(1) {
                    return;
                }
                marks.swap(i, marks.len() - 1 - i);
            }
        }
    }
}

#[cfg(test)]
mod non_degenerate_tests {
    use super::*;
    use crate::segm::boolean::ShapeCountBoolean;
    use i_float::int::point::IntPoint;

    /// Proves unchanged central splitting does not emit degenerate grid edges.
    ///
    /// # Panics
    /// Panics if a mark splits at an endpoint or creates an invalid directed segment.
    #[test]
    fn grid_intersections_do_not_create_zero_length_segments() {
        let points: Vec<_> = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |y| IntPoint::new(x, y)))
            .collect();
        let mut edges = Vec::new();
        for (i, &a) in points.iter().enumerate() {
            for &b in &points[i + 1..] {
                edges.push(Segment::<ShapeCountBoolean, i32>::subject(a, b));
            }
        }
        let mut splitter = SplitSolver::new();
        let mut buffer = Vec::new();
        for (i, &a) in edges.iter().enumerate() {
            for &b in &edges[i + 1..] {
                for radius_squared in [1, 2, 4, 16] {
                    splitter.marks.clear();
                    SplitSolver::cross(
                        0,
                        1,
                        &a.x_segment,
                        &b.x_segment,
                        &mut splitter.marks,
                        radius_squared,
                    );
                    let mut segments = alloc::vec![a, b];
                    for mark in &splitter.marks {
                        let edge = segments[mark.index].x_segment;
                        assert!(
                            mark.point != edge.a && mark.point != edge.b,
                            "endpoint split: {edge:?}, {:?}",
                            mark.point
                        );
                    }
                    // Multiple intersections can produce the same mark.
                    splitter.marks.extend_from_within(..);
                    splitter.apply(
                        &mut segments,
                        &mut buffer,
                        &Solver::LIST,
                        &mut (),
                        &mut crate::core::checked::Unchecked,
                    );
                    assert!(segments.iter().all(|s| s.x_segment.a < s.x_segment.b));
                }
            }
        }
    }
}

#[cfg(test)]
mod zoom_split_ordering_tests {
    use super::*;
    use i_float::int::point::IntPoint;

    /// Checks vertical and steep signed traversal, deterministic ties and unchanged shallow paths.
    /// Coordinates stay exact; the public apply path owns sort charging and cancellation checks.
    /// # Panics
    /// Panics if snapped marks backtrack on the dominant axis or shallow/equal-slope order changes.
    #[test]
    fn zoom_split_ordering_respects_directed_dominant_axis_and_ties() {
        let cases = [
            ((0, 0), (4, 40), [(3, 10), (2, 20), (3, 20), (1, 30)]),
            ((0, 40), (4, 0), [(1, 30), (2, 20), (3, 20), (3, 10)]),
            ((4, 0), (4, 40), [(3, 10), (2, 20), (3, 20), (1, 30)]),
            ((4, 40), (4, 0), [(1, 30), (2, 20), (3, 20), (3, 10)]),
            (
                (-4, -40),
                (0, 0),
                [(-1, -30), (-2, -20), (-1, -20), (-3, -10)],
            ),
            ((0, 0), (40, 4), [(10, 3), (20, 3), (20, 2), (30, 1)]),
            ((0, 0), (40, 40), [(10, 3), (20, 3), (20, 2), (30, 1)]),
        ];
        for (a, b, expected) in cases {
            let point = |(x, y)| IntPoint::<i64>::new(x, y);
            let mut marks = expected.map(|xy| LineMark {
                index: 0,
                point: point(xy),
            });
            marks.sort_unstable_by_key(|mark| (mark.point.x, mark.point.y));
            SplitSolver::sort_sub_marks(
                &mut marks,
                XSegment {
                    a: point(a),
                    b: point(b),
                },
                &mut Unchecked,
            );
            assert_eq!(marks.map(|mark| mark.point), expected.map(point));
        }
    }

    /// Instantiates the same steep signed ordering for all supported integer engine widths.
    /// # Panics
    /// Panics if widening or directed ties differ between i16, i32 and i64.
    #[test]
    fn zoom_split_ordering_covers_integer_engine_widths() {
        macro_rules! check {
            ($int:ty) => {{
                let mut marks = [
                    LineMark {
                        index: 0,
                        point: IntPoint::<$int>::new(1, 10),
                    },
                    LineMark {
                        index: 0,
                        point: IntPoint::<$int>::new(2, 20),
                    },
                    LineMark {
                        index: 0,
                        point: IntPoint::<$int>::new(3, 20),
                    },
                ];
                SplitSolver::sort_sub_marks(
                    &mut marks,
                    XSegment {
                        a: IntPoint::new(0, 40),
                        b: IntPoint::new(4, 0),
                    },
                    &mut Unchecked,
                );
                assert_eq!(
                    marks.map(|m| (m.point.x, m.point.y)),
                    [(2, 20), (3, 20), (1, 10)]
                );
            }};
        }
        check!(i16);
        check!(i32);
        check!(i64);
    }

    /// Widens legal i64 endpoint differences before dominant-axis selection without wrapping.
    /// # Panics
    /// Panics if near-limit coordinates overflow or select the wrong traversal coordinate.
    #[test]
    fn zoom_split_ordering_widens_signed_integer_bounds() {
        let bound = (1_i64 << 62) - 1;
        for descending in [false, true] {
            let a = IntPoint::new(-bound, if descending { bound } else { -bound });
            let b = IntPoint::new(bound - 1, if descending { -bound } else { bound });
            let mut marks = [
                LineMark {
                    index: 0,
                    point: IntPoint::new(-1, if descending { -2 } else { 2 }),
                },
                LineMark {
                    index: 0,
                    point: IntPoint::new(1, if descending { 2 } else { -2 }),
                },
            ];
            SplitSolver::sort_sub_marks(&mut marks, XSegment { a, b }, &mut Unchecked);
            assert_eq!(marks[0].point.x, 1);
            assert_eq!(marks[1].point.x, -1);
        }
    }
}
