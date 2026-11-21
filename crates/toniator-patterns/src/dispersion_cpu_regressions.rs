//! Retains the pre-compute dispersion producer for ordered CPU parallelism checks.
use super::*;

/// Produces the original serial population as an independent bit/order/error control.
/// # Errors
/// Returns the frozen producer's source, budget and cancellation diagnostics.
#[allow(clippy::too_many_arguments)]
fn reference_generate(
    layout: DispersionLayout,
    bounds: Bounds,
    pitch: f64,
    seed: u32,
    budget: usize,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    guide: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Point2>, PatternPipelineError> {
    let step = if layout == DispersionLayout::ClusterScreen {
        pitch * 0.5
    } else {
        pitch
    };
    validate_grid_coordinates(bounds, step)?;
    let x0 = (bounds.min.x / step).floor() as i64;
    let y0 = (bounds.min.y / step).floor() as i64;
    let x1 = (bounds.max.x / step).ceil() as i64;
    let y1 = (bounds.max.y / step).ceil() as i64;
    let count = (x1 - x0)
        .unsigned_abs()
        .saturating_mul((y1 - y0).unsigned_abs());
    if count > budget as u64 {
        return Err(PatternPipelineError::new(
            "coverage.candidate_limit",
            "dispersion exceeds the configured candidate limit",
        ));
    }
    let mut out = Vec::new();
    if matches!(
        layout,
        DispersionLayout::TangentSpacing
            | DispersionLayout::TangentSize
            | DispersionLayout::TangentBlend
    ) {
        return ribbons(
            bounds,
            pitch,
            budget,
            sample,
            guide,
            layout.spacing_fraction(),
            is_cancelled,
        );
    }
    if layout == DispersionLayout::InkQuadtree {
        let root = pitch * 8.0;
        let mut debt = 0.5;
        for y in (bounds.min.y / root).floor() as i64..(bounds.max.y / root).ceil() as i64 {
            for x in (bounds.min.x / root).floor() as i64..(bounds.max.x / root).ceil() as i64 {
                quadtree(
                    Point2::new(x as f64 * root, y as f64 * root),
                    root,
                    pitch,
                    0,
                    &mut debt,
                    &mut out,
                    budget,
                    sample,
                    is_cancelled,
                )?;
            }
        }
        return Ok(out);
    }
    let width = usize::try_from(x1 - x0).map_err(|_| {
        PatternPipelineError::new("coverage.candidate_limit", "dispersion width overflows")
    })?;
    let mut errors = Vec::new();
    errors.try_reserve_exact(width + 2).map_err(|_| {
        PatternPipelineError::new(
            "coverage.dispersion.allocation",
            "diffusion row allocation failed",
        )
    })?;
    errors.resize(width + 2, 0.0);
    let mut next = Vec::new();
    next.try_reserve_exact(width + 2).map_err(|_| {
        PatternPipelineError::new(
            "coverage.dispersion.allocation",
            "diffusion row allocation failed",
        )
    })?;
    next.resize(width + 2, 0.0);
    for y in y0..y1 {
        if is_cancelled() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "dispersion was cancelled",
            ));
        }
        let reverse = y.rem_euclid(2) == 1;
        for i in 0..width {
            let col = if reverse { width - 1 - i } else { i };
            let x = x0 + col as i64;
            let point = Point2::new((x as f64 + 0.5) * step, (y as f64 + 0.5) * step);
            let q = cell_tone(point, step, sample)?;
            let keep = match layout {
                DispersionLayout::Bayer => q > bayer(x, y),
                DispersionLayout::FloydSteinberg => {
                    let old = q + errors[col + 1];
                    let keep = old >= 0.5;
                    let error = old - f64::from(keep);
                    let direction = if reverse { -1_isize } else { 1 };
                    let k = (col + 1) as isize;
                    errors[(k + direction) as usize] += error * 7.0 / 16.0;
                    next[(k - direction) as usize] += error * 3.0 / 16.0;
                    next[k as usize] += error * 5.0 / 16.0;
                    next[(k + direction) as usize] += error / 16.0;
                    keep
                }
                DispersionLayout::ClusterScreen => {
                    let cell = pitch * 5.0;
                    let cx = (point.x / cell).floor() as i64;
                    let cy = (point.y / cell).floor() as i64;
                    let mut distance = f64::INFINITY;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let h = screen_hash(cx + dx, cy + dy, seed);
                            let px = (cx as f64
                                + dx as f64
                                + 0.1
                                + 0.8 * (h as u32 as f64 / u32::MAX as f64))
                                * cell;
                            let py = (cy as f64
                                + dy as f64
                                + 0.1
                                + 0.8 * ((h >> 32) as u32 as f64 / u32::MAX as f64))
                                * cell;
                            distance =
                                distance.min((point.x - px).powi(2) + (point.y - py).powi(2));
                        }
                    }
                    // Fixed source-independent radial screen, calibrated on its full population.
                    q >= 1.0 || q > 1.0 - (-std::f64::consts::PI * distance / (cell * cell)).exp()
                }
                _ => unreachable!("tree and ribbon layouts are handled above"),
            };
            if keep && q > 0.0 {
                try_push_family_value(
                    &mut out,
                    point,
                    "coverage.dispersion.allocation",
                    "dispersion site allocation failed",
                )?;
            }
        }
        std::mem::swap(&mut errors, &mut next);
        next.fill(0.0);
    }
    Ok(out)
}

/// Preserves every site and serpentine ordinal while parallel sampling and nucleus reuse accelerate independent layouts.
/// # Panics
/// Panics if one- or four-worker production output differs from the original serial producer.
#[test]
fn independent_layout_parallelism_matches_original_bits() {
    let bounds = Bounds::new(Point2::new(-7.0, -5.0), Point2::new(505.0, 507.0)).unwrap();
    let sample = |p: Point2| Ok(((p.x * 0.003).sin() + (p.y * 0.007).cos() + 2.0) * 0.25);
    for layout in [DispersionLayout::Bayer, DispersionLayout::ClusterScreen] {
        let expected = reference_generate(
            layout,
            bounds,
            1.0,
            43,
            2_000_000,
            &sample,
            &sample,
            &|| false,
        )
        .unwrap();
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let actual = pool
                .install(|| {
                    generate(
                        layout,
                        bounds,
                        1.0,
                        43,
                        2_000_000,
                        &sample,
                        &sample,
                        &|| false,
                    )
                })
                .unwrap();
            assert_eq!(actual.len(), expected.len());
            assert!(
                actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits())
            );
        }
    }
}
