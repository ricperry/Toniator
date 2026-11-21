use toniator_domain::{
    CanvasSpec, ChannelId, DensityMetric2D, Document, DocumentCommand, SourceMapping,
    SourceMappingComponent, SourceReference,
};
use toniator_geometry::{CanonicalFillRule, PathClosure};
use toniator_patterns::{
    GeometryResponseInput, GridInspectRequest, StrokeResponse,
    evaluate_typed_family_product_with_source_cancellable, interleaved_hatch_recipe,
    realize_typed_canonical_strokes_with_response_cancellable,
    resolve_document_pattern_pipeline_for_channel,
};
use toniator_sampling::SourceField;

/// Builds one current editable hatch recipe without introducing fixture-only generation authority.
/// # Panics
/// Panics when ordinary recipe publication or density resolution fails.
fn prepare(
    rings: bool,
) -> (
    Document,
    toniator_patterns::PatternPipelinePlan,
    toniator_patterns::TypedFamilyOutput,
) {
    prepare_at(
        rings,
        CanvasSpec {
            width: 64.0,
            height: 64.0,
        },
        6.25,
    )
}

/// Resolves ordinary hatch carriers at the requested document canvas and effective density.
/// # Panics
/// Panics if ordinary recipe publication, density resolution or bounded family construction fails.
fn prepare_at(
    rings: bool,
    canvas: CanvasSpec,
    density: f64,
) -> (
    Document,
    toniator_patterns::PatternPipelinePlan,
    toniator_patterns::TypedFamilyOutput,
) {
    let mut document = Document::new_default_document(canvas, SourceReference::Unassigned).unwrap();
    let base = document.pattern_settings().clone();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    document = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: interleaved_hatch_recipe(rings),
        })
        .unwrap()
        .0;
    let definition = document.pattern_definition_for(ChannelId(1)).unwrap();
    let plan = resolve_document_pattern_pipeline_for_channel(
        &document,
        definition,
        toniator_domain::SourceWeighting::canonical(SourceMappingComponent::Cyan),
    )
    .unwrap();
    let request = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: DensityMetric2D {
            density,
            aspect: 1.0,
        }
        .resolve(document.canvas())
        .unwrap(),
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 4.0,
        max_family_candidates: 2_000_000,
    };
    let family = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    (document, plan, family)
}

/// Checks empty/full tone, exact constant width, real annulus holes and no variable-size attenuation.
/// # Panics
/// Panics if carrier generation or canonical realization loses fixed-width vector semantics.
#[test]
fn interleaved_hatch_empty_full_width_and_annuli() {
    let source = SourceField::from_straight_rgba8(4, 4, [0, 255, 255, 255].repeat(16)).unwrap();
    for rings in [false, true] {
        let (document, plan, family) = prepare(rings);
        let realize = |input| {
            realize_typed_canonical_strokes_with_response_cancellable(
                &family,
                &plan,
                &source,
                document.canvas(),
                SourceMapping::canonical(SourceMappingComponent::Cyan),
                StrokeResponse {
                    minimum_thickness: 0.0,
                    maximum_thickness: 1.0,
                    bias: 0.0,
                },
                10.24,
                2_000_000,
                2_000_000,
                input,
                &|| false,
            )
            .unwrap()
        };
        assert!(
            realize(GeometryResponseInput::RawProbe(0.0))
                .output
                .strokes
                .is_empty()
        );
        let full = realize(GeometryResponseInput::RawProbe(2.0));
        let half = realize(GeometryResponseInput::RawProbe(1.0));
        assert!(!half.output.strokes.is_empty());
        assert!(half.output.strokes.len() < full.output.strokes.len());
        assert_eq!(
            full.output.strokes,
            realize(GeometryResponseInput::Identity).output.strokes
        );
        for stroke in &full.output.strokes {
            assert!(
                stroke
                    .profile
                    .iter()
                    .all(|p| p.width == stroke.nominal_basis && p.normalized_thickness == 1.0)
            );
            assert_eq!(stroke.outline.fill_rule, CanonicalFillRule::NonZero);
        }
        if rings {
            assert!(
                full.output
                    .strokes
                    .iter()
                    .any(|stroke| stroke.path.closure() == PathClosure::Closed
                        && stroke.outline.contours.len() == 2),
                "annuli retain both wound rails"
            );
        }
    }
}

/// Checks transparent artwork suppresses strokes and channel tone continuously splits retained paths.
/// # Panics
/// Panics if source alpha, channel mapping, threshold clipping or bias identity diverges.
#[test]
fn interleaved_hatch_channel_alpha_splits_and_bias_identity() {
    let (document, plan, family) = prepare(false);
    let mut rgba = Vec::new();
    for _ in 0..64 {
        for x in 0..64 {
            rgba.extend(if (20..44).contains(&x) {
                [0, 255, 255, 255]
            } else {
                [0, 255, 255, 0]
            });
        }
    }
    let source = SourceField::from_straight_rgba8(64, 64, rgba).unwrap();
    let realize = |component, bias| {
        realize_typed_canonical_strokes_with_response_cancellable(
            &family,
            &plan,
            &source,
            document.canvas(),
            SourceMapping::canonical(component),
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias,
            },
            10.24,
            2_000_000,
            2_000_000,
            GeometryResponseInput::Identity,
            &|| false,
        )
        .unwrap()
    };
    assert!(
        realize(SourceMappingComponent::Magenta, 0.0)
            .output
            .strokes
            .is_empty()
    );
    let cyan = realize(SourceMappingComponent::Cyan, 0.0);
    assert!(!cyan.output.strokes.is_empty());
    assert!(cyan.output.strokes.iter().all(|s| {
        let bounds = s.path.bounds().unwrap();
        bounds.min.x >= 18.0 && bounds.max.x <= 46.0
    }));
    assert_ne!(
        cyan.output.realization_fingerprint,
        realize(SourceMappingComponent::Cyan, 0.5)
            .output
            .realization_fingerprint
    );
}

/// Exercises continuous ring clipping with both immutable artwork fields through real cubic paths.
/// # Panics
/// Panics if threshold splitting breaks exact segment continuity or source decoding fails.
#[test]
fn concentric_hatch_artwork_cubic_threshold_continuity() {
    for (name, hint) in [
        (
            "raster-sample.png",
            toniator_sampling::SourceFormatHint::Png,
        ),
        (
            "vector-sample.svg",
            toniator_sampling::SourceFormatHint::Svg,
        ),
    ] {
        let bytes = std::fs::read(format!(
            "{}/../../assets/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let source = toniator_sampling::decode_source(&bytes, hint).unwrap();
        let (document, plan, family) = prepare(true);
        realize_typed_canonical_strokes_with_response_cancellable(
            &family,
            &plan,
            &source,
            document.canvas(),
            SourceMapping::canonical(SourceMappingComponent::Cyan),
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            },
            10.24,
            2_000_000,
            2_000_000,
            GeometryResponseInput::Identity,
            &|| false,
        )
        .unwrap();
    }
}

/// Uses nearest actually measured populations, with lower-ink ties and independent desired bounds.
/// # Panics
/// Panics if published calibration invents intermediate widths or differs from the selected raw class.
#[test]
fn hatch_nearest_population_ties_bounds_and_cancellation() {
    let (document, plan, family) = prepare(false);
    let source = SourceField::from_straight_rgba8(4, 4, [0, 255, 255, 255].repeat(16)).unwrap();
    let curve = toniator_patterns::CoverageCurve::new(
        (0..=8)
            .map(|i| toniator_patterns::CoverageKnot {
                response: f64::from(i) / 8.0,
                coverage: f64::from(i) / 8.0,
            })
            .collect(),
    )
    .unwrap();
    let realize = |mapping, input, cancel: &dyn Fn() -> bool| {
        realize_typed_canonical_strokes_with_response_cancellable(
            &family,
            &plan,
            &source,
            document.canvas(),
            mapping,
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            },
            10.24,
            2_000_000,
            2_000_000,
            input,
            cancel,
        )
    };
    for (tone, level) in [
        (0.0, 0),
        (0.0625, 0),
        (0.07, 1),
        (0.25, 2),
        (0.5, 4),
        (1.0, 8),
    ] {
        let mut mapping = SourceMapping::canonical(SourceMappingComponent::Cyan);
        mapping.gain = 0.0;
        mapping.bias = tone;
        let measured = realize(
            mapping,
            GeometryResponseInput::AbsoluteCoverage {
                curve: &curve,
                minimum: 0.0,
                maximum: 1.0,
            },
            &|| false,
        )
        .unwrap();
        let raw = realize(
            mapping,
            GeometryResponseInput::RawProbe(f64::from(level) / 4.0),
            &|| false,
        )
        .unwrap();
        assert_eq!(measured.output.strokes, raw.output.strokes);
    }
    let mapping = SourceMapping::canonical(SourceMappingComponent::Cyan);
    let bounded = realize(
        mapping,
        GeometryResponseInput::AbsoluteCoverage {
            curve: &curve,
            minimum: 0.25,
            maximum: 0.5,
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        bounded.output.strokes,
        realize(mapping, GeometryResponseInput::RawProbe(1.0), &|| false)
            .unwrap()
            .output
            .strokes
    );
    assert!(realize(mapping, GeometryResponseInput::Identity, &|| true).is_err());
}

/// Verifies circular rails at diagonal curvature and after partial-ring threshold clipping.
/// # Panics
/// Panics if a cubic ring becomes a diamond, has a center fill, or emits long outline spikes.
#[test]
fn hatch_ring_outlines_follow_mid_quadrant_curvature() {
    let (document, plan, family) = prepare(true);
    let source = SourceField::from_straight_rgba8(4, 4, [0, 255, 255, 255].repeat(16)).unwrap();
    let realization = realize_typed_canonical_strokes_with_response_cancellable(
        &family,
        &plan,
        &source,
        document.canvas(),
        SourceMapping::canonical(SourceMappingComponent::Cyan),
        StrokeResponse {
            minimum_thickness: 0.0,
            maximum_thickness: 1.0,
            bias: 0.0,
        },
        10.24,
        2_000_000,
        2_000_000,
        GeometryResponseInput::RawProbe(2.0),
        &|| false,
    )
    .unwrap();
    let center = toniator_patterns::Point2::new(32.0, 32.0);
    let mut checked = 0;
    for stroke in realization
        .output
        .strokes
        .iter()
        .filter(|s| s.path.closure() == PathClosure::Closed)
    {
        let bounds = stroke.path.bounds().unwrap();
        let r = (bounds.max.x - bounds.min.x) * 0.5;
        if r < stroke.nominal_basis * 2.0 {
            continue;
        }
        assert!(
            stroke.profile.len() > 8,
            "curved profile needs actual geometric refinement"
        );
        for contour in &stroke.outline.contours {
            for segment in &contour.segments {
                for parameter in [0.0, 0.5, 1.0] {
                    let p = segment.point_at(parameter).unwrap();
                    let distance = (p.x - center.x).hypot(p.y - center.y);
                    assert!(
                        distance >= r - stroke.nominal_basis * 0.5 - 0.25
                            && distance <= r + stroke.nominal_basis * 0.5 + 0.25,
                        "radius={r} distance={distance}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 100);
}

/// Reproduces fine concentric carrier construction without charging disjoint rings as crossings.
/// The ordinary Feature-size density and canonical circle paths remain authoritative.
/// # Panics
/// Panics if a valid quarter-size carrier hits generic crossing cleanup or loses nested topology.
#[test]
fn fine_concentric_carriers_keep_nested_circle_topology() {
    let (_, _, family) = prepare_at(
        true,
        CanvasSpec {
            width: 1360.0,
            height: 768.0,
        },
        400.0,
    );
    let logical = family.logical_tone_paths();
    assert!(logical.len() > 200);
    let center = toniator_patterns::Point2::new(680.0, 384.0);
    let outer_radius = logical
        .iter()
        .map(|path| {
            let point = path.instance.path.start();
            (point.x - center.x).hypot(point.y - center.y)
        })
        .fold(0.0_f64, f64::max);
    assert!(
        outer_radius >= 680.0_f64.hypot(384.0),
        "all four true canvas corners require carrier support"
    );
    for path in logical {
        if path.instance.id.repetition_index != 0 {
            assert_eq!(path.instance.path.closure(), PathClosure::Closed);
        }
        assert_eq!(path.instance.path.segments().len(), 4);
    }
}

/// Exercises quarter-size source-driven hatch profiles and exact outlines on both immutable artworks.
/// Source clipping changes membership and length while widths and contour authority remain canonical.
/// # Panics
/// Panics if fine family construction, source threshold splitting or bounded outlines fail.
#[test]
fn fine_concentric_artwork_keeps_constant_width_and_continuity() {
    for (name, hint) in [
        (
            "raster-sample.png",
            toniator_sampling::SourceFormatHint::Png,
        ),
        (
            "vector-sample.svg",
            toniator_sampling::SourceFormatHint::Svg,
        ),
    ] {
        let bytes = std::fs::read(format!(
            "{}/../../assets/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let source = toniator_sampling::decode_source(&bytes, hint).unwrap();
        let canvas = CanvasSpec {
            width: f64::from(source.identity().width),
            height: f64::from(source.identity().height),
        };
        let (document, plan, family) = prepare_at(true, canvas, 400.0);
        let realization = realize_typed_canonical_strokes_with_response_cancellable(
            &family,
            &plan,
            &source,
            document.canvas(),
            SourceMapping::canonical(SourceMappingComponent::Cyan),
            StrokeResponse {
                minimum_thickness: 0.0,
                maximum_thickness: 1.0,
                bias: 0.0,
            },
            10.24,
            64_000_000,
            8_000_000,
            GeometryResponseInput::Identity,
            &|| false,
        )
        .unwrap();
        assert!(!realization.output.strokes.is_empty());
        for stroke in &realization.output.strokes {
            assert!(
                stroke
                    .profile
                    .iter()
                    .all(|sample| sample.width == stroke.nominal_basis
                        && sample.normalized_thickness == 1.0)
            );
            for pair in stroke.path.segments().windows(2) {
                assert_eq!(pair[0].end(), pair[1].start());
            }
            assert_eq!(stroke.outline.fill_rule, CanonicalFillRule::NonZero);
            assert!(!stroke.outline.contours.is_empty());
        }
    }
}

/// Keeps exact shared crossing nodes for two intersecting hatch-circle dimensions.
/// The single-dimension disjointness proof cannot authorize bypassing a mixed arrangement.
/// # Panics
/// Panics if ordinary mixed-family publication fails or intersections lose shared canonical nodes.
#[test]
fn mixed_hatch_circle_dimensions_retain_solved_crossing_nodes() {
    use toniator_domain::{
        AuthoredPoint2, GenericGuidePrototypeDraft, GuideRepetition, PatternStructureRecipe,
    };
    let mut recipe = interleaved_hatch_recipe(true);
    let PatternStructureRecipe::OrderedOutputs { definition, .. } = &mut recipe.structure else {
        panic!("ordinary hatch outputs");
    };
    let PatternStructureRecipe::GenericGuides { dimensions, .. } = definition.as_mut() else {
        panic!("ordinary circle guides");
    };
    dimensions[0].prototype = GenericGuidePrototypeDraft::CircularArc {
        center: AuthoredPoint2 { x: -4.0, y: 0.0 },
        radius: 16.0,
        start_angle_degrees: 0.0,
        sweep_angle_degrees: 360.0,
    };
    let GuideRepetition::NormalOffset { spacing, .. } = &mut dimensions[0].repetition else {
        panic!("normal offset");
    };
    *spacing = 128.0;
    let mut second = dimensions[0].clone();
    let GenericGuidePrototypeDraft::CircularArc { center, .. } = &mut second.prototype else {
        panic!("circle");
    };
    center.x = 4.0;
    dimensions.push(second);
    let canvas = CanvasSpec {
        width: 64.0,
        height: 64.0,
    };
    let mut document = Document::new_default_document(canvas, SourceReference::Unassigned).unwrap();
    document = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: document.pattern_settings().clone(),
            base_definition: document.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let plan = resolve_document_pattern_pipeline_for_channel(
        &document,
        document.pattern_definition_for(ChannelId(1)).unwrap(),
        toniator_domain::SourceWeighting::canonical(SourceMappingComponent::Cyan),
    )
    .unwrap();
    let request = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: DensityMetric2D {
            density: 6.25,
            aspect: 1.0,
        }
        .resolve(document.canvas())
        .unwrap(),
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 0,
        support_radius: 0.0,
        max_family_candidates: 100_000,
    };
    let family = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    let roots = family
        .structural_path_set()
        .unwrap()
        .paths()
        .iter()
        .filter(|path| path.id.repetition_index == 0)
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    let nodes = |path: &toniator_geometry::StructuralPathInstance| {
        path.path
            .segments()
            .iter()
            .flat_map(|segment| [segment.start(), segment.end()])
            .collect::<Vec<_>>()
    };
    let first = nodes(roots[0]);
    let second = nodes(roots[1]);
    let mut shared = first
        .into_iter()
        .filter(|point| second.contains(point))
        .collect::<Vec<_>>();
    shared.sort_by(|a, b| a.y.total_cmp(&b.y));
    shared.dedup();
    assert_eq!(shared.len(), 2);
    assert!(shared[0].y < 32.0 && shared[1].y > 32.0);
    assert!(roots.iter().all(|path| path.path.segments().len() > 4));
}

/// Checks placed/aspect-resolved analytic support and retains preflight/cancellation authority.
/// # Panics
/// Panics if transformed canvas corners exceed the generated ring or a bounded request overallocates.
#[test]
fn analytic_concentric_placement_and_resource_limits_remain_bounded() {
    let (_, plan, _) = prepare(true);
    let canvas = CanvasSpec {
        width: 1360.,
        height: 768.,
    };
    let mut request = GridInspectRequest {
        canvas: canvas.clone(),
        density: DensityMetric2D {
            density: 100.,
            aspect: 2.5,
        }
        .resolve(&canvas)
        .unwrap(),
        rotation_degrees: 33.,
        translation_x: 430.,
        translation_y: -210.,
        guard_steps: 2,
        support_radius: 3.4,
        max_family_candidates: 16,
    };
    assert_eq!(
        evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request,
            None,
            &|| false
        )
        .unwrap_err()
        .path(),
        "coverage.curved_guides.instance_limit"
    );
    assert_eq!(
        evaluate_typed_family_product_with_source_cancellable(
            &plan.family,
            &request,
            None,
            &|| true
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    request.max_family_candidates = 2_000_000;
    let family = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    let center = toniator_patterns::Point2::new(1110., 174.);
    let reach = [(0., 0.), (1360., 0.), (0., 768.), (1360., 768.)]
        .into_iter()
        .map(|(x, y)| (x - center.x).hypot(y - center.y))
        .fold(0.0_f64, f64::max);
    let actual = family
        .logical_tone_paths()
        .iter()
        .map(|path| {
            let p = path.instance.path.start();
            (p.x - center.x).hypot(p.y - center.y)
        })
        .fold(0.0_f64, f64::max);
    assert!(actual >= reach);
}
