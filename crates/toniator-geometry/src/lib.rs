#![forbid(unsafe_code)]

//! Reusable finite two-dimensional primitives for headless pattern families.

use std::{collections::BTreeSet, error::Error, fmt};

use serde::Serialize;
use toniator_domain::{GuideDimensionId, PatternMechanismId};

mod curves;
mod guides;

pub use curves::{
    CubicBezierSegment, CurveError, CurvePath, CurveSegment, IntersectionKind, LineSegment,
    PathArcLength, PathClosure, PathIntersection, PathLocation, SegmentIntersection,
};
pub use guides::{
    GuideCoveragePlan, GuideDimensionCoverage, GuidePathInstance, GuidePathLocationProvenance,
    GuidePathSet, construct_circular_arc, resolve_guide_prototype,
};

/// A finite document- or pattern-local point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn dot(self, vector: Vector2) -> f64 {
        self.x.mul_add(vector.x, self.y * vector.y)
    }
}

/// A finite two-dimensional vector.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn perpendicular(self) -> Self {
        Self::new(-self.y, self.x)
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x.mul_add(other.x, self.y * other.y)
    }

    pub fn scale(self, amount: f64) -> Self {
        Self::new(self.x * amount, self.y * amount)
    }
}

/// An axis-aligned finite bounds rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Bounds {
    pub min: Point2,
    pub max: Point2,
}

impl Bounds {
    pub fn new(min: Point2, max: Point2) -> Option<Self> {
        (min.is_finite() && max.is_finite() && min.x <= max.x && min.y <= max.y)
            .then_some(Self { min, max })
    }

    pub fn from_points(points: impl IntoIterator<Item = Point2>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        if !first.is_finite() {
            return None;
        }
        let mut min = first;
        let mut max = first;
        for point in points {
            if !point.is_finite() {
                return None;
            }
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        Self::new(min, max)
    }

    pub fn expanded(self, amount: f64) -> Option<Self> {
        (amount.is_finite() && amount >= 0.0).then(|| {
            Self::new(
                Point2::new(self.min.x - amount, self.min.y - amount),
                Point2::new(self.max.x + amount, self.max.y + amount),
            )
            .expect("finite expansion preserves bounds")
        })
    }

    pub fn corners(self) -> [Point2; 4] {
        [
            self.min,
            Point2::new(self.min.x, self.max.y),
            self.max,
            Point2::new(self.max.x, self.min.y),
        ]
    }

    pub fn contains(self, point: Point2) -> bool {
        (self.min.x..=self.max.x).contains(&point.x) && (self.min.y..=self.max.y).contains(&point.y)
    }
}

/// A rotation about a canvas center followed by a document-axis translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineTransform2D {
    center: Point2,
    cos: f64,
    sin: f64,
    translation: Vector2,
}

impl AffineTransform2D {
    pub fn rotate_about_then_translate(
        center: Point2,
        degrees: f64,
        translation: Vector2,
    ) -> Option<Self> {
        if !center.is_finite()
            || !degrees.is_finite()
            || !translation.x.is_finite()
            || !translation.y.is_finite()
        {
            return None;
        }
        let radians = degrees.to_radians();
        Some(Self {
            center,
            cos: radians.cos(),
            sin: radians.sin(),
            translation,
        })
    }

    pub fn apply_point(self, point: Point2) -> Point2 {
        let centered = Vector2::new(point.x - self.center.x, point.y - self.center.y);
        Point2::new(
            self.center.x + self.cos * centered.x - self.sin * centered.y + self.translation.x,
            self.center.y + self.sin * centered.x + self.cos * centered.y + self.translation.y,
        )
    }

    pub fn inverse_point(self, point: Point2) -> Point2 {
        let translated = Vector2::new(
            point.x - self.translation.x - self.center.x,
            point.y - self.translation.y - self.center.y,
        );
        Point2::new(
            self.center.x + self.cos * translated.x + self.sin * translated.y,
            self.center.y - self.sin * translated.x + self.cos * translated.y,
        )
    }

    pub fn apply_vector(self, vector: Vector2) -> Vector2 {
        Vector2::new(
            self.cos * vector.x - self.sin * vector.y,
            self.sin * vector.x + self.cos * vector.y,
        )
    }

    pub fn inverse_bounds(self, bounds: Bounds) -> Option<Bounds> {
        Bounds::from_points(
            bounds
                .corners()
                .into_iter()
                .map(|point| self.inverse_point(point)),
        )
    }
}

/// Stable identity of one infinite straight-guide instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct GuideInstanceId {
    pub dimension_id: u64,
    pub index: i64,
}

impl GuideInstanceId {
    pub const fn new(dimension_id: GuideDimensionId, index: i64) -> Self {
        Self {
            dimension_id: dimension_id.0,
            index,
        }
    }
}

/// A finite representation of an infinite guide, extended across a local domain.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StraightGuide {
    pub id: GuideInstanceId,
    pub normal: Vector2,
    pub tangent: Vector2,
    pub offset: f64,
    /// Stable transformed local tangent origin.  Finite start/end coverage is
    /// presentation only; along-guide sequences are anchored here.
    #[serde(skip)]
    pub anchor: Point2,
    pub start: Point2,
    pub end: Point2,
}

/// Stable identity of a site generated from exactly two guide instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SiteId {
    pub first_dimension_id: u64,
    pub first_index: i64,
    pub second_dimension_id: u64,
    pub second_index: i64,
}

/// Required provenance for a straight-guide intersection site.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GuideIntersectionProvenance {
    pub contributors: Vec<GuideInstanceId>,
}

/// A scope marker based only on the final canvas, never topology construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteScope {
    Canvas,
    Guard,
}

/// A deterministic site generated from an intersection of two straight guides.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IntersectionSite {
    pub id: SiteId,
    pub position: Point2,
    pub scope: SiteScope,
    pub provenance: GuideIntersectionProvenance,
}

/// Stable, evaluator-emission identity for one reusable family site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FamilySiteId {
    pub mechanism_id: PatternMechanismId,
    pub ordinal: usize,
}

/// Truthful structural origin retained for a reusable family site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilySiteProvenance {
    GuideIntersection {
        contributors: Vec<GuideInstanceId>,
    },
    AlongGuide {
        guide_id: GuideInstanceId,
        guide_order: usize,
        sequence: i64,
        absolute_arc_position_bits: u64,
        local_arc_position_bits: u64,
    },
    Random {
        candidate_ordinal: usize,
        accepted_ordinal: usize,
        exclusion_neighbor_ordinal: Option<usize>,
    },
    /// A site from intersecting finite Stage 20D curve-guide instances.
    CurveGuideIntersection {
        contributors: Vec<GuidePathLocationProvenance>,
    },
    /// A site sampled by arc length along a finite Stage 20D curve guide.
    CurveAlongGuide {
        location: GuidePathLocationProvenance,
        guide_order: usize,
        sequence: i64,
        absolute_arc_position_bits: u64,
        local_arc_position_bits: u64,
    },
}

/// One deterministic evaluator-emitted site before topology or realization.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilySite {
    pub id: FamilySiteId,
    pub position: Point2,
    pub scope: SiteScope,
    pub provenance: FamilySiteProvenance,
}

/// Stable validation failure for a reusable family-site set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilySiteError {
    path: &'static str,
    message: &'static str,
}

impl FamilySiteError {
    /// Creates one stable validation failure without exposing mutable site-set state.
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    /// Returns the stable validation path owned by the family-site contract.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the stable human-readable validation message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for FamilySiteError {
    /// Formats the stable validation failure without adding evaluator context.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for FamilySiteError {}

/// Ordered, validated derived sites for exactly one structural product.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilySiteSet {
    family_fingerprint: String,
    product_mechanism_id: PatternMechanismId,
    sites: Vec<FamilySite>,
}

impl FamilySiteSet {
    /// Builds the sole reusable derived-site authority for one family result.
    ///
    /// The caller supplies deterministic evaluator order; this constructor never
    /// sorts or renumbers sites. It validates the supplied family identity before
    /// each emitted site and retains provenance contributor order verbatim.
    ///
    /// # Errors
    ///
    /// Returns a stable `FamilySiteError` when identity, ordering, finite
    /// geometry, or truthful provenance violates the family-site contract.
    pub fn new(
        family_fingerprint: String,
        product_mechanism_id: PatternMechanismId,
        sites: Vec<FamilySite>,
    ) -> Result<Self, FamilySiteError> {
        if family_fingerprint.is_empty() {
            return Err(FamilySiteError::new(
                "family_sites.family_fingerprint",
                "family fingerprint must be nonempty",
            ));
        }
        if product_mechanism_id.0 == 0 {
            return Err(FamilySiteError::new(
                "family_sites.product_mechanism_id",
                "product mechanism ID must be nonzero",
            ));
        }
        let mut ids = BTreeSet::new();
        for site in &sites {
            if !ids.insert(site.id) {
                return Err(FamilySiteError::new(
                    "family_sites.id.duplicate",
                    "family site IDs must be unique",
                ));
            }
        }
        for (ordinal, site) in sites.iter().enumerate() {
            if site.id.mechanism_id != product_mechanism_id {
                return Err(FamilySiteError::new(
                    "family_sites.id.mechanism_id_mismatch",
                    "family site mechanism ID must match the product mechanism ID",
                ));
            }
            if site.id.ordinal != ordinal {
                return Err(FamilySiteError::new(
                    "family_sites.id.ordinal",
                    "family site ordinal must equal its emission position",
                ));
            }
            if !site.position.is_finite() {
                return Err(FamilySiteError::new(
                    "family_sites.position",
                    "family site position must be finite",
                ));
            }
            Self::validate_provenance(&site.provenance)?;
        }
        Ok(Self {
            family_fingerprint,
            product_mechanism_id,
            sites,
        })
    }

    /// Validates provenance facts without inferring scope or changing authored order.
    ///
    /// # Errors
    ///
    /// Returns the stable provenance path for the first malformed fact.
    fn validate_provenance(provenance: &FamilySiteProvenance) -> Result<(), FamilySiteError> {
        match provenance {
            FamilySiteProvenance::GuideIntersection { contributors } => {
                let unique = contributors.iter().copied().collect::<BTreeSet<_>>();
                if contributors.len() < 2
                    || unique.len() != contributors.len()
                    || contributors.iter().any(|id| id.dimension_id == 0)
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.guide_intersection.contributors",
                        "guide intersections require at least two unique nonzero contributors",
                    ));
                }
            }
            FamilySiteProvenance::AlongGuide {
                guide_id,
                absolute_arc_position_bits,
                local_arc_position_bits,
                ..
            } => {
                if guide_id.dimension_id == 0 {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.along_guide.guide_id",
                        "along-guide provenance requires a nonzero guide dimension",
                    ));
                }
                if !f64::from_bits(*absolute_arc_position_bits).is_finite()
                    || !f64::from_bits(*local_arc_position_bits).is_finite()
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.along_guide.arc_position",
                        "along-guide arc positions must decode to finite values",
                    ));
                }
            }
            FamilySiteProvenance::CurveGuideIntersection { contributors } => {
                let unique = contributors.iter().collect::<BTreeSet<_>>();
                if contributors.len() < 2
                    || unique.len() != contributors.len()
                    || contributors.iter().any(|location| {
                        location.guide_id.dimension_id == 0
                            || !f64::from_bits(location.parameter_bits).is_finite()
                    })
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.curve_guide_intersection.contributors",
                        "curve guide intersections require at least two unique finite locations",
                    ));
                }
            }
            FamilySiteProvenance::CurveAlongGuide {
                location,
                absolute_arc_position_bits,
                local_arc_position_bits,
                ..
            } => {
                if location.guide_id.dimension_id == 0
                    || !f64::from_bits(location.parameter_bits).is_finite()
                    || !f64::from_bits(*absolute_arc_position_bits).is_finite()
                    || !f64::from_bits(*local_arc_position_bits).is_finite()
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.curve_along_guide",
                        "curve along-guide provenance requires finite nonzero guide locations",
                    ));
                }
            }
            FamilySiteProvenance::Random {
                candidate_ordinal,
                accepted_ordinal,
                exclusion_neighbor_ordinal,
            } => {
                if accepted_ordinal > candidate_ordinal
                    || exclusion_neighbor_ordinal
                        .is_some_and(|neighbor| neighbor >= *accepted_ordinal)
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.random.ordinals",
                        "random provenance ordinals are inconsistent",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns the evaluator-derived family identity without creating another cache key.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Returns the one product mechanism that owns every contained site ID.
    pub fn product_mechanism_id(&self) -> PatternMechanismId {
        self.product_mechanism_id
    }

    /// Returns sites in original deterministic evaluator-emission order.
    pub fn sites(&self) -> &[FamilySite] {
        &self.sites
    }

    /// Iterates sites in original deterministic evaluator-emission order.
    pub fn iter(&self) -> impl Iterator<Item = &FamilySite> {
        self.sites.iter()
    }

    /// Returns the bounded number of emitted sites.
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    /// Reports whether evaluation emitted no sites without inferring a failure.
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
}

/// A renderer-independent circular primitive realized from a family site.
///
/// The retained source identity, scope, and provenance make it possible to
/// diagnose realization without allowing realization to regenerate sites.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CanonicalCircleMark {
    pub source_site_id: SiteId,
    pub center: Point2,
    pub radius: f64,
    pub scope: SiteScope,
    pub provenance: GuideIntersectionProvenance,
}

impl CanonicalCircleMark {
    pub fn new(
        source_site_id: SiteId,
        center: Point2,
        radius: f64,
        scope: SiteScope,
        provenance: GuideIntersectionProvenance,
    ) -> Option<Self> {
        (center.is_finite() && radius.is_finite() && radius >= 0.0).then_some(Self {
            source_site_id,
            center,
            radius,
            scope,
            provenance,
        })
    }
}

/// Project a collection of points onto a unit direction.
pub fn projection_range(
    points: impl IntoIterator<Item = Point2>,
    direction: Vector2,
) -> Option<(f64, f64)> {
    let mut values = points.into_iter().map(|point| point.dot(direction));
    let first = values.next()?;
    if !first.is_finite() {
        return None;
    }
    let mut min = first;
    let mut max = first;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        min = min.min(value);
        max = max.max(value);
    }
    Some((min, max))
}
