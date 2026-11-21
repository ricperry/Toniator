//! Channel-driven placements publish ordinary sites; all consumers share canonical marks.
use super::*;
use toniator_domain::DispersionLayout;
#[cfg(test)]
#[path = "dispersion_cpu_regressions.rs"]
mod cpu_regressions;

/// Produces a complete, bounded layout in document coordinates without tile-local phases.
/// The sampler already owns channel mapping, alpha, response and cutoff.
/// # Errors
/// Returns sampling, cancellation, allocation or caller work-limit diagnostics without partial sites.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate(
    layout: DispersionLayout,
    bounds: Bounds,
    pitch: f64,
    seed: u32,
    budget: usize,
    sample: &(dyn Fn(Point2) -> Result<f64, PatternPipelineError> + Sync),
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
    let mut parallel_tone = matches!(
        layout,
        DispersionLayout::RankedBlueNoise | DispersionLayout::Bayer | DispersionLayout::ClusterScreen
    ) && count >= 262_144
        && width >= 128;
    let mut tone_block = Vec::new();
    if parallel_tone && tone_block.try_reserve_exact(width.min(1024)).is_err() {
        parallel_tone = false;
    }
    let mut nucleus_row: HashMap<i64, [(f64, f64); 9]> = HashMap::new();
    let mut nucleus_y = None;
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
            if parallel_tone && i.is_multiple_of(1024) {
                if is_cancelled() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "dispersion was cancelled",
                    ));
                }
                (i..(i + 1024).min(width))
                    .into_par_iter()
                    .map(|ordinal| {
                        let column = if reverse {
                            width - 1 - ordinal
                        } else {
                            ordinal
                        };
                        let position = Point2::new(
                            ((x0 + column as i64) as f64 + 0.5) * step,
                            (y as f64 + 0.5) * step,
                        );
                        cell_tone(position, step, sample)
                    })
                    .collect_into_vec(&mut tone_block);
            }
            let q = if parallel_tone {
                tone_block[i % 1024].clone()?
            } else {
                cell_tone(point, step, sample)?
            };
            let keep = match layout {
                DispersionLayout::RankedBlueNoise => q > ranked_blue_noise(x, y),
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
                    if nucleus_y != Some(cy) {
                        nucleus_row.clear();
                        nucleus_y = Some(cy);
                    }
                    if !nucleus_row.contains_key(&cx) && nucleus_row.try_reserve(1).is_ok() {
                        nucleus_row.insert(cx, cluster_nuclei(cx, cy, cell, seed));
                    }
                    let uncached;
                    let centers = if let Some(centers) = nucleus_row.get(&cx) {
                        centers
                    } else {
                        uncached = cluster_nuclei(cx, cy, cell, seed);
                        &uncached
                    };
                    let mut distance = f64::INFINITY;
                    for &(px, py) in centers {
                        distance = distance.min((point.x - px).powi(2) + (point.y - py).powi(2));
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

/// Rejects nonrepresentable signed grid coordinates before integer arithmetic or allocation.
/// # Errors
/// Returns a finite-grid diagnostic for underflowed spacing or excessive signed spans.
fn validate_grid_coordinates(bounds: Bounds, step: f64) -> Result<(), PatternPipelineError> {
    if !step.is_finite()
        || step <= 0.0
        || [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
            .iter()
            .any(|value| {
                !(*value / step).is_finite() || (*value / step).abs() > i64::MAX as f64 / 4.0
            })
    {
        return Err(PatternPipelineError::new(
            "coverage.dispersion.grid",
            "dispersion grid coordinates are not representable",
        ));
    }
    Ok(())
}

/// Averages four fixed subcell samples through the receiving channel's sampler.
/// # Errors
/// Propagates source sampling diagnostics.
fn cell_tone(
    p: Point2,
    step: f64,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
) -> Result<f64, PatternPipelineError> {
    let mut q = 0.0;
    for dy in [-0.25, 0.25] {
        for dx in [-0.25, 0.25] {
            q += sample(Point2::new(p.x + dx * step, p.y + dy * step))?;
        }
    }
    Ok((q * 0.25).clamp(0.0, 1.0))
}

/// Looks up a unique progressive rank with Euclidean wrapping of global logical coordinates.
/// The checked-in project-owned table is generated by `tools/generate_blue_noise.py`.
pub(super) fn ranked_blue_noise(x: i64, y: i64) -> f64 {
    const RANKS: &[u8; 8192] = include_bytes!("blue_noise_64.bin");
    let offset = (y.rem_euclid(64) as usize * 64 + x.rem_euclid(64) as usize) * 2;
    let rank = u16::from_le_bytes([RANKS[offset], RANKS[offset + 1]]);
    (f64::from(rank) + 0.5) / 4096.0
}

/// Evaluates the conventional recursively dispersed 8×8 Bayer threshold at a global cell.
fn bayer(x: i64, y: i64) -> f64 {
    let mut value = 0;
    for bit in 0..3 {
        let a = (x.rem_euclid(8) >> bit) & 1;
        let b = (y.rem_euclid(8) >> bit) & 1;
        value = 4 * value
            + match (a, b) {
                (0, 0) => 0,
                (1, 0) => 2,
                (0, 1) => 3,
                _ => 1,
            };
    }
    (value as f64 + bayer_subrank(x.div_euclid(8), y.div_euclid(8))) / 64.0
}

/// Resolves a second ordered rank across 8×8 blocks, preserving the dispersed weave at low ink.
fn bayer_subrank(x: i64, y: i64) -> f64 {
    let mut rank = 0;
    for bit in 0..3 {
        let a = (x.rem_euclid(8) >> bit) & 1;
        let b = (y.rem_euclid(8) >> bit) & 1;
        rank = 4 * rank
            + match (a, b) {
                (0, 0) => 0,
                (1, 0) => 2,
                (0, 1) => 3,
                _ => 1,
            };
    }
    let mut third = 0;
    for bit in 0..3 {
        let a = (x.div_euclid(8).rem_euclid(8) >> bit) & 1;
        let b = (y.div_euclid(8).rem_euclid(8) >> bit) & 1;
        third = 4 * third
            + match (a, b) {
                (0, 0) => 0,
                (1, 0) => 2,
                (0, 1) => 3,
                _ => 1,
            };
    }
    (rank as f64 + (third as f64 + 0.5) / 64.0) / 64.0
}

/// Builds the original ordered nine screen nuclei once per coarse row cell.
/// The original separate integer-to-f64 additions remain intact even at extreme coordinates.
fn cluster_nuclei(cx: i64, cy: i64, cell: f64, seed: u32) -> [(f64, f64); 9] {
    let mut nuclei = [(0.0, 0.0); 9];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let h = screen_hash(cx + dx, cy + dy, seed);
            nuclei[((dy + 1) * 3 + dx + 1) as usize] = (
                (cx as f64 + dx as f64 + 0.1 + 0.8 * (h as u32 as f64 / u32::MAX as f64)) * cell,
                (cy as f64 + dy as f64 + 0.1 + 0.8 * ((h >> 32) as u32 as f64 / u32::MAX as f64))
                    * cell,
            );
        }
    }
    nuclei
}

/// Hashes an unbounded nucleus cell independently of traversal bounds and source pixels.
fn screen_hash(x: i64, y: i64, seed: u32) -> u64 {
    let mut h =
        (x as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (y as u64).rotate_left(29) ^ u64::from(seed);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}

#[cfg(all(test, feature = "geometry-compute"))]
mod compute_tests {
    use super::*;

    /// Checks real hardware certificates against original binary64 ownership, including ties.
    /// The explicit ignored witness requires a guarded GPU run and never silently skips hardware.
    /// # Panics
    /// Panics when hardware admission fails, a certificate disagrees, or cancellation poisons reuse.
    #[test]
    #[ignore = "requires the guarded hardware GPU slot and TONIATOR_GEOMETRY_COMPUTE=gpu"]
    fn certified_nearest_hardware() {
        assert_eq!(
            std::env::var("TONIATOR_GEOMETRY_COMPUTE").as_deref(),
            Ok("gpu")
        );
        let mut index = RibbonIndex {
            pitch: 1.0,
            points: Vec::new(),
            cells: HashMap::new(),
        };
        for y in -6..70 {
            for x in -6..70 {
                let h = screen_hash(x, y, 43);
                index
                    .insert(
                        Point2::new(
                            x as f64 + (h as u32 as f64 / u32::MAX as f64) * 0.3,
                            y as f64 + ((h >> 32) as f64 / u32::MAX as f64) * 0.3,
                        ),
                        0,
                        0,
                    )
                    .unwrap();
            }
        }
        let owners = index
            .nearest_mesh(-8, 136, -8, 136, 1.0, &|| false)
            .expect("real integer kernel completes");
        let mut certified = 0;
        for y in -8..136 {
            for x in -8..136 {
                let ordinal = ((y + 8) * 144 + x + 8) as usize;
                let owner = owners[ordinal];
                let p = Point2::new((x as f64 + 0.5) * 0.5, (y as f64 + 0.5) * 0.5);
                if owner != u32::MAX {
                    assert_eq!(Some(owner as usize), index.nearest(p));
                    certified += 1;
                }
            }
        }
        assert!(
            certified > owners.len() * 9 / 10,
            "substantial work is certified"
        );
        assert!(index.nearest_mesh(0, 512, 0, 512, 1.0, &|| true).is_none());
        assert!(
            index.nearest_mesh(0, 32, 0, 32, 1.0, &|| false).is_some(),
            "fresh job succeeds after cancellation"
        );
        for exponent in [-100, 100] {
            let pitch = 2.0_f64.powi(10 - exponent);
            let mut scaled = RibbonIndex {
                pitch,
                points: Vec::new(),
                cells: HashMap::new(),
            };
            for point in &index.points {
                scaled
                    .insert(
                        Point2::new(point.point.x * pitch, point.point.y * pitch),
                        0,
                        0,
                    )
                    .unwrap();
            }
            let owners = scaled.nearest_mesh(0, 32, 0, 32, pitch, &|| false).unwrap();
            for y in 0..32 {
                for x in 0..32 {
                    let owner = owners[(y * 32 + x) as usize];
                    let p = Point2::new(
                        (x as f64 + 0.5) * pitch * 0.5,
                        (y as f64 + 0.5) * pitch * 0.5,
                    );
                    if owner != u32::MAX {
                        assert_eq!(Some(owner as usize), scaled.nearest(p));
                    }
                }
            }
        }
        for exponent in [-101, 101] {
            let pitch = 2.0_f64.powi(10 - exponent);
            assert!(index.nearest_mesh(0, 32, 0, 32, pitch, &|| false).is_none());
        }

        let mut ties = RibbonIndex {
            pitch: 1.0,
            points: Vec::new(),
            cells: HashMap::new(),
        };
        ties.insert(Point2::new(0.0, 0.25), 0, 0).unwrap();
        ties.insert(Point2::new(0.5, 0.25), 0, 0).unwrap();
        assert_eq!(
            ties.nearest_mesh(0, 1, 0, 1, 1.0, &|| false).unwrap(),
            vec![u32::MAX]
        );
    }
}

/// Integrates a global dyadic node with fixed quadrature and emits its ink centroid.
/// Leaves share mass debt; no accepted mass is replenished after filtering.
/// # Errors
/// Returns sampling, cancellation, allocation or explicit population-limit diagnostics.
#[allow(clippy::too_many_arguments)]
fn quadtree(
    origin: Point2,
    side: f64,
    pitch: f64,
    depth: u32,
    debt: &mut f64,
    out: &mut Vec<Point2>,
    budget: usize,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    cancel: &dyn Fn() -> bool,
) -> Result<(), PatternPipelineError> {
    if cancel() {
        return Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "quadtree was cancelled",
        ));
    }
    let mut mass = 0.0;
    let mut mx = 0.0;
    let mut my = 0.0;
    for y in 0..4 {
        for x in 0..4 {
            let p = Point2::new(
                origin.x + (x as f64 + 0.5) * side / 4.0,
                origin.y + (y as f64 + 0.5) * side / 4.0,
            );
            let q = sample(p)?;
            mass += q;
            mx += q * p.x;
            my += q * p.y;
        }
    }
    let ink = mass * side * side / (16.0 * pitch * pitch);
    if ink > 1.0 && depth < 8 {
        for (x, y) in [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.5, 0.5)] {
            quadtree(
                Point2::new(origin.x + x * side, origin.y + y * side),
                side / 2.0,
                pitch,
                depth + 1,
                debt,
                out,
                budget,
                sample,
                cancel,
            )?;
        }
    } else if mass > 0.0 {
        *debt += ink;
        if *debt >= 1.0 {
            *debt -= 1.0;
            if out.len() >= budget {
                return Err(PatternPipelineError::new(
                    "coverage.candidate_limit",
                    "quadtree population exceeds limit",
                ));
            }
            try_push_family_value(
                out,
                Point2::new(mx / mass, my / mass),
                "coverage.dispersion.allocation",
                "quadtree allocation failed",
            )?;
        }
    }
    Ok(())
}

/// Traces local unoriented artwork tangents with distance-controlled global reseeding.
/// All tone allocations share the same complete guides; only arc-length ink debt differs.
/// Neighbor seeds and a final uncovered-cell sweep replace tile-local frames and phase resets.
/// # Errors
/// Returns source, allocation, cancellation or explicit caller population diagnostics.
fn ribbons(
    bounds: Bounds,
    pitch: f64,
    budget: usize,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    guide: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    fraction: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Point2>, PatternPipelineError> {
    let field = RibbonField::new(bounds, pitch, guide, cancel)?;
    let mut index = RibbonIndex {
        pitch,
        points: Vec::new(),
        cells: HashMap::new(),
    };
    let mut seeds = Vec::new();
    try_push_family_value(
        &mut seeds,
        Point2::new(
            ((bounds.min.x + bounds.max.x) * 0.5 / pitch).floor() * pitch + pitch * 0.5,
            ((bounds.min.y + bounds.max.y) * 0.5 / pitch).floor() * pitch + pitch * 0.5,
        ),
        "coverage.dispersion.allocation",
        "ribbon seed allocation failed",
    )?;
    let mut cursor = 0;
    let mut line_id = 0;
    let mut paths = Vec::new();
    let x0 = (bounds.min.x / (pitch * 0.5)).floor() as i64;
    let x1 = (bounds.max.x / (pitch * 0.5)).ceil() as i64;
    let y0 = (bounds.min.y / (pitch * 0.5)).floor() as i64;
    let y1 = (bounds.max.y / (pitch * 0.5)).ceil() as i64;
    let mut sweep_x = x0;
    let mut sweep_y = y0;
    loop {
        if cancel() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "ribbon coverage was cancelled",
            ));
        }
        let seed = if let Some(seed) = seeds.get(cursor).copied() {
            cursor += 1;
            seed
        } else {
            if sweep_y >= y1 {
                break;
            }
            let seed = Point2::new(
                (sweep_x as f64 + 0.5) * pitch * 0.5,
                (sweep_y as f64 + 0.5) * pitch * 0.5,
            );
            sweep_x += 1;
            if sweep_x >= x1 {
                sweep_x = x0;
                sweep_y += 1;
            }
            seed
        };
        if !bounds.contains(seed) || index.conflict(seed, None, pitch * 0.92) {
            continue;
        }
        let line = trace_ribbon(seed, line_id, bounds, &field, &mut index, budget, cancel)?;
        line_id += 1;
        for ordinal in line.iter().step_by(4) {
            let point = index.points[*ordinal].point;
            let tangent = field.tangent(point);
            let normal = (-tangent.1, tangent.0);
            for sign in [-1.0, 1.0] {
                try_push_family_value(
                    &mut seeds,
                    Point2::new(
                        point.x + sign * normal.0 * pitch,
                        point.y + sign * normal.1 * pitch,
                    ),
                    "coverage.dispersion.allocation",
                    "ribbon frontier allocation failed",
                )?;
            }
        }
        try_push_family_value(
            &mut paths,
            line,
            "coverage.dispersion.allocation",
            "ribbon path table allocation failed",
        )?;
    }
    drop(seeds);
    // Global 2D quadrature assigns every represented cell to one nearest guide sample.
    // Ink opportunity therefore follows area, including terminated and one-point guides,
    // rather than assuming every curved stream represents exactly one pitch-wide strip.
    let mut mass = Vec::new();
    mass.try_reserve_exact(index.points.len()).map_err(|_| {
        PatternPipelineError::new(
            "coverage.dispersion.allocation",
            "ribbon area allocation failed",
        )
    })?;
    mass.resize(index.points.len(), 0.0);
    #[cfg(feature = "geometry-compute")]
    let mut owners: Option<Vec<u32>> = None;
    #[cfg(feature = "geometry-compute")]
    let mut owner_window = (x0, x0, y0, y0);
    #[cfg(feature = "geometry-compute")]
    let stripe_width = (x1 - x0).clamp(1, 1_048_576);
    #[cfg(feature = "geometry-compute")]
    let stripe_rows = (1_048_576 / stripe_width).max(1);
    for y in y0..y1 {
        if cancel() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "ribbon area ownership was cancelled",
            ));
        }
        for x in x0..x1 {
            if ((x - x0) as u64).is_multiple_of(1024) && cancel() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "ribbon area ownership was cancelled",
                ));
            }
            let point = Point2::new(
                (x as f64 + 0.5) * pitch * 0.5,
                (y as f64 + 0.5) * pitch * 0.5,
            );
            if !bounds.contains(point) {
                continue;
            }
            #[cfg(feature = "geometry-compute")]
            if x < owner_window.0 || x >= owner_window.1 || y >= owner_window.3 {
                let first = x0 + ((x - x0) / stripe_width) * stripe_width;
                owner_window = (
                    first,
                    (first + stripe_width).min(x1),
                    y,
                    (y + stripe_rows).min(y1),
                );
                owners = index.nearest_mesh(
                    owner_window.0,
                    owner_window.1,
                    owner_window.2,
                    owner_window.3,
                    pitch,
                    cancel,
                );
                if cancel() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "ribbon area ownership was cancelled",
                    ));
                }
            }
            #[cfg(feature = "geometry-compute")]
            let certified = owners.as_ref().and_then(|owners| {
                let ordinal = ((y - owner_window.2) as usize)
                    * (owner_window.1 - owner_window.0) as usize
                    + (x - owner_window.0) as usize;
                let owner = owners[ordinal];
                (owner != u32::MAX).then_some(owner as usize)
            });
            #[cfg(not(feature = "geometry-compute"))]
            let certified = None;
            let nearest =
                certified
                    .or_else(|| index.nearest(point))
                    .ok_or(PatternPipelineError::new(
                        "coverage.dispersion.coverage",
                        "ribbon coverage left an unrepresented quadrature cell",
                    ))?;
            let q = sample(point)?.clamp(0.0, 1.0);
            mass[nearest] += 0.25
                * if fraction == 0.0 {
                    1.0
                } else {
                    q.powf(fraction)
                };
        }
    }
    let mut out = Vec::new();
    let mut debt = 0.5;
    for path in paths {
        if cancel() {
            return Err(PatternPipelineError::new(
                "evaluation.cancelled",
                "ribbon area placement was cancelled",
            ));
        }
        let total = path.iter().map(|i| mass[*i]).sum::<f64>();
        debt += total;
        let count = debt.floor() as usize;
        debt -= count as f64;
        if count == 0 {
            continue;
        }
        if count > budget.saturating_sub(out.len()) {
            return Err(PatternPipelineError::new(
                "coverage.candidate_limit",
                "ribbon area population exceeds caller limit",
            ));
        }
        let first = index.points[path[0]].point;
        let phase = (screen_hash(
            (first.x / pitch).floor() as i64,
            (first.y / pitch).floor() as i64,
            43,
        ) >> 32) as f64
            / u32::MAX as f64;
        let mut cursor = 0;
        let mut cumulative = 0.0;
        for ordinal in 0..count {
            let target = (ordinal as f64 + phase) * total / count as f64;
            while cursor + 1 < path.len() && cumulative + mass[path[cursor]] < target {
                cumulative += mass[path[cursor]];
                cursor += 1;
            }
            let center = index.points[path[cursor]].point;
            let tangent = field.tangent(center);
            let before = if cursor > 0 {
                let p = index.points[path[cursor - 1]].point;
                Point2::new((p.x + center.x) * 0.5, (p.y + center.y) * 0.5)
            } else if path.len() > 1 {
                let p = index.points[path[1]].point;
                Point2::new(
                    center.x + (center.x - p.x) * 0.5,
                    center.y + (center.y - p.y) * 0.5,
                )
            } else {
                Point2::new(
                    center.x - tangent.0 * pitch * count as f64 * 0.5,
                    center.y - tangent.1 * pitch * count as f64 * 0.5,
                )
            };
            let after = if cursor + 1 < path.len() {
                let p = index.points[path[cursor + 1]].point;
                Point2::new((p.x + center.x) * 0.5, (p.y + center.y) * 0.5)
            } else if path.len() > 1 {
                let p = index.points[path[cursor - 1]].point;
                Point2::new(
                    center.x + (center.x - p.x) * 0.5,
                    center.y + (center.y - p.y) * 0.5,
                )
            } else {
                Point2::new(
                    center.x + tangent.0 * pitch * count as f64 * 0.5,
                    center.y + tangent.1 * pitch * count as f64 * 0.5,
                )
            };
            let t = if mass[path[cursor]] > 0.0 {
                ((target - cumulative) / mass[path[cursor]]).clamp(0.0, 1.0)
            } else {
                0.5
            };
            try_push_family_value(
                &mut out,
                Point2::new(
                    before.x + (after.x - before.x) * t,
                    before.y + (after.y - before.y) * t,
                ),
                "coverage.dispersion.allocation",
                "ribbon site allocation failed",
            )?;
        }
    }
    Ok(out)
}

/// Smooth unoriented normal samples retain original channel guidance across all tone modes.
struct RibbonField {
    x0: i64,
    y0: i64,
    nx: usize,
    ny: usize,
    step: f64,
    normals: Vec<(f64, f64)>,
}
impl RibbonField {
    /// Samples a fixed half-pitch mesh and propagates nearby orientation through flat pixels.
    /// Double angles prevent opposite dark/light gradients from canceling the same tangent.
    /// # Errors
    /// Returns checked-grid, source, allocation or cancellation diagnostics.
    fn new(
        bounds: Bounds,
        pitch: f64,
        guide: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, PatternPipelineError> {
        let step = pitch * 0.5;
        validate_grid_coordinates(bounds, step)?;
        let x0 = (bounds.min.x / step).floor() as i64;
        let y0 = (bounds.min.y / step).floor() as i64;
        let nx = ((bounds.max.x / step).ceil() as i64 - x0 + 1) as usize;
        let ny = ((bounds.max.y / step).ceil() as i64 - y0 + 1) as usize;
        let count = nx.checked_mul(ny).ok_or(PatternPipelineError::new(
            "coverage.dispersion.grid",
            "ribbon mesh size overflows",
        ))?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            PatternPipelineError::new(
                "coverage.dispersion.allocation",
                "ribbon mesh allocation failed",
            )
        })?;
        for y in 0..ny {
            if cancel() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "ribbon guide was cancelled",
                ));
            }
            for x in 0..nx {
                values.push(cell_tone(
                    Point2::new((x0 + x as i64) as f64 * step, (y0 + y as i64) as f64 * step),
                    pitch * 2.0,
                    guide,
                )?);
            }
        }
        let mut normals = Vec::new();
        normals.try_reserve_exact(count).map_err(|_| {
            PatternPipelineError::new(
                "coverage.dispersion.allocation",
                "ribbon normal allocation failed",
            )
        })?;
        let mut queue = Vec::new();
        queue.try_reserve_exact(count).map_err(|_| {
            PatternPipelineError::new(
                "coverage.dispersion.allocation",
                "ribbon orientation queue allocation failed",
            )
        })?;
        for y in 0..ny {
            if cancel() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "ribbon normals were cancelled",
                ));
            }
            for x in 0..nx {
                let left = x.saturating_sub(1);
                let right = (x + 1).min(nx - 1);
                let top = y.saturating_sub(1);
                let bottom = (y + 1).min(ny - 1);
                let gx =
                    (values[y * nx + right] - values[y * nx + left]) / (right - left).max(1) as f64;
                let gy =
                    (values[bottom * nx + x] - values[top * nx + x]) / (bottom - top).max(1) as f64;
                let norm = gx * gx + gy * gy;
                if norm > 1e-16 {
                    normals.push(((gx * gx - gy * gy) / norm, 2.0 * gx * gy / norm));
                    queue.push(y * nx + x);
                } else {
                    normals.push((0.0, 0.0));
                }
            }
        }
        drop(values);
        if queue.is_empty() {
            let a = 250.0_f64.to_radians();
            normals.fill((a.cos(), a.sin()));
        } else {
            let mut cursor = 0;
            while cursor < queue.len() {
                if cursor % 4096 == 0 && cancel() {
                    return Err(PatternPipelineError::new(
                        "evaluation.cancelled",
                        "ribbon flat orientation was cancelled",
                    ));
                }
                let i = queue[cursor];
                cursor += 1;
                let (x, y) = (i % nx, i / nx);
                for j in [
                    x.checked_sub(1).map(|_| i - 1),
                    (x + 1 < nx).then_some(i + 1),
                    y.checked_sub(1).map(|_| i - nx),
                    (y + 1 < ny).then_some(i + nx),
                ]
                .into_iter()
                .flatten()
                {
                    if normals[j] == (0.0, 0.0) {
                        normals[j] = normals[i];
                        queue.push(j);
                    }
                }
            }
        }
        Ok(Self {
            x0,
            y0,
            nx,
            ny,
            step,
            normals,
        })
    }
    /// Bilinearly interpolates the real unoriented tangent; integration chooses its local sign.
    fn tangent(&self, p: Point2) -> (f64, f64) {
        let x = (p.x / self.step - self.x0 as f64).clamp(0.0, (self.nx - 1) as f64);
        let y = (p.y / self.step - self.y0 as f64).clamp(0.0, (self.ny - 1) as f64);
        let ix = (x.floor() as usize).min(self.nx - 2);
        let iy = (y.floor() as usize).min(self.ny - 2);
        let fx = x - ix as f64;
        let fy = y - iy as f64;
        let mut a = 0.0;
        let mut b = 0.0;
        for (dx, dy, w) in [
            (0, 0, (1.0 - fx) * (1.0 - fy)),
            (1, 0, fx * (1.0 - fy)),
            (0, 1, (1.0 - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let n = self.normals[(iy + dy) * self.nx + ix + dx];
            a += w * n.0;
            b += w * n.1;
        }
        let angle = 0.5 * b.atan2(a);
        (-angle.sin(), angle.cos())
    }
}

/// One guide sample retains line and signed path index solely for self-neighbor exclusion.
struct RibbonPoint {
    point: Point2,
    line: usize,
    sequence: i64,
}
/// Global guide exclusion uses spatial bins; it is independent of tone-mode mark occupancy.
struct RibbonIndex {
    pitch: f64,
    points: Vec<RibbonPoint>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}
impl RibbonIndex {
    /// Certifies independent mesh ownership with bounded integer distance intervals.
    ///
    /// Power-of-two scaling in the ordinary exponent range bounds quantization and
    /// CPU binary64 distance-rounding error inside each shader interval. Strictly
    /// separated intervals identify the original CPU winner; every overlap, tie,
    /// out-of-range candidate, allocation/device failure or unsupported scale falls
    /// back to `nearest`. The original bucket membership and mass reduction order
    /// remain authoritative. No source values or floating reductions run on the GPU.
    #[cfg(feature = "geometry-compute")]
    fn nearest_mesh(
        &self,
        x0: i64,
        x1: i64,
        y0: i64,
        y1: i64,
        pitch: f64,
        cancel: &dyn Fn() -> bool,
    ) -> Option<Vec<u32>> {
        use crate::geometry_compute::{Kernel, admitted, dispatch};
        let count = usize::try_from(x1.checked_sub(x0)?)
            .ok()?
            .checked_mul(usize::try_from(y1.checked_sub(y0)?).ok()?)?;
        if !admitted(count, 65_536) || count > 3_000_000 || self.points.is_empty() || cancel() {
            return None;
        }
        let exponent = 10 - pitch.log2().floor() as i32;
        if !(-100..=100).contains(&exponent) {
            return None;
        }
        let scale = 2.0_f64.powi(exponent);
        let quantize = |value: f64| -> Option<u32> {
            let scaled = value * scale;
            (scaled.is_finite() && scaled.abs() <= (1_u32 << 29) as f64)
                .then_some((scaled.floor() as i32) as u32)
        };
        let min_x = self.cells.keys().map(|p| p.0).min()?;
        let max_x = self.cells.keys().map(|p| p.0).max()?;
        let min_y = self.cells.keys().map(|p| p.1).min()?;
        let max_y = self.cells.keys().map(|p| p.1).max()?;
        let nx = usize::try_from(max_x.checked_sub(min_x)?.checked_add(1)?).ok()?;
        let ny = usize::try_from(max_y.checked_sub(min_y)?.checked_add(1)?).ok()?;
        let buckets = nx.checked_mul(ny)?;
        let words = 10_usize
            .checked_add(self.points.len().checked_mul(3)?)?
            .checked_add(buckets + 1)?
            .checked_add(
                usize::try_from(x1 - x0)
                    .ok()?
                    .checked_add(usize::try_from(y1 - y0).ok()?)?
                    .checked_mul(2)?,
            )?;
        if words > 16 * 1024 * 1024
            || min_x < i32::MIN as i64
            || min_y < i32::MIN as i64
            || max_x > i32::MAX as i64
            || max_y > i32::MAX as i64
        {
            return None;
        }
        let mut data = Vec::new();
        data.try_reserve_exact(words).ok()?;
        data.resize(10, 0);
        data[0] = u32::try_from(count).ok()?;
        data[1] = data.len() as u32;
        for (i, point) in self.points.iter().enumerate() {
            if i.is_multiple_of(4096) && cancel() {
                return None;
            }
            data.extend([quantize(point.point.x)?, quantize(point.point.y)?]);
        }
        data[2] = data.len() as u32;
        let offsets_start = data.len();
        data.resize(data.len() + buckets + 1, 0);
        data[3] = data.len() as u32;
        let ids_start = data.len();
        for y in min_y..=max_y {
            if cancel() {
                return None;
            }
            for x in min_x..=max_x {
                let bucket = ((y - min_y) as usize) * nx + (x - min_x) as usize;
                data[offsets_start + bucket] = (data.len() - ids_start) as u32;
                if let Some(ids) = self.cells.get(&(x, y)) {
                    for id in ids {
                        data.push(u32::try_from(*id).ok()?);
                    }
                }
            }
        }
        data[offsets_start + buckets] = (data.len() - ids_start) as u32;
        data[4] = data.len() as u32;
        data[5] = u32::try_from(nx).ok()?;
        data[6] = u32::try_from(ny).ok()?;
        data[7] = min_x as i32 as u32;
        data[8] = min_y as i32 as u32;
        data[9] = u32::try_from(x1 - x0).ok()?;
        for x in x0..x1 {
            if cancel() {
                return None;
            }
            let position = (x as f64 + 0.5) * pitch * 0.5;
            let cell = (position / self.pitch).floor();
            if cell.abs() > (1_u32 << 29) as f64 {
                return None;
            }
            data.extend([quantize(position)?, cell as i32 as u32]);
        }
        for y in y0..y1 {
            if cancel() {
                return None;
            }
            let position = (y as f64 + 0.5) * pitch * 0.5;
            let cell = (position / self.pitch).floor();
            if cell.abs() > (1_u32 << 29) as f64 {
                return None;
            }
            data.extend([quantize(position)?, cell as i32 as u32]);
        }
        let owners = dispatch(Kernel::Nearest, &data, count, count, cancel)?;
        if owners
            .iter()
            .any(|owner| *owner != u32::MAX && *owner as usize >= self.points.len())
        {
            return None;
        }
        if std::env::var_os("TONIATOR_GEOMETRY_COMPUTE_TRACE").is_some() {
            eprintln!(
                "geometry-compute nearest certified={} corrected={}",
                owners.iter().filter(|owner| **owner != u32::MAX).count(),
                owners.iter().filter(|owner| **owner == u32::MAX).count()
            );
        }
        Some(owners)
    }
    /// Detects nearby completed guides and nonadjacent parts of the current guide.
    fn conflict(&self, p: Point2, own: Option<(usize, i64)>, distance: f64) -> bool {
        let x = (p.x / self.pitch).floor() as i64;
        let y = (p.y / self.pitch).floor() as i64;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(points) = self.cells.get(&(x + dx, y + dy)) {
                    for i in points {
                        let old = &self.points[*i];
                        if own.is_some_and(|(line, sequence)| {
                            old.line == line && (old.sequence - sequence).unsigned_abs() < 12
                        }) {
                            continue;
                        }
                        if point_distance(p, old.point) < distance {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    /// Resolves deterministic nearest-guide area ownership over the globally covered field.
    fn nearest(&self, p: Point2) -> Option<usize> {
        let x = (p.x / self.pitch).floor() as i64;
        let y = (p.y / self.pitch).floor() as i64;
        let mut best = None;
        let mut squared = f64::INFINITY;
        for dy in -2..=2 {
            for dx in -2..=2 {
                if let Some(points) = self.cells.get(&(x + dx, y + dy)) {
                    for i in points {
                        let old = self.points[*i].point;
                        let d = (p.x - old.x).powi(2) + (p.y - old.y).powi(2);
                        if d < squared || (d == squared && best.is_none_or(|old| *i < old)) {
                            squared = d;
                            best = Some(*i);
                        }
                    }
                }
            }
        }
        best
    }

    /// Inserts one guide sample with fallible storage; IDs remain traversal-ordered.
    /// # Errors
    /// Returns allocation diagnostics without publishing a partial family.
    fn insert(
        &mut self,
        p: Point2,
        line: usize,
        sequence: i64,
    ) -> Result<(), PatternPipelineError> {
        self.cells.try_reserve(1).map_err(|_| {
            PatternPipelineError::new(
                "coverage.dispersion.allocation",
                "ribbon spatial allocation failed",
            )
        })?;
        let key = (
            (p.x / self.pitch).floor() as i64,
            (p.y / self.pitch).floor() as i64,
        );
        let ordinal = self.points.len();
        try_push_family_value(
            &mut self.points,
            RibbonPoint {
                point: p,
                line,
                sequence,
            },
            "coverage.dispersion.allocation",
            "ribbon point allocation failed",
        )?;
        let bucket = self.cells.entry(key).or_default();
        try_push_family_value(
            bucket,
            ordinal,
            "coverage.dispersion.allocation",
            "ribbon bucket allocation failed",
        )
    }
}

/// Integrates both directions using midpoint tangent steps and global guide-distance admission.
/// # Errors
/// Returns cancellation, allocation or caller work-bound diagnostics without partial output.
fn trace_ribbon(
    seed: Point2,
    line: usize,
    bounds: Bounds,
    field: &RibbonField,
    index: &mut RibbonIndex,
    budget: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<usize>, PatternPipelineError> {
    let seed_ordinal = index.points.len();
    index.insert(seed, line, 0)?;
    let mut halves = [Vec::new(), Vec::new()];
    let step = index.pitch * 0.25;
    for (half, sign) in halves.iter_mut().zip([-1.0, 1.0]) {
        let mut p = seed;
        let initial = field.tangent(seed);
        let mut direction = (initial.0 * sign, initial.1 * sign);
        let mut sequence = 0_i64;
        loop {
            if cancel() {
                return Err(PatternPipelineError::new(
                    "evaluation.cancelled",
                    "ribbon integration was cancelled",
                ));
            }
            sequence += 1;
            let signed = if sign < 0.0 { -sequence } else { sequence };
            let middle = Point2::new(
                p.x + direction.0 * step * 0.5,
                p.y + direction.1 * step * 0.5,
            );
            let mut tangent = field.tangent(middle);
            if tangent.0 * direction.0 + tangent.1 * direction.1 < 0.0 {
                tangent.0 = -tangent.0;
                tangent.1 = -tangent.1;
            }
            let next = Point2::new(p.x + tangent.0 * step, p.y + tangent.1 * step);
            if !bounds.contains(next)
                || index.conflict(next, Some((line, signed)), index.pitch * 0.82)
            {
                break;
            }
            if index.points.len() >= budget.saturating_mul(64) {
                return Err(PatternPipelineError::new(
                    "coverage.candidate_limit",
                    "ribbon guide work exceeds caller limit",
                ));
            }
            index.insert(next, line, signed)?;
            try_push_family_value(
                half,
                index.points.len() - 1,
                "coverage.dispersion.allocation",
                "ribbon path allocation failed",
            )?;
            p = next;
            direction = tangent;
        }
    }
    let mut points = Vec::new();
    points
        .try_reserve(halves[0].len() + halves[1].len() + 1)
        .map_err(|_| {
            PatternPipelineError::new(
                "coverage.dispersion.allocation",
                "ribbon path join allocation failed",
            )
        })?;
    points.extend(halves[0].drain(..).rev());
    points.push(seed_ordinal);
    points.append(&mut halves[1]);
    Ok(points)
}

#[cfg(test)]
mod ranked_tests {
    use super::*;

    /// Protects all unique progressive ranks, signed phase wrapping, nesting and sparse exclusion.
    /// # Panics
    /// Panics if the checked-in screen loses its rank permutation or blue-noise sparse spacing.
    #[test]
    fn ranked_screen_permutation_signed_phase_nesting_and_sparse_quality() {
        let mut ranks = std::collections::BTreeSet::new();
        let mut sparse = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                let q = ranked_blue_noise(x, y);
                assert_eq!(q, ranked_blue_noise(x - 128, y - 64));
                let rank = (q * 4096.0).floor() as u16;
                ranks.insert(rank);
                if rank < 256 {
                    sparse.push((x, y));
                }
            }
        }
        assert_eq!(
            ranks.into_iter().collect::<Vec<_>>(),
            (0..4096).collect::<Vec<u16>>()
        );
        assert_eq!(sparse.len(), 256);
        for (i, &(x, y)) in sparse.iter().enumerate() {
            for &(a, b) in &sparse[i + 1..] {
                let dx = (x - a).abs().min(64 - (x - a).abs());
                let dy = (y - b).abs().min(64 - (y - b).abs());
                assert!(
                    dx * dx + dy * dy >= 8,
                    "sparse sites must retain toroidal exclusion"
                );
            }
        }
        for tone in [0.0, 0.0625, 0.25, 0.5, 0.75, 1.0] {
            let count = (0..64)
                .flat_map(|y| (0..64).map(move |x| (x, y)))
                .filter(|&(x, y)| tone > ranked_blue_noise(x, y))
                .count();
            assert_eq!(count, (tone * 4096.0) as usize);
        }
    }

    /// Keeps independent parallel sampling/compaction in the identical signed row order.
    /// # Panics
    /// Panics if worker count changes screen membership, emission order or endpoint occupancy.
    #[test]
    fn ranked_screen_parallel_workers_and_global_subregion() {
        let bounds = Bounds::new(Point2::new(-256.0, -256.0), Point2::new(256.0, 256.0)).unwrap();
        let render = |workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    generate(
                        toniator_domain::DispersionLayout::RankedBlueNoise,
                        bounds,
                        1.0,
                        0,
                        300_000,
                        &|p| Ok((p.x / 512.0 + 0.5).clamp(0.0, 1.0)),
                        &|_| Ok(1.0),
                        &|| false,
                    )
                    .unwrap()
                })
        };
        assert_eq!(render(1), render(4));
        let sub = Bounds::new(Point2::new(-128.0, -128.0), Point2::new(128.0, 128.0)).unwrap();
        let full = render(1)
            .into_iter()
            .filter(|p| sub.contains(*p))
            .collect::<Vec<_>>();
        let partial = generate(
            toniator_domain::DispersionLayout::RankedBlueNoise,
            sub,
            1.0,
            0,
            300_000,
            &|p| Ok((p.x / 512.0 + 0.5).clamp(0.0, 1.0)),
            &|_| Ok(1.0),
            &|| false,
        )
        .unwrap();
        assert_eq!(full, partial);
    }
}
