//! Indexed final connection selection and source-independent short-component repair.
use super::{
    ConnectionPathDiagnostics, ConnectionPathError, ConnectionPathLimits, Work, priority,
    priority_bucket,
};
use crate::{
    CurveSegment, FamilySiteId, LineSegment, Point2, SiteAdjacencyEdge, SiteAdjacencyGraph,
};
use std::collections::BinaryHeap;
use toniator_domain::{ConnectionCyclePolicy, ConnectionProgram, ConnectionSelectionIntent};

/// Reserves addressable selection storage and distinguishes arithmetic from allocator refusal.
/// # Errors
/// Returns count overflow or actual allocation failure before growth.
pub(super) fn reserve<T>(v: &mut Vec<T>, count: usize) -> Result<(), ConnectionPathError> {
    let total = v.len().checked_add(count).ok_or(error(
        "connection.selection.count",
        "selection count overflows",
    ))?;
    let bytes = total.checked_mul(std::mem::size_of::<T>()).ok_or(error(
        "connection.selection.count",
        "selection byte count overflows",
    ))?;
    if bytes > isize::MAX as usize {
        return Err(error(
            "connection.selection.count",
            "selection storage is not addressable",
        ));
    }
    v.try_reserve(count).map_err(|_| {
        error(
            "connection.selection.allocation",
            "selection allocation failed",
        )
    })
}
/// Reserves a heap slot with the same checked addressability distinction as vector storage.
/// # Errors
/// Returns count/address overflow separately from actual allocator refusal.
fn reserve_heap<T>(heap: &mut BinaryHeap<T>) -> Result<(), ConnectionPathError> {
    let count = heap
        .len()
        .checked_add(1)
        .ok_or(error("connection.selection.count", "heap count overflows"))?;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(error("connection.selection.count", "heap bytes overflow"))?;
    if bytes > isize::MAX as usize {
        return Err(error(
            "connection.selection.count",
            "heap storage is not addressable",
        ));
    }
    heap.try_reserve(1).map_err(|_| {
        error(
            "connection.selection.allocation",
            "Prim heap allocation failed",
        )
    })
}
/// Constructs a stable selection diagnostic without retaining partial geometry.
fn error(path: &'static str, message: &'static str) -> ConnectionPathError {
    ConnectionPathError::new(path, message)
}
/// Creates initialized indexed storage through checked fallible reservation.
/// # Errors
/// Propagates addressability or allocator refusal.
fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, ConnectionPathError> {
    let mut v = Vec::new();
    reserve(&mut v, n)?;
    v.resize(n, value);
    Ok(v)
}

/// Orders checked scratch records with cancellation at each merge observation.
/// # Errors
/// Propagates work/cancellation/addressability/allocation diagnostics.
fn sort<T: Copy>(
    values: &mut [T],
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    work: &mut Work<'_>,
) -> Result<(), ConnectionPathError> {
    if values.len() < 2 {
        return Ok(());
    }
    let mut scratch = Vec::new();
    reserve(&mut scratch, values.len())?;
    for value in values.iter().copied() {
        work.tick()?;
        scratch.push(value);
    }
    let mut width = 1usize;
    while width < values.len() {
        let mut start = 0;
        while start < values.len() {
            let mid = start.saturating_add(width).min(values.len());
            let end = mid.saturating_add(width).min(values.len());
            let (mut a, mut b) = (start, mid);
            for slot in &mut scratch[start..end] {
                work.tick()?;
                if a < mid && (b >= end || !compare(&values[a], &values[b]).is_gt()) {
                    *slot = values[a];
                    a += 1;
                } else {
                    *slot = values[b];
                    b += 1;
                }
            }
            start = end;
        }
        for (value, copy) in values.iter_mut().zip(&scratch) {
            work.tick()?;
            *value = *copy;
        }
        width = width.saturating_mul(2);
    }
    Ok(())
}
/// Partitions a unique-key slice at its median using cancellable in-place selection.
/// # Errors
/// Propagates work or cancellation; there is no numerical or creative item limit.
fn median<T: Copy>(
    values: &mut [T],
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    work: &mut Work<'_>,
) -> Result<(), ConnectionPathError> {
    if values.len() < 2 {
        return Ok(());
    }
    let target = values.len() / 2;
    let (mut lo, mut hi) = (0, values.len());
    while hi - lo > 1 {
        work.tick()?;
        let pivot = values[lo + (hi - lo) / 2];
        let mut a = lo;
        let mut b = hi;
        loop {
            while a < hi && compare(&values[a], &pivot).is_lt() {
                work.tick()?;
                a += 1;
            }
            while b > lo && compare(&values[b - 1], &pivot).is_gt() {
                work.tick()?;
                b -= 1;
            }
            if a >= b {
                break;
            }
            work.tick()?;
            values.swap(a, b - 1);
            a += 1;
            b -= 1;
        }
        if target < b {
            hi = b;
        } else if target >= a {
            lo = a;
        } else {
            break;
        }
    }
    Ok(())
}

/// Owns disjoint sets, final degrees and minimum-ID representatives independently of source response.
struct State {
    parent: Vec<usize>,
    size: Vec<usize>,
    degree: Vec<usize>,
    minimum: Vec<usize>,
    head: Vec<usize>,
    tail: Vec<usize>,
    next: Vec<Option<usize>>,
}
impl State {
    /// Allocates one finite state slot per eligible family vertex.
    /// # Errors
    /// Propagates checked storage failures.
    fn new(n: usize) -> Result<Self, ConnectionPathError> {
        let mut parent = Vec::new();
        reserve(&mut parent, n)?;
        parent.extend(0..n);
        let mut minimum = Vec::new();
        reserve(&mut minimum, n)?;
        minimum.extend(0..n);
        let mut head = Vec::new();
        reserve(&mut head, n)?;
        head.extend(0..n);
        let mut tail = Vec::new();
        reserve(&mut tail, n)?;
        tail.extend(0..n);
        Ok(Self {
            parent,
            size: filled(n, 1)?,
            degree: filled(n, 0)?,
            minimum,
            head,
            tail,
            next: filled(n, None)?,
        })
    }
    /// Finds a component with cancellation and compresses its traversed links.
    /// # Errors
    /// Propagates cancellation or the caller's finite inspection policy.
    fn root(&mut self, mut node: usize, work: &mut Work<'_>) -> Result<usize, ConnectionPathError> {
        let start = node;
        while self.parent[node] != node {
            work.tick()?;
            node = self.parent[node];
        }
        let root = node;
        node = start;
        while self.parent[node] != node {
            work.tick()?;
            let next = self.parent[node];
            self.parent[node] = root;
            node = next;
        }
        Ok(root)
    }
    /// Admits an ordinary/rescue edge under final degree and graph-cycle restrictions.
    /// # Errors
    /// Propagates work, cancellation or checked degree/component count failures.
    fn admit(
        &mut self,
        a: usize,
        b: usize,
        p: ConnectionSelectionIntent,
        forest: bool,
        work: &mut Work<'_>,
    ) -> Result<bool, ConnectionPathError> {
        work.tick()?;
        if p.maximum_final_degree
            .is_some_and(|max| self.degree[a] >= max as usize || self.degree[b] >= max as usize)
        {
            return Ok(false);
        }
        let ra = self.root(a, work)?;
        let rb = self.root(b, work)?;
        if ra == rb && (forest || p.cycles == ConnectionCyclePolicy::Forbid) {
            return Ok(false);
        }
        self.degree[a] = self.degree[a].checked_add(1).ok_or(error(
            "connection.selection.count",
            "final degree overflows",
        ))?;
        self.degree[b] = self.degree[b].checked_add(1).ok_or(error(
            "connection.selection.count",
            "final degree overflows",
        ))?;
        if ra != rb {
            let (root, other) = if self.size[ra] >= self.size[rb] {
                (ra, rb)
            } else {
                (rb, ra)
            };
            self.size[root] = self.size[root].checked_add(self.size[other]).ok_or(error(
                "connection.selection.count",
                "component size overflows",
            ))?;
            self.minimum[root] = self.minimum[root].min(self.minimum[other]);
            self.parent[other] = root;
            self.next[self.tail[root]] = Some(self.head[other]);
            self.tail[root] = self.tail[other];
        }
        Ok(true)
    }
}
/// Certifies exact collinearity of three finite binary64 coordinates without allocation.
/// Six signed original-coordinate products accumulate modulo 2^4224 in 66 limbs.
/// Each significand product has at most 106 bits, its shift is at most 4090, and
/// six terms have magnitude below 2^4199; therefore modular zero is exact zero.
/// This is solely a shared-endpoint overlap certificate, not a new curve tolerance.
/// # Errors
/// Returns cancellation or explicit inspection exhaustion before any candidate is admitted.
fn exact_collinear(
    origin: Point2,
    x: Point2,
    y: Point2,
    work: &mut Work<'_>,
) -> Result<bool, ConnectionPathError> {
    if (x.x == origin.x && y.x == origin.x) || (x.y == origin.y && y.y == origin.y) {
        work.tick()?;
        return Ok(true);
    }
    /// Decodes a finite binary64 scalar as signed integer significand times a power of two.
    fn parts(value: f64) -> (bool, u64, i32) {
        let bits = value.to_bits();
        let raw = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1u64 << 52) - 1);
        if raw == 0 {
            (bits >> 63 != 0, fraction, -1074)
        } else {
            (bits >> 63 != 0, fraction | (1u64 << 52), raw - 1075)
        }
    }
    let mut total = [0u64; 66];
    for (a, b, subtract) in [
        (x.x, y.y, false),
        (x.x, origin.y, true),
        (origin.x, y.y, true),
        (x.y, y.x, true),
        (x.y, origin.x, false),
        (origin.y, y.x, false),
    ] {
        work.tick()?;
        let (sa, ma, ea) = parts(a);
        let (sb, mb, eb) = parts(b);
        let product = u128::from(ma) * u128::from(mb);
        if product == 0 {
            continue;
        }
        let shift = (ea + eb + 2148) as usize;
        let word = shift / 64;
        let bits = shift % 64;
        let low = product as u64;
        let high = (product >> 64) as u64;
        let negative = sa ^ sb ^ subtract;
        let mut carry = false;
        for (i, slot) in total.iter_mut().enumerate() {
            work.tick()?;
            let value = if i == word {
                low << bits
            } else if i == word + 1 {
                if bits == 0 {
                    high
                } else {
                    (low >> (64 - bits)) | (high << bits)
                }
            } else if i == word + 2 && bits != 0 {
                high >> (64 - bits)
            } else {
                0
            };
            if negative {
                let (a, c1) = slot.overflowing_sub(value);
                let (b, c2) = a.overflowing_sub(u64::from(carry));
                *slot = b;
                carry = c1 || c2;
            } else {
                let (a, c1) = slot.overflowing_add(value);
                let (b, c2) = a.overflowing_add(u64::from(carry));
                *slot = b;
                carry = c1 || c2;
            }
        }
    }
    Ok(total.iter().all(|limb| *limb == 0))
}

/// Stores one finite axis-aligned segment box without changing numerical predicates.
#[derive(Clone, Copy)]
struct Box2 {
    lo: Point2,
    hi: Point2,
}
impl Box2 {
    /// Bounds finite endpoints with the existing curve tolerance and outward-rounded envelopes.
    /// # Errors
    /// Rejects an unrepresentable numerical tolerance; exterior infinite envelope endpoints remain conservative.
    fn segment(a: Point2, b: Point2) -> Result<Self, ConnectionPathError> {
        let pad = crate::curves::tolerance([a, b]).map_err(|_| {
            error(
                "connection.selection.numeric",
                "segment envelope tolerance is not finite",
            )
        })?;
        Ok(Self {
            lo: Point2::new(
                (a.x.min(b.x) - pad).next_down(),
                (a.y.min(b.y) - pad).next_down(),
            ),
            hi: Point2::new(
                (a.x.max(b.x) + pad).next_up(),
                (a.y.max(b.y) + pad).next_up(),
            ),
        })
    }
    /// Combines child envelopes conservatively.
    fn union(self, b: Self) -> Self {
        Self {
            lo: Point2::new(self.lo.x.min(b.lo.x), self.lo.y.min(b.lo.y)),
            hi: Point2::new(self.hi.x.max(b.hi.x), self.hi.y.max(b.hi.y)),
        }
    }
    /// Rejects only disjoint conservative boxes.
    fn overlaps(self, b: Self) -> bool {
        self.lo.x <= b.hi.x && b.lo.x <= self.hi.x && self.lo.y <= b.hi.y && b.lo.y <= self.hi.y
    }
}
/// Retains segment leaves and binary envelopes in checked contiguous storage.
struct SegmentIndex {
    nodes: Vec<SegmentNode>,
    roots: Vec<Option<usize>>,
    base: Option<usize>,
}
#[derive(Clone, Copy)]
struct SegmentNode {
    bounds: Box2,
    children: Option<(usize, usize)>,
    edge: SiteAdjacencyEdge,
}
impl SegmentIndex {
    /// Creates an empty dynamic broad-phase index.
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            base: None,
        }
    }
    /// Builds a spatially balanced immutable tree for all ordinary selected centerlines.
    /// Later rescue segments use a separate bounded-depth binary forest.
    /// # Errors
    /// Propagates checked storage, geometry and work/cancellation failures.
    fn build(
        edges: &[SiteAdjacencyEdge],
        lookup: &Lookup<'_>,
        work: &mut Work<'_>,
    ) -> Result<Self, ConnectionPathError> {
        let mut leaves = Vec::new();
        reserve(&mut leaves, edges.len())?;
        for edge in edges {
            work.tick()?;
            leaves.push(SegmentNode {
                bounds: Box2::segment(lookup.position(edge.first)?, lookup.position(edge.second)?)?,
                children: None,
                edge: *edge,
            });
        }
        /// Recursively creates logarithmic-depth spatial envelopes from finite edge centers.
        /// # Errors
        /// Propagates work and fallible node storage failures.
        fn build(
            nodes: &mut Vec<SegmentNode>,
            leaves: &mut [SegmentNode],
            depth: usize,
            work: &mut Work<'_>,
        ) -> Result<usize, ConnectionPathError> {
            work.tick()?;
            if leaves.len() == 1 {
                reserve(nodes, 1)?;
                let i = nodes.len();
                nodes.push(leaves[0]);
                return Ok(i);
            }
            median(
                leaves,
                |a, b| {
                    let center = |v: &SegmentNode| {
                        if depth.is_multiple_of(2) {
                            v.bounds.lo.x / 2.0 + v.bounds.hi.x / 2.0
                        } else {
                            v.bounds.lo.y / 2.0 + v.bounds.hi.y / 2.0
                        }
                    };
                    center(a).total_cmp(&center(b)).then(a.edge.cmp(&b.edge))
                },
                work,
            )?;
            let mid = leaves.len() / 2;
            let (a, b) = leaves.split_at_mut(mid);
            let a = build(nodes, a, depth + 1, work)?;
            let b = build(nodes, b, depth + 1, work)?;
            reserve(nodes, 1)?;
            let i = nodes.len();
            nodes.push(SegmentNode {
                bounds: nodes[a].bounds.union(nodes[b].bounds),
                children: Some((a, b)),
                edge: nodes[a].edge,
            });
            Ok(i)
        }
        let mut index = Self::new();
        if !leaves.is_empty() {
            index.base = Some(build(&mut index.nodes, &mut leaves, 0, work)?);
        }
        Ok(index)
    }
    /// Inserts an accepted centerline; allocation is complete before tree mutation.
    /// # Errors
    /// Propagates allocation, cancellation and explicit work exhaustion.
    fn insert(
        &mut self,
        edge: SiteAdjacencyEdge,
        bounds: Box2,
        work: &mut Work<'_>,
    ) -> Result<(), ConnectionPathError> {
        reserve(&mut self.nodes, 1)?;
        let mut root = self.nodes.len();
        self.nodes.push(SegmentNode {
            bounds,
            children: None,
            edge,
        });
        let mut level = 0;
        loop {
            work.tick()?;
            if level == self.roots.len() {
                reserve(&mut self.roots, 1)?;
                self.roots.push(None);
            }
            if let Some(other) = self.roots[level].take() {
                reserve(&mut self.nodes, 1)?;
                let branch = self.nodes.len();
                self.nodes.push(SegmentNode {
                    bounds: self.nodes[other].bounds.union(self.nodes[root].bounds),
                    children: Some((other, root)),
                    edge,
                });
                root = branch;
                level += 1;
            } else {
                self.roots[level] = Some(root);
                return Ok(());
            }
        }
    }
    /// Rejects any rescue contact except a common graph-ID endpoint; overlap never qualifies.
    /// # Errors
    /// Propagates numerical, storage, cancellation and work diagnostics atomically.
    fn clear(
        &self,
        edge: SiteAdjacencyEdge,
        lookup: &Lookup<'_>,
        work: &mut Work<'_>,
    ) -> Result<bool, ConnectionPathError> {
        let a = lookup.position(edge.first)?;
        let b = lookup.position(edge.second)?;
        let bounds = Box2::segment(a, b)?;
        let line = CurveSegment::Line(
            LineSegment::new(a, b)
                .map_err(|_| error("connection.geometry", "rescue segment is not finite"))?,
        );
        let mut stack = Vec::new();
        reserve(
            &mut stack,
            self.roots.len().checked_add(1).ok_or(error(
                "connection.selection.count",
                "segment root count overflows",
            ))?,
        )?;
        stack.extend(self.roots.iter().flatten().copied());
        stack.extend(self.base);
        while let Some(index) = stack.pop() {
            work.tick()?;
            let node = self.nodes[index];
            if !bounds.overlaps(node.bounds) {
                continue;
            }
            if let Some((a, b)) = node.children {
                reserve(&mut stack, 2)?;
                stack.push(b);
                stack.push(a);
                continue;
            }
            let other = node.edge;
            let c = lookup.position(other.first)?;
            let d = lookup.position(other.second)?;
            let other_line = CurveSegment::Line(
                LineSegment::new(c, d)
                    .map_err(|_| error("connection.geometry", "selected segment is not finite"))?,
            );
            // Exact finite rays remain meaningful even where the general solver classifies tiny lines as points.
            if let Some(shared) = [edge.first, edge.second]
                .into_iter()
                .find(|id| *id == other.first || *id == other.second)
            {
                let origin = lookup.position(shared)?;
                let x = lookup.position(if edge.first == shared {
                    edge.second
                } else {
                    edge.first
                })?;
                let y = lookup.position(if other.first == shared {
                    other.second
                } else {
                    other.first
                })?;
                if exact_collinear(origin, x, y, work)? {
                    let same_ray = if x.x != origin.x {
                        y.x != origin.x && (x.x > origin.x) == (y.x > origin.x)
                    } else {
                        y.y != origin.y && (x.y > origin.y) == (y.y > origin.y)
                    };
                    if same_ray {
                        return Ok(false);
                    }
                }
            }
            let contacts = match line.intersections(&other_line) {
                Ok(v) => v,
                Err(e) if e.path() == "curve.path.intersections.overlap" => return Ok(false),
                Err(_) => {
                    return Err(error(
                        "connection.selection.intersection",
                        "rescue intersection cannot be certified",
                    ));
                }
            };
            for contact in contacts {
                work.tick()?;
                let shared = [edge.first, edge.second]
                    .into_iter()
                    .find(|id| *id == other.first || *id == other.second);
                let Some(shared) = shared else {
                    return Ok(false);
                };
                let position = lookup.position(shared)?;
                if !crate::curves::coincident(contact.point(), position).map_err(|_| {
                    error(
                        "connection.selection.numeric",
                        "shared contact is not finite",
                    )
                })? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
/// Maps stable sparse family IDs to dense state slots without reauthoring site identity.
struct Lookup<'a> {
    graph: &'a SiteAdjacencyGraph,
    ids: Vec<(FamilySiteId, usize)>,
}
impl<'a> Lookup<'a> {
    /// Builds sorted lookup storage from the authoritative filtered family.
    /// # Errors
    /// Propagates storage/work failures or duplicate family IDs.
    fn new(
        graph: &'a SiteAdjacencyGraph,
        work: &mut Work<'_>,
    ) -> Result<Self, ConnectionPathError> {
        let mut ids = Vec::new();
        reserve(&mut ids, graph.nodes().len())?;
        for (i, node) in graph.nodes().iter().enumerate() {
            work.tick()?;
            ids.push((node.id, i));
        }
        sort(&mut ids, |a, b| a.cmp(b), work)?;
        if ids.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(error(
                "connection.graph.identity",
                "duplicate family node IDs",
            ));
        }
        Ok(Self { graph, ids })
    }
    /// Resolves a published stable ID without relying on its ordinal as an array offset.
    /// # Errors
    /// Rejects an edge referencing a node absent from the filtered family.
    fn index(&self, id: FamilySiteId) -> Result<usize, ConnectionPathError> {
        self.ids
            .binary_search_by_key(&id, |v| v.0)
            .map(|i| self.ids[i].1)
            .map_err(|_| error("connection.graph.identity", "edge node is absent"))
    }
    /// Returns the original emitted position.
    /// # Errors
    /// Rejects an unknown stable ID.
    fn position(&self, id: FamilySiteId) -> Result<Point2, ConnectionPathError> {
        Ok(self.graph.nodes()[self.index(id)?].position)
    }
}
/// Builds a balanced vertex index whose bounded pages do not limit artistic candidates.
struct PointIndex {
    order: Vec<usize>,
}
impl PointIndex {
    /// Partitions node indices by alternating axes with stable ID ties.
    /// # Errors
    /// Propagates allocation, cancellation or work limits.
    fn new(lookup: &Lookup<'_>, work: &mut Work<'_>) -> Result<Self, ConnectionPathError> {
        /// Recursively partitions shrinking slices; recursion depth is logarithmic.
        /// # Errors
        /// Propagates checked work/cancellation.
        fn partition(
            values: &mut [usize],
            depth: usize,
            lookup: &Lookup<'_>,
            work: &mut Work<'_>,
        ) -> Result<(), ConnectionPathError> {
            work.tick()?;
            if values.len() < 2 {
                return Ok(());
            }
            let axis = depth % 2;
            let mid = values.len() / 2;
            median(
                values,
                |a, b| {
                    let a = &lookup.graph.nodes()[*a];
                    let b = &lookup.graph.nodes()[*b];
                    let x = if axis == 0 {
                        a.position.x
                    } else {
                        a.position.y
                    };
                    let y = if axis == 0 {
                        b.position.x
                    } else {
                        b.position.y
                    };
                    x.total_cmp(&y).then(a.id.cmp(&b.id))
                },
                work,
            )?;
            let (a, b) = values.split_at_mut(mid);
            partition(a, depth + 1, lookup, work)?;
            partition(&mut b[1..], depth + 1, lookup, work)
        }
        let mut order = Vec::new();
        reserve(&mut order, lookup.ids.len())?;
        order.extend(0..lookup.ids.len());
        partition(&mut order, 0, lookup, work)?;
        Ok(Self { order })
    }
    /// Streams the next 64 exact-distance candidates after a stable distance/ID cursor.
    /// # Errors
    /// Returns checked work/numeric errors without collecting all radius pairs.
    fn page(
        &self,
        origin: usize,
        radius: f64,
        after: Option<(f64, FamilySiteId)>,
        lookup: &Lookup<'_>,
        work: &mut Work<'_>,
    ) -> Result<Vec<(f64, FamilySiteId, usize)>, ConnectionPathError> {
        /// Visits bounded index branches and retains only the best next page.
        /// # Errors
        /// Propagates work/numeric failures.
        #[allow(clippy::too_many_arguments)] // Explicit bounded query context; no hidden mutable global state.
        fn visit(
            values: &[usize],
            depth: usize,
            origin: usize,
            radius: f64,
            after: Option<(f64, FamilySiteId)>,
            lookup: &Lookup<'_>,
            out: &mut Vec<(f64, FamilySiteId, usize)>,
            work: &mut Work<'_>,
        ) -> Result<(), ConnectionPathError> {
            if values.is_empty() {
                return Ok(());
            }
            work.tick()?;
            let mid = values.len() / 2;
            let i = values[mid];
            let a = &lookup.graph.nodes()[origin];
            let b = &lookup.graph.nodes()[i];
            let separation = if depth.is_multiple_of(2) {
                a.position.x - b.position.x
            } else {
                a.position.y - b.position.y
            };
            if i != origin {
                let dx = a.position.x - b.position.x;
                let dy = a.position.y - b.position.y;
                // Overflowing differences/norms are provably beyond every finite authored radius.
                let d = if dx.abs() > radius.next_up() || dy.abs() > radius.next_up() {
                    f64::INFINITY
                } else {
                    dx.hypot(dy)
                };
                if d.is_finite()
                    && d > 0.0
                    && d <= radius
                    && after.is_none_or(|(last, id)| d.total_cmp(&last).then(b.id.cmp(&id)).is_gt())
                {
                    let at =
                        out.partition_point(|v| v.0.total_cmp(&d).then(v.1.cmp(&b.id)).is_lt());
                    if at < 64 {
                        if out.len() == 64 {
                            out.pop();
                        }
                        out.insert(at, (d, b.id, i));
                    }
                }
            }
            let (near, far) = if separation <= 0.0 {
                (&values[..mid], &values[mid + 1..])
            } else {
                (&values[mid + 1..], &values[..mid])
            };
            visit(near, depth + 1, origin, radius, after, lookup, out, work)?;
            let bound = if out.len() == 64 {
                radius.min(out[63].0)
            } else {
                radius
            };
            if !separation.is_finite() || separation.abs() <= bound.next_up() {
                visit(far, depth + 1, origin, radius, after, lookup, out, work)?;
            }
            Ok(())
        }
        let mut out = Vec::new();
        reserve(&mut out, 64)?;
        visit(
            &self.order,
            0,
            origin,
            radius,
            after,
            lookup,
            &mut out,
            work,
        )?;
        Ok(out)
    }
}

/// Selects ordinary edges, repairs short components and returns retained structural singleton slots.
/// # Errors
/// Returns atomic validation, numeric, checked-storage, work or cancellation diagnostics.
pub(super) fn select(
    graph: &SiteAdjacencyGraph,
    program: &ConnectionProgram,
    policy: ConnectionSelectionIntent,
    limits: ConnectionPathLimits,
    work: &mut Work<'_>,
) -> Result<
    (
        Vec<SiteAdjacencyEdge>,
        Vec<usize>,
        ConnectionPathDiagnostics,
    ),
    ConnectionPathError,
> {
    policy
        .validate()
        .map_err(|e| error(e.path(), e.message()))?;
    let lookup = Lookup::new(graph, work)?;
    let n = graph.nodes().len();
    let mut state = State::new(n)?;
    for (rank, &(_, index)) in lookup.ids.iter().enumerate() {
        work.tick()?;
        state.minimum[index] = rank;
    }
    let mut edges = Vec::new();
    let mut ranked = Vec::new();
    reserve(&mut ranked, graph.edges().len())?;
    for edge in graph.edges() {
        work.tick()?;
        let a = lookup.index(edge.first)?;
        let b = lookup.index(edge.second)?;
        let d = (graph.nodes()[a].position.x - graph.nodes()[b].position.x)
            .hypot(graph.nodes()[a].position.y - graph.nodes()[b].position.y);
        ranked.push((*edge, a, b, d));
    }
    match program {
        ConnectionProgram::NearestLinks { .. } => {
            sort(
                &mut ranked,
                |a, b| a.3.total_cmp(&b.3).then(a.0.cmp(&b.0)),
                work,
            )?;
            for (edge, a, b, _) in &ranked {
                if state.admit(*a, *b, policy, false, work)? {
                    push_edge(&mut edges, *edge, limits)?;
                }
            }
        }
        ConnectionProgram::RandomLinks {
            adjacency,
            minimum_degree,
            seed,
        } => {
            sort(
                &mut ranked,
                |a, b| {
                    (priority(*seed, a.0.first, a.0.second), a.0)
                        .cmp(&(priority(*seed, b.0.first, b.0.second), b.0))
                },
                work,
            )?;
            let mut targets = Vec::new();
            reserve(&mut targets, n)?;
            for node in graph.nodes() {
                work.tick()?;
                targets.push(
                    *minimum_degree as usize
                        + priority_bucket(
                            priority(*seed, node.id, node.id),
                            (adjacency.maximum_degree - minimum_degree + 1) as usize,
                        ),
                );
            }
            let mut chosen = filled(ranked.len(), false)?;
            for pass in 0..2 {
                for (i, (edge, a, b, _)) in ranked.iter().enumerate() {
                    work.tick()?;
                    if chosen[i] {
                        continue;
                    }
                    let need = if pass == 0 {
                        state.degree[*a] < *minimum_degree as usize
                            || state.degree[*b] < *minimum_degree as usize
                    } else {
                        state.degree[*a] < targets[*a] || state.degree[*b] < targets[*b]
                    };
                    if need
                        && state.degree[*a] < (adjacency.maximum_degree as usize)
                        && state.degree[*b] < (adjacency.maximum_degree as usize)
                        && state.admit(*a, *b, policy, false, work)?
                    {
                        push_edge(&mut edges, *edge, limits)?;
                        chosen[i] = true;
                    }
                }
            }
        }
        ConnectionProgram::GridSpanningTree { seed, .. } => {
            let mut offsets = filled(
                n.checked_add(1)
                    .ok_or(error("connection.selection.count", "CSR count overflows"))?,
                0usize,
            )?;
            for (_, a, b, _) in &ranked {
                work.tick()?;
                offsets[*a + 1] = offsets[*a + 1]
                    .checked_add(1)
                    .ok_or(error("connection.selection.count", "CSR count overflows"))?;
                offsets[*b + 1] = offsets[*b + 1]
                    .checked_add(1)
                    .ok_or(error("connection.selection.count", "CSR count overflows"))?;
            }
            for i in 1..offsets.len() {
                work.tick()?;
                offsets[i] = offsets[i]
                    .checked_add(offsets[i - 1])
                    .ok_or(error("connection.selection.count", "CSR count overflows"))?;
            }
            let mut cursor = Vec::new();
            reserve(&mut cursor, offsets.len())?;
            cursor.extend_from_slice(&offsets);
            let mut incident = filled(offsets[n], 0usize)?;
            for (i, (_, a, b, _)) in ranked.iter().enumerate() {
                work.tick()?;
                incident[cursor[*a]] = i;
                cursor[*a] += 1;
                incident[cursor[*b]] = i;
                cursor[*b] += 1;
            }
            let mut visited = filled(n, false)?;
            let mut heap = BinaryHeap::new();
            for &(_, root) in &lookup.ids {
                work.tick()?;
                if visited[root] {
                    continue;
                }
                visited[root] = true;
                for &i in &incident[offsets[root]..offsets[root + 1]] {
                    reserve_heap(&mut heap)?;
                    let e = ranked[i].0;
                    heap.push(std::cmp::Reverse((
                        priority(*seed, e.first, e.second),
                        e,
                        i,
                    )));
                }
                while let Some(std::cmp::Reverse((_, _, i))) = heap.pop() {
                    work.tick()?;
                    let (edge, a, b, _) = ranked[i];
                    if visited[a] == visited[b] {
                        continue;
                    }
                    if !state.admit(a, b, policy, true, work)? {
                        continue;
                    }
                    let next = if visited[a] { b } else { a };
                    visited[next] = true;
                    push_edge(&mut edges, edge, limits)?;
                    for &i in &incident[offsets[next]..offsets[next + 1]] {
                        work.tick()?;
                        reserve_heap(&mut heap)?;
                        let e = ranked[i].0;
                        heap.push(std::cmp::Reverse((
                            priority(*seed, e.first, e.second),
                            e,
                            i,
                        )));
                    }
                }
            }
        }
    }
    let points = PointIndex::new(&lookup, work)?;
    let mut segments = SegmentIndex::build(&edges, &lookup, work)?;
    for (rank, &(_, source_root)) in lookup.ids.iter().enumerate() {
        let mut root = state.root(source_root, work)?;
        if state.minimum[root] != rank {
            continue;
        }
        while state.size[root] == 1 || state.size[root] < (policy.minimum_component_points as usize)
        {
            let mut best: Option<(f64, FamilySiteId, FamilySiteId, usize, usize)> = None;
            let mut member = Some(state.head[root]);
            while let Some(a) = member {
                work.tick()?;
                member = state.next[a];
                let source_id = graph.nodes()[a].id;
                if policy
                    .maximum_final_degree
                    .is_some_and(|v| state.degree[a] >= v as usize)
                {
                    continue;
                }
                let mut after = None;
                'pages: loop {
                    let page = points.page(
                        a,
                        program.adjacency().maximum_distance,
                        after,
                        &lookup,
                        work,
                    )?;
                    if page.is_empty() {
                        break;
                    }
                    after = page.last().map(|v| (v.0, v.1));
                    let count = page.len();
                    for (distance, target_id, b) in page {
                        work.tick()?;
                        if state.root(b, work)? == root
                            || policy
                                .maximum_final_degree
                                .is_some_and(|v| state.degree[b] >= v as usize)
                        {
                            continue;
                        }
                        let edge = canonical(source_id, target_id);
                        if !segments.clear(edge, &lookup, work)? {
                            continue;
                        }
                        if best.as_ref().is_none_or(|v| {
                            distance
                                .total_cmp(&v.0)
                                .then(source_id.cmp(&v.1))
                                .then(target_id.cmp(&v.2))
                                .is_lt()
                        }) {
                            best = Some((distance, source_id, target_id, a, b));
                        }
                        break 'pages;
                    }
                    if count < 64 {
                        break;
                    }
                }
            }
            let Some((_, a_id, b_id, a, b)) = best else {
                break;
            };
            let edge = canonical(a_id, b_id);
            if !state.admit(a, b, policy, true, work)? {
                return Err(error(
                    "connection.selection.state",
                    "rescue admission changed without mutation",
                ));
            }
            push_edge(&mut edges, edge, limits)?;
            segments.insert(
                edge,
                Box2::segment(lookup.position(a_id)?, lookup.position(b_id)?)?,
                work,
            )?;
            root = state.root(a, work)?;
        }
    }
    let mut retained = Vec::new();
    reserve(&mut retained, edges.len())?;
    for edge in edges {
        work.tick()?;
        let root = state.root(lookup.index(edge.first)?, work)?;
        if state.size[root] >= policy.minimum_component_points as usize {
            retained.push(edge);
        }
    }
    sort(&mut retained, |a, b| a.cmp(b), work)?;
    let mut singletons = Vec::new();
    let mut diagnostics = ConnectionPathDiagnostics::default();
    for &(_, i) in &lookup.ids {
        work.tick()?;
        let root = state.root(i, work)?;
        if state.size[root] < policy.minimum_component_points as usize {
            continue;
        }
        if state.degree[i] == 0 {
            reserve(&mut diagnostics.isolated_nodes, 1)?;
            diagnostics.isolated_nodes.push(graph.nodes()[i].id);
            reserve(&mut singletons, 1)?;
            singletons.push(i);
        }
        if let ConnectionProgram::RandomLinks { minimum_degree, .. } = program
            && state.degree[i] < *minimum_degree as usize
        {
            reserve(&mut diagnostics.under_connected_nodes, 1)?;
            diagnostics.under_connected_nodes.push(graph.nodes()[i].id);
        }
    }
    Ok((retained, singletons, diagnostics))
}
/// Appends one selected edge under the explicit caller limit.
/// # Errors
/// Propagates count, allocation or edge-policy exhaustion.
fn push_edge(
    edges: &mut Vec<SiteAdjacencyEdge>,
    edge: SiteAdjacencyEdge,
    limits: ConnectionPathLimits,
) -> Result<(), ConnectionPathError> {
    if edges.len() >= limits.maximum_selected_edges {
        return Err(error(
            "connection.limits.selected_edges",
            "selected edges exceed caller policy",
        ));
    }
    reserve(edges, 1)?;
    edges.push(edge);
    Ok(())
}
/// Orders one undirected edge without changing endpoint identity.
fn canonical(a: FamilySiteId, b: FamilySiteId) -> SiteAdjacencyEdge {
    SiteAdjacencyEdge {
        first: a.min(b),
        second: a.max(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FamilySite, FamilySiteProvenance, FamilySiteSet, NominalCellBasis, SiteAdjacencyLimits,
        SiteAdjacencyPolicy, SiteScope, Vector2, build_site_adjacency_cancellable,
    };
    use toniator_domain::PatternMechanismId;
    /// Builds finite graph nodes for independent broad/narrow-phase and pagination checks.
    /// # Panics
    /// Panics if finite unit-basis graph setup fails.
    fn lookup_graph(points: &[(f64, f64)]) -> SiteAdjacencyGraph {
        let sites = FamilySiteSet::new(
            "index-check".into(),
            PatternMechanismId(9),
            points
                .iter()
                .enumerate()
                .map(|(i, p)| FamilySite {
                    id: FamilySiteId {
                        mechanism_id: PatternMechanismId(9),
                        ordinal: i,
                    },
                    position: Point2::new(p.0, p.1),
                    nominal_cell_basis: NominalCellBasis::new(
                        Vector2::new(1., 0.),
                        Vector2::new(0., 1.),
                    )
                    .unwrap(),
                    scope: SiteScope::Canvas,
                    provenance: FamilySiteProvenance::Random {
                        candidate_ordinal: i,
                        accepted_ordinal: i,
                        exclusion_neighbor_ordinal: None,
                    },
                })
                .collect(),
        )
        .unwrap();
        build_site_adjacency_cancellable(
            &sites,
            SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: 1,
                maximum_distance: 1000.,
            },
            SiteAdjacencyLimits::default(),
            &|| false,
        )
        .unwrap()
    }
    /// Rejects crossing, unrelated endpoint touch, T contacts and overlap while allowing shared-ID joints.
    /// # Panics
    /// Panics if existing exact intersection authority admits an unsafe rescue or rejects a safe joint.
    #[test]
    fn rescue_contacts_follow_graph_endpoint_identity_and_existing_tolerance() {
        for (points, clear) in [
            (vec![(0., 0.), (2., 2.), (0., 2.), (2., 0.)], false),
            (vec![(0., 0.), (2., 0.), (1., 0.), (1., 1.)], false),
            (vec![(0., 0.), (2., 0.), (1., 0.), (3., 0.)], false),
            (vec![(0., 0.), (2., 0.), (2., 0.), (3., 1.)], false),
            (vec![(0., 0.), (2., 0.), (0., 1.), (2., 1.)], true),
        ] {
            let graph = lookup_graph(&points);
            let mut work = Work::new(100000, &|| false);
            let lookup = Lookup::new(&graph, &mut work).unwrap();
            let edge = canonical(graph.nodes()[0].id, graph.nodes()[1].id);
            let index = SegmentIndex::build(&[edge], &lookup, &mut work).unwrap();
            assert_eq!(
                index
                    .clear(
                        canonical(graph.nodes()[2].id, graph.nodes()[3].id),
                        &lookup,
                        &mut work
                    )
                    .unwrap(),
                clear
            );
        }
        for (target, clear) in [
            ((1e-10, 0.0), false),
            ((-1e-10, 0.0), true),
            ((0.0, 1e-10), true),
        ] {
            let graph = lookup_graph(&[(0., 0.), (2e-10, 0.), target]);
            let mut work = Work::new(100000, &|| false);
            let lookup = Lookup::new(&graph, &mut work).unwrap();
            let edge = canonical(graph.nodes()[0].id, graph.nodes()[1].id);
            let index = SegmentIndex::build(&[edge], &lookup, &mut work).unwrap();
            assert_eq!(
                index
                    .clear(
                        canonical(graph.nodes()[0].id, graph.nodes()[2].id),
                        &lookup,
                        &mut work
                    )
                    .unwrap(),
                clear
            );
        }
        let graph = lookup_graph(&[(0.123456789, 1.987654321), (4.321, 3.123), (5.777, 0.111)]);
        let mut work = Work::new(100000, &|| false);
        let lookup = Lookup::new(&graph, &mut work).unwrap();
        let edge = canonical(graph.nodes()[0].id, graph.nodes()[1].id);
        let index = SegmentIndex::build(&[edge], &lookup, &mut work).unwrap();
        assert!(
            index
                .clear(
                    canonical(graph.nodes()[1].id, graph.nodes()[2].id),
                    &lookup,
                    &mut work
                )
                .unwrap()
        );
    }
    /// Proves pages retain equality/ID ties and all later candidates, without a 64-neighbor ceiling.
    /// # Panics
    /// Panics if pagination repeats, skips or truncates a radius candidate.
    #[test]
    fn radius_pages_continue_past_64_rejected_candidates() {
        let points = (0..151).map(|i| (i as f64, 0.)).collect::<Vec<_>>();
        let graph = lookup_graph(&points);
        let mut work = Work::new(1000000, &|| false);
        let lookup = Lookup::new(&graph, &mut work).unwrap();
        let index = PointIndex::new(&lookup, &mut work).unwrap();
        let mut cursor = None;
        let mut found = Vec::new();
        loop {
            let page = index.page(0, 200., cursor, &lookup, &mut work).unwrap();
            if page.is_empty() {
                break;
            }
            assert!(page.len() <= 64);
            cursor = page.last().map(|v| (v.0, v.1));
            found.extend(page.into_iter().map(|v| v.2));
        }
        assert_eq!(found, (1..151).collect::<Vec<_>>());
    }
    /// Measures spatial pruning for a full-radius nearest page and shuffled original segment insertion.
    /// # Panics
    /// Panics if broad-phase work approaches an exhaustive scan on these separated finite fixtures.
    #[test]
    fn spatial_indexes_prune_full_radius_and_shuffled_original_edges() {
        let coords = (0..4096)
            .map(|i| (i as f64, ((i % 7) as f64) * 0.01))
            .collect::<Vec<_>>();
        let graph = lookup_graph(&coords);
        let mut work = Work::new(10000000, &|| false);
        let lookup = Lookup::new(&graph, &mut work).unwrap();
        let point_index = PointIndex::new(&lookup, &mut work).unwrap();
        let before = work.remaining;
        let page = point_index
            .page(0, 100000., None, &lookup, &mut work)
            .unwrap();
        let point_work = before - work.remaining;
        assert_eq!(page.len(), 64);
        assert!(point_work < 512, "{point_work}");
        let mut edges = (0..2048)
            .map(|i| canonical(graph.nodes()[i * 2].id, graph.nodes()[i * 2 + 1].id))
            .collect::<Vec<_>>();
        edges.sort_by_key(|edge| priority(17, edge.first, edge.second));
        let index = SegmentIndex::build(&edges, &lookup, &mut work).unwrap();
        let before = work.remaining;
        let _ = index
            .clear(
                canonical(graph.nodes()[100].id, graph.nodes()[102].id),
                &lookup,
                &mut work,
            )
            .unwrap();
        let segment_work = before - work.remaining;
        assert!(segment_work < 1024, "{segment_work}");
        println!(
            "4096 nodes: first full-radius page inspections={point_work}; shuffled 2048 segment query inspections={segment_work}"
        );
    }

    /// Checks finite extreme-range and tiny angled exact collinearity without underflow or false-zero rounding.
    /// # Panics
    /// Panics if bounded exact-zero arithmetic misclassifies its independently obvious determinant cases.
    #[test]
    fn exact_shared_endpoint_collinearity_covers_full_binary64_range() {
        let mut work = Work::new(100000, &|| false);
        for (o, x, y, expected) in [
            (
                Point2::new(0., 0.),
                Point2::new(2e-10, 4e-10),
                Point2::new(1e-10, 2e-10),
                true,
            ),
            (
                Point2::new(0., 0.),
                Point2::new(2e-10, 4e-10),
                Point2::new(1e-10, (2e-10f64).next_up()),
                false,
            ),
            (
                Point2::new(0., 0.),
                Point2::new(f64::from_bits(1), f64::from_bits(2)),
                Point2::new(f64::from_bits(2), f64::from_bits(4)),
                true,
            ),
            (
                Point2::new(-8e307, -8e307),
                Point2::new(8e307, 8e307),
                Point2::new(0., 0.),
                true,
            ),
            (
                Point2::new(-8e307, -8e307),
                Point2::new(8e307, 8e307),
                Point2::new(0., 1.),
                false,
            ),
        ] {
            assert_eq!(exact_collinear(o, x, y, &mut work).unwrap(), expected);
        }
    }

    /// Distinguishes count/address failures from actual reservation refusal in audited new storage.
    /// # Panics
    /// Panics if impossible storage is mislabeled as a physical allocator failure.
    #[test]
    fn impossible_selection_storage_reports_count_not_allocator() {
        let mut values = Vec::<u64>::new();
        assert_eq!(
            reserve(&mut values, usize::MAX).unwrap_err().path(),
            "connection.selection.count"
        );
        let mut bytes = Vec::<u8>::new();
        assert_eq!(
            reserve(&mut bytes, isize::MAX as usize).unwrap_err().path(),
            "connection.selection.allocation"
        );
    }
}
