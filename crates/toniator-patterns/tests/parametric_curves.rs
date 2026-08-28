use toniator_domain::{
    CanvasSpec, CoveragePolicy, CurveWinding, GuideRepetition, MarkOrientation, MarkPrototype,
    OffsetCleanup, OffsetSides, ParametricCurve, PathStrokeStyle, PatternDefinition,
    PatternDefinitionId, PatternFamily, PatternMechanism, PatternMechanismId, PatternModulation,
    PatternOutputLayer, PatternOutputLayerId, PatternOutputRealization, ResolvedDensityMetric2D,
    SpiralCurve, SpiralShape,
};
use toniator_patterns::{
    GridInspectRequest, StructuralProductCapability, evaluate_typed_family,
    evaluate_typed_family_product_with_source_progress_cancellable, resolve_pattern_pipeline,
};

/// Builds a deterministic bounded request with room for one finite source.
fn request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 320.0,
            height: 240.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 32.0,
            across_y: 24.0,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 2.0,
        max_family_candidates: 16_384,
    }
}

/// Builds a homogeneous raw-path or equal-arc-site parametric spiral definition.
fn definition(shape: SpiralShape, sites: bool) -> PatternDefinition {
    let curve_id = PatternMechanismId(91);
    let site_id = PatternMechanismId(92);
    PatternDefinition {
        id: PatternDefinitionId(90),
        name: "Stage 20K spiral".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id: sites.then_some(site_id),
        },
        mechanisms: if sites {
            vec![
                PatternMechanism::ParametricCurveSource {
                    id: curve_id,
                    curve: ParametricCurve::Spiral(SpiralCurve {
                        shape,
                        turns: 4.0,
                        radial_spacing: 24.0,
                        phase_degrees: 0.0,
                        winding: CurveWinding::CounterClockwise,
                    }),
                    repetition: GuideRepetition::Single,
                },
                PatternMechanism::AlongParametricCurveSites {
                    id: site_id,
                    curve_mechanism_id: curve_id,
                    interval: 18.0,
                    phase: 0.25,
                },
            ]
        } else {
            vec![PatternMechanism::ParametricCurveSource {
                id: curve_id,
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape,
                    turns: 4.0,
                    radial_spacing: 24.0,
                    phase_degrees: 0.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: GuideRepetition::Single,
            }]
        },
        output_layers: if sites {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(93),
                PatternOutputRealization::MarkPrototype {
                    site_mechanism_id: site_id,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            )]
        } else {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(93),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: curve_id,
                    style: PathStrokeStyle::default(),
                },
            )]
        },
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    }
}

/// Resolves round raw paths and observes completed parametric/generic family work.
#[test]
fn round_spiral_publishes_one_cubic_path_product() {
    let plan =
        resolve_pattern_pipeline(&definition(SpiralShape::Round, false)).expect("valid plan");
    assert_eq!(
        plan.family.product,
        StructuralProductCapability::ParametricPaths
    );
    let progress = std::sync::Mutex::new(Vec::new());
    let output = evaluate_typed_family_product_with_source_progress_cancellable(
        &plan.family,
        &request(),
        None,
        &|| false,
        &|completed, total| progress.lock().unwrap().push((completed, total)),
    )
    .expect("valid output");
    let paths = output.structural_path_set().expect("raw path output");
    assert_eq!(paths.paths().len(), 1);
    assert!(matches!(
        paths.paths()[0].id.source,
        toniator_patterns::StructuralPathSourceId::ParametricCurve(PatternMechanismId(91))
    ));
    assert!(
        paths.paths()[0]
            .path
            .segments()
            .iter()
            .all(|segment| matches!(segment, toniator_patterns::CurveSegment::CubicBezier(_)))
    );
    let progress = progress.into_inner().unwrap();
    assert!(
        progress
            .iter()
            .any(|&(completed, total)| completed > 0 && completed < total),
        "parametric construction and finite-path work advance within the family stage"
    );
    assert_eq!(progress.last(), Some(&(1_000, 1_000)));
    assert!(progress.windows(2).all(|pair| pair[0].0 <= pair[1].0));
}

/// Locks parametric geometry and identity outside the centered grid-prototype transform contract.
///
/// # Panics
///
/// Panics when the parametric structural-source adapter changes its established placement or
/// family identity while local grid prototypes adopt the centered origin.
#[test]
fn parametric_family_geometry_and_identity_ignore_grid_local_origin_corrections() {
    let output = evaluate_typed_family(&definition(SpiralShape::Round, true), &request())
        .expect("parametric family evaluates");
    assert_eq!(
        output.family_fingerprint(),
        "toniator-stage-20d-guide-family-v1:fnv1a64:c9d411712b5a44b4:nominal-cell-basis:fnv1a64:4ec48ee5f2b4ecf5"
    );
    assert_eq!(
        output.site_set().sites()[0].position,
        toniator_patterns::Point2::new(162.024_440_794_682_28, 123.336_300_100_532_98)
    );
}

/// Proves NormalOffset cleanup components retain one ordered parametric source and phase sequence.
#[test]
fn normal_offset_sites_keep_path_neutral_repetition_identity() {
    let mut definition = definition(SpiralShape::Square, true);
    let PatternMechanism::ParametricCurveSource {
        repetition,
        curve: ParametricCurve::Spiral(spiral),
        ..
    } = &mut definition.mechanisms[0]
    else {
        panic!("fixture starts with spiral source");
    };
    spiral.turns = 0.25;
    *repetition = GuideRepetition::NormalOffset {
        spacing: 36.0,
        sides: OffsetSides::Both,
        cleanup: OffsetCleanup::DissolveCrossings,
    };
    let output = evaluate_typed_family(&definition, &request()).expect("offset sites");
    let paths = output.structural_path_set().expect("paths");
    assert!(paths.paths().iter().all(|path| matches!(
        path.id.source,
        toniator_patterns::StructuralPathSourceId::ParametricCurve(PatternMechanismId(91))
    )));
    assert!(output.site_set().sites().iter().all(|site| matches!(&site.provenance, toniator_patterns::FamilySiteProvenance::AlongParametricCurve { location, .. } if matches!(location.path.source, toniator_patterns::StructuralPathSourceId::ParametricCurve(PatternMechanismId(91))))));
}

/// Proves parametric sites use their authored absolute interval tangentially and their
/// source repetition spacing normally, independent of anisotropic document density.
#[test]
fn parametric_site_basis_separates_absolute_interval_from_radial_spacing() {
    let mut request = request();
    request.density.across_x = 80.0;
    request.density.across_y = 12.0;
    let output = evaluate_typed_family(&definition(SpiralShape::Round, true), &request)
        .expect("anisotropic parametric sites");
    assert!(output.site_set().sites().iter().all(|site| {
        (site
            .nominal_cell_basis
            .axis_a
            .x
            .hypot(site.nominal_cell_basis.axis_a.y)
            - 18.0)
            .abs()
            < 1.0e-9
            && (site
                .nominal_cell_basis
                .axis_b
                .x
                .hypot(site.nominal_cell_basis.axis_b.y)
                - 24.0)
                .abs()
                < 1.0e-9
    }));
}

/// Proves transform-stack parametric repetition publishes multiple path instances with
/// path-neutral identity and retains the resolved stack spacing as the normal site basis.
#[test]
fn transform_stack_parametric_sites_retain_path_neutral_spacing_authority() {
    let mut definition = definition(SpiralShape::Square, true);
    let PatternMechanism::ParametricCurveSource { repetition, .. } = &mut definition.mechanisms[0]
    else {
        panic!("fixture starts with parametric source");
    };
    *repetition = GuideRepetition::TransformStack {
        direction_degrees: 0.0,
        spacing_multiplier: 1.5,
    };
    let output = evaluate_typed_family(&definition, &request()).expect("stack output");
    let paths = output.structural_path_set().expect("stack paths");
    assert!(paths.paths().len() > 1);
    assert!(paths.paths().iter().all(|path| matches!(
        path.id.source,
        toniator_patterns::StructuralPathSourceId::ParametricCurve(PatternMechanismId(91))
    )));
    assert!(output.site_set().sites().iter().all(|site| {
        (site
            .nominal_cell_basis
            .axis_b
            .x
            .hypot(site.nominal_cell_basis.axis_b.y)
            - 15.0)
            .abs()
            < 1.0e-9
    }));
}

/// Preserves exact square corners and equal-arc sites without turning a path family into marks.
#[test]
fn square_spiral_sites_follow_the_shared_curve_product() {
    let output = evaluate_typed_family(&definition(SpiralShape::Square, true), &request())
        .expect("valid output");
    let paths = output
        .structural_path_set()
        .expect("source path remains available");
    assert!(
        paths.paths()[0]
            .path
            .segments()
            .iter()
            .all(|segment| matches!(segment, toniator_patterns::CurveSegment::Line(_)))
    );
    assert!(!output.site_set().sites().is_empty());
}

/// Reuses the accepted normal-offset evaluator for a finite parametric source without canvas endpoints.
#[test]
fn parametric_paths_compose_with_stage20j_normal_offset() {
    let mut definition = definition(SpiralShape::Square, false);
    let PatternMechanism::ParametricCurveSource {
        curve: ParametricCurve::Spiral(spiral),
        repetition,
        ..
    } = &mut definition.mechanisms[0]
    else {
        panic!("fixture starts with spiral source");
    };
    spiral.turns = 0.25;
    *repetition = GuideRepetition::NormalOffset {
        spacing: 36.0,
        sides: OffsetSides::Both,
        cleanup: OffsetCleanup::DissolveCrossings,
    };
    let output = evaluate_typed_family(&definition, &request()).expect("offset output");
    assert!(output.structural_path_set().expect("paths").paths().len() > 1);
}
