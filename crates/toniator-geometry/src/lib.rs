#![forbid(unsafe_code)]

//! Reusable finite two-dimensional primitives for headless pattern families.

use serde::Serialize;
use toniator_domain::GuideDimensionId;

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
    pub contributors: [GuideInstanceId; 2],
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
