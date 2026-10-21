//! Final sampled decisions over exact mapped f64 vertices, not canonical/area geometry.
//! Outward intervals enclose each operation. Uncertain orientations use robust1.2.0 only in
//! the conservative Shewchuk/Spade safe domain after one common exact power-of-two scaling.
use super::{Point2, RasterWork, RenderError};

/// Closed enclosure of exact real arithmetic on supplied binary64 values.
#[derive(Clone, Copy, Debug)]
pub(super) struct Interval {
    pub(super) low: f64,
    pub(super) high: f64,
}
impl Interval {
    /// Embeds one exact finite binary64 input without inventing a coordinate tolerance.
    pub(super) fn point(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self {
            low: value,
            high: value,
        })
    }
    /// Rounds each finite endpoint outward by one representable neighbor, including signed zero.
    /// Nonfinite enclosures are refused rather than clamped or treated as an exact sign.
    fn outward(low: f64, high: f64) -> Option<Self> {
        let low = low.next_down();
        let high = high.next_up();
        (low.is_finite() && high.is_finite() && low <= high).then_some(Self { low, high })
    }
    /// Encloses addition of all endpoint combinations under correctly rounded binary64 arithmetic.
    pub(super) fn add(self, other: Self) -> Option<Self> {
        Self::outward(self.low + other.low, self.high + other.high)
    }
    /// Encloses subtraction, retaining the original expression's operand order.
    pub(super) fn sub(self, other: Self) -> Option<Self> {
        Self::outward(self.low - other.high, self.high - other.low)
    }
    /// Encloses all four product endpoints; underflowed zero expands to both adjacent subnormals.
    pub(super) fn mul(self, other: Self) -> Option<Self> {
        let values = [
            self.low * other.low,
            self.low * other.high,
            self.high * other.low,
            self.high * other.high,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            return None;
        }
        Self::outward(
            values.into_iter().fold(f64::INFINITY, f64::min),
            values.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
    /// Encloses division only when the entire denominator interval excludes zero.
    pub(super) fn div(self, other: Self) -> Option<Self> {
        if other.low <= 0.0 && other.high >= 0.0 {
            return None;
        }
        let values = [
            self.low / other.low,
            self.low / other.high,
            self.high / other.low,
            self.high / other.high,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            return None;
        }
        Self::outward(
            values.into_iter().fold(f64::INFINITY, f64::min),
            values.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}

/// Refuses arithmetic whose sign/frame cannot be certified; this is not a document coordinate cap.
pub(super) fn uncertainty() -> RenderError {
    RenderError::new(
        "raster.sample.numeric",
        "sampled decision has no certified finite arithmetic frame",
    )
}

/// Encloses the original anchored-start determinant without reassociating its operations.
fn determinant(a: Point2, b: Point2, p: Point2) -> Option<Interval> {
    let ax = Interval::point(a.x)?;
    let ay = Interval::point(a.y)?;
    let left = Interval::point(b.x)?
        .sub(ax)?
        .mul(Interval::point(p.y)?.sub(ay)?)?;
    let right = Interval::point(p.x)?
        .sub(ax)?
        .mul(Interval::point(b.y)?.sub(ay)?)?;
    left.sub(right)
}

/// Returns floor(log2(abs(value))) for a finite nonzero exact binary64 coordinate.
/// # Panics
/// The private caller supplies finite nonzero input; zero would violate the subnormal bit invariant.
fn exponent(value: f64) -> i32 {
    let bits = value.abs().to_bits();
    let encoded = ((bits >> 52) & 0x7ff) as i32;
    if encoded != 0 {
        encoded - 1023
    } else {
        (63 - bits.leading_zeros()) as i32 - 1074
    }
}

/// Scales by exact positive powers of two in bounded chunks without losing any significand bits.
/// Upward chunks cannot overflow when their final safe-domain value is finite. Downward chunks
/// must remain normal: the final lower bound2^-142 proves every intermediate is normal too.
/// # Errors
/// Returns cancellation or uncertainty for an invalid/nonexact frame, never lossy coordinates.
fn scale(value: f64, shift: i32, work: &RasterWork<'_>) -> Result<f64, RenderError> {
    if !value.is_finite() {
        return Err(uncertainty());
    }
    if value == 0.0 {
        return Ok(value);
    }
    let mut value = value;
    let mut left = shift;
    while left != 0 {
        work.check()?;
        let step = left.clamp(-512, 512);
        let factor = 2.0_f64.powi(step);
        // This precondition proves exact exponent changes; a rounded roundtrip is not the proof.
        if exponent(value) + step > 1023 || (step < 0 && exponent(value) + step < -1022) {
            return Err(uncertainty());
        }
        let next = value * factor;
        if !next.is_finite() || next == 0.0 || (step < 0 && next.abs() < f64::MIN_POSITIVE) {
            return Err(uncertainty());
        }
        value = next;
        left -= step;
    }
    Ok(value)
}

/// Admits one common exact power-of-two frame for the ORIGINAL coordinate triple.
/// Every nonzero result lies in[2^-142,2^201], the conservative Spade/Shewchuk domain;
/// translating, rounding deltas, clamping and discarding tiny values are forbidden.
/// # Errors
/// Returns cancellation or uncertainty if no common proof-valid frame exists.
fn safe_frame(
    a: Point2,
    b: Point2,
    p: Point2,
    work: &RasterWork<'_>,
) -> Result<[Point2; 3], RenderError> {
    let mut lower = i32::MIN;
    let mut upper = i32::MAX;
    for value in [a.x, a.y, b.x, b.y, p.x, p.y] {
        work.check()?;
        if !value.is_finite() {
            return Err(uncertainty());
        }
        if value == 0.0 {
            continue;
        }
        let e = exponent(value);
        let bits = value.abs().to_bits();
        let is_power = if bits >> 52 == 0 {
            bits.is_power_of_two()
        } else {
            bits & ((1_u64 << 52) - 1) == 0
        };
        lower = lower.max(-142 - e);
        upper = upper.min(if is_power { 201 - e } else { 200 - e });
    }
    if lower > upper {
        return Err(uncertainty());
    }
    let shift = 0_i32.clamp(lower, upper);
    let mut result = [a, b, p];
    for point in &mut result {
        point.x = scale(point.x, shift, work)?;
        point.y = scale(point.y, shift, work)?;
        for value in [point.x, point.y] {
            if value != 0.0 && !(2.0_f64.powi(-142)..=2.0_f64.powi(201)).contains(&value.abs()) {
                return Err(uncertainty());
            }
        }
    }
    Ok(result)
}

/// Certifies an orientation sign by enclosure, falling back to mature exact adaptive arithmetic.
/// The sign concerns exact mapped flattened vertices, not error lost before this consumer boundary.
/// The request's Boolean/edge ledgers are neither charged nor reset by sampled queries.
/// # Errors
/// Returns cancellation or numeric uncertainty if the mature predicate has no valid common frame.
pub(super) fn orientation(
    a: Point2,
    b: Point2,
    p: Point2,
    work: &RasterWork<'_>,
) -> Result<i8, RenderError> {
    work.check()?;
    if let Some(bound) = determinant(a, b, p) {
        if bound.low > 0.0 {
            return Ok(1);
        }
        if bound.high < 0.0 {
            return Ok(-1);
        }
    }
    let [a, b, p] = safe_frame(a, b, p, work)?;
    work.check()?;
    let value = robust::orient2d(
        robust::Coord { x: a.x, y: a.y },
        robust::Coord { x: b.x, y: b.y },
        robust::Coord { x: p.x, y: p.y },
    );
    work.check()?;
    if !value.is_finite() {
        return Err(uncertainty());
    }
    Ok(if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    })
}

/// Encloses each active edge's exact crossing and the original nominal crossing evaluation.
/// # Errors
/// Returns numeric uncertainty for nonfinite operations or a denominator enclosing zero.
pub(super) fn crossing(a: Point2, b: Point2, y: f64) -> Result<Interval, RenderError> {
    let evaluate = || {
        let ax = Interval::point(a.x)?;
        let ay = Interval::point(a.y)?;
        let offset = Interval::point(b.x)?
            .sub(ax)?
            .mul(Interval::point(y)?.sub(ay)?)?
            .div(Interval::point(b.y)?.sub(ay)?)?;
        ax.add(offset)
    };
    evaluate().ok_or_else(uncertainty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RasterAntialiasing, RasterizationLimits};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Checks independent exact integer/dyadic arithmetic, signed zero, underflow and failed enclosures.
    /// # Panics
    /// Panics if any exact rational result lies outside its outward interval.
    #[test]
    fn outward_intervals_enclose_exact_dyadic_operations() {
        let point = |x| Interval::point(x).unwrap();
        let a = point(2.0_f64.powi(53));
        let sum = a.add(point(1.0)).unwrap();
        let exact = (1_i128 << 53) + 1;
        assert!((sum.low as i128) <= exact && (sum.high as i128) >= exact);
        let product = point((1_u64 << 27) as f64 + 1.0)
            .mul(point((1_u64 << 27) as f64 - 1.0))
            .unwrap();
        let exact = (1_i128 << 54) - 1;
        assert!((product.low as i128) <= exact && (product.high as i128) >= exact);
        let quotient = point(3.0).div(point(8.0)).unwrap();
        assert!(quotient.low <= 0.375 && quotient.high >= 0.375);
        let cancellation = point(1.0).sub(point(1.0)).unwrap();
        assert!(cancellation.low < 0.0 && cancellation.high > 0.0);
        let underflow = point(f64::from_bits(1)).mul(point(0.5)).unwrap();
        assert!(underflow.low <= 0.0 && underflow.high >= f64::from_bits(1));
        assert!(point(1.0).div(cancellation).is_none());
        assert!(point(f64::MAX).mul(point(2.0)).is_none());
        let negative_zero = point(-0.0).add(point(0.0)).unwrap();
        assert!(negative_zero.low < 0.0 && negative_zero.high > 0.0);
    }

    /// Proves exact common frame scaling, in-domain finite cancellation and strict refusal outside it.
    /// # Panics
    /// Panics if signs, exponent bounds or reversible original-coordinate scaling change.
    #[test]
    fn adaptive_orientation_admits_exact_common_frames_only() {
        let work = RasterWork::new(
            RasterizationLimits::new(100).unwrap(),
            RasterAntialiasing::Off,
            &|| false,
        );
        for r in [1.0e18, 1.0e100] {
            let a = Point2::new(r, r);
            let b = Point2::new(0.0, 0.0);
            let p = Point2::new(1.5, 0.5);
            assert_eq!(orientation(a, b, p, &work).unwrap(), 1);
            assert_eq!(orientation(b, a, p, &work).unwrap(), -1);
            assert_eq!(orientation(a, b, Point2::new(0.5, 1.5), &work).unwrap(), -1);
            assert_eq!(orientation(a, b, Point2::new(0.5, 0.5), &work).unwrap(), 0);
            let scaled = safe_frame(a, b, p, &work).unwrap();
            let ratio = scaled[0].x / a.x;
            assert!(ratio.is_finite() && ratio > 0.0);
            assert_eq!(scaled[2].x / ratio, p.x);
            assert_eq!(scaled[2].y / ratio, p.y);
        }
        assert!(
            safe_frame(
                Point2::new(1.0e200, 1.0e200),
                Point2::new(0.0, 0.0),
                Point2::new(1.5, 0.5),
                &work
            )
            .is_err()
        );
        assert!(
            safe_frame(
                Point2::new(2.0_f64.powi(201).next_up(), 0.0),
                Point2::new(0.0, 2.0_f64.powi(-142)),
                Point2::new(0.0, 0.0),
                &work
            )
            .is_err()
        );
        let tiny = f64::from_bits(1);
        assert_eq!(
            orientation(
                Point2::new(0.0, 0.0),
                Point2::new(tiny, 0.0),
                Point2::new(0.0, tiny),
                &work
            )
            .unwrap(),
            1
        );
        assert!(
            safe_frame(
                Point2::new(tiny, 1.0),
                Point2::new(0.0, 0.0),
                Point2::new(0.0, 0.0),
                &work
            )
            .is_err()
        );
        for value in [2.0_f64.powi(-142), 2.0_f64.powi(201)] {
            let frame = safe_frame(
                Point2::new(value, 0.0),
                Point2::new(0.0, value),
                Point2::new(0.0, 0.0),
                &work,
            )
            .unwrap();
            assert_eq!(frame[0].x, value);
        }
    }

    /// Polls cancellation inside the mature fallback and preserves native edge/Boolean allowances.
    /// # Panics
    /// Panics if a sampled query consumes an AA allowance or ignores cancellation after entry.
    #[test]
    fn fallback_cancellation_and_allowances_remain_request_local() {
        let checks = AtomicUsize::new(0);
        let cancel = || checks.fetch_add(1, Ordering::SeqCst) >= 3;
        let limits = RasterizationLimits::new(100)
            .unwrap()
            .with_max_boolean_operations(0);
        let work = RasterWork::new(limits, RasterAntialiasing::Off, &cancel);
        let error = orientation(
            Point2::new(1e100, 1e100),
            Point2::new(0.0, 0.0),
            Point2::new(0.5, 0.5),
            &work,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert!(checks.load(Ordering::SeqCst) > 3);
        assert_eq!(work.remaining_edges, 100);
        assert_eq!(work.remaining_boolean_operations, 0);
        let work = RasterWork::new(limits, RasterAntialiasing::Off, &|| false);
        assert_eq!(
            orientation(
                Point2::new(1e100, 1e100),
                Point2::new(0.0, 0.0),
                Point2::new(1.5, 0.5),
                &work
            )
            .unwrap(),
            1
        );
        assert_eq!(work.remaining_edges, 100);
        assert_eq!(work.remaining_boolean_operations, 0);
    }
}
