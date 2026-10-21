//! Source-free authored mark transforms retained when certified exterior materialization is omitted.
use crate::curves::{check_curve_cancelled, reserve_curve};
use crate::{
    Bounds, CanonicalFillRule, CanonicalPathMark, ConstructionContext, ConstructionCounter,
    CurveError, CurvePath, CurveSegment, FamilySiteId, FamilySiteProvenance,
    MaterializationCoverage, PathClosure, Point2, SiteScope,
};
use std::sync::Arc;
/// Immutable original prototype shared by resolved decisions; no artwork or document authority is retained.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedMarkPrototype {
    path: Arc<CurvePath>,
    anchor: Point2,
    fingerprint: u64,
}
/// A semantically surviving authored mark whose exact materialization is certified outside its canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct DeferredPathMark {
    pub prototype: Arc<PreparedMarkPrototype>,
    pub scale: f64,
    pub orientation_degrees: f64,
    pub destination: Point2,
    pub source_site_id: FamilySiteId,
    pub scope: SiteScope,
    pub provenance: FamilySiteProvenance,
    pub fill_rule: CanonicalFillRule,
    pub bounds: Bounds,
    pub coverage: MaterializationCoverage,
}
impl DeferredPathMark {
    /// Validates public deferred metadata without constructing or sampling the omitted path.
    pub fn is_valid(&self) -> bool {
        self.scale.is_finite()
            && self.scale >= 0.0
            && self.orientation_degrees.is_finite()
            && self.destination.is_finite()
            && self.prototype.anchor.is_finite()
            && self.prototype.path.closure() == PathClosure::Closed
            && self.bounds.min.is_finite()
            && self.bounds.max.is_finite()
            && self.bounds.min.x <= self.bounds.max.x
            && self.bounds.min.y <= self.bounds.max.y
            && matches!(self.coverage,MaterializationCoverage::Canvas{canvas,retained,contract} if canvas.min.is_finite()&&canvas.max.is_finite()&&retained.min.is_finite()&&retained.max.is_finite()&&canvas.min.x<=canvas.max.x&&canvas.min.y<=canvas.max.y&&retained.min.x<=canvas.min.x&&retained.min.y<=canvas.min.y&&retained.max.x>=canvas.max.x&&retained.max.y>=canvas.max.y&&!contract.is_empty())
            && self.coverage.exterior(self.bounds) != 0
    }

    /// Restores the original complete closed mark using only its frozen resolved transform.
    /// # Errors
    /// Propagates cancellation, finite-coordinate, addressability or actual reserve failures.
    pub fn materialize(
        &self,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CanonicalPathMark, CurveError> {
        ConstructionContext::current().count(ConstructionCounter::FullRematerializations, 1);
        CanonicalPathMark::new(
            self.source_site_id,
            transform_prepared_mark(
                &self.prototype.path,
                self.prototype.anchor,
                self.scale,
                self.orientation_degrees,
                self.destination,
                cancelled,
            )?,
            self.scope,
            self.provenance.try_clone_cancellable(cancelled)?,
            self.fill_rule,
        )
    }
}
/// Applies the original scale-before-rotation arithmetic to one prototype point.
/// # Errors
/// Rejects nonfinite transformed coordinates rather than certifying invalid omitted geometry.
pub fn prepared_mark_point(
    point: Point2,
    anchor: Point2,
    scale: f64,
    orientation: f64,
    destination: Point2,
) -> Result<Point2, CurveError> {
    let (sin, cos) = orientation.to_radians().sin_cos();
    let x = (point.x - anchor.x) * scale;
    let y = (point.y - anchor.y) * scale;
    let p = Point2::new(
        destination.x + cos * x - sin * y,
        destination.y + sin * x + cos * y,
    );
    if p.is_finite() {
        Ok(p)
    } else {
        Err(CurveError::new(
            "realization.mark.transform",
            "transformed closed-shape coordinates must remain finite",
        ))
    }
}
/// Constructs the original exact closed shape through fallible storage, without source sampling.
/// # Errors
/// Returns original finite/closure diagnostics, cancellation or actual allocation failure atomically.
pub fn transform_prepared_mark(
    path: &CurvePath,
    anchor: Point2,
    scale: f64,
    orientation: f64,
    destination: Point2,
    cancelled: &dyn Fn() -> bool,
) -> Result<CurvePath, CurveError> {
    if !scale.is_finite() || scale < 0.0 || !orientation.is_finite() || !destination.is_finite() {
        return Err(CurveError::new(
            "realization.mark.transform",
            "closed-shape scale and destination must be finite",
        ));
    }
    let context = ConstructionContext::current();
    let mut segments = Vec::new();
    reserve_curve(&mut segments, path.segments().len())?;
    context.count(
        ConstructionCounter::MarkReservedBytes,
        segments
            .capacity()
            .saturating_mul(std::mem::size_of::<CurveSegment>()),
    );
    let (sin, cos) = orientation.to_radians().sin_cos();
    let map = |point: Point2| -> Result<Point2, CurveError> {
        let x = (point.x - anchor.x) * scale;
        let y = (point.y - anchor.y) * scale;
        let p = Point2::new(
            destination.x + cos * x - sin * y,
            destination.y + sin * x + cos * y,
        );
        if p.is_finite() {
            Ok(p)
        } else {
            Err(CurveError::new(
                "realization.mark.transform",
                "transformed closed-shape coordinates must remain finite",
            ))
        }
    };
    for segment in path.segments() {
        check_curve_cancelled(cancelled)?;
        let transformed = match segment {
            CurveSegment::Line(v) => {
                CurveSegment::Line(crate::LineSegment::new(map(v.start())?, map(v.end())?)?)
            }
            CurveSegment::CubicBezier(v) => {
                CurveSegment::CubicBezier(crate::CubicBezierSegment::new(
                    map(v.start())?,
                    map(v.control_1())?,
                    map(v.control_2())?,
                    map(v.end())?,
                )?)
            }
        };
        context.count(ConstructionCounter::MarkConstructors, 1);
        segments.push(transformed);
    }
    CurvePath::new(segments, PathClosure::Closed)
}
impl PreparedMarkPrototype {
    /// Borrows the frozen original prototype; its identity cannot become stale through public mutation.
    pub fn path(&self) -> &CurvePath {
        &self.path
    }
    /// Returns the original exact transform anchor without deriving it from restricted geometry.
    pub const fn anchor(&self) -> Point2 {
        self.anchor
    }
    /// Returns the once-computed ordered path identity under the existing FNV fingerprint authority.
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Shares one immutable prototype and its exact-bit identity across all deferred decisions.
    pub fn new(path: CurvePath, anchor: Point2) -> Self {
        Self::new_cancellable(path, anchor, &|| false)
            .expect("uncancelled immutable prototype fingerprint")
    }
    /// Shares one frozen prototype after cancellable exact typed identity computation.
    /// # Errors
    /// Returns cancellation before publishing the immutable prepared prototype.
    pub fn new_cancellable(
        path: CurvePath,
        anchor: Point2,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurveError> {
        ConstructionContext::current().count(
            ConstructionCounter::PrototypeBytes,
            path.segments()
                .len()
                .saturating_mul(std::mem::size_of::<CurveSegment>())
                .saturating_add(std::mem::size_of::<Self>()),
        );
        let mut fingerprint = 0xcbf29ce484222325_u64;
        let mut add = |bytes: &[u8]| {
            for byte in bytes {
                fingerprint ^= u64::from(*byte);
                fingerprint = fingerprint.wrapping_mul(0x100000001b3);
            }
        };
        add(&anchor.x.to_bits().to_le_bytes());
        add(&anchor.y.to_bits().to_le_bytes());
        add(&(path.segments().len() as u64).to_le_bytes());
        add(&[match path.closure() {
            PathClosure::Open => 1,
            PathClosure::Closed => 2,
        }]);
        for segment in path.segments() {
            check_curve_cancelled(cancelled)?;
            match segment {
                CurveSegment::Line(line) => {
                    add(&[1]);
                    for point in [line.start(), line.end()] {
                        add(&point.x.to_bits().to_le_bytes());
                        add(&point.y.to_bits().to_le_bytes());
                    }
                }
                CurveSegment::CubicBezier(curve) => {
                    add(&[2]);
                    for point in [
                        curve.start(),
                        curve.control_1(),
                        curve.control_2(),
                        curve.end(),
                    ] {
                        add(&point.x.to_bits().to_le_bytes());
                        add(&point.y.to_bits().to_le_bytes());
                    }
                }
            }
        }
        Ok(Self {
            path: Arc::new(path),
            anchor,
            fingerprint,
        })
    }
}
/// Computes actual original transformed control/endpoint hulls without constructing path segments.
/// A caller retains extrema-bearing shapes so current raster normalization remains unchanged.
/// # Errors
/// Returns cancellation or original finite-transform diagnostics before a mark can be omitted.
pub fn prepared_mark_hulls(
    path: &CurvePath,
    anchor: Point2,
    scale: f64,
    orientation: f64,
    destination: Point2,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Bounds, Bounds), CurveError> {
    if !scale.is_finite()
        || scale < 0.0
        || !orientation.is_finite()
        || !destination.is_finite()
        || !anchor.is_finite()
    {
        return Err(CurveError::new(
            "realization.mark.transform",
            "closed-shape scale and destination must be finite",
        ));
    }
    let (sin, cos) = orientation.to_radians().sin_cos();
    let map = |point: Point2| -> Result<Point2, CurveError> {
        let x = (point.x - anchor.x) * scale;
        let y = (point.y - anchor.y) * scale;
        let p = Point2::new(
            destination.x + cos * x - sin * y,
            destination.y + sin * x + cos * y,
        );
        if p.is_finite() {
            Ok(p)
        } else {
            Err(CurveError::new(
                "realization.mark.transform",
                "transformed closed-shape coordinates must remain finite",
            ))
        }
    };
    let mut hull = None;
    let mut endpoints = None;
    for segment in path.segments() {
        check_curve_cancelled(cancelled)?;
        for point in [segment.start(), segment.end()] {
            let p = map(point)?;
            extend(&mut hull, p);
            extend(&mut endpoints, p);
        }
        if let CurveSegment::CubicBezier(v) = segment {
            for point in [v.control_1(), v.control_2()] {
                extend(&mut hull, map(point)?);
            }
        }
    }
    let mut hull = hull.ok_or(CurveError::new(
        "realization.mark.transform",
        "closed-shape prototype has no finite segments",
    ))?;
    hull.min.x = hull.min.x.next_down();
    hull.min.y = hull.min.y.next_down();
    hull.max.x = hull.max.x.next_up();
    hull.max.y = hull.max.y.next_up();
    Ok((hull, endpoints.expect("prototype has endpoints")))
}
/// Extends an exact endpoint envelope without allocation or replacing original floating arithmetic.
fn extend(bounds: &mut Option<Bounds>, point: Point2) {
    *bounds = Some(match *bounds {
        None => Bounds {
            min: point,
            max: point,
        },
        Some(b) => Bounds {
            min: Point2::new(b.min.x.min(point.x), b.min.y.min(point.y)),
            max: Point2::new(b.max.x.max(point.x), b.max.y.max(point.y)),
        },
    });
}

impl FamilySiteProvenance {
    /// Copies only immutable provenance, checking its variable contributor storage and cancellation.
    /// # Errors
    /// Returns cancellation or an actual contributor allocation failure before publication.
    pub fn try_clone_cancellable(&self, cancelled: &dyn Fn() -> bool) -> Result<Self, CurveError> {
        check_curve_cancelled(cancelled)?;
        match self {
            Self::GuideIntersection { contributors } => {
                let mut copy = Vec::new();
                reserve_curve(&mut copy, contributors.len())?;
                for contributor in contributors {
                    check_curve_cancelled(cancelled)?;
                    copy.push(*contributor);
                }
                Ok(Self::GuideIntersection { contributors: copy })
            }
            Self::CurveGuideIntersection { contributors } => {
                let mut copy = Vec::new();
                reserve_curve(&mut copy, contributors.len())?;
                for contributor in contributors {
                    check_curve_cancelled(cancelled)?;
                    copy.push(*contributor);
                }
                Ok(Self::CurveGuideIntersection { contributors: copy })
            }
            _ => Ok(self.clone()),
        }
    }
}
