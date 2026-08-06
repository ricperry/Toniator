#![forbid(unsafe_code)]

//! Deterministic straight-guide family evaluation.

use std::{error::Error, fmt};

use serde::Serialize;
use toniator_domain::{CanvasSpec, DensityMetric2D, GuideDimensionId};
pub use toniator_geometry::{
    AffineTransform2D, Bounds, CanonicalCircleMark, GuideInstanceId, GuideIntersectionProvenance,
    IntersectionSite, Point2, SiteId, SiteScope, StraightGuide, Vector2, projection_range,
};
use toniator_sampling::{SamplingError, SourceComponent, SourceField, SourcePlacement};

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

/// Immutable, renderer-independent circular realization of an existing family.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CircularMarkRealization {
    pub family_fingerprint: String,
    pub realization_fingerprint: String,
    pub source_identity: toniator_sampling::SourceIdentity,
    pub source_component: SourceComponent,
    pub placement: SourcePlacement,
    pub response: MarkResponse,
    pub marks: Vec<CanonicalCircleMark>,
}

impl CircularMarkRealization {
    pub fn has_only_finite_marks(&self) -> bool {
        self.marks
            .iter()
            .all(|mark| mark.center.is_finite() && mark.radius.is_finite())
    }
}

/// The bounded diameter response used to realize canonical radii.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MarkResponse {
    pub minimum_size: f64,
    pub maximum_size: f64,
}

/// A realization-boundary failure. Family generation errors remain `GridError`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizationError {
    path: &'static str,
    message: &'static str,
}

impl RealizationError {
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

impl fmt::Display for RealizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RealizationError {}

impl From<SamplingError> for RealizationError {
    fn from(error: SamplingError) -> Self {
        Self::new(error.path(), error.message())
    }
}

/// Realizes every supplied family site in its existing stable order.
///
/// This function deliberately receives `GridFamilyOutput` rather than a grid
/// request so a size change cannot recreate guides, sites, or provenance.
pub fn realize_circular_marks(
    family: &GridFamilyOutput,
    source: &SourceField,
    canvas: &CanvasSpec,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> Result<CircularMarkRealization, RealizationError> {
    validate_response(response)?;
    if !family.has_only_finite_geometry() {
        return Err(RealizationError::new(
            "realization.family",
            "family geometry must be finite",
        ));
    }
    let mut marks = Vec::with_capacity(family.sites.len());
    for site in &family.sites {
        let ink = source.sample_mark_ink(site.position, canvas, placement, component)?;
        if !ink.is_finite() {
            return Err(RealizationError::new(
                "realization.sample",
                "effective mark ink must be finite",
            ));
        }
        let radius = radius_from_ink(ink, response)?;
        let mark = CanonicalCircleMark::new(
            site.id,
            site.position,
            radius,
            site.scope,
            site.provenance.clone(),
        )
        .ok_or(RealizationError::new(
            "realization.mark",
            "mark geometry must be finite",
        ))?;
        marks.push(mark);
    }
    let output = CircularMarkRealization {
        family_fingerprint: family.family_fingerprint.clone(),
        realization_fingerprint: realization_fingerprint(
            family, source, placement, component, response,
        ),
        source_identity: source.identity().clone(),
        source_component: component,
        placement,
        response,
        marks,
    };
    output
        .has_only_finite_marks()
        .then_some(output)
        .ok_or(RealizationError::new(
            "realization.mark",
            "realization produced non-finite marks",
        ))
}

/// Maps an effective mark-ink response linearly to radius using the authored
/// diameter bounds. Source sampling owns component polarity and alpha handling.
pub fn radius_from_ink(ink: f64, response: MarkResponse) -> Result<f64, RealizationError> {
    validate_response(response)?;
    if !ink.is_finite() {
        return Err(RealizationError::new(
            "realization.ink",
            "effective mark ink must be finite",
        ));
    }
    let ink = ink.clamp(0.0, 1.0);
    Ok((response.minimum_size + ink * (response.maximum_size - response.minimum_size)) / 2.0)
}

fn validate_response(response: MarkResponse) -> Result<(), RealizationError> {
    if !response.minimum_size.is_finite() || !response.maximum_size.is_finite() {
        return Err(RealizationError::new(
            "realization.response",
            "diameters must be finite",
        ));
    }
    if response.minimum_size < 2.0
        || response.maximum_size > 9.0
        || response.minimum_size > response.maximum_size
    {
        return Err(RealizationError::new(
            "realization.response",
            "diameters must satisfy 2.0 <= minimum <= maximum <= 9.0",
        ));
    }
    Ok(())
}

fn realization_fingerprint(
    family: &GridFamilyOutput,
    source: &SourceField,
    placement: SourcePlacement,
    component: SourceComponent,
    response: MarkResponse,
) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let placement = match placement {
        SourcePlacement::StretchToCanvas => 1_u8,
    };
    let component = match component {
        SourceComponent::Luminance => 1_u8,
        SourceComponent::Alpha => 2_u8,
    };
    let format = match source.identity().format {
        toniator_sampling::SourceFormat::Png => 1_u8,
        toniator_sampling::SourceFormat::Svg => 2_u8,
    };
    for byte in b"toniator-stage-4-circular-realization-v2-alpha-associated"
        .iter()
        .copied()
        .chain(family.family_fingerprint.bytes())
        .chain(source.identity().content_hash.bytes())
        .chain(source.identity().decoded_pixel_hash.bytes())
        .chain([format, placement, component])
        .chain(source.identity().width.to_le_bytes())
        .chain(source.identity().height.to_le_bytes())
        .chain(response.minimum_size.to_bits().to_le_bytes())
        .chain(response.maximum_size.to_bits().to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod realization_tests {
    use super::*;
    use toniator_sampling::{SourceFormatHint, decode_source};

    fn family() -> GridFamilyOutput {
        evaluate_straight_grid(&GridInspectRequest {
            canvas: CanvasSpec {
                width: 90.0,
                height: 60.0,
            },
            density: DensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
                aspect_locked: true,
            },
            rotation_degrees: 17.0,
            translation_x: 3.25,
            translation_y: -4.5,
            guard_steps: 2,
            support_radius: 4.5,
        })
        .unwrap()
    }

    fn field() -> SourceField {
        let bytes = std::fs::read(format!(
            "{}/../../assets/raster-sample.png",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        decode_source(&bytes, SourceFormatHint::Png).unwrap()
    }

    #[test]
    fn png_alpha_associated_ink_reaches_canonical_radii_without_hidden_rgb_fringes() {
        let image = image::RgbaImage::from_raw(
            8,
            1,
            vec![
                0, 0, 0, 0, // transparent black
                255, 255, 255, 0, // transparent white
                255, 0, 0, 0, // transparent saturated red
                0, 0, 0, 255, // opaque black
                255, 255, 255, 255, // opaque white
                0, 0, 0, 0, // same black RGB, alpha 0
                0, 0, 0, 128, // same black RGB, alpha about 0.5
                0, 0, 0, 255, // same black RGB, alpha 1
            ],
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let source = decode_source(&bytes, SourceFormatHint::Png).unwrap();
        let mut grid = family();
        let prototype = grid.sites[0].clone();
        grid.sites = (0..8)
            .map(|x| {
                let mut site = prototype.clone();
                site.position = Point2::new(f64::from(x), 0.0);
                site
            })
            .collect();
        let canvas = CanvasSpec {
            width: 7.0,
            height: 1.0,
        };
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        };
        let luminance = realize_circular_marks(
            &grid,
            &source,
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            response,
        )
        .unwrap();
        let alpha = realize_circular_marks(
            &grid,
            &source,
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Alpha,
            response,
        )
        .unwrap();
        assert_eq!(
            luminance.marks[0].radius, 1.0,
            "transparent black is the minimum mark radius"
        );
        assert!(
            [1, 2, 5]
                .into_iter()
                .all(|index| luminance.marks[index].radius == luminance.marks[0].radius),
            "all zero-alpha hidden RGB variants map to minimum radius"
        );
        assert_eq!(luminance.marks[3].radius, 4.5);
        assert_eq!(luminance.marks[4].radius, 1.0);
        let half_alpha_radius = (2.0 + (128.0 / 255.0) * 7.0) / 2.0;
        assert!((luminance.marks[6].radius - half_alpha_radius).abs() < 1e-12);
        assert_eq!(luminance.marks[7].radius, 4.5);
        assert!(
            alpha.marks[5].radius > alpha.marks[6].radius
                && alpha.marks[6].radius > alpha.marks[7].radius,
            "Alpha response has one decreasing alpha polarity, without squaring"
        );
        assert!((alpha.marks[6].radius - (2.0 + (127.0 / 255.0) * 7.0) / 2.0).abs() < 1e-12);
        assert_ne!(
            luminance.realization_fingerprint,
            alpha.realization_fingerprint
        );
    }

    #[test]
    fn diameter_response_uses_effective_ink_and_stores_radius() {
        let response = MarkResponse {
            minimum_size: 2.0,
            maximum_size: 9.0,
        };
        assert_eq!(radius_from_ink(0.0, response).unwrap(), 1.0);
        assert_eq!(radius_from_ink(0.5, response).unwrap(), 2.75);
        assert_eq!(radius_from_ink(1.0, response).unwrap(), 4.5);
        assert!(radius_from_ink(f64::NAN, response).is_err());
        assert!(
            radius_from_ink(
                0.5,
                MarkResponse {
                    minimum_size: 1.0,
                    maximum_size: 9.0
                }
            )
            .is_err()
        );
    }

    #[test]
    fn size_changes_reuse_every_site_and_keep_guards_without_clipping() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let first = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
        )
        .unwrap();
        let second = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_size: 3.0,
                maximum_size: 8.0,
            },
        )
        .unwrap();
        assert_eq!(first.family_fingerprint, family.family_fingerprint);
        assert_eq!(first.marks.len(), family.sites.len());
        assert!(
            first
                .marks
                .iter()
                .any(|mark| mark.scope == SiteScope::Guard)
        );
        for ((mark_a, mark_b), site) in first.marks.iter().zip(&second.marks).zip(&family.sites) {
            assert_eq!(mark_a.source_site_id, site.id);
            assert_eq!(mark_a.center, site.position);
            assert_eq!(mark_a.scope, site.scope);
            assert_eq!(mark_a.provenance, site.provenance);
            assert_eq!(mark_a.source_site_id, mark_b.source_site_id);
            assert_eq!(mark_a.center, mark_b.center);
        }
        assert!(
            first
                .marks
                .iter()
                .zip(&second.marks)
                .any(|(left, right)| left.radius != right.radius)
        );
        assert_ne!(
            first.realization_fingerprint,
            second.realization_fingerprint
        );
    }

    #[test]
    fn canonical_marks_and_fingerprint_are_presentation_independent() {
        let family = family();
        let canvas = CanvasSpec {
            width: 90.0,
            height: 60.0,
        };
        let left = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
        )
        .unwrap();
        // Color, opacity, and visibility have no realization inputs by design.
        let right = realize_circular_marks(
            &family,
            &field(),
            &canvas,
            SourcePlacement::StretchToCanvas,
            SourceComponent::Luminance,
            MarkResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&left.marks).unwrap(),
            serde_json::to_vec(&right.marks).unwrap()
        );
        assert_eq!(left.realization_fingerprint, right.realization_fingerprint);
        assert!(left.has_only_finite_marks());
    }
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
