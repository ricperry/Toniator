//! Transformed coverage derives extent without changing authored spacing, prefix or tone authority.
use toniator_domain::*;
use toniator_geometry::{
    AffineTransform2D, Bounds, ConstructionContext, ConstructionMode, Point2, Vector2,
};
use toniator_patterns::*;

/// Creates a current analytic source with explicit extent intent and an optional absolute site cadence.
fn spiral(
    shape: SpiralShape,
    policy: SpiralCoveragePolicy,
    phase: f64,
    winding: CurveWinding,
    sites: bool,
) -> PatternDefinition {
    let mut definition = PatternDefinition::parametric_curve(
        PatternDefinitionId(1),
        "Coverage placement",
        PatternMechanismId(1),
        sites.then_some(PatternMechanismId(2)),
        PatternOutputLayerId(1),
        ParametricCurve::Spiral(SpiralCurve {
            shape,
            turns: 1.375,
            radial_spacing: 12.0,
            phase_degrees: phase,
            winding,
        }),
        GuideRepetition::Single,
        sites.then_some(ParametricCurveSiteDraft {
            interval: 17.0,
            phase: 0.125,
        }),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let PatternMechanism::ParametricCurveSource {
        spiral_coverage, ..
    } = &mut definition.mechanisms[0]
    else {
        unreachable!("constructor supplies source")
    };
    *spiral_coverage = policy;
    definition
}

/// Resolves real Feature/aspect and substantial placement through the production request boundary.
/// # Panics
/// Panics if bounded positive fixture density cannot resolve.
fn request(
    width: f64,
    height: f64,
    feature: f64,
    aspect: f64,
    rotation: f64,
    x: f64,
    y: f64,
) -> GridInspectRequest {
    let canvas = CanvasSpec { width, height };
    let default = DensityMetric2D::default_for_canvas(&canvas).unwrap();
    let density = DensityMetric2D {
        density: default.density / feature,
        aspect,
    }
    .resolve(&canvas)
    .unwrap();
    GridInspectRequest {
        canvas,
        density,
        rotation_degrees: rotation,
        translation_x: x,
        translation_y: y,
        guard_steps: 2,
        support_radius: 7.0,
        max_family_candidates: 200_000,
    }
}

/// Independently bounds the actual padded corners in the analytic spiral frame, avoiding inverse-AABB overgrowth.
/// # Panics
/// Panics if finite fixture transforms or support bounds fail.
fn required_extent(
    req: &GridInspectRequest,
    shape: SpiralShape,
    phase: f64,
    feature: f64,
    sites: bool,
) -> (f64, Bounds, AffineTransform2D) {
    let margin = req.support_radius
        + ANTIALIAS_MARGIN
        + f64::from(req.guard_steps)
            * if sites {
                17.0 * feature
            } else {
                12.0 * feature
            };
    let padded = Bounds::new(
        Point2::new(0.0, 0.0),
        Point2::new(req.canvas.width, req.canvas.height),
    )
    .unwrap()
    .expanded(margin)
    .unwrap();
    let angle = req.rotation_degrees
        + if shape == SpiralShape::Square {
            phase
        } else {
            0.0
        };
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        angle,
        Vector2::new(
            req.canvas.width * 0.5 + req.translation_x,
            req.canvas.height * 0.5 + req.translation_y,
        ),
    )
    .unwrap();
    let extent = padded
        .corners()
        .into_iter()
        .map(|corner| {
            let point = transform.inverse_point(corner);
            if shape == SpiralShape::Round {
                point.x.hypot(point.y)
            } else {
                point.x.abs().max(point.y.abs())
            }
        })
        .fold(0.0_f64, f64::max);
    (extent, padded, transform)
}

/// Proves shape-correct sufficient and tight derived turns across phase, winding, Feature, aspect and placement.
/// # Panics
/// Panics if transformed coverage is insufficient, inverse-AABB inflation appears, or authored policy changes.
#[test]
fn cover_extent_brackets_actual_support_corners_and_preserves_authored_intent() {
    for (width, height, feature, aspect, rotation, x, y) in [
        (320.0, 120.0, 1.0, 1.0, 0.0, -256.0, 192.0),
        (120.0, 320.0, 0.75, 2.0, 37.0, 384.0, -448.0),
        (320.0, 120.0, 1.5, 0.5, -71.0, -576.0, -256.0),
        (120.0, 320.0, 1.0, 3.0, 123.0, 256.0, 512.0),
    ] {
        let req = request(width, height, feature, aspect, rotation, x, y);
        for shape in [SpiralShape::Round, SpiralShape::Square] {
            for (phase, winding, sites) in [
                (0.0, CurveWinding::Clockwise, false),
                (37.0, CurveWinding::CounterClockwise, true),
            ] {
                let definition = spiral(
                    shape,
                    SpiralCoveragePolicy::CoverCanvas,
                    phase,
                    winding,
                    sites,
                );
                let before = definition.clone();
                let full = ConstructionContext::new(ConstructionMode::Complete, None, false)
                    .run(|| evaluate_typed_family(&definition, &req))
                    .unwrap();
                let pruned = ConstructionContext::new(ConstructionMode::Canvas, None, false)
                    .run(|| evaluate_typed_family(&definition, &req))
                    .unwrap();
                assert_eq!(full, pruned);
                assert_eq!(definition, before);
                let logical = &full.logical_tone_paths()[0];
                let (extent, padded, transform) =
                    required_extent(&req, shape, phase, feature, sites);
                let end = transform.inverse_point(logical.instance.path.end());
                let actual = if shape == SpiralShape::Round {
                    end.x.hypot(end.y)
                } else {
                    end.x.abs().max(end.y.abs())
                };
                let expected_turns = (extent / (12.0 * feature)).ceil() + 1.0;
                assert!(
                    (actual - expected_turns * 12.0 * feature).abs() < 1e-7,
                    "{shape:?} actual={actual} expected={expected_turns} extent={extent}"
                );
                assert!((expected_turns - 1.0) * 12.0 * feature >= extent);
                assert_eq!(full.planned_support_radius(), req.support_radius);
                assert_eq!(full.guard_steps(), req.guard_steps);
                assert!(!logical.instance.path.segments().is_empty());
                for fragment in full.structural_path_set().unwrap().paths() {
                    for segment in fragment.path.segments() {
                        for point in [segment.start(), segment.end()] {
                            assert!(
                                point.x >= padded.min.x - 1e-7 && point.x <= padded.max.x + 1e-7
                            );
                            assert!(
                                point.y >= padded.min.y - 1e-7 && point.y <= padded.max.y + 1e-7
                            );
                        }
                    }
                }
                let reached_boundaries = [
                    (0, padded.min.x),
                    (0, padded.max.x),
                    (1, padded.min.y),
                    (1, padded.max.y),
                ]
                .into_iter()
                .filter(|(axis, boundary)| {
                    full.structural_path_set()
                        .unwrap()
                        .paths()
                        .iter()
                        .any(|fragment| {
                            [fragment.path.start(), fragment.path.end()]
                                .into_iter()
                                .any(|point| {
                                    ((if *axis == 0 { point.x } else { point.y }) - boundary).abs()
                                        < 1e-7
                                })
                        })
                })
                .count();
                assert!(
                    reached_boundaries >= 2,
                    "published padded envelope is not reached for {shape:?}"
                );
                if sites {
                    assert!(
                        full.site_set()
                            .iter()
                            .any(|site| site.scope != toniator_geometry::SiteScope::Canvas)
                    );
                }
            }
        }
    }
}

/// Keeps Fixed output and the authored minimum exact while extension preserves canonical prefix and tone cadence.
/// # Panics
/// Panics if placement stretches finite authored geometry or introduces a source knot at a continued edge.
#[test]
fn fixed_extent_prefix_and_internal_continuation_keep_source_cadence() {
    let req = request(120.0, 80.0, 1.0, 1.0, 37.0, 0.0, 0.0);
    for shape in [SpiralShape::Round, SpiralShape::Square] {
        let fixed = spiral(
            shape,
            SpiralCoveragePolicy::Fixed,
            17.0,
            CurveWinding::Clockwise,
            false,
        );
        let cover = spiral(
            shape,
            SpiralCoveragePolicy::CoverCanvas,
            17.0,
            CurveWinding::Clockwise,
            false,
        );
        let finite = evaluate_typed_family(&fixed, &req).unwrap();
        let covered = evaluate_typed_family(&cover, &req).unwrap();
        let a = &finite.logical_tone_paths()[0];
        let b = &covered.logical_tone_paths()[0];
        assert_eq!(
            &b.instance.path.segments()[..a.instance.path.segments().len()],
            a.instance.path.segments()
        );
        assert_eq!(a.domain, b.domain);
        assert_eq!(a.nominal_basis, b.nominal_basis);
        assert_eq!(
            finite
                .tone_sample_interval(ToneSampleSpacing::default())
                .unwrap(),
            covered
                .tone_sample_interval(ToneSampleSpacing::default())
                .unwrap()
        );
        if shape == SpiralShape::Square {
            let prefix_length = a.instance.path.measure_arc_length().unwrap().total_length();
            let profile = prepare_tone_profile_cancellable(
                &b.instance.path,
                b.domain,
                2.13,
                b.nominal_basis,
                100_000,
                &|_| Ok(0.6),
                &|| false,
            )
            .unwrap();
            assert!(
                profile
                    .knots
                    .iter()
                    .all(|knot| (knot.distance - prefix_length).abs() > 1e-8)
            );
        }
    }
    let mut cover = spiral(
        SpiralShape::Round,
        SpiralCoveragePolicy::CoverCanvas,
        0.0,
        CurveWinding::Clockwise,
        false,
    );
    let PatternMechanism::ParametricCurveSource {
        curve: ParametricCurve::Spiral(curve),
        ..
    } = &mut cover.mechanisms[0]
    else {
        unreachable!()
    };
    curve.turns = 120.375;
    let large = evaluate_typed_family(&cover, &req).unwrap();
    let end = large.logical_tone_paths()[0].instance.path.end();
    let center = Point2::new(
        req.canvas.width * 0.5 + req.translation_x,
        req.canvas.height * 0.5 + req.translation_y,
    );
    assert!(((end.x - center.x).hypot(end.y - center.y) - 120.375 * 12.0).abs() < 1e-7);
}

/// Rejects overflowing cover proofs and cancellation before publishing a family, retaining constructor limits.
/// # Panics
/// Panics if invalid derived extent leaks partial geometry or loses resource authority.
#[test]
fn cover_extent_overflow_cancellation_and_explicit_limits_fail_atomically() {
    let definition = spiral(
        SpiralShape::Round,
        SpiralCoveragePolicy::CoverCanvas,
        17.0,
        CurveWinding::Clockwise,
        false,
    );
    let plan = resolve_pattern_pipeline(&definition).unwrap();
    let mut req = request(120.0, 80.0, 1.0, 1.0, 0.0, 0.0, 0.0);
    req.translation_x = f64::MAX;
    assert!(
        evaluate_typed_family(&definition, &req)
            .unwrap_err()
            .path()
            .starts_with("coverage.parametric")
    );
    let req = request(120.0, 80.0, 1.0, 1.0, 0.0, -192.0, 256.0);
    assert_eq!(
        evaluate_typed_family_product_cancellable(&plan.family, &req, &|| true)
            .unwrap_err()
            .path(),
        "evaluation.cancelled"
    );
    let limits =
        toniator_geometry::CurveWorkLimits::new(4, usize::MAX, usize::MAX, usize::MAX).unwrap();
    let error = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
        &plan.family,
        &req,
        None,
        limits,
        &|| false,
        &|_, _| {},
    )
    .unwrap_err();
    assert!(error.path().contains("segment_limit"));
}

/// Checks dense deterministic guide coverage without imposing a uniform-occupancy rule on scatter.
/// # Panics
/// Panics if an infinite guide family's finite generation misses a final-canvas quadrant tile.
fn assert_guide_tiles(family: &TypedFamilyOutput, req: &GridInspectRequest) {
    for y in 0..4 {
        for x in 0..4 {
            assert!(
                family.site_set().iter().any(|site| {
                    site.position.x >= req.canvas.width * f64::from(x) / 4.0
                        && site.position.x <= req.canvas.width * f64::from(x + 1) / 4.0
                        && site.position.y >= req.canvas.height * f64::from(y) / 4.0
                        && site.position.y <= req.canvas.height * f64::from(y + 1) / 4.0
                }),
                "missing guide tile ({x},{y}) for {req:?}"
            );
        }
    }
}

/// Proves both straight production planners preserve full transformed families while fixed-grid pruning saves work.
/// # Panics
/// Panics if translation/rotation/anisotropy loses required sites, changes output identity or disables reached pruning.
#[test]
fn straight_planners_preserve_translated_support_and_omit_dead_candidate_work() {
    let coverage = CoveragePolicy {
        guard_steps: 2,
        additional_margin: 0.0,
    };
    let fixed = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "Fixed grid coverage",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        coverage.clone(),
    );
    let general = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(2),
        "General guide coverage",
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternOutputLayerId(2),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(3),
                baseline_angle_degrees: 0.0,
                phase: 0.125,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(4),
                baseline_angle_degrees: 63.0,
                phase: 0.25,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(3), GuideDimensionId(4)],
            merge_epsilon: 1e-9,
        },
        MarkOrientation::Fixed,
        coverage,
    );
    let mut reached_saving = false;
    for req in [
        request(320.0, 120.0, 2.0, 2.0, 45.0, -640.0, 256.0),
        request(120.0, 320.0, 2.0, 0.5, -37.0, 256.0, -640.0),
    ] {
        for definition in [&fixed, &general] {
            let complete = ConstructionContext::new(ConstructionMode::Complete, None, true);
            let canvas = ConstructionContext::new(ConstructionMode::Canvas, None, true);
            let a = complete
                .run(|| evaluate_typed_family(definition, &req))
                .unwrap();
            let b = canvas
                .run(|| evaluate_typed_family(definition, &req))
                .unwrap();
            assert_eq!(a, b);
            assert_guide_tiles(&a, &req);
            if definition.id == fixed.id {
                let full = complete.measurements.as_ref().unwrap().snapshot();
                let pruned = canvas.measurements.as_ref().unwrap().snapshot();
                assert_eq!(full.logical_grid_candidates, pruned.logical_grid_candidates);
                reached_saving |=
                    pruned.grid_positions + pruned.grid_search_positions < full.grid_positions;
            }
        }
    }
    assert!(reached_saving);
}

/// Installs a genuine gallery recipe so authored resources and receiving-channel weighting use normal authority.
/// # Panics
/// Panics if the current typed fixture cannot be found or installed.
fn gallery_document(id: &str, canvas: CanvasSpec) -> Document {
    let registry = PresetRegistry::bundled();
    let recipe = registry
        .entries()
        .iter()
        .find(|entry| entry.metadata.id == id)
        .unwrap()
        .recipe
        .clone();
    let base = Document::new_default_document(canvas, SourceReference::Unassigned).unwrap();
    base.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
        base: base.pattern_settings().clone(),
        base_definition: base.pattern_definition_bundles()[0].definition.clone(),
        recipe,
    })
    .unwrap()
    .0
}

/// Preserves deterministic random guard generation under large placement and zero source weighting.
/// Stochastic empty visible areas remain valid; only this fixture's exterior guard authority is asserted.
/// # Panics
/// Panics if the random planner drops transformed exterior guards or construction mode changes family semantics.
#[test]
fn random_and_zero_weighted_planners_retain_translated_exterior_guard_authority() {
    let black = toniator_sampling::decode_source(br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="black"/></svg>"#, toniator_sampling::SourceFormatHint::Svg).unwrap();
    for (id, req) in [
        (
            "even-random-circles",
            request(320.0, 120.0, 1.0, 2.0, 37.0, -640.0, 256.0),
        ),
        (
            "source-weighted-dispersion-voronoi",
            request(120.0, 320.0, 1.0, 0.5, -71.0, 256.0, -640.0),
        ),
    ] {
        let document = gallery_document(id, req.canvas.clone());
        let definition = &document
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.id == document.pattern_settings().definition_id)
            .unwrap()
            .definition;
        let plan = resolve_document_pattern_pipeline_for_channel(
            &document,
            definition,
            SourceWeighting::canonical(SourceMappingComponent::Luminance),
        )
        .unwrap();
        let a = ConstructionContext::new(ConstructionMode::Complete, None, false)
            .run(|| {
                evaluate_typed_family_product_with_source_cancellable(
                    &plan.family,
                    &req,
                    Some(&black),
                    &|| false,
                )
            })
            .unwrap();
        let b = ConstructionContext::new(ConstructionMode::Canvas, None, false)
            .run(|| {
                evaluate_typed_family_product_with_source_cancellable(
                    &plan.family,
                    &req,
                    Some(&black),
                    &|| false,
                )
            })
            .unwrap();
        assert_eq!(a, b);
        let sites = a.site_set().sites();
        for side in 0..4 {
            assert!(
                sites.iter().any(|site| match side {
                    0 => site.position.x < 0.0,
                    1 => site.position.x > req.canvas.width,
                    2 => site.position.y < 0.0,
                    _ => site.position.y > req.canvas.height,
                }),
                "missing exterior side {side} for {id}"
            );
        }
    }
}

/// Preserves one anchored tone lattice across the actual rotated rank43 short-component numeric regression.
/// Final canonical component metrics define both domain recurrence and the strict consumer; source-zero is never reset.
/// # Panics
/// Panics if the real folded-ray witness loses a component, changes strict continuity or fails tone preparation.
#[test]
fn authored_ray_tone_domains_follow_final_world_metric_under_combined_placement() {
    let family = PatternStructureRecipe::GenericGuides {
        name: "World metric regression".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
        dimensions: vec![GenericGuideDimensionDraft {
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference { resource_index: 0 },
            repetition: GuideRepetition::NormalOffset {
                spacing: 16.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        product: GeneralizedSiteProductDraft::AlongGuides {
            dimension_indices: vec![0],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    };
    let recipe = PatternDefinitionRecipe::connected(PatternStructureRecipe::AuthoredResources {
        resources: vec![
            AuthoredStructureDraft::new(
                AuthoredStructureKind::OpenPath,
                vec![AuthoredCurveSegment::CubicBezier {
                    start: AuthoredPoint2 { x: -64.0, y: 0.0 },
                    control_1: AuthoredPoint2 { x: -20.0, y: -24.0 },
                    control_2: AuthoredPoint2 { x: 20.0, y: 0.0 },
                    end: AuthoredPoint2 { x: 64.0, y: 0.0 },
                }],
            )
            .unwrap(),
        ],
        definition: Box::new(PatternStructureRecipe::OrderedOutputs {
            definition: Box::new(family),
            outputs: vec![PatternOutputRealizationRecipe::StructuralPaths {
                tone_sample_spacing: ToneSampleSpacing::default(),
                style: Default::default(),
            }],
        }),
    });
    let canvas = CanvasSpec {
        width: 1024.0,
        height: 1024.0,
    };
    let base = Document::new_default_document(canvas.clone(), SourceReference::Unassigned).unwrap();
    let document = base
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: base.pattern_settings().clone(),
            base_definition: base.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let definition = document.pattern_definition_for(ChannelId(3)).unwrap();
    let family = evaluate_document_typed_family_cancellable(
        &document,
        definition,
        &GridInspectRequest {
            canvas: canvas.clone(),
            density: DensityMetric2D::default_for_canvas(&canvas)
                .unwrap()
                .resolve(&canvas)
                .unwrap(),
            rotation_degrees: 91.0,
            translation_x: -256.0,
            translation_y: 384.0,
            guard_steps: 2,
            support_radius: 16.0,
            max_family_candidates: 200_000,
        },
        &|| false,
    )
    .unwrap();
    let paths = family.logical_tone_paths();
    assert!(
        paths
            .iter()
            .any(|path| path.instance.id.repetition_index == 43
                && path.instance.id.component_ordinal == 6)
    );
    let mut index = 0;
    while index < paths.len() {
        let first = &paths[index];
        let count = paths[index..]
            .iter()
            .take_while(|path| {
                path.instance.id.source == first.instance.id.source
                    && path.instance.id.repetition_index == first.instance.id.repetition_index
            })
            .count();
        let inputs = paths[index..index + count]
            .iter()
            .map(|path| (&path.instance.path, path.domain))
            .collect::<Vec<_>>();
        for pair in inputs.windows(2) {
            let length = pair[0]
                .0
                .measure_arc_length_cancellable(Default::default(), &|| false)
                .unwrap()
                .total_length();
            assert_eq!(
                (pair[0].1.start_distance + length).to_bits(),
                pair[1].1.start_distance.to_bits()
            );
        }
        prepare_tone_group_profiles_cancellable(
            &inputs,
            16.0,
            16.0,
            100_000,
            &|_| Ok(1.0),
            &|| false,
        )
        .unwrap();
        index += count;
    }
}

#[cfg(feature = "test-evidence")]
mod exact_world_offset_line_tests {
    use toniator_geometry::*;
    use toniator_patterns::artistic_tone_evidence::{
        canonicalize_world_offset_component_for_evidence as remove_exact_zero_world_offset_lines,
        offset_group_tone_domains,
    };

    /// Builds source-traced derived paths without assigning renderer or tangent authority to zero motion.
    /// # Panics
    /// Panics if finite connected witness construction fails.
    fn component(segments: Vec<CurveSegment>) -> toniator_geometry::OffsetPathComponent {
        let source_start = toniator_geometry::PathLocation::new(1, 0.0).unwrap();
        let source_end = toniator_geometry::PathLocation::new(1, 1.0).unwrap();
        let source_intervals = (0..segments.len())
            .map(|i| {
                (
                    toniator_geometry::PathLocation::new(1, i as f64 / segments.len() as f64)
                        .unwrap(),
                    toniator_geometry::PathLocation::new(1, (i + 1) as f64 / segments.len() as f64)
                        .unwrap(),
                )
            })
            .collect();
        toniator_geometry::OffsetPathComponent {
            component_ordinal: 3,
            source_start,
            source_end,
            path: CurvePath::new(segments, toniator_geometry::PathClosure::Open).unwrap(),
            source_intervals,
            planar_switch_nodes: vec![Point2::new(0.0, 0.0)],
        }
    }

    /// Reproduces the positive local2.84e-14 line collapsing at the exact reported91-degree world transform.
    /// Only its worldzero trace is removed; component/sourcezero metadata and the fixed neighbor node stay exact.
    /// # Panics
    /// Panics if positive source motion is misclassified or canonical geometry/provenance changes beyond zero motion.
    #[test]
    fn transformed_positive_line_collapse_removes_only_exact_zero_with_trace() {
        let start = Point2::new(-201.42514823951015, 542.2055437274599);
        let end = Point2::new(-201.42514823951012, 542.2055437274599);
        assert_ne!(start, end);
        let transform = AffineTransform2D::rotate_about_then_translate(
            Point2::new(0.0, 0.0),
            91.0,
            Vector2::new(256.0, 896.0),
        )
        .unwrap();
        let a = transform.apply_point(start);
        let b = transform.apply_point(end);
        assert_eq!(a, b);
        let line = |a, b| CurveSegment::Line(toniator_geometry::LineSegment::new(a, b).unwrap());
        let mut value = component(vec![
            line(Point2::new(a.x - 1.0, a.y), a),
            line(a, b),
            line(b, Point2::new(a.x, a.y + 1.0)),
        ]);
        let before = value.clone();
        remove_exact_zero_world_offset_lines(&mut value, CurveWorkLimits::default(), &|| false)
            .unwrap();
        assert_eq!(value.component_ordinal, before.component_ordinal);
        assert_eq!(value.source_start, before.source_start);
        assert_eq!(value.source_end, before.source_end);
        assert_eq!(value.planar_switch_nodes, before.planar_switch_nodes);
        assert_eq!(
            value.path.segments(),
            &[before.path.segments()[0], before.path.segments()[2]]
        );
        assert_eq!(
            value.source_intervals,
            vec![before.source_intervals[0], before.source_intervals[2]]
        );
        assert_eq!(
            value.path.segments()[0].end(),
            value.path.segments()[1].start()
        );
        let zero = toniator_geometry::PathLocation::new(1, 0.5).unwrap();
        let domains = offset_group_tone_domains(&[value], zero, false, &|| false).unwrap();
        assert_eq!(domains[0].start_distance, -1.0);
        assert!(
            before.path.segments()[1]
                .limiting_unit_tangent_at(0.0)
                .is_err()
        );
    }

    /// Keeps positive short lines and cubic loops, rejects allzero components and honors cancellation atomically.
    /// # Panics
    /// Panics if exactzero cleanup gains a tolerance/cubic filter or publishes partial mutation after failure.
    #[test]
    fn exact_zero_cleanup_preserves_short_curves_and_rejects_complete_collapse() {
        let p = Point2::new(0.0, 0.0);
        let q = Point2::new(1.0e-14, 0.0);
        let line = |a, b| CurveSegment::Line(toniator_geometry::LineSegment::new(a, b).unwrap());
        let cubic = CurveSegment::CubicBezier(
            CubicBezierSegment::new(q, Point2::new(1.0, 1.0), Point2::new(-1.0, 1.0), q).unwrap(),
        );
        let mut value = component(vec![line(p, p), line(p, q), cubic]);
        let before = value.clone();
        assert!(
            remove_exact_zero_world_offset_lines(&mut value, CurveWorkLimits::default(), &|| true)
                .is_err()
        );
        assert_eq!(value, before);
        remove_exact_zero_world_offset_lines(&mut value, CurveWorkLimits::default(), &|| false)
            .unwrap();
        assert_eq!(value.path.segments(), &before.path.segments()[1..]);
        let mut zero = component(vec![line(p, p)]);
        let unchanged = zero.clone();
        let error =
            remove_exact_zero_world_offset_lines(&mut zero, CurveWorkLimits::default(), &|| false)
                .unwrap_err();
        assert_eq!(error.path(), "coverage.curved_guides.world_collapse");
        assert_eq!(zero, unchanged);
    }
}
