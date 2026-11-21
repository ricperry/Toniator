//! Constant-width interleaved structural strokes through the canonical outline authority.
use super::*;

/// Realizes source-clipped carrier intervals while retaining exact line/cubic centerlines.
/// Eight bit-reversed global repetition classes add spatially interleaved strokes as ink rises.
/// Width is feature-relative and constant: source tone changes membership and length only.
/// # Errors
/// Returns source, cancellation, allocation, profile or outline budget diagnostics atomically.
#[allow(clippy::too_many_arguments)]
pub(super) fn realize(
    family: &TypedFamilyOutput,
    plan: &PatternPipelinePlan,
    source: &SourceField,
    canvas: &CanvasSpec,
    mapping: SourceMapping,
    response: StrokeResponse,
    max_profile_samples: usize,
    max_outline_segments: usize,
    input: GeometryResponseInput<'_>,
    cancel: &dyn Fn() -> bool,
) -> Result<TypedRealization<CanonicalStrokeRealization>, PatternPipelineError> {
    let provenance = realization_provenance(family, plan)?;
    let output = &plan.ordered_outputs[0];
    let (_, style) = output.guide_paths().ok_or(PatternPipelineError::new(
        "pattern.hatch.output",
        "hatching requires structural paths",
    ))?;
    let hatch = style.hatch.ok_or(PatternPipelineError::new(
        "pattern.hatch.output",
        "hatching requires an explicit fixed-width policy",
    ))?;
    hatch
        .validate()
        .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?;
    let mut strokes = Vec::new();
    let mut samples_used = 0usize;
    let mut segments_used = 0usize;
    // Resolve tone once at each sampled carrier position; probes bypass source and cutoff.
    let sample = |point| {
        let ink = match input {
            GeometryResponseInput::RawProbe(size) => return Ok(size / 2.0),
            GeometryResponseInput::ConstantProbe(q) => return Ok(q),
            _ => source
                .sample_mapping_response(point, canvas, mapping)
                .map_err(|e| PatternPipelineError::new(e.path(), e.message()))?,
        };
        if mapping.suppresses(ink) {
            return Ok(0.0);
        }
        match input {
            GeometryResponseInput::AbsoluteCoverage {
                curve,
                minimum,
                maximum,
            } => {
                let desired = minimum + ink * (maximum - minimum);
                // Equal error keeps the lower population; interpolation never invents width.
                let mut best = &curve.knots()[0];
                for knot in &curve.knots()[1..] {
                    if (knot.coverage - desired).abs() < (best.coverage - desired).abs() {
                        best = knot;
                    }
                }
                Ok(best.response)
            }
            _ => {
                input.resolve_raw_size(ink, response.minimum_thickness, response.maximum_thickness)
            }
        }
    };
    for logical in family.logical_tone_paths() {
        check_cancel(cancel)?;
        let circle = circular_carrier(family, logical);
        let threshold = threshold(logical.instance.id.repetition_index);
        let width = logical.nominal_basis * hatch.width;
        let source_step = (canvas.width / f64::from(source.identity().width))
            .min(canvas.height / f64::from(source.identity().height))
            * 0.5;
        let step = family
            .tone_sample_interval(output.tone_sample_spacing().unwrap())?
            .min(width * 0.5)
            .min(source_step)
            .max(1.0 / 4096.0);
        let forced = match input {
            GeometryResponseInput::RawProbe(size) => Some(size / 2.0),
            GeometryResponseInput::ConstantProbe(q) => Some(q),
            _ => None,
        };
        let fragments = if let Some(q) = forced {
            if q > threshold {
                vec![logical.instance.path.try_clone_cancellable(cancel)?]
            } else {
                Vec::new()
            }
        } else {
            clipped_intervals(
                &logical.instance.path,
                step,
                threshold,
                &sample,
                &mut samples_used,
                max_profile_samples,
                cancel,
            )?
        };
        for path in fragments {
            check_cancel(cancel)?;
            // Reuse the ordinary source-free geometric refinement: endpoint-only cubic
            // profiles would replace quarter circles with chords in the outline authority.
            let profile = prepare_tone_profile_cancellable(
                &path,
                TonePathDomain::finite(),
                logical.nominal_basis,
                logical.nominal_basis,
                max_profile_samples.saturating_sub(samples_used),
                &|_| Ok(hatch.width),
                cancel,
            )?
            .samples;
            samples_used =
                samples_used
                    .checked_add(profile.len())
                    .ok_or(PatternPipelineError::new(
                        "realization.hatch.profile_limit",
                        "hatch profile accounting overflows",
                    ))?;
            let tolerance = (width * 0.02).min(1.0 / 8.0);
            let limits = VariableWidthOutlineLimits::new(
                max_outline_segments.saturating_sub(segments_used).max(1),
            )?;
            let curved = match circle {
                Some((center, radius, winding)) => {
                    toniator_geometry::build_circular_stroke_outline_cancellable(
                        &path,
                        center,
                        radius,
                        winding,
                        width,
                        response.bias,
                        tolerance,
                        limits,
                        cancel,
                    )?
                }
                None => None,
            };
            let outline = if let Some(outline) = curved {
                outline
            } else {
                let widths = profile
                    .iter()
                    .map(|p| VariableWidthPathSample {
                        location: p.location,
                        width: p.width,
                    })
                    .collect::<Vec<_>>();
                build_variable_width_outline_cancellable(
                    &path,
                    &widths,
                    style,
                    response.bias,
                    tolerance,
                    limits,
                    cancel,
                )?
            };
            segments_used = segments_used
                .checked_add(outline.logical_segment_count()?)
                .ok_or(PatternPipelineError::new(
                    "realization.hatch.outline_limit",
                    "hatch outline accounting overflows",
                ))?;
            if segments_used > max_outline_segments {
                return Err(PatternPipelineError::new(
                    "realization.hatch.outline_limit",
                    "hatch outline exceeds the configured budget",
                ));
            }
            reserve_tone(
                &mut strokes,
                1,
                "realization.hatch.allocation",
                "hatch stroke allocation failed",
            )?;
            strokes.push(CanonicalStroke::new(
                logical.instance.id,
                logical.instance.source_structure_id,
                path,
                logical.nominal_basis,
                style,
                profile,
                outline,
            )?);
        }
    }
    let mut identity = family.family_fingerprint().as_bytes().to_vec();
    identity.extend(b"|interleaved-hatch-v3|");
    if strokes
        .iter()
        .any(|stroke| stroke.outline.has_nonnegative_winding_certificate())
    {
        identity.extend(b"|certified-radial-rails-v1|");
    }
    identity.extend(response.minimum_thickness.to_bits().to_le_bytes());
    identity.extend(response.maximum_thickness.to_bits().to_le_bytes());
    identity.extend(response.bias.to_bits().to_le_bytes());
    append_output_capability_identity(&mut identity, output);
    append_source_identity(&mut identity, source);
    append_source_mapping_identity(&mut identity, mapping);
    for stroke in &strokes {
        append_curve_path_identity(&mut identity, &stroke.path);
        identity.extend(stroke.nominal_basis.to_bits().to_le_bytes());
        identity.extend(stroke.style.hatch.unwrap().width.to_bits().to_le_bytes());
    }
    Ok(TypedRealization {
        provenance,
        output: CanonicalStrokeRealization {
            family_fingerprint: family.family_fingerprint().to_owned(),
            realization_fingerprint: fnv1a64(identity),
            source_identity: source.identity().clone(),
            response,
            strokes,
        },
    })
}

/// Returns midpoint thresholds in bit-reversed repetition order, including negative indices.
fn threshold(index: i64) -> f64 {
    let value = index.rem_euclid(8) as u32;
    let rank = ((value & 1) << 2) | (value & 2) | ((value & 4) >> 2);
    (f64::from(rank) + 0.5) / 8.0
}

/// Polls cancellation at all bounded sample/refinement and materialization boundaries.
/// # Errors
/// Returns cancellation without retaining partial geometry.
fn check_cancel(cancel: &dyn Fn() -> bool) -> Result<(), PatternPipelineError> {
    if cancel() {
        Err(PatternPipelineError::new(
            "evaluation.cancelled",
            "hatching was cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Charges every source query, including threshold bisection, against the caller's profile budget.
/// # Errors
/// Returns cancellation, sampling, overflow or the current profile-budget diagnostic.
fn query(
    point: Point2,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    used: &mut usize,
    limit: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<f64, PatternPipelineError> {
    check_cancel(cancel)?;
    *used = used.checked_add(1).ok_or(PatternPipelineError::new(
        "realization.hatch.profile_limit",
        "hatch sample accounting overflows",
    ))?;
    if *used > limit {
        return Err(PatternPipelineError::new(
            "realization.hatch.profile_limit",
            "hatch sampling exceeds the configured profile budget",
        ));
    }
    sample(point)
}

/// Splits continuous carrier intervals at refined source-threshold crossings.
/// Full closed carriers remain closed; partial seam intervals join before round caps are built.
/// Subpixel source quadrature bounds detail discovery; it is not analytic contour extraction.
/// # Errors
/// Returns sampling, allocation, curve, budget or cancellation diagnostics without partial paths.
#[allow(clippy::too_many_arguments)]
fn clipped_intervals(
    path: &CurvePath,
    step: f64,
    threshold: f64,
    sample: &dyn Fn(Point2) -> Result<f64, PatternPipelineError>,
    used: &mut usize,
    limit: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<CurvePath>, PatternPipelineError> {
    let mut runs: Vec<Vec<CurveSegment>> = Vec::new();
    let mut current = Vec::new();
    let mut all_positive = true;
    for segment in path.segments() {
        let length = segment.arc_length()?;
        let count_f = (length / step).ceil().max(1.0);
        if !count_f.is_finite() || count_f > limit.saturating_sub(*used) as f64 {
            return Err(PatternPipelineError::new(
                "realization.hatch.profile_limit",
                "hatch sampling exceeds the configured profile budget",
            ));
        }
        let count = count_f as usize;
        let mut left = 0.0;
        let mut positive = query(segment.start(), sample, used, limit, cancel)? > threshold;
        let mut start = positive.then_some(0.0);
        all_positive &= positive;
        for index in 1..=count {
            let right = index as f64 / count as f64;
            let next = query(
                if right == 1.0 {
                    segment.end()
                } else {
                    segment.point_at(right)?
                },
                sample,
                used,
                limit,
                cancel,
            )? > threshold;
            all_positive &= next;
            if next != positive {
                let mut lo = left;
                let mut hi = right;
                // Refine in the original parameter domain; no screen-space or tile phase reset.
                for _ in 0..20 {
                    let mid = (lo + hi) * 0.5;
                    if (query(segment.point_at(mid)?, sample, used, limit, cancel)? > threshold)
                        == positive
                    {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let crossing = (lo + hi) * 0.5;
                if positive {
                    push_range(
                        &mut current,
                        *segment,
                        start.take().unwrap_or(0.0),
                        crossing,
                    )?;
                    if !current.is_empty() {
                        reserve_tone(
                            &mut runs,
                            1,
                            "realization.hatch.allocation",
                            "hatch interval allocation failed",
                        )?;
                        runs.push(std::mem::take(&mut current));
                    }
                } else {
                    start = Some(crossing);
                }
                positive = next;
            }
            left = right;
        }
        if positive {
            push_range(&mut current, *segment, start.unwrap_or(0.0), 1.0)?;
        }
    }
    if !current.is_empty() {
        reserve_tone(
            &mut runs,
            1,
            "realization.hatch.allocation",
            "hatch interval allocation failed",
        )?;
        runs.push(current);
    }
    let closed = path.closure() == PathClosure::Closed;
    if closed && !all_positive && runs.len() > 1 {
        let seam = path.segments()[0].start();
        if runs.first().unwrap()[0].start() == seam
            && runs.last().unwrap().last().unwrap().end() == seam
        {
            let mut last = runs.pop().unwrap();
            let first = runs.remove(0);
            reserve_tone(
                &mut last,
                first.len(),
                "realization.hatch.allocation",
                "hatch seam allocation failed",
            )?;
            last.extend(first);
            runs.insert(0, last);
        }
    }
    let mut result = Vec::new();
    reserve_tone(
        &mut result,
        runs.len(),
        "realization.hatch.allocation",
        "hatch path allocation failed",
    )?;
    for run in runs {
        result.push(CurvePath::new(
            run,
            if closed && all_positive {
                PathClosure::Closed
            } else {
                PathClosure::Open
            },
        )?);
    }
    Ok(result)
}

/// Extracts an exact original line/cubic subrange rather than replacing curves with sample chords.
/// # Errors
/// Returns finite curve construction or fallible segment allocation diagnostics.
fn push_range(
    out: &mut Vec<CurveSegment>,
    segment: CurveSegment,
    start: f64,
    end: f64,
) -> Result<(), PatternPipelineError> {
    if end <= start {
        return Ok(());
    }
    let selected = if start == 0.0 && end == 1.0 {
        segment
    } else {
        match segment {
            CurveSegment::Line(line) => CurveSegment::Line(LineSegment::new(
                if start == 0.0 {
                    line.start()
                } else {
                    CurveSegment::Line(line).point_at(start)?
                },
                if end == 1.0 {
                    line.end()
                } else {
                    CurveSegment::Line(line).point_at(end)?
                },
            )?),
            CurveSegment::CubicBezier(cubic) => {
                let left = if end < 1.0 {
                    cubic.split(end)?.0
                } else {
                    cubic
                };
                CurveSegment::CubicBezier(if start > 0.0 {
                    left.split(start / end)?.1
                } else {
                    left
                })
            }
        }
    };
    reserve_tone(
        out,
        1,
        "realization.hatch.allocation",
        "hatch segment allocation failed",
    )?;
    out.push(selected);
    Ok(())
}

/// Resolves actual world-space circle geometry only from an eligible full-circle guide carrier.
/// Opposite canonical quarter endpoints retain placement/phase; the outline service independently
/// certifies actual cubic radial accuracy and declines distortion or numerical ambiguity.
fn circular_carrier(
    family: &TypedFamilyOutput,
    logical: &LogicalTonePath,
) -> Option<(Point2, f64, f64)> {
    let generic = family.family().generic_guides.as_ref()?;
    if !generic.analytic_hatch_circles || logical.instance.path.segments().len() != 4 {
        return None;
    }
    let StructuralPathSourceId::GuideDimension(id) = logical.instance.id.source else {
        return None;
    };
    let dimension = generic
        .dimensions
        .iter()
        .find(|dimension| dimension.id == id)?;
    let GuidePrototype::CircularArc {
        sweep_angle_degrees,
        ..
    } = dimension.prototype
    else {
        return None;
    };
    if sweep_angle_degrees.abs() != 360.0
        || !matches!(dimension.repetition, GuideRepetition::NormalOffset { .. })
    {
        return None;
    }
    let first = logical.instance.path.start();
    let opposite = logical.instance.path.segments()[2].start();
    let center = Point2::new(
        first.x + (opposite.x - first.x) * 0.5,
        first.y + (opposite.y - first.y) * 0.5,
    );
    let radius = (first.x - center.x).hypot(first.y - center.y);
    (center.is_finite() && radius.is_finite() && radius > 0.0).then_some((
        center,
        radius,
        sweep_angle_degrees.signum(),
    ))
}

/// Plans both signed analytic-circle ranks against the four actual inverse-placed padded corners.
/// Convex quarter-cubic circles enclose their nominal disk; the farthest corner therefore bounds
/// the required outer radius independently of placement rotation, translation or density aspect.
/// Original index zero, winding and pitch remain authoritative; only derived finite rank bounds change.
/// # Errors
/// Returns finite/rank-overflow diagnostics before allocation; the caller retains candidate limits.
pub(super) fn circle_coverage_bounds(
    center: Point2,
    radius: f64,
    sweep: f64,
    spacing: f64,
    corners: [Point2; 4],
    guard: u32,
) -> Result<(i64, i64), PatternPipelineError> {
    if !center.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !spacing.is_finite()
        || spacing <= 0.0
        || sweep.abs() != 360.0
        || corners.iter().any(|point| !point.is_finite())
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.proof",
            "analytic circle coverage requires finite geometry and positive spacing",
        ));
    }
    let reach = corners
        .into_iter()
        .map(|point| (point.x - center.x).hypot(point.y - center.y))
        .fold(0.0_f64, f64::max)
        .next_up();
    let outer = ((reach - radius).max(0.0) / spacing).ceil();
    let inner = (radius / spacing).ceil();
    if !outer.is_finite()
        || !inner.is_finite()
        || outer >= i64::MAX as f64
        || inner >= i64::MAX as f64
    {
        return Err(PatternPipelineError::new(
            "coverage.curved_guides.numeric_overflow",
            "analytic circle coverage rank arithmetic overflowed",
        ));
    }
    let guarded = |rank: f64| {
        (rank as i64)
            .checked_add(i64::from(guard))
            .ok_or(PatternPipelineError::new(
                "coverage.curved_guides.numeric_overflow",
                "analytic circle coverage rank arithmetic overflowed",
            ))
    };
    let (outer, inner) = (guarded(outer)?, guarded(inner)?);
    if sweep > 0.0 {
        Ok((-outer, inner))
    } else {
        Ok((-inner, outer))
    }
}

/// Constructs a known full-circle offset directly, avoiding approximation of a tiny remote carrier.
/// Positive left-normal offsets shrink positive-winding circles; nonpositive radii collapse exactly.
/// Existing circular-arc construction owns cubic geometry and the strict closed seam is explicit.
/// # Errors
/// Returns finite curve, cancellation or allocation diagnostics without partial support paths.
#[allow(clippy::too_many_arguments)]
pub(super) fn circle_offset(
    center: toniator_domain::AuthoredPoint2,
    radius: f64,
    start: f64,
    sweep: f64,
    distance: f64,
    baseline: AffineTransform2D,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<toniator_geometry::OffsetPathComponent>, PatternPipelineError> {
    check_cancel(cancel)?;
    let radius = radius - distance * sweep.signum();
    if radius <= 0.0 {
        return Ok(Vec::new());
    }
    let open = resolve_guide_prototype(
        &toniator_domain::GuidePrototype::CircularArc {
            center,
            radius,
            start_angle_degrees: start,
            sweep_angle_degrees: sweep,
        },
        None,
    )?;
    let mut segments = open.segments().to_vec();
    let seam = segments[0].start();
    let last = segments.len() - 1;
    let CurveSegment::CubicBezier(cubic) = segments[last] else {
        return Err(PatternPipelineError::new(
            "pattern.hatch.circle",
            "analytic circle requires cubic arc segments",
        ));
    };
    segments[last] = CurveSegment::CubicBezier(CubicBezierSegment::new(
        cubic.start(),
        cubic.control_1(),
        cubic.control_2(),
        seam,
    )?);
    let path = CurvePath::new(segments, PathClosure::Closed)?.transformed_with_limits_cancellable(
        baseline,
        CurveWorkLimits::default(),
        cancel,
    )?;
    Ok(vec![toniator_geometry::OffsetPathComponent {
        component_ordinal: 0,
        source_start: PathLocation::new(0, 0.0)?,
        source_end: PathLocation::new(path.segments().len() - 1, 1.0)?,
        source_intervals: identity_source_intervals(&path)?,
        path,
        planar_switch_nodes: Vec::new(),
    }])
}

#[cfg(test)]
mod circle_coverage_tests {
    use super::*;
    /// Proves radial reach against actual inverse-placed corners for rotations, translations and pitches.
    /// Different pitches cover the scalar spacing produced by density/aspect; both windings preserve ranks.
    /// # Panics
    /// Panics if a valid bounded plan leaves a padded corner outside the last ring or loses inward collapse.
    #[test]
    fn analytic_circle_corners_are_covered_in_every_placed_frame() {
        let center = Point2::new(23., -17.);
        let radius = 2.56;
        let domain = Bounds::new(Point2::new(-9., -9.), Point2::new(1369., 777.)).unwrap();
        for angle in [0., 17., 90., 233.] {
            for translation in [Vector2::new(680., 384.), Vector2::new(1180., 84.)] {
                let placement = AffineTransform2D::rotate_about_then_translate(
                    Point2::new(0., 0.),
                    angle,
                    translation,
                )
                .unwrap();
                let corners = domain.corners().map(|point| placement.inverse_point(point));
                let reach = corners
                    .iter()
                    .map(|point| (point.x - center.x).hypot(point.y - center.y))
                    .fold(0.0_f64, f64::max);
                for pitch in [0.02, 0.85, 2.56] {
                    for sweep in [-360., 360.] {
                        let (first, last) =
                            circle_coverage_bounds(center, radius, sweep, pitch, corners, 2)
                                .unwrap();
                        let outer = if sweep > 0. { first } else { last };
                        let inner = if sweep > 0. { last } else { first };
                        assert!(radius - outer as f64 * pitch * sweep.signum() >= reach);
                        assert!(radius - inner as f64 * pitch * sweep.signum() <= 0.);
                        assert!(first <= 0 && last >= 0);
                    }
                }
            }
        }
    }

    /// Rejects nonfinite and unrepresentable analytic coverage before its caller allocates rank storage.
    /// # Panics
    /// Panics if invalid geometry or overflowing rank arithmetic acquires a finite coverage plan.
    #[test]
    fn analytic_circle_coverage_keeps_numeric_bounds() {
        let corners = [Point2::new(1., 1.); 4];
        assert!(circle_coverage_bounds(Point2::new(0., 0.), 1., 360., 0., corners, 2).is_err());
        assert!(circle_coverage_bounds(Point2::new(0., 0.), 1., 360., 1e-300, corners, 2).is_err());
        assert!(
            circle_coverage_bounds(
                Point2::new(0., 0.),
                1.,
                360.,
                1.,
                [Point2::new(f64::INFINITY, 0.); 4],
                2
            )
            .is_err()
        );
    }
}
