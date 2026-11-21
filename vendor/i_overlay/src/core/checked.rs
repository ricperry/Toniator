// Toniator local patch (2026-10-13): opt-in checked Boolean work; see PROVENANCE.md.
//! Opt-in bounded i64 splitting for Toniator native area rasterization.
//!
//! Certificates bound actual split mutations relative to the supplied integer segments only.
//! They do not certify initial float quantization, curve flattening, or graph extraction.

use crate::core::{
    fill_rule::FillRule, overlay::Overlay, overlay_rule::OverlayRule, solver::Precision,
};
use alloc::vec::Vec;
use i_float::int::{number::int::IntNumber, point::IntPoint};
use i_shape::int::shape::IntShapes;

/// One bounded group of exact split triangles; its box encloses every triangle vertex.
/// Group boxes may overlap, but each triangle contributes its absolute area exactly once.
#[derive(Clone, Debug, Default)]
pub struct CheckedSplitRegion {
    pub bounds: [i64; 4],
    pub doubled_area: u128,
}

/// Fixed-size optional proof storage, independent of the number of input edges or intersections.
struct SpatialCertificate {
    bounds: [i64; 4],
    regions: Vec<CheckedSplitRegion>,
    single: Option<(usize, CheckedSplitRegion)>,
    disabled: bool,
}

impl SpatialCertificate {
    /// Initializes allocation-free proof storage; invalid bounds decline the optional proof.
    fn new(bounds: [i64; 4]) -> Option<Self> {
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
            return None;
        }
        Some(Self {
            bounds,
            regions: Vec::new(),
            single: None,
            disabled: false,
        })
    }

    /// Clears discarded-attempt support and area without resetting request work.
    fn clear(&mut self) {
        self.regions.fill(CheckedSplitRegion::default());
        self.single = None;
    }

    /// Assigns a full triangle to one bucket and retains its complete support, including long edges.
    /// The global checked sum already proves that every bucket addition fits in u128.
    /// One occupied bucket stays inline; a second lazily allocates at most 1024 slots. Refusal
    /// disables all spatial proof, preserving the original global scalar charge.
    /// # Panics
    /// The promotion invariant requires the previously occupied inline bucket to exist.
    fn record(&mut self, a: IntPoint<i64>, p: IntPoint<i64>, b: IntPoint<i64>, area: u128) {
        if area == 0 || self.disabled {
            return;
        }
        let bounds = [
            a.x.min(p.x).min(b.x),
            a.y.min(p.y).min(b.y),
            a.x.max(p.x).max(b.x),
            a.y.max(p.y).max(b.y),
        ];
        let cell = |axis: usize| {
            let lo = i128::from(self.bounds[axis]);
            let extent = i128::from(self.bounds[axis + 2]) - lo;
            let midpoint = (i128::from(bounds[axis]) + i128::from(bounds[axis + 2])) / 2;
            ((midpoint - lo) * 32 / extent).clamp(0, 31) as usize
        };
        let index = cell(0) + 32 * cell(1);
        if self.regions.is_empty() && self.single.as_ref().is_some_and(|(old, _)| *old != index) {
            if self.regions.try_reserve_exact(1024).is_err() {
                self.disabled = true;
                self.single = None;
                return;
            }
            self.regions.resize(1024, CheckedSplitRegion::default());
            let (old, region) = self.single.take().unwrap();
            self.regions[old] = region;
        }
        let region = if self.regions.is_empty() {
            &mut self
                .single
                .get_or_insert_with(|| (index, CheckedSplitRegion::default()))
                .1
        } else {
            &mut self.regions[index]
        };
        if region.doubled_area == 0 {
            region.bounds = bounds;
        } else {
            for axis in 0..2 {
                region.bounds[axis] = region.bounds[axis].min(bounds[axis]);
                region.bounds[axis + 2] = region.bounds[axis + 2].max(bounds[axis + 2]);
            }
        }
        region.doubled_area += area;
    }
}

/// Stable failures from the opt-in checked Boolean route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckedOverlayError {
    /// Cancellation was observed at a controlled work boundary.
    Cancelled,
    /// The request's separate Boolean operation allowance was exhausted.
    WorkLimit,
    /// All bounded precision attempts failed to finish their split rounds.
    NonConvergent,
    /// Exact certificate arithmetic overflowed.
    CertificateOverflow,
    /// The caller requested float cleanup which this integer certificate does not cover.
    UncertifiedCleanup,
}

/// Successful geometry and its accepted-attempt absolute doubled-area certificate.
pub struct CheckedOverlayResult {
    pub shapes: IntShapes<i64>,
    pub doubled_area: u128,
}

/// Request-local operation accounting shared across retries and strategy fallbacks.
///
/// This allowance is a resource bound, not a proof of asymptotic complexity. Existing upstream
/// allocation, sorting, fill, and graph extraction retain their synchronous cancellation gaps.
pub struct CheckedOverlayControl<'a> {
    remaining: usize,
    initial: usize,
    cancelled: &'a dyn Fn() -> bool,
    error: Option<CheckedOverlayError>,
    doubled_area: u128,
    rounds: usize,
    max_rounds: usize,
    attempts: usize,
    total_rounds: usize,
    poll_at: usize,
    spatial: Option<SpatialCertificate>,
}

impl<'a> CheckedOverlayControl<'a> {
    /// Initializes a separate Boolean allowance and cancellation callback; attempts use at most 16 rounds.
    pub fn new(max_operations: usize, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            remaining: max_operations,
            initial: max_operations,
            cancelled,
            error: None,
            doubled_area: 0,
            rounds: 0,
            max_rounds: 16,
            attempts: 0,
            total_rounds: 0,
            poll_at: 0,
            spatial: None,
        }
    }

    /// Requests bounded triangle-support metadata without changing geometry, work or scalar certification.
    /// Invalid bounds or allocation refusal leave the original full scalar certificate authoritative.
    pub fn with_spatial_certificate(mut self, bounds: [i64; 4]) -> Self {
        self.spatial = SpatialCertificate::new(bounds);
        self
    }

    /// Takes accepted-attempt support; missing storage requires the caller to charge the full certificate.
    /// Only nonempty buckets remain, and retained capacity never exceeds 1024 buckets.
    pub fn take_spatial_certificate(&mut self) -> Option<Vec<CheckedSplitRegion>> {
        let mut spatial = self.spatial.take()?;
        if spatial.disabled {
            return None;
        }
        if let Some((_, region)) = spatial.single {
            spatial.regions.try_reserve_exact(1).ok()?;
            spatial.regions.push(region);
        } else {
            spatial.regions.retain(|region| region.doubled_area != 0);
        }
        Some(spatial.regions)
    }

    /// Reduces the fixed per-attempt round ceiling for stricter callers and focused failure tests.
    pub fn with_max_split_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds.min(16);
        self
    }

    /// Returns all charged operations, including discarded attempts.
    pub fn operations(&self) -> usize {
        self.initial - self.remaining
    }

    /// Returns the number of attempted fixed radii, including a failed final attempt.
    pub fn attempts(&self) -> usize {
        self.attempts
    }

    /// Returns split rounds across all attempts and fallback strategies.
    pub fn split_rounds(&self) -> usize {
        self.total_rounds
    }

    /// Records a sticky failure and prevents further checked work.
    fn fail(&mut self, error: CheckedOverlayError) -> bool {
        self.error = Some(error);
        false
    }

    /// Reserves work and polls cancellation at intervals of at most 1024 charged scalar operations.
    /// Bulk reservations poll before synchronous upstream phases, not inside those phases.
    fn charge_work(&mut self, units: usize) -> bool {
        if self.error.is_some() {
            return false;
        }
        let used = self.operations();
        if used.saturating_add(units) > self.poll_at || used >= self.poll_at || units >= 1024 {
            if (self.cancelled)() {
                return self.fail(CheckedOverlayError::Cancelled);
            }
            self.poll_at = used.saturating_add(1024);
        }
        match self.remaining.checked_sub(units) {
            Some(left) => {
                self.remaining = left;
                true
            }
            None => self.fail(CheckedOverlayError::WorkLimit),
        }
    }

    /// Starts one precision attempt while retaining all request work already charged.
    fn begin_attempt(&mut self) {
        self.attempts += 1;
        self.rounds = 0;
        self.doubled_area = 0;
        if let Some(spatial) = &mut self.spatial {
            spatial.clear();
        }
        self.error = None;
        self.poll_at = self.operations();
    }

    /// Accumulates the exact absolute triangle for one directed current-remainder split.
    /// Optional bounded support metadata receives the same full triangle only after checked summation.
    ///
    /// # Errors
    /// Stores certificate overflow or work/cancellation failure without publishing geometry.
    fn triangle(&mut self, a: IntPoint<i64>, p: IntPoint<i64>, b: IntPoint<i64>) -> bool {
        if !self.charge_work(2) {
            return false;
        }
        let bx = i128::from(b.x) - i128::from(a.x);
        let by = i128::from(b.y) - i128::from(a.y);
        let px = i128::from(p.x) - i128::from(a.x);
        let py = i128::from(p.y) - i128::from(a.y);
        let area = bx
            .checked_mul(py)
            .and_then(|first| {
                by.checked_mul(px)
                    .and_then(|second| first.checked_sub(second))
            })
            .and_then(i128::checked_abs)
            .and_then(|value| u128::try_from(value).ok());
        if let Some(total) = area.and_then(|value| self.doubled_area.checked_add(value)) {
            self.doubled_area = total;
            if let Some(spatial) = &mut self.spatial {
                spatial.record(a, p, b, area.unwrap());
            }
            true
        } else {
            self.fail(CheckedOverlayError::CertificateOverflow)
        }
    }
}

/// Internal compile-time optional controls preserve the ordinary API and strategy implementation.
pub(crate) trait SplitControl<I: IntNumber> {
    #[cfg(feature = "allow_multithreading")]
    fn is_checked(&self) -> bool {
        false
    }
    fn charge(&mut self, _units: usize) -> bool {
        true
    }
    fn round(&mut self) -> bool {
        true
    }
    fn certify(&mut self, _a: IntPoint<I>, _p: IntPoint<I>, _b: IntPoint<I>) -> bool {
        true
    }
    /// Reserves an n times (ceil(log2(n)) plus one) synchronous sort-work allowance.
    fn sort(&mut self, len: usize) -> bool {
        let log = if len > 1 {
            (usize::BITS - (len - 1).leading_zeros()) as usize
        } else {
            0
        };
        self.charge(len.saturating_mul(log.saturating_add(1)))
    }
}

pub(crate) struct Unchecked;
impl<I: IntNumber> SplitControl<I> for Unchecked {
    /// Keeps ordinary sorting free of checked accounting or numeric changes.
    fn sort(&mut self, _len: usize) -> bool {
        true
    }
}

impl SplitControl<i64> for CheckedOverlayControl<'_> {
    /// Identifies the checked route so fragment enumeration stays serial and cancellable.
    #[cfg(feature = "allow_multithreading")]
    fn is_checked(&self) -> bool {
        true
    }
    /// Charges the same request allowance through strategy fallbacks.
    fn charge(&mut self, units: usize) -> bool {
        self.charge_work(units)
    }
    /// Enforces one attempt's shared round ceiling across fallback strategies.
    fn round(&mut self) -> bool {
        if !self.charge_work(1) {
            return false;
        }
        if self.rounds >= self.max_rounds {
            return self.fail(CheckedOverlayError::NonConvergent);
        }
        self.rounds += 1;
        self.total_rounds += 1;
        true
    }
    /// Certifies exactly one central split before it mutates the current directed edge.
    fn certify(&mut self, a: IntPoint<i64>, p: IntPoint<i64>, b: IntPoint<i64>) -> bool {
        self.triangle(a, p, b)
    }
}

impl Overlay<i64> {
    /// Lifts original integer segments exactly by two binary fraction bits before checked splitting.
    /// Positive scaling preserves the common origin, winding, ordering and source geometry.
    /// Lifted input and accepted output coordinates remain exactly representable as f64 integers; nonzero area filters
    /// and unavailable fallible storage decline this optional refinement without changing input.
    /// All scans and solver retries consume the supplied cumulative work/cancellation control.
    /// # Errors
    /// Returns checked cancellation, work, convergence or certificate-overflow failures.
    pub(crate) fn overlay_checked_refined(
        &self,
        overlay_rule: OverlayRule,
        fill_rule: FillRule,
        control: &mut CheckedOverlayControl<'_>,
    ) -> Result<Option<CheckedOverlayResult>, CheckedOverlayError> {
        const FACTOR: i64 = 4;
        const MAXIMUM_COORDINATE: i64 = 1_i64 << 53;
        if self.options.min_output_area != 0 {
            return Ok(None);
        }
        let mut refined = Overlay::new_custom(0, self.options, self.solver);
        if refined
            .segments
            .try_reserve_exact(self.segments.len())
            .is_err()
        {
            return Ok(None);
        }
        for source in &self.segments {
            if !control.charge_work(1) {
                return Err(control.error.unwrap_or(CheckedOverlayError::WorkLimit));
            }
            let mut segment = *source;
            for point in [&mut segment.x_segment.a, &mut segment.x_segment.b] {
                for coordinate in [&mut point.x, &mut point.y] {
                    let Some(lifted) = coordinate.checked_mul(FACTOR) else {
                        return Ok(None);
                    };
                    if !(-MAXIMUM_COORDINATE..=MAXIMUM_COORDINATE).contains(&lifted)
                        || lifted / FACTOR != *coordinate
                    {
                        return Ok(None);
                    }
                    *coordinate = lifted;
                }
            }
            refined.segments.push(segment);
        }
        let result = refined.overlay_checked(overlay_rule, fill_rule, control)?;
        for point in result
            .shapes
            .iter()
            .flat_map(|shape| shape.iter())
            .flat_map(|contour| contour.iter())
        {
            if !control.charge_work(1) {
                return Err(control.error.unwrap_or(CheckedOverlayError::WorkLimit));
            }
            if !(-MAXIMUM_COORDINATE..=MAXIMUM_COORDINATE).contains(&point.x)
                || !(-MAXIMUM_COORDINATE..=MAXIMUM_COORDINATE).contains(&point.y)
            {
                return Ok(None);
            }
        }
        Ok(Some(result))
    }

    /// Runs original split algorithms at fixed radii 1, 2, then 4, with bounded retries.
    ///
    /// Each attempt starts from the original integer segments. Failed-attempt geometry and
    /// certificates are discarded; all attempts consume the same Boolean work allowance.
    /// Ordinary overlay consumers keep their existing behavior. No partial shapes escape.
    ///
    /// # Errors
    /// Returns cancellation, work exhaustion, exhausted convergence, or certificate overflow.
    pub fn overlay_checked(
        &mut self,
        overlay_rule: OverlayRule,
        fill_rule: FillRule,
        control: &mut CheckedOverlayControl<'_>,
    ) -> Result<CheckedOverlayResult, CheckedOverlayError> {
        let original = &self.segments;
        for exponent in [0, 2, 4] {
            control.begin_attempt();
            if !control.charge_work(original.len()) {
                return Err(control.error.unwrap());
            }
            let mut segments = original.clone();
            let solver = crate::core::solver::Solver {
                precision: Precision {
                    start: exponent,
                    progression: 0,
                },
                multithreading: None,
                ..self.solver
            };
            let mut split_solver = crate::split::solver::SplitSolver::new();
            split_solver.split_segments_with_control(&mut segments, &solver, &mut (), control);
            if let Some(error) = control.error {
                if error == CheckedOverlayError::NonConvergent {
                    continue;
                }
                return Err(error);
            }
            if !control.sort(segments.len()) {
                return Err(control.error.unwrap());
            }
            let mut buffer = self.boolean_buffer.take().unwrap_or_default();
            let shapes = self
                .graph_builder
                .build_boolean_overlay(fill_rule, overlay_rule, self.options, &solver, &segments)
                .extract_shapes(overlay_rule, &mut buffer);
            self.boolean_buffer = Some(buffer);
            // Graph extraction and allocation are synchronous upstream phases. Account their
            // outputs and poll before publication, without claiming in-phase cancellation.
            let points = shapes
                .iter()
                .flat_map(|s| s.iter())
                .map(|c| c.len())
                .try_fold(0usize, |sum, contour| sum.checked_add(contour))
                .ok_or(CheckedOverlayError::WorkLimit)?;
            if !control.charge_work(points) || (control.cancelled)() {
                return Err(control.error.unwrap_or(CheckedOverlayError::Cancelled));
            }
            return Ok(CheckedOverlayResult {
                shapes,
                doubled_area: control.doubled_area,
            });
        }
        Err(CheckedOverlayError::NonConvergent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Keeps zero/single-bucket paths allocation-free and retains complete support through promotion.
    /// # Panics
    /// Panics if grouping loses any vertex, exact area, retry reset, or conservative refusal fallback.
    #[test]
    fn spatial_support_is_bounded_lazy_and_attempt_local() {
        let mut control = CheckedOverlayControl::new(1000, &|| false)
            .with_spatial_certificate([-100, -100, 100, 100]);
        assert!(control.spatial.as_ref().unwrap().regions.is_empty());
        assert!(control.triangle(
            IntPoint::new(-90, -90),
            IntPoint::new(90, -90),
            IntPoint::new(90, -89)
        ));
        assert!(control.spatial.as_ref().unwrap().regions.is_empty());
        let single = control.spatial.as_ref().unwrap().single.as_ref().unwrap();
        assert_eq!(single.1.bounds, [-90, -90, 90, -89]);
        assert_eq!(single.1.doubled_area, 180);
        assert!(control.triangle(
            IntPoint::new(70, 70),
            IntPoint::new(90, 70),
            IntPoint::new(90, 71)
        ));
        assert_eq!(control.spatial.as_ref().unwrap().regions.len(), 1024);
        let used = control.operations();
        control.begin_attempt();
        assert_eq!(control.operations(), used);
        assert_eq!(control.doubled_area, 0);
        assert!(
            control
                .spatial
                .as_ref()
                .unwrap()
                .regions
                .iter()
                .all(|r| r.doubled_area == 0)
        );
        assert!(control.triangle(
            IntPoint::new(0, 0),
            IntPoint::new(1, 0),
            IntPoint::new(1, 1)
        ));
        assert_eq!(
            control
                .take_spatial_certificate()
                .unwrap()
                .iter()
                .map(|r| r.doubled_area)
                .sum::<u128>(),
            1
        );
        let mut refused = CheckedOverlayControl::new(1000, &|| false)
            .with_spatial_certificate([-100, -100, 100, 100]);
        // Deterministically exercise the same no-proof state as optional allocation refusal.
        refused.spatial.as_mut().unwrap().disabled = true;
        assert!(refused.triangle(
            IntPoint::new(0, 0),
            IntPoint::new(1, 0),
            IntPoint::new(1, 1)
        ));
        assert_eq!(refused.doubled_area, 1);
        assert!(refused.take_spatial_certificate().is_none());
        assert!(
            CheckedOverlayControl::new(10, &|| false)
                .with_spatial_certificate([0; 4])
                .take_spatial_certificate()
                .is_none()
        );
    }

    /// Verifies exact source lifting improves a rational crossing without mutating winding inputs.
    /// # Panics
    /// Panics if the bounded refinement changes source endpoints or invents split area at an exact node.
    #[test]
    fn refined_integer_lift_preserves_input_and_improves_certificate() {
        let bow = vec![
            IntPoint::new(0, 0),
            IntPoint::new(10, 7),
            IntPoint::new(0, 7),
            IntPoint::new(10, 0),
        ];
        let mut overlay = Overlay::from_subj(&bow);
        let before = overlay.segments.clone();
        let coarse = overlay
            .overlay_checked(
                OverlayRule::Subject,
                FillRule::EvenOdd,
                &mut CheckedOverlayControl::new(100_000, &|| false),
            )
            .unwrap();
        let mut control = CheckedOverlayControl::new(100_000, &|| false);
        let fine = overlay
            .overlay_checked_refined(OverlayRule::Subject, FillRule::EvenOdd, &mut control)
            .unwrap()
            .unwrap();
        assert!(coarse.doubled_area > fine.doubled_area);
        assert_eq!(fine.doubled_area, 0);
        assert!(
            fine.shapes
                .iter()
                .flatten()
                .flatten()
                .any(|p| *p == IntPoint::new(20, 14))
        );
        assert_eq!(before.len(), overlay.segments.len());
        for (a, b) in before.iter().zip(&overlay.segments) {
            assert_eq!(a.x_segment.a, b.x_segment.a);
            assert_eq!(a.x_segment.b, b.x_segment.b);
        }
        assert!(control.operations() >= before.len());
    }

    /// Keeps optional refinement within exact float-integer, source range and cumulative work bounds.
    /// # Panics
    /// Panics if a nonrepresentable lift publishes geometry or cancellation/work exhaustion is hidden.
    #[test]
    fn refined_integer_guards_and_cancellation_keep_authority() {
        let huge = 1_i64 << 51;
        let outside = vec![
            IntPoint::new(huge + 1, 0),
            IntPoint::new(huge + 5, 10),
            IntPoint::new(huge + 1, 10),
        ];
        let overlay = Overlay::from_subj(&outside);
        let mut control = CheckedOverlayControl::new(100_000, &|| false);
        assert!(
            overlay
                .overlay_checked_refined(OverlayRule::Subject, FillRule::NonZero, &mut control)
                .unwrap()
                .is_none()
        );
        assert!(control.operations() > 0);
        let small = Overlay::from_subj(&vec![
            IntPoint::new(0, 0),
            IntPoint::new(10, 10),
            IntPoint::new(0, 10),
        ]);
        assert_eq!(
            small
                .overlay_checked_refined(
                    OverlayRule::Subject,
                    FillRule::NonZero,
                    &mut CheckedOverlayControl::new(0, &|| false)
                )
                .err(),
            Some(CheckedOverlayError::WorkLimit)
        );
        assert_eq!(
            small
                .overlay_checked_refined(
                    OverlayRule::Subject,
                    FillRule::NonZero,
                    &mut CheckedOverlayControl::new(100_000, &|| true)
                )
                .err(),
            Some(CheckedOverlayError::Cancelled)
        );
    }

    /// Proves multiple split triangles use current remainder endpoints and never signed cancellation.
    ///
    /// # Panics
    /// Panics if a later mark is certified against the original edge or opposite errors cancel.
    #[test]
    fn sequential_triangles_are_exact_and_absolute() {
        let mut control = CheckedOverlayControl::new(100, &|| false);
        let a = IntPoint::new(0, 0);
        let b = IntPoint::new(10, 0);
        let first = IntPoint::new(3, 2);
        let second = IntPoint::new(7, -1);
        assert!(control.triangle(a, first, b));
        assert!(control.triangle(first, second, b));
        assert_eq!(control.doubled_area, 33);
    }

    /// Proves hole direction, minimal sliver triangles, and unchanged retained paint are absolute.
    ///
    /// # Panics
    /// Panics if reversing an edge changes its certificate or a unit sliver disappears.
    #[test]
    fn reversed_holes_and_unit_slivers_keep_absolute_certificate() {
        let mut control = CheckedOverlayControl::new(100, &|| false);
        let a = IntPoint::new(0, 0);
        let b = IntPoint::new(1, 0);
        let p = IntPoint::new(0, 1);
        assert!(control.triangle(a, p, b));
        assert!(control.triangle(b, p, a));
        assert!(control.triangle(
            IntPoint::new(0, 0),
            IntPoint::new(100, 0),
            IntPoint::new(1000, 0)
        ));
        assert_eq!(control.doubled_area, 2);
    }

    /// Proves unresolved signed determinant or cumulative unsigned overflow fails explicitly.
    ///
    /// # Panics
    /// Panics if overflow wraps or permits uncertified geometry.
    #[test]
    fn certificate_overflow_fails_without_wrapping() {
        let mut control = CheckedOverlayControl::new(100, &|| false);
        assert!(!control.triangle(
            IntPoint::new(i64::MIN, i64::MIN),
            IntPoint::new(i64::MAX, i64::MIN),
            IntPoint::new(i64::MAX, i64::MAX)
        ));
        assert_eq!(
            control.error,
            Some(CheckedOverlayError::CertificateOverflow)
        );
        let mut control = CheckedOverlayControl::new(100, &|| false);
        control.doubled_area = u128::MAX;
        assert!(!control.triangle(
            IntPoint::new(0, 0),
            IntPoint::new(0, 1),
            IntPoint::new(1, 0)
        ));
        assert_eq!(
            control.error,
            Some(CheckedOverlayError::CertificateOverflow)
        );
    }
    /// Proves a two-unit reservation polls before crossing the 1024-unit cancellation interval.
    ///
    /// # Panics
    /// Panics if a triangle can push scalar work to 1025 between cancellation polls.
    #[test]
    fn reservation_crossing_polls_before_mutation() {
        let polls = core::cell::Cell::new(0);
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() >= 2
        };
        let mut control = CheckedOverlayControl::new(10_000, &cancelled);
        assert!(control.charge_work(1023));
        assert!(!control.charge_work(2));
        assert_eq!(control.operations(), 1023);
        assert_eq!(control.error, Some(CheckedOverlayError::Cancelled));
    }
}
