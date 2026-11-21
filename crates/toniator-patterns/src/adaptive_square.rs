//! Full-cell nearest-area or blue-noise silhouettes; all decisions precede final consumer clipping.
use super::*;
use std::sync::OnceLock;

/// Symmetric source quadrature uses 16² cell-centered samples, independent of output pixels.
const N: usize = 16;
/// Subdivided silhouette masks use 256² effective samples and fractional boundary weights.
const SUB: usize = 16;
const AREAS: [f64; 5] = [
    0.0,
    std::f64::consts::FRAC_PI_4 - 0.5,
    0.5,
    std::f64::consts::FRAC_PI_4,
    1.0,
];

/// Fixed reference order implements circle-first then clockwise reference-pose ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Segment,
    Triangle,
    Sector,
    Circle,
    Square,
}

/// Returns exact analytic area of the shared full-cell footprint.
fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Segment => AREAS[1],
        Shape::Triangle => AREAS[2],
        Shape::Sector | Shape::Circle => AREAS[3],
        Shape::Square => 1.0,
    }
}

/// Rotates a unit coordinate clockwise around the cell center in artwork y-down axes.
fn rotate(mut x: f64, mut y: f64, turns: usize) -> (f64, f64) {
    for _ in 0..turns {
        (x, y) = (1.0 - y, x);
    }
    (x, y)
}

/// Tests one analytic reference silhouette, including its diagonal chord boundary.
fn contains(shape: Shape, x: f64, y: f64) -> bool {
    match shape {
        Shape::Segment => x * x + (y - 1.0) * (y - 1.0) <= 1.0 && y <= x,
        Shape::Triangle => y >= x,
        Shape::Sector => x * x + (y - 1.0) * (y - 1.0) <= 1.0,
        Shape::Circle => (x - 0.5) * (x - 0.5) + (y - 0.5) * (y - 0.5) <= 0.25,
        Shape::Square => true,
    }
}

/// Retains source-independent fractional masks in a fixed shape/pose order.
/// Each mask is normalized to analytic area, ensuring identical uniform overlap scores.
fn masks() -> &'static Vec<(Shape, usize, Vec<f64>)> {
    static MASKS: OnceLock<Vec<(Shape, usize, Vec<f64>)>> = OnceLock::new();
    MASKS.get_or_init(|| {
        let mut masks = Vec::new();
        for shape in [
            Shape::Circle,
            Shape::Segment,
            Shape::Triangle,
            Shape::Sector,
            Shape::Square,
        ] {
            for turn in 0..if matches!(shape, Shape::Circle | Shape::Square) {
                1
            } else {
                4
            } {
                let mut weights = vec![0.0; N * N];
                for y in 0..N {
                    for x in 0..N {
                        let mut count = 0;
                        for j in 0..SUB {
                            for i in 0..SUB {
                                let (px, py) = rotate(
                                    (x as f64 + (i as f64 + 0.5) / SUB as f64) / N as f64,
                                    (y as f64 + (j as f64 + 0.5) / SUB as f64) / N as f64,
                                    (4 - turn) % 4,
                                );
                                count += usize::from(contains(shape, px, py));
                            }
                        }
                        weights[y * N + x] = count as f64 / (SUB * SUB * N * N) as f64;
                    }
                }
                let normalization = area(shape) / weights.iter().sum::<f64>();
                for weight in &mut weights {
                    *weight *= normalization;
                }
                masks.push((shape, turn, weights));
            }
        }
        masks
    })
}

/// Distributes adjacent analytic area levels with a stable progressive threshold.
/// Exact endpoints and levels remain exact; this changes no silhouette or overlap arithmetic.
fn mixed_level(mean: f64, threshold: f64) -> usize {
    let mean = mean.clamp(0.0, 1.0);
    for (low, levels) in AREAS.windows(2).enumerate() {
        let high = low + 1;
        if mean <= levels[1] {
            let proportion = (mean - levels[0]) / (levels[1] - levels[0]);
            return if proportion > threshold { high } else { low };
        }
    }
    AREAS.len() - 1
}

/// Reads existing blue-noise ranks using signed logical guide indices, without channel phase.
/// # Errors
/// Rejects unsupported provenance instead of inventing an ordinal-based location.
fn cell_threshold(site: &FamilySite) -> Result<f64, PatternPipelineError> {
    let FamilySiteProvenance::GuideIntersection { contributors } = &site.provenance else {
        return Err(PatternPipelineError::new(
            "pattern.adaptive_blue_noise.cell",
            "Expected guide intersection",
        ));
    };
    if contributors.len() != 2 {
        return Err(PatternPipelineError::new(
            "pattern.adaptive_blue_noise.cell",
            "Expected two square-grid guides",
        ));
    }
    let (a, b) = if contributors[0].dimension_id < contributors[1].dimension_id {
        (contributors[0], contributors[1])
    } else {
        (contributors[1], contributors[0])
    };
    Ok(super::dispersion::ranked_blue_noise(a.index, b.index))
}


/// Chooses nearest analytic area with lower-level midpoint ties, then fixed-precision overlap.
fn choose(ink: &[f64]) -> Option<usize> {
    let mean = ink.iter().sum::<f64>() / (N * N) as f64;
    let mut level = 0;
    for i in 1..AREAS.len() {
        if mean > (AREAS[i - 1] + AREAS[i]) * 0.5 + 1e-12 {
            level = i;
        }
    }
    choose_at_level(ink, level)
}

/// Fits the existing silhouette and pose at a selected area, preserving overlap ties.
fn choose_at_level(ink: &[f64], level: usize) -> Option<usize> {
    if level == 0 {
        return None;
    }
    let mut best = None;
    let mut score = i64::MIN;
    for (index, (shape, _, weights)) in masks().iter().enumerate() {
        if area(*shape) != AREAS[level] {
            continue;
        }
        let candidate =
            (ink.iter().zip(weights).map(|(i, w)| i * w).sum::<f64>() * 1e9).round() as i64;
        if candidate > score {
            score = candidate;
            best = Some(index);
        }
    }
    best
}

/// Maps unit coordinates through complete immutable family cell axes, retaining centered placement.
fn position(site: &FamilySite, x: f64, y: f64) -> Point2 {
    let tangent = site.nominal_cell_basis.axis_a;
    // Native guides are identified by their normals: the first tangent is +Y at 0°.
    // Undo that quarter turn for reference +X; its clockwise perpendicular supplies +Y.
    Point2::new(
        site.position.x + (x - 0.5) * tangent.y + (y - 0.5) * tangent.x,
        site.position.y - (x - 0.5) * tangent.x + (y - 0.5) * tangent.y,
    )
}

/// Constructs compact shared canonical geometry; the quarter arc uses the standard tangent cubic.
/// The maximum radial error is 0.000273 cell pitches and area error is below 0.00023 cell areas.
/// # Errors
/// Propagates finite curve validation without publishing partial geometry.
fn geometry(
    site: &FamilySite,
    shape: Shape,
    turns: usize,
) -> Result<CanonicalMark, PatternPipelineError> {
    if shape == Shape::Circle {
        return Ok(CanonicalMark::Circle {
            source_site_id: site.id,
            center: site.position,
            radius: site
                .nominal_cell_basis
                .axis_a
                .x
                .hypot(site.nominal_cell_basis.axis_a.y)
                * 0.5,
            scope: site.scope,
            provenance: site.provenance.clone(),
            fill_rule: CanonicalFillRule::EvenOdd,
        });
    }
    let p = |x, y| {
        let (x, y) = rotate(x, y, turns);
        position(site, x, y)
    };
    let mut segments = Vec::new();
    let line = |a, b| {
        toniator_geometry::LineSegment::new(a, b)
            .map(CurveSegment::Line)
            .map_err(|e| PatternPipelineError::new(e.path(), e.message()))
    };
    if matches!(shape, Shape::Segment | Shape::Sector) {
        let k = 4.0 * (2.0_f64.sqrt() - 1.0) / 3.0;
        segments.push(CurveSegment::CubicBezier(
            CubicBezierSegment::new(p(0.0, 0.0), p(k, 0.0), p(1.0, 1.0 - k), p(1.0, 1.0))
                .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?,
        ));
        if shape == Shape::Sector {
            segments.push(line(p(1.0, 1.0), p(0.0, 1.0))?);
            segments.push(line(p(0.0, 1.0), p(0.0, 0.0))?);
        } else {
            segments.push(line(p(1.0, 1.0), p(0.0, 0.0))?);
        }
    } else {
        let vertices = if shape == Shape::Triangle {
            vec![p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)]
        } else {
            vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]
        };
        for i in 0..vertices.len() {
            segments.push(line(vertices[i], vertices[(i + 1) % vertices.len()])?);
        }
    }
    let path = CurvePath::new(segments, PathClosure::Closed)
        .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?;
    Ok(CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            site.id,
            path,
            site.scope,
            site.provenance.clone(),
            CanonicalFillRule::EvenOdd,
        )
        .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?,
    ))
}

/// Samples complete guard cells before choosing area/pose and footprint-associated source paint.
/// Source quadrature error for arbitrary images is not bounded; mask area error is bounded by
/// perimeter times sqrt(2)/256 plus four boundary-grid pixels before analytic normalization.
/// Stable indexed parallel collection preserves publication order and fixed score arithmetic.
/// # Errors
/// Rejects unsupported Uniform probes, sampling, geometry, allocation bounds or cancellation.
#[allow(clippy::too_many_arguments)]
pub(super) fn realize(
    document: &Document,
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    request: CanonicalMarkRequest,
    input: GeometryResponseInput<'_>,
    blue_noise: bool,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<TypedRealization<CanonicalMarkRealization>, PatternPipelineError> {
    let _ = document;
    preflight_transformed_curve_segment_instances(
        family.site_set().len(),
        4,
        request.max_transformed_curve_segment_instances,
    )?;
    let probe = matches!(
        input,
        GeometryResponseInput::RawProbe(_) | GeometryResponseInput::ConstantProbe(_)
    );
    let prepared_mapping = if !probe {
        let pixels = u64::from(source.identity().width) * u64::from(source.identity().height);
        let budget = if (family.site_set().len() as u64).saturating_mul((N * N) as u64) >= pixels {
            16 * 1024 * 1024
        } else {
            0
        };
        Some(
            source
                .prepare_mapping_response(canvas, request.mapping, budget, cancel)
                .map_err(|error| PatternPipelineError::new(error.path(), error.message()))?,
        )
    } else {
        None
    };
    let results: Vec<_> = family
        .site_set()
        .sites()
        .par_iter()
        .map(|site| {
            if cancel() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "Adaptive shapes cancelled",
                ));
            }
            let mut ink = vec![0.0; N * N];
            let mut colors = request
                .sampled_paint
                .then(|| vec![(0.0, 0.0, 0.0, 0.0); N * N]);
            for y in 0..N {
                for x in 0..N {
                    let index = y * N + x;
                    let point = position(
                        site,
                        (x as f64 + 0.5) / N as f64,
                        (y as f64 + 0.5) / N as f64,
                    );
                    let mapped = match input {
                        GeometryResponseInput::RawProbe(raw) => raw.clamp(0.0, 1.0),
                        GeometryResponseInput::ConstantProbe(tone) => tone,
                        GeometryResponseInput::UniformRadius(_)
                        | GeometryResponseInput::UniformRadiusProbe(_) => {
                            return Err(PatternPipelineError::new(
                                "pattern.adaptive_square.uniform",
                                "Adaptive shapes use cell area, not Uniform radius",
                            ));
                        }
                        _ => prepared_mapping
                            .as_ref()
                            .expect("non-probe prepared mapping")
                            .sample(point)
                            .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?,
                    };
                    let probe = matches!(
                        input,
                        GeometryResponseInput::RawProbe(_)
                            | GeometryResponseInput::ConstantProbe(_)
                    );
                    ink[index] = if !probe && request.mapping.suppresses(mapped) {
                        0.0
                    } else {
                        let (minimum, maximum) = match input {
                            GeometryResponseInput::AbsoluteCoverage {
                                minimum, maximum, ..
                            } => (minimum, maximum),
                            _ => (request.response.minimum_fill, request.response.maximum_fill),
                        };
                        if probe {
                            mapped
                        } else {
                            minimum + (maximum - minimum) * mapped
                        }
                    };
                    if let Some(colors) = &mut colors {
                        colors[index] = source
                            .sample_associated_color(point, canvas)
                            .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?;
                    }
                }
            }
            let selected = if blue_noise {
                let mean = ink.iter().sum::<f64>() / (N * N) as f64;
                choose_at_level(&ink, mixed_level(mean, cell_threshold(site)?))
            } else {
                choose(&ink)
            };
            let Some(index) = selected else {
                return Ok(None);
            };
            let (shape, turns, weights) = &masks()[index];
            let paint = if let Some(colors) = colors {
                let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
                for ((red, green, blue, alpha), w) in colors.iter().zip(weights) {
                    r += red * w;
                    g += green * w;
                    b += blue * w;
                    a += alpha * w;
                }
                if a <= 0.0 {
                    return Ok(None);
                }
                Some(SampledSourcePaint {
                    red: (r / a).clamp(0.0, 1.0),
                    green: (g / a).clamp(0.0, 1.0),
                    blue: (b / a).clamp(0.0, 1.0),
                    alpha: 1.0,
                })
            } else {
                None
            };
            Ok(Some((geometry(site, *shape, *turns)?, paint, site.id)))
        })
        .collect();
    let mut marks = Vec::new();
    let mut paints = request.sampled_paint.then(Vec::new);
    let mut used = Vec::new();
    for result in results {
        if let Some((mark, paint, id)) = result? {
            marks.push(mark);
            used.push(id);
            if let Some(paint) = paint {
                paints.as_mut().expect("sampled storage").push(paint);
            }
        }
    }
    let mut bytes = family.family_fingerprint().as_bytes().to_vec();
    bytes.extend(if blue_noise {
        b"adaptive-square-blue-noise64-v1".as_slice()
    } else {
        b"adaptive-square-nearest-area-v1".as_slice()
    });
    append_source_identity(&mut bytes, source);
    append_source_mapping_identity(&mut bytes, request.mapping);
    for mark in &marks {
        append_canonical_mark_identity(&mut bytes, mark);
    }
    if let Some(paints) = &paints {
        for paint in paints {
            for component in [paint.red, paint.green, paint.blue, paint.alpha] {
                bytes.extend(component.to_bits().to_le_bytes());
            }
        }
    }
    Ok(TypedRealization {
        provenance: realization_provenance(family, plan)?,
        output: CanonicalMarkRealization {
            family_fingerprint: family.family_fingerprint().to_owned(),
            realization_fingerprint: fnv1a64(bytes),
            source_identity: source.identity().clone(),
            response: request.response,
            marks,
            paints,
            usage: SiteUsageSet::new(family.site_set().product_mechanism_id(), used)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Bounds a complete rank tile's mixed area error and keeps every exact analytic level exact.
    /// # Panics
    /// Panics if mixed selection changes endpoints, analytic levels or progressive-rank arithmetic.
    #[test]
    fn adaptive_blue_noise_mixing_preserves_levels_and_tile_mean() {
        for mean in AREAS.into_iter().chain([0.05, 0.2, 0.4, 0.65, 0.9]) {
            let mut area = 0.0;
            for y in -32..32 {
                for x in -32..32 {
                    let selected = mixed_level(mean, super::super::dispersion::ranked_blue_noise(x, y));
                    if AREAS.contains(&mean) { assert_eq!(AREAS[selected], mean); }
                    area += AREAS[selected];
                }
            }
            assert!((area / 4096.0 - mean).abs() <= 1.0 / 4096.0);
        }
    }

    /// Verifies every exact analytic bucket, lower midpoint ties and uniform circle preference.
    #[test]
    fn adaptive_area_thresholds_and_circle_ties() {
        for i in 0..AREAS.len() {
            let selected = choose(&vec![AREAS[i]; N * N]);
            if i == 0 {
                assert!(selected.is_none());
            } else {
                assert_eq!(area(masks()[selected.unwrap()].0), AREAS[i]);
            }
        }
        for i in 1..AREAS.len() {
            let midpoint = (AREAS[i - 1] + AREAS[i]) * 0.5;
            let chosen = choose(&vec![midpoint; N * N]);
            assert_eq!(chosen.map_or(0.0, |j| area(masks()[j].0)), AREAS[i - 1]);
        }
        assert_eq!(
            masks()[choose(&vec![AREAS[3]; N * N]).unwrap()].0,
            Shape::Circle
        );
    }
    /// Verifies diagonal ink chooses the reference triangle and its opposite half turns it twice.
    #[test]
    fn adaptive_diagonal_half_ink_fits_pose() {
        for reverse in [false, true] {
            let mut ink = vec![0.0; N * N];
            for y in 0..N {
                for x in 0..N {
                    ink[y * N + x] = if x == y {
                        0.5
                    } else if (y > x) ^ reverse {
                        1.0
                    } else {
                        0.0
                    };
                }
            }
            let chosen = choose(&ink).unwrap();
            assert_eq!(
                (masks()[chosen].0, masks()[chosen].1),
                (Shape::Triangle, if reverse { 2 } else { 0 })
            );
        }
    }
}
