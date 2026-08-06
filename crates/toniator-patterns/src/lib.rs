#![forbid(unsafe_code)]

//! Deterministic straight-guide family evaluation.

use std::{error::Error, fmt};

use serde::Serialize;
use toniator_domain::{CanvasSpec, DensityMetric2D, GuideDimensionId};
use toniator_geometry::{
    AffineTransform2D, Bounds, GuideInstanceId, GuideIntersectionProvenance, IntersectionSite,
    Point2, SiteId, SiteScope, StraightGuide, Vector2, projection_range,
};

/// The finite antialiasing envelope included in every Stage 3 generation plan.
pub const ANTIALIAS_MARGIN: f64 = 1.0;

/// Stable IDs for the two fixed rectangular straight-guide dimensions.
pub const FIRST_DIMENSION_ID: GuideDimensionId = GuideDimensionId(1);
pub const SECOND_DIMENSION_ID: GuideDimensionId = GuideDimensionId(2);

/// Headless input to the two-dimension straight-grid family.
#[derive(Clone, Debug, PartialEq)]
pub struct GridInspectRequest {
    pub canvas: CanvasSpec,
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    /// Authored document-axis translation; it is never replaced by phase.
    pub translation_x: f64,
    /// Authored document-axis translation; it is never replaced by phase.
    pub translation_y: f64,
    pub guard_steps: u32,
    pub support_radius: f64,
}

/// A coverage result for one stable guide dimension.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GuideCoverage {
    pub dimension_id: u64,
    pub spacing: f64,
    /// The phase is normalized only for reporting; authored translation remains input state.
    pub normalized_phase: f64,
    pub first_index: i64,
    pub last_index: i64,
}

/// Deterministic, off-canvas family output before any realization or clipping.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GridFamilyOutput {
    pub family_fingerprint: String,
    pub guard_steps: u32,
    pub support_radius: f64,
    pub antialias_margin: f64,
    pub generation_domain: Bounds,
    pub coverage: [GuideCoverage; 2],
    pub guides: Vec<StraightGuide>,
    pub sites: Vec<IntersectionSite>,
}

impl GridFamilyOutput {
    pub fn has_only_finite_geometry(&self) -> bool {
        self.generation_domain.min.is_finite()
            && self.generation_domain.max.is_finite()
            && self.guides.iter().all(|guide| {
                guide.normal.x.is_finite()
                    && guide.normal.y.is_finite()
                    && guide.tangent.x.is_finite()
                    && guide.tangent.y.is_finite()
                    && guide.offset.is_finite()
                    && guide.start.is_finite()
                    && guide.end.is_finite()
            })
            && self.sites.iter().all(|site| site.position.is_finite())
    }
}

/// A schema-scoped failure before geometric generation begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridError {
    path: &'static str,
    message: &'static str,
}

impl GridError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for GridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for GridError {}

/// Evaluates two stable-ID, straight dimensions and all of their intersections.
///
/// The canvas contributes only the padded local extent. It never contributes a
/// guide, a site, or topology. Returned lines are finite presentations of
/// infinite guides and deliberately extend beyond that planned local extent.
pub fn evaluate_straight_grid(request: &GridInspectRequest) -> Result<GridFamilyOutput, GridError> {
    validate(request)?;

    let spacing_x = directional_spacing(&request.canvas, &request.density, Vector2::new(1.0, 0.0))?;
    let spacing_y = directional_spacing(&request.canvas, &request.density, Vector2::new(0.0, 1.0))?;
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(request.canvas.width / 2.0, request.canvas.height / 2.0),
        request.rotation_degrees,
        Vector2::new(request.translation_x, request.translation_y),
    )
    .ok_or(GridError::new(
        "channel.pattern.layout",
        "transform is not finite",
    ))?;

    let document_canvas = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(request.canvas.width, request.canvas.height),
    )
    .expect("validated canvas creates finite bounds");
    let planning_margin = request.support_radius
        + ANTIALIAS_MARGIN
        + f64::from(request.guard_steps) * spacing_x.max(spacing_y);
    let padded_document_canvas = document_canvas
        .expanded(planning_margin)
        .expect("validated finite margin expands finite canvas");
    let generation_domain =
        transform
            .inverse_bounds(padded_document_canvas)
            .ok_or(GridError::new(
                "coverage",
                "inverse transform produced non-finite bounds",
            ))?;

    let dimensions = [
        DimensionPlan::new(FIRST_DIMENSION_ID, Vector2::new(1.0, 0.0), spacing_x),
        DimensionPlan::new(SECOND_DIMENSION_ID, Vector2::new(0.0, 1.0), spacing_y),
    ];
    let plans = [
        dimensions[0].coverage(generation_domain, transform, request)?,
        dimensions[1].coverage(generation_domain, transform, request)?,
    ];

    let extension = planning_margin;
    let mut guides = Vec::new();
    for (dimension, coverage) in dimensions.iter().zip(plans.iter()) {
        guides.extend(dimension.guides(*coverage, generation_domain, transform, extension));
    }

    let mut sites = Vec::new();
    for first_index in plans[0].first_index..=plans[0].last_index {
        for second_index in plans[1].first_index..=plans[1].last_index {
            let local = Point2::new(
                first_index as f64 * dimensions[0].spacing,
                second_index as f64 * dimensions[1].spacing,
            );
            let position = transform.apply_point(local);
            let first = GuideInstanceId::new(FIRST_DIMENSION_ID, first_index);
            let second = GuideInstanceId::new(SECOND_DIMENSION_ID, second_index);
            sites.push(IntersectionSite {
                id: SiteId {
                    first_dimension_id: FIRST_DIMENSION_ID.0,
                    first_index,
                    second_dimension_id: SECOND_DIMENSION_ID.0,
                    second_index,
                },
                position,
                scope: if document_canvas.contains(position) {
                    SiteScope::Canvas
                } else {
                    SiteScope::Guard
                },
                provenance: GuideIntersectionProvenance {
                    contributors: [first, second],
                },
            });
        }
    }

    let output = GridFamilyOutput {
        family_fingerprint: fingerprint(request, spacing_x, spacing_y),
        guard_steps: request.guard_steps,
        support_radius: request.support_radius,
        antialias_margin: ANTIALIAS_MARGIN,
        generation_domain,
        coverage: plans,
        guides,
        sites,
    };
    if output.has_only_finite_geometry() {
        Ok(output)
    } else {
        Err(GridError::new(
            "coverage",
            "generation produced non-finite geometry",
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct DimensionPlan {
    id: GuideDimensionId,
    normal: Vector2,
    tangent: Vector2,
    spacing: f64,
}

impl DimensionPlan {
    fn new(id: GuideDimensionId, normal: Vector2, spacing: f64) -> Self {
        Self {
            id,
            normal,
            tangent: normal.perpendicular(),
            spacing,
        }
    }

    fn coverage(
        self,
        domain: Bounds,
        transform: AffineTransform2D,
        request: &GridInspectRequest,
    ) -> Result<GuideCoverage, GridError> {
        let (minimum, maximum) = projection_range(domain.corners(), self.normal)
            .ok_or(GridError::new("coverage", "could not project local domain"))?;
        let first_index = checked_index((minimum / self.spacing).floor())?;
        let last_index = checked_index((maximum / self.spacing).ceil())?;
        let document_normal = transform.apply_vector(self.normal);
        let translated_phase = request
            .translation_x
            .mul_add(document_normal.x, request.translation_y * document_normal.y);
        Ok(GuideCoverage {
            dimension_id: self.id.0,
            spacing: self.spacing,
            normalized_phase: translated_phase.rem_euclid(self.spacing),
            first_index,
            last_index,
        })
    }

    fn guides(
        self,
        coverage: GuideCoverage,
        domain: Bounds,
        transform: AffineTransform2D,
        extension: f64,
    ) -> Vec<StraightGuide> {
        let (minimum_tangent, maximum_tangent) =
            projection_range(domain.corners(), self.tangent).expect("finite domain projects");
        (coverage.first_index..=coverage.last_index)
            .map(|index| {
                let offset = index as f64 * self.spacing;
                let start_local = point_on_line(
                    self.normal,
                    self.tangent,
                    offset,
                    minimum_tangent - extension,
                );
                let end_local = point_on_line(
                    self.normal,
                    self.tangent,
                    offset,
                    maximum_tangent + extension,
                );
                StraightGuide {
                    id: GuideInstanceId::new(self.id, index),
                    normal: transform.apply_vector(self.normal),
                    tangent: transform.apply_vector(self.tangent),
                    offset,
                    start: transform.apply_point(start_local),
                    end: transform.apply_point(end_local),
                }
            })
            .collect()
    }
}

fn point_on_line(
    normal: Vector2,
    tangent: Vector2,
    normal_offset: f64,
    tangent_offset: f64,
) -> Point2 {
    Point2::new(
        normal.x.mul_add(normal_offset, tangent.x * tangent_offset),
        normal.y.mul_add(normal_offset, tangent.y * tangent_offset),
    )
}

fn checked_index(value: f64) -> Result<i64, GridError> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(GridError::new(
            "coverage",
            "guide index is outside the supported range",
        ));
    }
    Ok(value as i64)
}

fn validate(request: &GridInspectRequest) -> Result<(), GridError> {
    validate_positive(request.canvas.width, "canvas.width")?;
    validate_positive(request.canvas.height, "canvas.height")?;
    validate_positive(
        request.density.across_x,
        "channel.pattern.layout.density.across_x",
    )?;
    validate_positive(
        request.density.across_y,
        "channel.pattern.layout.density.across_y",
    )?;
    validate_finite(
        request.rotation_degrees,
        "channel.pattern.layout.rotation_degrees",
    )?;
    validate_finite(
        request.translation_x,
        "channel.pattern.layout.translation_x",
    )?;
    validate_finite(
        request.translation_y,
        "channel.pattern.layout.translation_y",
    )?;
    validate_finite(request.support_radius, "coverage.support_radius")?;
    if request.support_radius < 0.0 {
        return Err(GridError::new(
            "coverage.support_radius",
            "value must not be negative",
        ));
    }
    Ok(())
}

fn validate_finite(value: f64, path: &'static str) -> Result<(), GridError> {
    value
        .is_finite()
        .then_some(())
        .ok_or(GridError::new(path, "value must be finite"))
}

fn validate_positive(value: f64, path: &'static str) -> Result<(), GridError> {
    validate_finite(value, path)?;
    (value > 0.0)
        .then_some(())
        .ok_or(GridError::new(path, "value must be greater than zero"))
}

/// Resolves the guide spacing from the documented directional-frequency metric.
pub fn directional_spacing(
    canvas: &CanvasSpec,
    density: &DensityMetric2D,
    unit_normal: Vector2,
) -> Result<f64, GridError> {
    validate_positive(canvas.width, "canvas.width")?;
    validate_positive(canvas.height, "canvas.height")?;
    validate_positive(density.across_x, "channel.pattern.layout.density.across_x")?;
    validate_positive(density.across_y, "channel.pattern.layout.density.across_y")?;
    let spacing_x = canvas.width / density.across_x;
    let spacing_y = canvas.height / density.across_y;
    let frequency = (unit_normal.x / spacing_x).hypot(unit_normal.y / spacing_y);
    validate_positive(frequency, "density.directional_frequency")?;
    Ok(frequency.recip())
}

fn fingerprint(request: &GridInspectRequest, spacing_x: f64, spacing_y: f64) -> String {
    let values = [
        request.canvas.width,
        request.canvas.height,
        request.density.across_x,
        request.density.across_y,
        request.rotation_degrees,
        request.translation_x,
        request.translation_y,
        request.support_radius,
        spacing_x,
        spacing_y,
    ];
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in b"toniator-stage-3-straight-grid-v1"
        .iter()
        .copied()
        .chain(request.guard_steps.to_le_bytes())
        .chain(FIRST_DIMENSION_ID.0.to_le_bytes())
        .chain(SECOND_DIMENSION_ID.0.to_le_bytes())
        .chain(
            values
                .into_iter()
                .flat_map(|value| value.to_bits().to_le_bytes()),
        )
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}
