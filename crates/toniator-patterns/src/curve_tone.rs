//! Complete logical tone profiles, independent of presentation clipping and source resolution.

use super::*;
use toniator_geometry::{ArcLengthWorkLimits, PathArcLength};

/// A completed source response at one logical arc-length anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneKnot {
    pub distance: f64,
    pub normalized_thickness: f64,
}

/// Full retained source knots and independently refined geometric profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneProfile {
    pub knots: Arc<Vec<ToneKnot>>,
    pub samples: Vec<StrokeProfileSample>,
}

/// Generator-owned continuation at artificial outer group bounds, never inferred from clipping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TonePathContinuation {
    Finite,
    StraightTerminalRays,
    AuthoredTerminalRays,
    RepeatedMotifCopies,
}

/// Logical arc authority for a full centerline; artificial ends use terminal continuation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonePathDomain {
    /// Signed distance of this path's start from its authored/group zero.
    pub start_distance: f64,
    pub genuine_start: bool,
    pub genuine_end: bool,
    pub continuation: TonePathContinuation,
}

impl TonePathDomain {
    /// Declares a complete authored finite path with genuine endpoints.
    pub const fn finite() -> Self {
        Self {
            start_distance: 0.0,
            genuine_start: true,
            genuine_end: true,
            continuation: TonePathContinuation::Finite,
        }
    }
}

/// Samples completed thickness only at the anchored logical lattice and genuine endpoints.
/// Refinement has no source callback and cannot add source knots. Caller policy bounds retained
/// geometric samples; arc preparation uses machine-addressable aggregate admission.
/// # Errors
/// Returns invalid interval/domain, finite/count, allocation, cancellation or fixed numeric errors.
#[allow(clippy::too_many_arguments)]
pub fn prepare_tone_profile_cancellable(
    path: &CurvePath,
    domain: TonePathDomain,
    interval: f64,
    basis: f64,
    maximum_samples: usize,
    sample_completed_thickness: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ToneProfile, PatternPipelineError> {
    let mut profiles = prepare_tone_group_profiles_cancellable(
        &[(path, domain)],
        interval,
        basis,
        maximum_samples,
        sample_completed_thickness,
        cancelled,
    )?;
    profiles.pop().ok_or(PatternPipelineError::new(
        "realization.tone.empty",
        "tone path requires a complete profile",
    ))
}

/// Prepares one lattice over a full ordered group, sharing its knots across every derived piece.
/// Internal piece/segment boundaries reconstruct completed response and never sample artwork.
/// The outer artificial bounds may continue only existing straight terminal rays.
/// # Errors
/// Returns cancellation, interval/trace/numeric, explicit profile policy or fallible storage errors.
#[allow(clippy::too_many_arguments)]
pub fn prepare_tone_group_profiles_cancellable(
    paths: &[(&CurvePath, TonePathDomain)],
    interval: f64,
    basis: f64,
    maximum_samples: usize,
    sample_completed_thickness: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ToneProfile>, PatternPipelineError> {
    prepare_tone_group_profiles_with_continuation_cancellable(
        paths,
        interval,
        basis,
        maximum_samples,
        sample_completed_thickness,
        None,
        cancelled,
    )
}

/// Prepares complete group knots with an explicit generator-owned continuation for artificial ends.
/// # Errors
/// Returns the same atomic tone/numeric/source/allocation diagnostics as the public group API.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_tone_group_profiles_with_continuation_cancellable(
    paths: &[(&CurvePath, TonePathDomain)],
    interval: f64,
    basis: f64,
    maximum_samples: usize,
    sample_completed_thickness: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    continuation: Option<&dyn Fn(f64) -> Result<Point2, PatternPipelineError>>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ToneProfile>, PatternPipelineError> {
    if !(interval.is_finite() && interval > 0.0 && basis.is_finite() && basis > 0.0)
        || paths
            .iter()
            .any(|(_, domain)| !domain.start_distance.is_finite())
    {
        return Err(PatternPipelineError::new(
            "realization.tone.interval",
            "tone distance and spacing must be finite and spacing positive",
        ));
    }
    let Some(&(first_path, first_domain)) = paths.first() else {
        return Ok(Vec::new());
    };
    let mut metrics = Vec::new();
    let mut ends = Vec::<f64>::new();
    for &(path, domain) in paths {
        tone_cancelled(cancelled)?;
        let metric =
            path.measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?;
        let end = domain.start_distance + metric.total_length();
        if !end.is_finite() {
            return Err(PatternPipelineError::new(
                "realization.tone.numeric",
                "logical tone distance must remain finite",
            ));
        }
        if let Some(previous) = ends.last().copied() {
            let tolerance =
                32.0 * f64::EPSILON * end.abs().max(domain.start_distance.abs()).max(1.0);
            if (domain.start_distance - previous).abs() > tolerance {
                return Err(PatternPipelineError::new(
                    "realization.tone.domain",
                    "tone group domains must be ordered and arc-contiguous",
                ));
            }
        }
        reserve_tone(
            &mut ends,
            1,
            "realization.tone.allocation",
            "tone metric allocation failed",
        )?;
        reserve_tone(
            &mut metrics,
            1,
            "realization.tone.allocation",
            "tone metric allocation failed",
        )?;
        ends.push(end);
        metrics.push(metric);
    }
    let end_distance = ends[ends.len() - 1];
    let (_, last_domain) = paths[paths.len() - 1];
    let first = (first_domain.start_distance / interval).floor();
    let last = (end_distance / interval).ceil();
    if first.abs().max(last.abs()) > (1_u64 << 53) as f64 || !first.is_finite() || !last.is_finite()
    {
        return Err(PatternPipelineError::new(
            "realization.tone.ordinal",
            "tone lattice ordinals must remain exactly representable",
        ));
    }
    let mut distances = Vec::new();
    let mut ordinal = first as i64;
    let last = last as i64;
    while ordinal <= last {
        tone_cancelled(cancelled)?;
        let distance = ordinal as f64 * interval;
        if (distance >= first_domain.start_distance || !first_domain.genuine_start)
            && (distance <= end_distance || !last_domain.genuine_end)
        {
            reserve_tone(
                &mut distances,
                1,
                "realization.tone.allocation",
                "tone anchor allocation failed",
            )?;
            distances.push(distance);
        }
        ordinal = ordinal.checked_add(1).ok_or(PatternPipelineError::new(
            "realization.tone.count",
            "tone ordinal count overflows",
        ))?;
    }
    reserve_tone(
        &mut distances,
        2,
        "realization.tone.allocation",
        "tone endpoint allocation failed",
    )?;
    if first_domain.genuine_start {
        distances.push(first_domain.start_distance);
    }
    if last_domain.genuine_end {
        distances.push(end_distance);
    }
    distances.sort_unstable_by(f64::total_cmp);
    distances.dedup_by(|a, b| *a == *b);
    let mut knots = Vec::<ToneKnot>::new();
    let mut resolved_locations = Vec::new();
    let last_path = paths[paths.len() - 1].0;
    for distance in distances {
        tone_cancelled(cancelled)?;
        let mut resolved = None;
        let point = if distance < first_domain.start_distance || distance > end_distance {
            if let Some(continuation) = continuation {
                continuation(distance)?
            } else {
                let (endpoint, segment, parameter, excess) =
                    if distance < first_domain.start_distance {
                        (
                            first_path.start(),
                            &first_path.segments()[0],
                            0.0,
                            distance - first_domain.start_distance,
                        )
                    } else {
                        (
                            last_path.end(),
                            &last_path.segments()[last_path.segments().len() - 1],
                            1.0,
                            distance - end_distance,
                        )
                    };
                let policy = if distance < first_domain.start_distance {
                    first_domain.continuation
                } else {
                    last_domain.continuation
                };
                let authorized = match policy {
                    TonePathContinuation::StraightTerminalRays => {
                        matches!(segment, CurveSegment::Line(_))
                    }
                    TonePathContinuation::AuthoredTerminalRays => true,
                    TonePathContinuation::Finite | TonePathContinuation::RepeatedMotifCopies => {
                        false
                    }
                };
                if !authorized {
                    return Err(PatternPipelineError::new(
                        "realization.tone.continuation",
                        "artificial group ends require explicit generator continuation authority",
                    ));
                }
                let tangent = segment.limiting_unit_tangent_at(parameter)?;
                Point2::new(
                    endpoint.x + tangent.x * excess,
                    endpoint.y + tangent.y * excess,
                )
            }
        } else {
            let index = ends
                .partition_point(|end| *end < distance)
                .min(paths.len() - 1);
            let (path, domain) = paths[index];
            let local =
                (distance - domain.start_distance).clamp(0.0, metrics[index].total_length());
            let location = metrics[index].location_at_length_cancellable(local, cancelled)?;
            resolved = Some((index, location));
            path.segments()[location.segment_index()].point_at(location.parameter())?
        };
        if !point.is_finite() {
            return Err(PatternPipelineError::new(
                "realization.tone.numeric",
                "tone source anchors must remain finite",
            ));
        }
        let thickness = if paths.len() == 1
            && first_path.closure() == PathClosure::Closed
            && distance == end_distance
            && first_domain.genuine_start
            && last_domain.genuine_end
            && first_path.start() == last_path.end()
        {
            match knots.first() {
                Some(knot) => knot.normalized_thickness,
                None => sample_completed_thickness(point)?,
            }
        } else {
            sample_completed_thickness(point)?
        };
        if !thickness.is_finite() || thickness < 0.0 {
            return Err(PatternPipelineError::new(
                "realization.tone.response",
                "completed tone responses must be finite and nonnegative",
            ));
        }
        reserve_tone(
            &mut knots,
            1,
            "realization.tone.allocation",
            "tone knot allocation failed",
        )?;
        reserve_tone(
            &mut resolved_locations,
            1,
            "realization.tone.allocation",
            "tone resolved-location allocation failed",
        )?;
        resolved_locations.push(resolved);
        knots.push(ToneKnot {
            distance,
            normalized_thickness: thickness,
        });
    }
    let knots = Arc::new(knots);
    let mut profiles = Vec::new();
    let mut total = 0;
    for (piece_index, ((path, domain), metric)) in paths.iter().copied().zip(&metrics).enumerate() {
        let length = metric.total_length();
        let mut knot_locations = Vec::new();
        let lo = knots.partition_point(|knot| knot.distance < domain.start_distance);
        let hi = knots.partition_point(|knot| knot.distance <= domain.start_distance + length);
        let mut exact_anchors = Vec::new();
        for index in lo..hi {
            let Some((piece, location)) = resolved_locations[index] else {
                continue;
            };
            if piece != piece_index {
                continue;
            }
            let thickness = knots[index].normalized_thickness;
            append_exact_anchor(&mut exact_anchors, location, thickness)?;
            if location.parameter() == 1.0 && location.segment_index() + 1 < path.segments().len() {
                append_exact_anchor(
                    &mut exact_anchors,
                    PathLocation::new(location.segment_index() + 1, 0.0)?,
                    thickness,
                )?;
            }
            if location.parameter() == 0.0 && location.segment_index() > 0 {
                append_exact_anchor(
                    &mut exact_anchors,
                    PathLocation::new(location.segment_index() - 1, 1.0)?,
                    thickness,
                )?;
            }
            if location.parameter() > 0.0 && location.parameter() < 1.0 {
                reserve_tone(
                    &mut knot_locations,
                    1,
                    "realization.tone.allocation",
                    "tone location allocation failed",
                )?;
                knot_locations.push(location);
            }
        }
        exact_anchors.sort_unstable_by(|a, b| compare_tone_locations(a.0, b.0));
        exact_anchors.dedup_by(|a, b| a.0 == b.0);
        let mut next_knot = 0;
        let mut samples = Vec::new();
        for (index, segment) in path.segments().iter().enumerate() {
            tone_cancelled(cancelled)?;
            let left = reconstructed_sample(
                segment,
                index,
                0.0,
                metric,
                domain,
                &knots,
                &exact_anchors,
                basis,
                cancelled,
            )?;
            append_stroke_sample(&mut samples, &mut total, maximum_samples, left, cancelled)?;
            let mut previous = 0.0;
            while next_knot < knot_locations.len()
                && knot_locations[next_knot].segment_index() == index
            {
                let t = knot_locations[next_knot].parameter();
                next_knot += 1;
                if t > previous && t < 1.0 {
                    refine_tone_interval(
                        segment,
                        index,
                        previous,
                        t,
                        0,
                        metric,
                        domain,
                        &knots,
                        &exact_anchors,
                        basis,
                        &mut samples,
                        &mut total,
                        maximum_samples,
                        cancelled,
                    )?;
                    previous = t;
                }
            }
            refine_tone_interval(
                segment,
                index,
                previous,
                1.0,
                0,
                metric,
                domain,
                &knots,
                &exact_anchors,
                basis,
                &mut samples,
                &mut total,
                maximum_samples,
                cancelled,
            )?;
        }
        reserve_tone(
            &mut profiles,
            1,
            "realization.tone.allocation",
            "tone profile allocation failed",
        )?;
        profiles.push(ToneProfile {
            knots: Arc::clone(&knots),
            samples,
        });
    }
    Ok(profiles)
}

/// Checks the cancellation boundary before source/numeric/retained-profile work.
/// # Errors
/// Returns the existing atomic evaluation cancellation diagnostic.
fn tone_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), PatternPipelineError> {
    if cancelled() {
        Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "evaluation was cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Reconstructs completed thickness linearly in logical arc distance without artwork access.
/// # Errors
/// Returns fixed numeric metric/location diagnostics or cancellation.
#[allow(clippy::too_many_arguments)]
fn reconstructed_sample(
    segment: &CurveSegment,
    index: usize,
    parameter: f64,
    metric: &PathArcLength,
    domain: TonePathDomain,
    knots: &[ToneKnot],
    exact_anchors: &[(PathLocation, f64)],
    basis: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<StrokeProfileSample, PatternPipelineError> {
    let location = PathLocation::new(index, parameter)?;
    let distance =
        domain.start_distance + metric.length_at_location_cancellable(location, cancelled)?;
    let right = knots.partition_point(|knot| knot.distance < distance);
    let reconstructed = if right == 0 {
        knots[0].normalized_thickness
    } else if right == knots.len() {
        knots[right - 1].normalized_thickness
    } else {
        let a = knots[right - 1];
        let b = knots[right];
        let t = ((distance - a.distance) / (b.distance - a.distance)).clamp(0.0, 1.0);
        a.normalized_thickness + (b.normalized_thickness - a.normalized_thickness) * t
    };
    let thickness = exact_anchors
        .binary_search_by(|anchor| compare_tone_locations(anchor.0, location))
        .ok()
        .map_or(reconstructed, |index| exact_anchors[index].1);
    Ok(StrokeProfileSample {
        location,
        center: segment.point_at(parameter)?,
        normalized_thickness: thickness,
        width: thickness * basis,
    })
}

/// Refines only analytic shape and reconstructed width; source sampling is impossible here.
/// # Errors
/// Returns cancellation, fixed depth/accuracy, numeric or caller profile policy diagnostics.
#[allow(clippy::too_many_arguments)]
fn refine_tone_interval(
    segment: &CurveSegment,
    index: usize,
    a: f64,
    b: f64,
    depth: u8,
    metric: &PathArcLength,
    domain: TonePathDomain,
    knots: &[ToneKnot],
    exact_anchors: &[(PathLocation, f64)],
    basis: f64,
    samples: &mut Vec<StrokeProfileSample>,
    total: &mut usize,
    maximum: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PatternPipelineError> {
    tone_cancelled(cancelled)?;
    let left = reconstructed_sample(
        segment,
        index,
        a,
        metric,
        domain,
        knots,
        exact_anchors,
        basis,
        cancelled,
    )?;
    let right = reconstructed_sample(
        segment,
        index,
        b,
        metric,
        domain,
        knots,
        exact_anchors,
        basis,
        cancelled,
    )?;
    let mid = (a + b) * 0.5;
    let middle = reconstructed_sample(
        segment,
        index,
        mid,
        metric,
        domain,
        knots,
        exact_anchors,
        basis,
        cancelled,
    )?;
    let center_error = (middle.center.x - (left.center.x + right.center.x) * 0.5)
        .hypot(middle.center.y - (left.center.y + right.center.y) * 0.5)
        .max(cubic_subcurve_flatness(segment, a, b));
    let response_error = (middle.normalized_thickness
        - (left.normalized_thickness + right.normalized_thickness) * 0.5)
        .abs();
    if center_error > STROKE_CENTERLINE_TOLERANCE || response_error > STROKE_RESPONSE_TOLERANCE {
        if depth >= MAX_STROKE_SUBDIVISION_DEPTH {
            return Err(PatternPipelineError::new(
                "realization.stroke.subdivision_depth",
                "reconstructed tone exceeds geometric subdivision depth",
            ));
        }
        refine_tone_interval(
            segment,
            index,
            a,
            mid,
            depth + 1,
            metric,
            domain,
            knots,
            exact_anchors,
            basis,
            samples,
            total,
            maximum,
            cancelled,
        )?;
        refine_tone_interval(
            segment,
            index,
            mid,
            b,
            depth + 1,
            metric,
            domain,
            knots,
            exact_anchors,
            basis,
            samples,
            total,
            maximum,
            cancelled,
        )
    } else {
        append_stroke_sample(samples, total, maximum, right, cancelled)
    }
}

/// Resolves Feature-relative reference distance from the complete density authority.
/// # Errors
/// Returns the same density/finite-positive distance diagnostics as family preparation.
pub(super) fn tone_reference_interval(
    canvas: &CanvasSpec,
    density: &ResolvedDensityMetric2D,
) -> Result<f64, PatternPipelineError> {
    scale_feature_length(
        canvas.width.max(canvas.height) / 100.0,
        resolved_pattern_size_scale(canvas, density)?,
        "realization.tone.interval",
        "tone interval must remain finite and positive",
    )
}

/// Builds exact identity source spans for an unmodified full curve.
/// # Errors
/// Returns fallible trace allocation or location diagnostics.
pub(super) fn identity_source_intervals(
    path: &CurvePath,
) -> Result<Vec<(PathLocation, PathLocation)>, PatternPipelineError> {
    let mut trace = Vec::new();
    reserve_tone(
        &mut trace,
        path.segments().len(),
        "realization.tone.allocation",
        "source trace allocation failed",
    )?;
    for index in 0..path.segments().len() {
        trace.push((
            PathLocation::new(index, 0.0)?,
            PathLocation::new(index, 1.0)?,
        ));
    }
    Ok(trace)
}

/// Composes a derived location with retained original-source intervals, never geometric proximity.
/// # Errors
/// Returns invalid trace/location diagnostics instead of inventing a source origin.
pub fn compose_source_location(
    location: PathLocation,
    trace: &[(PathLocation, PathLocation)],
) -> Result<PathLocation, PatternPipelineError> {
    let &(a, b) = trace
        .get(location.segment_index())
        .ok_or(PatternPipelineError::new(
            "coverage.curved_guides.source_trace",
            "offset source location has no retained trace",
        ))?;
    if original_source_key(a) == original_source_key(b) {
        return Ok(a);
    }
    if a.segment_index() != b.segment_index() {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.source_trace",
            "offset source trace must stay within one original segment",
        ));
    }
    Ok(PathLocation::new(
        a.segment_index(),
        a.parameter() + (b.parameter() - a.parameter()) * location.parameter(),
    )?)
}

/// Assigns complete component-group arc distances around the transported authored origin.
/// A removed source span collapses to its first surviving relink boundary in source traversal order;
/// disconnected pieces accumulate retained arc lengths and never restart their lattice.
/// Paths already carry the final canonical transform. Consecutive starts use the same
/// world-metric addition as the consumer, avoiding independent prefix subtraction at joins.
/// # Errors
/// Returns cancellation, fixed numeric/trace or fallible storage diagnostics atomically.
pub fn offset_group_tone_domains(
    paths: &[toniator_geometry::OffsetPathComponent],
    zero: PathLocation,
    genuine: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<TonePathDomain>, PatternPipelineError> {
    let key = original_source_key;
    let mut lengths = Vec::new();
    let mut total = 0.0;
    let mut anchor = None;
    for component in paths {
        tone_cancelled(cancelled)?;
        reserve_tone(
            &mut lengths,
            1,
            "realization.tone.allocation",
            "logical origin allocation failed",
        )?;
        let metric = component
            .path
            .measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?;
        if component.source_intervals.len() != component.path.segments().len() {
            return Err(PatternPipelineError::new(
                "coverage.curved_guides.source_trace",
                "every full derived segment requires source correspondence",
            ));
        }
        if anchor.is_none() {
            for (index, &(a, b)) in component.source_intervals.iter().enumerate() {
                let (ak, bk, z) = (key(a), key(b), key(zero));
                if ak.max(bk) < z {
                    continue;
                }
                let parameter = if z == bk && ak != bk {
                    1.0
                } else if ak.min(bk) <= z
                    && ak != bk
                    && a.segment_index() == b.segment_index()
                    && zero.segment_index() == a.segment_index()
                {
                    ((zero.parameter() - a.parameter()) / (b.parameter() - a.parameter()))
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                anchor = Some(
                    total
                        + metric.length_at_location_cancellable(
                            PathLocation::new(index, parameter)?,
                            cancelled,
                        )?,
                );
                break;
            }
        }
        lengths.push(metric.total_length());
        total += metric.total_length();
        if !total.is_finite() {
            return Err(PatternPipelineError::new(
                "realization.tone.numeric",
                "group arc distances must remain finite",
            ));
        }
    }
    let anchor = anchor.unwrap_or(total);
    let mut domains = Vec::new();
    reserve_tone(
        &mut domains,
        paths.len(),
        "realization.tone.allocation",
        "logical domain allocation failed",
    )?;
    let mut start_distance = -anchor;
    for (index, length) in lengths.into_iter().enumerate() {
        domains.push(TonePathDomain {
            start_distance,
            genuine_start: genuine && index == 0,
            genuine_end: genuine && index + 1 == paths.len(),
            continuation: if genuine {
                TonePathContinuation::Finite
            } else {
                TonePathContinuation::AuthoredTerminalRays
            },
        });
        start_distance += length;
        if !start_distance.is_finite() {
            return Err(PatternPipelineError::new(
                "realization.tone.numeric",
                "group arc distances must remain finite",
            ));
        }
    }
    Ok(domains)
}

/// Derives full straight-guide domains from the stable guide anchor and tangent.
/// # Errors
/// Returns missing basis/anchor or fallible logical-path allocation diagnostics.
pub(super) fn straight_logical_tone_paths(
    paths: &[StructuralPathInstance],
    guides: &[StraightGuide],
    bases: &BTreeMap<StructuralPathInstanceId, f64>,
) -> Result<Vec<LogicalTonePath>, PatternPipelineError> {
    let mut logical = Vec::new();
    for (path, guide) in paths.iter().zip(guides) {
        let start_distance = Vector2::new(
            path.path.start().x - guide.anchor.x,
            path.path.start().y - guide.anchor.y,
        )
        .dot(guide.tangent);
        let nominal_basis = *bases.get(&path.id).ok_or(PatternPipelineError::new(
            "realization.stroke.basis",
            "guide requires its own nominal basis",
        ))?;
        reserve_tone(
            &mut logical,
            1,
            "realization.tone.allocation",
            "logical straight path allocation failed",
        )?;
        logical.push(LogicalTonePath {
            instance: path.clone(),
            domain: TonePathDomain {
                start_distance,
                genuine_start: false,
                genuine_end: false,
                continuation: TonePathContinuation::StraightTerminalRays,
            },
            nominal_basis,
        });
    }
    Ok(logical)
}

/// Returns actual repeated endpoint motif-copy positions, preserving its analytic line/cubic shape.
/// This continuation is distinct from a tangent ray: repeated copies translate by their exact chord.
/// # Errors
/// Returns finite/copy/source metric diagnostics or cancellation without constructing distant copies.
pub(super) fn motif_tone_continuation<'a>(
    path: &'a CurvePath,
    count: usize,
    domain: TonePathDomain,
    cancelled: &'a dyn Fn() -> bool,
) -> Result<impl Fn(f64) -> Result<Point2, PatternPipelineError> + 'a, PatternPipelineError> {
    if count == 0 || count > path.segments().len() {
        return Err(PatternPipelineError::new(
            "curve_motif.cadence",
            "motif requires complete endpoint copies",
        ));
    }
    let mut first_segments = Vec::new();
    let mut last_segments = Vec::new();
    reserve_tone(
        &mut first_segments,
        count,
        "realization.tone.allocation",
        "motif continuation allocation failed",
    )?;
    reserve_tone(
        &mut last_segments,
        count,
        "realization.tone.allocation",
        "motif continuation allocation failed",
    )?;
    first_segments.extend_from_slice(&path.segments()[..count]);
    last_segments.extend_from_slice(&path.segments()[path.segments().len() - count..]);
    let first = CurvePath::new_with_limits_cancellable(
        first_segments,
        PathClosure::Open,
        CurveWorkLimits::default(),
        cancelled,
    )?;
    let last = CurvePath::new_with_limits_cancellable(
        last_segments,
        PathClosure::Open,
        CurveWorkLimits::default(),
        cancelled,
    )?;
    let first_metric =
        first.measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?;
    let last_metric =
        last.measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?;
    let full_length = path
        .measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?
        .total_length();
    Ok(move |distance: f64| {
        tone_cancelled(cancelled)?;
        let before = distance < domain.start_distance;
        let (copy, metric, relative) = if before {
            (&first, &first_metric, distance - domain.start_distance)
        } else {
            (
                &last,
                &last_metric,
                distance - domain.start_distance - full_length + last_metric.total_length(),
            )
        };
        let length = metric.total_length();
        if !(length.is_finite() && length > 0.0) {
            return Err(PatternPipelineError::new(
                "curve_motif.cadence",
                "motif copy arc must remain finite and positive",
            ));
        }
        let ordinal = (relative / length).floor();
        let local = (relative - ordinal * length).clamp(0.0, length);
        let location = metric.location_at_length_cancellable(local, cancelled)?;
        let point = copy.point_at(location)?;
        let chord = Vector2::new(copy.end().x - copy.start().x, copy.end().y - copy.start().y);
        let result = Point2::new(point.x + chord.x * ordinal, point.y + chord.y * ordinal);
        if result.is_finite() {
            Ok(result)
        } else {
            Err(PatternPipelineError::new(
                "curve_motif.numeric",
                "motif continuation must remain finite",
            ))
        }
    })
}

/// Chains complete motif placement and transports the underlying authored origin into motif arc.
/// A source position between adjacent sites maps linearly into that copy's measured motif arc;
/// a deleted source span collapses to the first surviving row boundary. Complete retained row
/// lengths accumulate across derived components, independently of presentation clipping.
/// # Errors
/// Returns existing chaining, source-trace, numeric, allocation or cancellation diagnostics.
pub fn prepare_curve_motif_tone_rows_cancellable(
    family: &TypedFamilyOutput,
    motif: &CurvePath,
    mirror_alternate_rows: bool,
    alternate_row_phase: Option<f64>,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<CurveMotifToneRow>, PatternPipelineError> {
    let sites = family.logical_motif_site_set();
    let paths = chain_curve_motif_rows_with_progress_cancellable(
        sites,
        motif,
        mirror_alternate_rows,
        alternate_row_phase,
        cancelled,
        progress,
    )?;
    let mut records = Vec::new();
    let mut logical = Vec::new();
    reserve_tone(
        &mut logical,
        family.logical_tone_paths().len(),
        "realization.tone.allocation",
        "motif origin index allocation failed",
    )?;
    logical.extend(
        family
            .logical_tone_paths()
            .iter()
            .map(|path| (path.instance.id, path)),
    );
    logical.sort_unstable_by_key(|entry| entry.0);
    for site in sites.sites() {
        tone_cancelled(cancelled)?;
        let (guide, curved, sequence, position) = match site.provenance {
            FamilySiteProvenance::AlongGuide {
                guide_id, sequence, ..
            } => (guide_id, false, sequence, sequence as f64),
            FamilySiteProvenance::CurveAlongGuide {
                location,
                sequence,
                local_arc_position_bits,
                ..
            } => {
                let path = logical
                    .binary_search_by_key(&location.path, |entry| entry.0)
                    .ok()
                    .map(|index| logical[index].1)
                    .ok_or(PatternPipelineError::new(
                        "curve_motif.source_trace",
                        "motif placement requires its full logical guide",
                    ))?;
                (
                    location
                        .path
                        .guide_instance()
                        .ok_or(PatternPipelineError::new(
                            "curve_motif.source_trace",
                            "motif guide identity must be retained",
                        ))?,
                    true,
                    sequence,
                    path.domain.start_distance + f64::from_bits(local_arc_position_bits),
                )
            }
            _ => {
                return Err(PatternPipelineError::new(
                    "curve_motif.sites",
                    "motif requires along-guide placement",
                ));
            }
        };
        reserve_tone(
            &mut records,
            1,
            "realization.tone.allocation",
            "motif origin positions allocation failed",
        )?;
        records.push((guide, curved, sequence, position));
    }
    records.sort_unstable_by_key(|record| (record.0, record.1, record.2));
    let mut specs = Vec::new();
    let mut start = 0;
    while start < records.len() {
        let guide = records[start].0;
        let curved = records[start].1;
        let end = start
            + records[start..]
                .iter()
                .take_while(|record| record.0 == guide && record.1 == curved)
                .count();
        let mut run: Vec<(i64, f64)> = Vec::new();
        for &(_, _, sequence, position) in &records[start..end] {
            if run
                .last()
                .is_some_and(|previous| sequence.checked_sub(previous.0) != Some(1))
            {
                if run.len() > 1 {
                    reserve_tone(
                        &mut specs,
                        1,
                        "realization.tone.allocation",
                        "motif origin runs allocation failed",
                    )?;
                    specs.push((guide, curved, std::mem::take(&mut run)));
                } else {
                    run.clear();
                }
            }
            reserve_tone(
                &mut run,
                1,
                "realization.tone.allocation",
                "motif origin run allocation failed",
            )?;
            run.push((sequence, position));
        }
        if run.len() > 1 {
            reserve_tone(
                &mut specs,
                1,
                "realization.tone.allocation",
                "motif origin runs allocation failed",
            )?;
            specs.push((guide, curved, run));
        }
        start = end;
    }
    if paths.len() != specs.len() {
        return Err(PatternPipelineError::new(
            "curve_motif.source_trace",
            "motif rows and full placement must agree",
        ));
    }
    let mut rows = Vec::new();
    let count = motif.segments().len();
    let mut group_start = 0;
    let mut active = None;
    let mut group_length = 0.0;
    let mut group_anchor = None;
    for (path, (guide, curved, positions)) in paths.into_iter().zip(specs) {
        tone_cancelled(cancelled)?;
        let key = (guide.dimension_id, guide.index);
        if active.is_some_and(|previous| previous != key) {
            finish_motif_group(
                &mut rows[group_start..],
                group_anchor.unwrap_or(group_length),
            );
            group_start = rows.len();
            group_length = 0.0;
            group_anchor = None;
        }
        active = Some(key);
        let metric =
            path.measure_arc_length_cancellable(ArcLengthWorkLimits::default(), cancelled)?;
        if group_anchor.is_none() {
            for (copy, pair) in positions.windows(2).enumerate() {
                let a = pair[0].1;
                let b = pair[1].1;
                if b < 0.0 {
                    continue;
                }
                let left = metric.length_at_location_cancellable(
                    PathLocation::new(copy * count, 0.0)?,
                    cancelled,
                )?;
                let right = metric.length_at_location_cancellable(
                    PathLocation::new((copy + 1) * count - 1, 1.0)?,
                    cancelled,
                )?;
                let fraction = if a < 0.0 && b > a {
                    (-a / (b - a)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                group_anchor = Some(group_length + left + (right - left) * fraction);
                break;
            }
        }
        let (genuine_start, genuine_end) = if curved {
            let full = family
                .logical_tone_paths()
                .iter()
                .find(|logical| logical.instance.id.guide_instance() == Some(guide))
                .ok_or(PatternPipelineError::new(
                    "curve_motif.source_trace",
                    "motif requires a retained logical guide component",
                ))?;
            (full.domain.genuine_start, full.domain.genuine_end)
        } else {
            (false, false)
        };
        reserve_tone(
            &mut rows,
            1,
            "realization.tone.allocation",
            "logical motif rows allocation failed",
        )?;
        rows.push(CurveMotifToneRow {
            path,
            domain: TonePathDomain {
                start_distance: group_length,
                genuine_start,
                genuine_end,
                continuation: TonePathContinuation::RepeatedMotifCopies,
            },
            motif_segment_count: count,
            source_guide: Some(guide),
        });
        group_length += metric.total_length();
    }
    if group_start < rows.len() {
        finish_motif_group(
            &mut rows[group_start..],
            group_anchor.unwrap_or(group_length),
        );
    }
    Ok(rows)
}

/// Commits one already-measured row group's single origin without restarting derived pieces.
fn finish_motif_group(rows: &mut [CurveMotifToneRow], anchor: f64) {
    for row in rows {
        row.domain.start_distance -= anchor;
    }
}

/// Reserves new tone-route storage after checking address/count arithmetic separately.
/// # Errors
/// Returns count_overflow for unaddressable storage and only reports allocation on actual reserve refusal.
pub(super) fn reserve_tone<T>(
    values: &mut Vec<T>,
    additional: usize,
    path: &'static str,
    message: &'static str,
) -> Result<(), PatternPipelineError> {
    values
        .len()
        .checked_add(additional)
        .and_then(|count| count.checked_mul(std::mem::size_of::<T>()))
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(PatternPipelineError::new(
            "realization.tone.count_overflow",
            "tone storage exceeds machine addressability",
        ))?;
    #[cfg(test)]
    if values.capacity().saturating_sub(values.len()) < additional
        && FAIL_CURVE_RESERVATION.with(|flag| flag.replace(false))
    {
        return Err(PatternPipelineError::new(path, message));
    }
    values
        .try_reserve(additional)
        .map_err(|_| PatternPipelineError::new(path, message))
}

/// Orders exact stored path coordinates without proximity-based snapping.
fn compare_tone_locations(a: PathLocation, b: PathLocation) -> std::cmp::Ordering {
    a.segment_index()
        .cmp(&b.segment_index())
        .then_with(|| a.parameter().total_cmp(&b.parameter()))
}

/// Retains one exact source value at a known path location or its genuine segment-joint alias.
/// # Errors
/// Returns only checked count or actual reserve failure before publication.
fn append_exact_anchor(
    anchors: &mut Vec<(PathLocation, f64)>,
    location: PathLocation,
    thickness: f64,
) -> Result<(), PatternPipelineError> {
    reserve_tone(
        anchors,
        1,
        "realization.tone.allocation",
        "exact tone anchor allocation failed",
    )?;
    anchors.push((location, thickness));
    Ok(())
}

/// Orders original source coordinates exactly while identifying t=1 and next-segment t=0 boundaries.
/// Segment ordinals stay integers instead of losing their low bits in a floating global parameter.
fn original_source_key(location: PathLocation) -> (usize, u64) {
    if location.parameter() == 1.0
        && let Some(index) = location.segment_index().checked_add(1)
    {
        (index, 0)
    } else {
        (location.segment_index(), location.parameter().to_bits())
    }
}
