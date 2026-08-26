#![forbid(unsafe_code)]

//! Reusable finite two-dimensional primitives for headless pattern families.

use std::{collections::BTreeSet, error::Error, fmt};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use toniator_domain::{GuideDimensionId, PatternMechanismId};

mod canonical_regions;
mod connection_paths;
mod curves;
mod guide_faces;
mod guides;
mod maze_walls;
mod outlines;
mod path_offsets;
mod planar_arrangement;
mod region_treatment;
mod site_adjacency;
mod voronoi_regions;

pub use canonical_regions::{
    CANONICAL_REGION_CONTRACT_ID, CanonicalRegion, CanonicalRegionDiagnostics,
    CanonicalRegionError, CanonicalRegionId, CanonicalRegionLimits, CanonicalRegionProposal,
    CanonicalRegionSet, CanonicalRegionSourceGroup, CanonicalRegionSourceId,
    DEFAULT_MAX_REGION_INSPECTIONS, DEFAULT_MAX_REGION_SEGMENTS, DEFAULT_MAX_REGION_SOURCE_GROUPS,
    DEFAULT_MAX_REGIONS, TaggedCanonicalRegionSourceGroup, build_canonical_regions,
    build_canonical_regions_cancellable, build_tagged_canonical_regions_cancellable,
};
pub use connection_paths::{
    CONNECTION_NEAREST_SELECTION_CONTRACT_ID, CONNECTION_PATH_CONTRACT_ID,
    CONNECTION_PRIM_SELECTION_CONTRACT_ID, CONNECTION_RANDOM_SELECTION_CONTRACT_ID,
    CONNECTION_TRAIL_CONTRACT_ID, ConnectionPath, ConnectionPathDiagnostics, ConnectionPathError,
    ConnectionPathId, ConnectionPathLimits, ConnectionPathSet, build_connection_paths_cancellable,
    connection_program_contract_id,
};
pub use curves::{
    CubicBezierSegment, CurveError, CurvePath, CurveSegment, IntersectionKind, LineSegment,
    PathArcLength, PathClosure, PathIntersection, PathLocation, SegmentIntersection,
    construct_parametric_curve_path_cancellable,
};
pub use guide_faces::{
    GUIDE_FACE_CONTRACT_ID, GuideFaceDiagnostics, GuideFaceError, GuideFaceLimits,
    GuideFaceRequest, GuideFaceResult, build_guide_faces_cancellable,
};
pub use guides::{
    GuideCoveragePlan, GuideDimensionCoverage, StructuralPathInstance, StructuralPathSet,
    construct_circular_arc, resolve_guide_prototype,
};
pub use maze_walls::{
    MAZE_WALL_CONTRACT_ID, MazeCell, MazeCellId, MazeDiagnostics, MazeDualEdge, MazeDualEdgeId,
    MazeError, MazeGuideAxis, MazeLimits, MazeOpening, MazeOpeningSide, MazeProgramResult,
    MazeSolution, MazeSourceSite, MazeVertexId, MazeWall, MazeWallId, MazeWallPath, MazeWallPathId,
    build_maze_walls_cancellable, build_maze_walls_from_sites_cancellable,
};
pub use outlines::{
    CanonicalFilledOutline, CanonicalOutlineContour, VariableWidthOutlineLimits,
    VariableWidthPathSample, build_variable_width_outline_cancellable,
};
pub(crate) use path_offsets::offset_path_with_work_region_round_cancellable;
pub use path_offsets::{
    MAX_PATH_OFFSET_CLEANUP_PAIRS, MAX_PATH_OFFSET_COMPONENTS, MAX_PATH_OFFSET_CUSP_ISOLATION_WORK,
    MAX_PATH_OFFSET_SEGMENTS, MAX_PATH_OFFSET_SUBDIVISION_DEPTH, OffsetPathComponent,
    PATH_OFFSET_ALGORITHM_CONTRACT_ID, PathOffsetCleanup, PathOffsetEndpointPolicy,
    PathOffsetLimits, PathOffsetRequest, PathOffsetResult, PathOffsetWork, offset_path_cancellable,
    offset_path_with_work_cancellable,
};
pub use region_treatment::{
    REGION_TREATMENT_CONTRACT_ID, RegionReference, RegionTreatment, RegionTreatmentError,
    RegionTreatmentLimits, RegionTreatmentProvenance, RegionTreatmentRequest,
    RegionTreatmentResult, treat_region_requests_cancellable, treat_regions_cancellable,
};
pub use site_adjacency::{
    SITE_ADJACENCY_CONTRACT_ID, SiteAdjacencyComponent, SiteAdjacencyEdge, SiteAdjacencyError,
    SiteAdjacencyGraph, SiteAdjacencyLimits, SiteAdjacencyNode, SiteAdjacencyPolicy,
    build_site_adjacency_cancellable,
};
pub use voronoi_regions::{
    VORONOI_REGION_CONTRACT_ID, VoronoiRegionDiagnostics, VoronoiRegionError, VoronoiRegionLimits,
    VoronoiRegionRequest, build_voronoi_regions_cancellable, voronoi_region_references,
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
    /// Distinguishes ordered cleanup fragments emitted for one signed repetition index.
    pub component_ordinal: u32,
}

impl GuideInstanceId {
    pub const fn new(dimension_id: GuideDimensionId, index: i64) -> Self {
        Self {
            dimension_id: dimension_id.0,
            index,
            component_ordinal: 0,
        }
    }

    /// Builds the complete identity for one ordered normal-offset cleanup component.
    pub const fn with_component(
        dimension_id: GuideDimensionId,
        index: i64,
        component_ordinal: u32,
    ) -> Self {
        Self {
            dimension_id: dimension_id.0,
            index,
            component_ordinal,
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
    /// Per-site longer cell diagonal captured before realization; it never changes site identity.
    pub nominal_cell_diameter: f64,
    pub scope: SiteScope,
    pub provenance: GuideIntersectionProvenance,
}

/// Stable, evaluator-emission identity for one reusable family site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FamilySiteId {
    pub mechanism_id: PatternMechanismId,
    pub ordinal: usize,
}

/// Names the structural source of one finite derived path without treating a
/// parametric curve as a guide dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSourceId {
    GuideDimension(GuideDimensionId),
    ParametricCurve(PatternMechanismId),
}

impl StructuralPathSourceId {
    /// Reports whether this typed structural source contains a nonzero authority ID.
    pub const fn is_valid(self) -> bool {
        match self {
            Self::GuideDimension(id) => id.0 != 0,
            Self::ParametricCurve(id) => id.0 != 0,
        }
    }
}

impl Serialize for StructuralPathSourceId {
    /// Serializes the typed source without requiring domain IDs to expose a serde contract.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StructuralPathSourceId", 2)?;
        match self {
            Self::GuideDimension(id) => {
                state.serialize_field("kind", "guide_dimension")?;
                state.serialize_field("id", &id.0)?;
            }
            Self::ParametricCurve(id) => {
                state.serialize_field("kind", "parametric_curve")?;
                state.serialize_field("id", &id.0)?;
            }
        }
        state.end()
    }
}

/// Stable ordered identity for one repeated structural path component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPathInstanceId {
    pub source: StructuralPathSourceId,
    pub repetition_index: i64,
    pub component_ordinal: u32,
}

impl Serialize for StructuralPathInstanceId {
    /// Serializes stable structural-path identity as source, repetition, and component fields.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StructuralPathInstanceId", 3)?;
        state.serialize_field("source", &self.source)?;
        state.serialize_field("repetition_index", &self.repetition_index)?;
        state.serialize_field("component_ordinal", &self.component_ordinal)?;
        state.end()
    }
}

impl StructuralPathInstanceId {
    /// Builds one ordered finite component identity for a guide-dimension source.
    pub const fn guide_dimension(
        dimension_id: GuideDimensionId,
        repetition_index: i64,
        component_ordinal: u32,
    ) -> Self {
        Self {
            source: StructuralPathSourceId::GuideDimension(dimension_id),
            repetition_index,
            component_ordinal,
        }
    }

    /// Builds one ordered finite component identity for a parametric source mechanism.
    pub const fn parametric_curve(
        mechanism_id: PatternMechanismId,
        repetition_index: i64,
        component_ordinal: u32,
    ) -> Self {
        Self {
            source: StructuralPathSourceId::ParametricCurve(mechanism_id),
            repetition_index,
            component_ordinal,
        }
    }

    /// Returns the retained guide identity only when this structural path originated from a guide dimension.
    pub const fn guide_instance(self) -> Option<GuideInstanceId> {
        match self.source {
            StructuralPathSourceId::GuideDimension(dimension_id) => Some(GuideInstanceId {
                dimension_id: dimension_id.0,
                index: self.repetition_index,
                component_ordinal: self.component_ordinal,
            }),
            StructuralPathSourceId::ParametricCurve(_) => None,
        }
    }
}

/// Exact segment-local provenance for a path-derived site or realization witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct StructuralPathLocationProvenance {
    pub path: StructuralPathInstanceId,
    pub segment_index: usize,
    pub parameter_bits: u64,
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
        contributors: Vec<StructuralPathLocationProvenance>,
    },
    /// A site sampled by arc length along a finite Stage 20D curve guide.
    CurveAlongGuide {
        location: StructuralPathLocationProvenance,
        guide_order: usize,
        sequence: i64,
        absolute_arc_position_bits: u64,
        local_arc_position_bits: u64,
    },
    /// A site sampled along an analytic parametric structural path.
    AlongParametricCurve {
        location: StructuralPathLocationProvenance,
        path_order: usize,
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
    pub nominal_cell_basis: NominalCellBasis,
    pub scope: SiteScope,
    pub provenance: FamilySiteProvenance,
}

/// Immutable local cell axes used only to normalize ordinary mark fill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NominalCellBasis {
    pub axis_a: Vector2,
    pub axis_b: Vector2,
}

impl NominalCellBasis {
    /// Builds a finite positive nominal cell basis without changing site identity or provenance.
    ///
    /// # Errors
    ///
    /// Returns a stable family-site diagnostic when an axis or either diagonal is not finite and positive.
    pub fn new(axis_a: Vector2, axis_b: Vector2) -> Result<Self, FamilySiteError> {
        let basis = Self { axis_a, axis_b };
        if !basis.is_valid() {
            return Err(FamilySiteError::new(
                "family_sites.nominal_cell_basis",
                "nominal cell axes and diameter must be finite and positive",
            ));
        }
        Ok(basis)
    }

    /// Returns the longer local cell diagonal used by normalized mark fill.
    pub fn diameter(self) -> f64 {
        let positive = Vector2::new(self.axis_a.x + self.axis_b.x, self.axis_a.y + self.axis_b.y);
        let negative = Vector2::new(self.axis_a.x - self.axis_b.x, self.axis_a.y - self.axis_b.y);
        positive
            .x
            .hypot(positive.y)
            .max(negative.x.hypot(negative.y))
    }

    /// Reports whether both ordered axes and the derived diameter satisfy the immutable basis contract.
    pub fn is_valid(self) -> bool {
        self.axis_a.x.is_finite()
            && self.axis_a.y.is_finite()
            && self.axis_b.x.is_finite()
            && self.axis_b.y.is_finite()
            && self.axis_a.x.hypot(self.axis_a.y) > 0.0
            && self.axis_b.x.hypot(self.axis_b.y) > 0.0
            && self.diameter().is_finite()
            && self.diameter() > 0.0
    }
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
            if !site.nominal_cell_basis.is_valid() {
                return Err(FamilySiteError::new(
                    "family_sites.nominal_cell_basis",
                    "nominal cell axes and diameter must be finite and positive",
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
                        !location.path.source.is_valid()
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
                if !location.path.source.is_valid()
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
            FamilySiteProvenance::AlongParametricCurve {
                location,
                absolute_arc_position_bits,
                local_arc_position_bits,
                ..
            } => {
                if !matches!(
                    location.path.source,
                    StructuralPathSourceId::ParametricCurve(_)
                ) || !f64::from_bits(location.parameter_bits).is_finite()
                    || !f64::from_bits(*absolute_arc_position_bits).is_finite()
                    || !f64::from_bits(*local_arc_position_bits).is_finite()
                {
                    return Err(FamilySiteError::new(
                        "family_sites.provenance.along_parametric_curve",
                        "parametric provenance requires finite parametric-path locations",
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

    /// Projects an ordered subset without renumbering stable family-site identities.
    ///
    /// Membership is interpreted relative to this complete family, so unknown IDs are ignored and
    /// retained sites remain in evaluator order. The base-family fingerprint and mechanism identity
    /// remain unchanged because filtering is realization intent rather than a second family product.
    pub fn filtered(&self, members: &BTreeSet<FamilySiteId>) -> Self {
        Self {
            family_fingerprint: self.family_fingerprint.clone(),
            product_mechanism_id: self.product_mechanism_id,
            sites: self
                .sites
                .iter()
                .filter(|site| members.contains(&site.id))
                .cloned()
                .collect(),
        }
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

/// Explicit fill behavior for canonical closed mark geometry.
///
/// The renderer consumes this immutable semantic rather than deriving winding
/// behavior from source artwork or a frontend setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CanonicalFillRule {
    EvenOdd,
    /// The contour direction determines coverage, preserving overlapping outline windings.
    NonZero,
}

/// A renderer-independent closed curve mark realized from one existing family site.
///
/// The stored path remains editable cubic/line construction geometry. Bounds,
/// source identity, scope, and provenance are retained so no consumer recreates
/// sites or changes closure topology while clipping to its canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalPathMark {
    pub source_site_id: FamilySiteId,
    pub path: CurvePath,
    pub bounds: Bounds,
    pub scope: SiteScope,
    pub provenance: FamilySiteProvenance,
    pub fill_rule: CanonicalFillRule,
}

/// One sampled response knot retained by a canonical variable-width stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeProfileSample {
    /// Exact centerline location; no consumer infers this from sampled coordinates.
    pub location: PathLocation,
    pub center: Point2,
    /// Authored normalized thickness in the connected response range `[0, 2]`.
    pub normalized_thickness: f64,
    /// Resolved document-space outline width.
    pub width: f64,
}

/// Typed origin identity for canonical strokes; renderers consume strokes but never reinterpret their topology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalStrokeSourceId {
    Structural(StructuralPathInstanceId),
    Connection(ConnectionPathId),
    Maze(MazeWallPathId),
}

/// Ordered canonical stroke geometry derived from exactly one structural or connection path.
///
/// The original centerline remains immutable and is never clipped by this value.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalStroke {
    pub source_id: CanonicalStrokeSourceId,
    pub source_structure_id: Option<toniator_domain::AuthoredStructureId>,
    pub path: CurvePath,
    pub nominal_basis: f64,
    pub style: toniator_domain::PathStrokeStyle,
    pub profile: Vec<StrokeProfileSample>,
    pub outline: CanonicalFilledOutline,
}

impl CanonicalStroke {
    /// Builds one finite ordered stroke with a reusable derived filled outline.
    ///
    /// # Errors
    ///
    /// Rejects empty/non-finite profiles, a mismatched outline, or invalid nominal bases before a
    /// renderer can consume it. The outline is derived geometry and is never clipped here.
    pub fn new(
        source_path_id: StructuralPathInstanceId,
        source_structure_id: Option<toniator_domain::AuthoredStructureId>,
        path: CurvePath,
        nominal_basis: f64,
        style: toniator_domain::PathStrokeStyle,
        profile: Vec<StrokeProfileSample>,
        outline: CanonicalFilledOutline,
    ) -> Result<Self, CurveError> {
        Self::new_with_source(
            CanonicalStrokeSourceId::Structural(source_path_id),
            source_structure_id,
            path,
            nominal_basis,
            style,
            profile,
            outline,
        )
    }

    /// Validates one canonical stroke while preserving its sole typed source identity.
    ///
    /// # Errors
    ///
    /// Rejects malformed finite profile or outline geometry before a renderer can consume either
    /// structural or connection provenance. It never derives an alternate source identifier.
    fn new_with_source(
        source_id: CanonicalStrokeSourceId,
        source_structure_id: Option<toniator_domain::AuthoredStructureId>,
        path: CurvePath,
        nominal_basis: f64,
        style: toniator_domain::PathStrokeStyle,
        profile: Vec<StrokeProfileSample>,
        outline: CanonicalFilledOutline,
    ) -> Result<Self, CurveError> {
        if !nominal_basis.is_finite() || nominal_basis <= 0.0 {
            return Err(CurveError::new(
                "canonical.stroke.basis",
                "stroke nominal basis must be positive and finite",
            ));
        }
        if profile.is_empty() {
            return Err(CurveError::new(
                "canonical.stroke.profile.empty",
                "canonical strokes require at least one profile sample",
            ));
        }
        if path.bounds().is_err()
            || profile.iter().any(|sample| {
                !sample.center.is_finite()
                    || !sample.normalized_thickness.is_finite()
                    || !(0.0..=2.0).contains(&sample.normalized_thickness)
                    || !sample.width.is_finite()
                    || sample.width < 0.0
                    || (sample.normalized_thickness * nominal_basis - sample.width).abs()
                        > 1.0 / 4096.0
            })
        {
            return Err(CurveError::new(
                "canonical.stroke.profile",
                "stroke profile samples must be finite and nonnegative",
            ));
        }
        for (index, sample) in profile.iter().enumerate() {
            let point = path.point_at(sample.location).map_err(|_| {
                CurveError::new(
                    "canonical.stroke.location",
                    "stroke profile locations must address the centerline",
                )
            })?;
            if ((point.x - sample.center.x).powi(2) + (point.y - sample.center.y).powi(2)).sqrt()
                > 1.0 / 4096.0
            {
                return Err(CurveError::new(
                    "canonical.stroke.center",
                    "stroke profile center must match its exact path location",
                ));
            }
            if index > 0 {
                let previous = profile[index - 1].location;
                let wrap = path.closure() == PathClosure::Closed
                    && index + 1 == profile.len()
                    && sample.location == profile[0].location;
                if !wrap
                    && (sample.location.segment_index() < previous.segment_index()
                        || (sample.location.segment_index() == previous.segment_index()
                            && sample.location.parameter() < previous.parameter()))
                {
                    return Err(CurveError::new(
                        "canonical.stroke.profile.order",
                        "stroke profile locations must follow authored path order",
                    ));
                }
            }
        }
        if outline.fill_rule != CanonicalFillRule::NonZero {
            return Err(CurveError::new(
                "canonical.stroke.outline.fill_rule",
                "canonical stroke outlines require nonzero winding",
            ));
        }
        if outline.contours.iter().any(|contour| {
            contour.segments.is_empty()
                || contour
                    .segments
                    .windows(2)
                    .any(|pair| pair[0].end() != pair[1].start())
                || contour
                    .segments
                    .last()
                    .is_some_and(|last| last.end() != contour.segments[0].start())
                || contour
                    .segments
                    .iter()
                    .any(|segment| segment.bounds().is_err())
        }) {
            return Err(CurveError::new(
                "canonical.stroke.outline",
                "canonical stroke outline contours must be finite connected closed geometry",
            ));
        }
        if outline
            .bounds
            .is_some_and(|bounds| !bounds.min.is_finite() || !bounds.max.is_finite())
        {
            return Err(CurveError::new(
                "canonical.stroke.outline.bounds",
                "canonical stroke outline bounds must remain finite",
            ));
        }
        Ok(Self {
            source_id,
            source_structure_id,
            path,
            nominal_basis,
            style,
            profile,
            outline,
        })
    }

    /// Builds one finite connection stroke while retaining its non-structural path identity.
    ///
    /// # Errors
    ///
    /// Preserves all canonical stroke geometry validation and never substitutes connection identity
    /// for a structural fingerprint in pre-existing callers.
    pub fn new_connection(
        connection_path_id: ConnectionPathId,
        path: CurvePath,
        nominal_basis: f64,
        style: toniator_domain::PathStrokeStyle,
        profile: Vec<StrokeProfileSample>,
        outline: CanonicalFilledOutline,
    ) -> Result<Self, CurveError> {
        Self::new_with_source(
            CanonicalStrokeSourceId::Connection(connection_path_id),
            None,
            path,
            nominal_basis,
            style,
            profile,
            outline,
        )
    }

    /// Builds one finite maze-wall stroke while retaining its typed arrangement-path identity.
    ///
    /// # Errors
    ///
    /// Preserves canonical stroke validation and never turns a maze wall into a structural or
    /// connection path for renderer or cache consumers.
    pub fn new_maze(
        maze_wall_path_id: MazeWallPathId,
        path: CurvePath,
        nominal_basis: f64,
        style: toniator_domain::PathStrokeStyle,
        profile: Vec<StrokeProfileSample>,
        outline: CanonicalFilledOutline,
    ) -> Result<Self, CurveError> {
        Self::new_with_source(
            CanonicalStrokeSourceId::Maze(maze_wall_path_id),
            None,
            path,
            nominal_basis,
            style,
            profile,
            outline,
        )
    }
}

impl CanonicalPathMark {
    /// Creates one finite closed path mark after validating its exact geometry bounds.
    ///
    /// # Errors
    ///
    /// Returns the existing curve error when the path is open or its bounds
    /// cannot remain finite; no consumer-side clipping or topology repair occurs.
    pub fn new(
        source_site_id: FamilySiteId,
        path: CurvePath,
        scope: SiteScope,
        provenance: FamilySiteProvenance,
        fill_rule: CanonicalFillRule,
    ) -> Result<Self, CurveError> {
        if path.closure() != PathClosure::Closed {
            return Err(CurveError::new(
                "canonical.path_mark.closure",
                "canonical filled path marks require a closed curve path",
            ));
        }
        let bounds = path.bounds()?;
        Ok(Self {
            source_site_id,
            path,
            bounds,
            scope,
            provenance,
            fill_rule,
        })
    }
}

/// Truthful canonical filled mark geometry shared by typed shape and circle outputs.
///
/// Unlike the retained Stage 3 diagnostic circle adapter, every variant retains
/// the family-emission ID and provenance that the family evaluator published.
#[derive(Clone, Debug, PartialEq)]
pub enum CanonicalMark {
    Circle {
        source_site_id: FamilySiteId,
        center: Point2,
        radius: f64,
        scope: SiteScope,
        provenance: FamilySiteProvenance,
        fill_rule: CanonicalFillRule,
    },
    ClosedPath(CanonicalPathMark),
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
