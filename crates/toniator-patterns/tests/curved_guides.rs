use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, DensityMetric2D, Document, DocumentId,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
    MarkOrientation, OffsetCleanup, PatternDefinition, PatternDefinitionBundle,
    PatternDefinitionId, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputSettings, ResolvedDensityMetric2D, SourceComponent,
    SourcePlacement, SourceReference,
};
use toniator_geometry::{AffineTransform2D, FamilySiteProvenance, Point2, SiteScope, Vector2};
use toniator_patterns::{
    GridInspectRequest, MarkResponse, directional_spacing,
    evaluate_document_typed_family_cancellable,
    evaluate_typed_family_product_with_source_progress_cancellable, maximum_nominal_cell_diameter,
    realize_typed_diagnostic_outputs, resolve_document_pattern_pipeline, resolve_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds a document-aware generic guide definition with the fixed Stage 20D root identities.
fn definition(
    dimensions: Vec<GuideDimension>,
    product: GeneralizedSiteProduct,
) -> PatternDefinition {
    PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "curves",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        dimensions,
        product,
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    )
}

/// Builds the modeled document boundary that owns supplied generic-guide resources.
fn document(definition: PatternDefinition, structures: Vec<AuthoredStructure>) -> Document {
    document_with_canvas(
        definition,
        structures,
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
    )
}

/// Builds the modeled document boundary at an explicit canvas for support-bound tests.
fn document_with_canvas(
    definition: PatternDefinition,
    structures: Vec<AuthoredStructure>,
    canvas: CanvasSpec,
) -> Document {
    let base = Document::new_default_document(canvas, SourceReference::Unassigned).unwrap();
    Document::with_source_topology_and_authored_structures(
        DocumentId(1),
        base.canvas().clone(),
        SourceReference::Unassigned,
        vec![PatternDefinitionBundle {
            output_settings: definition
                .output_layers
                .iter()
                .map(|output| PatternOutputSettings {
                    output_layer_id: output.id(),
                    response: match &output.realization {
                        PatternOutputRealization::CircularMarks { .. }
                        | PatternOutputRealization::MarkPrototype { .. } => {
                            PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                                minimum_fill: 0.0,
                                maximum_fill: 1.0,
                            })
                        }
                        _ => panic!("curved guide fixtures own only mark outputs"),
                    },
                })
                .collect(),
            definition: definition.clone(),
        }],
        {
            let mut settings = base.pattern_settings().clone();
            settings.definition_id = definition.id;
            settings
        },
        base.channel_model().unwrap().to_owned(),
        base.channel_topology().unwrap().clone(),
        structures,
    )
    .unwrap()
}

/// Builds one resolved authored open-line guide without manufacturing curve geometry in patterns.
fn line(id: u64, start: AuthoredPoint2, end: AuthoredPoint2) -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(id),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line { start, end }],
    )
    .unwrap()
}

/// Builds one deterministic document-space request at an explicit aggregate work limit.
fn request(max_family_candidates: usize) -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        density: ResolvedDensityMetric2D {
            across_x: 10.0,
            across_y: 20.0,
        },
        rotation_degrees: 17.0,
        translation_x: 4.0,
        translation_y: -3.0,
        guard_steps: 1,
        support_radius: 4.5,
        max_family_candidates,
    }
}

/// Builds the square test request at one artist-facing Pattern size.
///
/// # Panics
///
/// Panics only if the fixed positive square canvas or density fixture ceases to validate.
fn request_at_pattern_size(max_family_candidates: usize, pattern_size: f64) -> GridInspectRequest {
    let mut request = request(max_family_candidates);
    let default = DensityMetric2D::default_for_canvas(&request.canvas)
        .expect("fixed square canvas has a default density");
    request.density = DensityMetric2D {
        density: default.density / pattern_size,
        aspect: 1.0,
    }
    .resolve(&request.canvas)
    .expect("fixed positive Pattern size resolves");
    request
}

/// Proves resolved authored guides report work and merge contributors in selected order.
#[test]
fn curved_guides_reuse_existing_site_products_with_truthful_guide_and_site_sets() {
    let dimensions = vec![
        GuideDimension {
            id: GuideDimensionId(9),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(9),
            },
            repetition: GuideRepetition::Single,
        },
        GuideDimension {
            id: GuideDimensionId(2),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(2),
            },
            repetition: GuideRepetition::Single,
        },
        GuideDimension {
            id: GuideDimensionId(7),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(7),
            },
            repetition: GuideRepetition::Single,
        },
    ];
    let definition = definition(
        dimensions,
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![
                GuideDimensionId(9),
                GuideDimensionId(2),
                GuideDimensionId(7),
            ],
            merge_epsilon: 0.01,
        },
    );
    let document = document(
        definition.clone(),
        vec![
            line(
                9,
                AuthoredPoint2 { x: -50.0, y: 0.0 },
                AuthoredPoint2 { x: 50.0, y: 0.0 },
            ),
            line(
                2,
                AuthoredPoint2 { x: 0.0, y: -50.0 },
                AuthoredPoint2 { x: 0.0, y: 50.0 },
            ),
            line(
                7,
                AuthoredPoint2 { x: -50.0, y: -50.0 },
                AuthoredPoint2 { x: 50.0, y: 50.0 },
            ),
        ],
    );
    assert_eq!(
        resolve_pattern_pipeline(&definition).unwrap_err().path(),
        "pattern.pipeline.guide_resources"
    );
    let plan = resolve_document_pattern_pipeline(&document, &definition)
        .expect("document-aware authored curve pipeline resolves");
    let progress = std::sync::Mutex::new(Vec::new());
    let output = evaluate_typed_family_product_with_source_progress_cancellable(
        &plan.family,
        &request(64),
        None,
        &|| false,
        &|completed, total| progress.lock().unwrap().push((completed, total)),
    )
    .expect("document-aware authored curve product evaluates");
    assert_eq!(output.structural_path_set().unwrap().paths().len(), 3);
    assert_eq!(
        output.site_set().sites().len(),
        1,
        "three pair contacts merge once"
    );
    let site = &output.site_set().sites()[0];
    assert_eq!(site.scope, SiteScope::Canvas);
    match &site.provenance {
        FamilySiteProvenance::CurveGuideIntersection { contributors } => assert_eq!(
            contributors
                .iter()
                .map(|value| match value.path.source {
                    toniator_patterns::StructuralPathSourceId::GuideDimension(id) => id.0,
                    toniator_patterns::StructuralPathSourceId::ParametricCurve(id) => id.0,
                })
                .collect::<Vec<_>>(),
            vec![9, 2, 7],
            "selected dimension order, not numeric ID order, owns merged provenance"
        ),
        other => panic!("expected curved intersection provenance, got {other:?}"),
    }
    let progress = progress.into_inner().unwrap();
    assert!(
        progress
            .iter()
            .any(|&(completed, total)| completed > 0 && completed < total),
        "generic guide expansion and curve contacts advance within the family stage"
    );
    assert_eq!(progress.last(), Some(&(1_000, 1_000)));
    assert!(progress.windows(2).all(|pair| pair[0].0 <= pair[1].0));
}

/// Places generic curve-guide prototypes from their shared local grid origin.
///
/// # Panics
///
/// Panics when local generic prototypes do not rotate about zero and then translate from the
/// geometric canvas center, or when their typed family loses the zero-index intersection.
#[test]
fn generic_curve_grid_prototypes_share_the_centered_local_origin_transform() {
    let definition = definition(
        vec![
            GuideDimension {
                id: GuideDimensionId(71),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(71),
                },
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(72),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(72),
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(71), GuideDimensionId(72)],
            merge_epsilon: 1e-9,
        },
    );
    let document = document(
        definition.clone(),
        vec![
            line(
                71,
                AuthoredPoint2 { x: -40.0, y: 0.0 },
                AuthoredPoint2 { x: 40.0, y: 0.0 },
            ),
            line(
                72,
                AuthoredPoint2 { x: 0.0, y: -40.0 },
                AuthoredPoint2 { x: 0.0, y: 40.0 },
            ),
        ],
    );
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request(1_000),
        &|| false,
    )
    .expect("local curve-grid prototypes evaluate");
    let expected = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        17.0,
        Vector2::new(54.0, 47.0),
    )
    .expect("finite centered transform")
    .apply_point(Point2::new(0.0, 0.0));
    assert!(
        output
            .site_set()
            .sites()
            .iter()
            .any(|site| (site.position.x - expected.x).abs() < 1.0e-9
                && (site.position.y - expected.y).abs() < 1.0e-9)
    );
}

/// Proves curved guide limits, tangencies, overlaps, and cancellation fail before any family output publishes.
#[test]
fn curved_guide_limits_cancellation_and_geometry_failures_publish_no_partial_output() {
    let tangent_dimensions = vec![
        GuideDimension {
            id: GuideDimensionId(1),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: -10.0, y: 0.0 },
                radius: 10.0,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
            },
            repetition: GuideRepetition::Single,
        },
        GuideDimension {
            id: GuideDimensionId(2),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: 10.0, y: 0.0 },
                radius: 10.0,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
            },
            repetition: GuideRepetition::Single,
        },
    ];
    let tangent_definition = definition(
        tangent_dimensions.clone(),
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
    );
    let tangent_document = document(tangent_definition.clone(), vec![]);
    assert!(resolve_document_pattern_pipeline(&tangent_document, &tangent_definition).is_ok());
    let tangent = evaluate_document_typed_family_cancellable(
        &tangent_document,
        &tangent_definition,
        &request(256),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        tangent.site_set().sites().len(),
        1,
        "a tangency remains a site"
    );
    assert_eq!(
        evaluate_document_typed_family_cancellable(
            &tangent_document,
            &tangent_definition,
            &request(20),
            &|| false,
        )
        .unwrap_err()
        .path(),
        "coverage.curved_guides.merge_limit",
        "preflight bounds segment-pair merge work before intersection allocation"
    );
    assert_eq!(
        evaluate_document_typed_family_cancellable(
            &tangent_document,
            &tangent_definition,
            &request(64),
            &|| true,
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    let mut identical = tangent_dimensions[0].clone();
    identical.id = GuideDimensionId(2);
    let overlap_definition = definition(
        vec![tangent_dimensions[0].clone(), identical],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
    );
    assert_eq!(
        evaluate_document_typed_family_cancellable(
            &document(overlap_definition.clone(), vec![]),
            &overlap_definition,
            &request(256),
            &|| false,
        )
        .unwrap_err()
        .path(),
        "curve.path.intersections.overlap",
        "geometry failure returns before a family-site set can publish"
    );
}

/// Proves transform-stack coverage retains raw phase, final document scope, and variable anisotropic intervals.
#[test]
fn curved_transform_stacks_and_along_guides_preserve_scope_phase_and_variable_intervals() {
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(19),
            baseline_angle_degrees: 0.0,
            phase: 7.0,
            prototype: GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
            },
            repetition: GuideRepetition::TransformStack {
                direction_degrees: 0.0,
                spacing_multiplier: 0.5,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(19)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let document = document(definition.clone(), vec![]);
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request(20_000),
        &|| false,
    )
    .expect("stacked finite arcs publish bounded along-guide sites");
    let guides = output.structural_path_set().unwrap().paths();
    assert!(guides.len() > 1);
    assert!(guides.windows(2).all(|pair| {
        (pair[0].id.repetition_index, pair[0].id.component_ordinal)
            < (pair[1].id.repetition_index, pair[1].id.component_ordinal)
    }));
    let raw_phase = guides
        .iter()
        .find(|guide| guide.id.repetition_index == 0)
        .expect(
            "index zero must retain the raw authored phase rather than a normalized lattice label",
        );
    let expected = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        17.0,
        Vector2::new(54.0, 47.0),
    )
    .unwrap()
    .apply_point(Point2::new(20.0 + 7.0, 0.0));
    assert!((raw_phase.path.start().x - expected.x).abs() < 1.0e-10);
    assert!((raw_phase.path.start().y - expected.y).abs() < 1.0e-10);
    let next = guides
        .iter()
        .find(|guide| guide.id.repetition_index == 1)
        .expect("the raw phase lattice also retains index one");
    let expected_next = AffineTransform2D::rotate_about_then_translate(
        Point2::new(0.0, 0.0),
        17.0,
        Vector2::new(54.0, 47.0),
    )
    .unwrap()
    .apply_point(Point2::new(20.0 + 7.0 + 5.0, 0.0));
    assert!((next.path.start().x - expected_next.x).abs() < 1.0e-10);
    assert!((next.path.start().y - expected_next.y).abs() < 1.0e-10);
    let sites = output.site_set().sites();
    assert!(sites.iter().any(|site| site.scope == SiteScope::Canvas));
    assert!(sites.iter().any(|site| site.scope == SiteScope::Guard));
    let intervals = sites
        .windows(2)
        .filter_map(|pair| match (&pair[0].provenance, &pair[1].provenance) {
            (
                FamilySiteProvenance::CurveAlongGuide {
                    location: first_location,
                    absolute_arc_position_bits: first,
                    ..
                },
                FamilySiteProvenance::CurveAlongGuide {
                    location: second_location,
                    absolute_arc_position_bits: second,
                    ..
                },
            ) if first_location.path == second_location.path => {
                Some(f64::from_bits(*second) - f64::from_bits(*first))
            }
            _ => None,
        })
        .filter(|interval| *interval > 0.0)
        .collect::<Vec<_>>();
    assert!(!intervals.is_empty());
    let spec = CanvasSpec {
        width: 100.0,
        height: 100.0,
    };
    let density = ResolvedDensityMetric2D {
        across_x: 10.0,
        across_y: 20.0,
    };
    let horizontal_normal = directional_spacing(&spec, &density, Vector2::new(1.0, 0.0)).unwrap();
    let vertical_normal = directional_spacing(&spec, &density, Vector2::new(0.0, 1.0)).unwrap();
    assert_eq!(horizontal_normal.to_bits(), 10.0_f64.to_bits());
    assert_eq!(vertical_normal.to_bits(), 5.0_f64.to_bits());
    assert!(
        intervals
            .iter()
            .any(|interval| *interval < horizontal_normal),
        "sampled local tangents select the smaller anisotropic interval where appropriate"
    );
}

/// Proves one curved repetition populates every canvas tile across wide, tall, and rotated layouts.
///
/// This is an independent realized-site witness rather than a planner-bound assertion: every
/// quarter of the final canvas in both axes must contain at least one canvas-scoped site. The
/// coverage contract therefore cannot pass merely because some repeated geometry remains visible.
///
/// # Panics
///
/// Panics when the repetition cannot evaluate or leaves any final-canvas tile without geometry.
fn assert_curved_repetition_covers_canvas(repetition_name: &str, repetition: GuideRepetition) {
    for canvas in [
        CanvasSpec {
            width: 320.0,
            height: 120.0,
        },
        CanvasSpec {
            width: 120.0,
            height: 320.0,
        },
    ] {
        for rotation_degrees in [0.0, 37.0] {
            let definition = definition(
                vec![GuideDimension {
                    id: GuideDimensionId(91),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    prototype: GuidePrototype::AuthoredOpenPath {
                        structure_id: AuthoredStructureId(91),
                    },
                    repetition: repetition.clone(),
                }],
                GeneralizedSiteProduct::AlongGuides {
                    dimensions: vec![GuideDimensionId(91)],
                    interval_multiplier: 0.75,
                    phase: 0.0,
                },
            );
            let curve = AuthoredStructure::new(
                AuthoredStructureId(91),
                AuthoredStructureKind::OpenPath,
                vec![AuthoredCurveSegment::CubicBezier {
                    start: AuthoredPoint2 {
                        x: canvas.width * -0.5,
                        y: 0.0,
                    },
                    control_1: AuthoredPoint2 {
                        x: canvas.width / -6.0,
                        y: canvas.height * -0.18,
                    },
                    control_2: AuthoredPoint2 {
                        x: canvas.width / 6.0,
                        y: 0.0,
                    },
                    end: AuthoredPoint2 {
                        x: canvas.width * 0.5,
                        y: 0.0,
                    },
                }],
            )
            .expect("coverage witness curve validates");
            let document = document_with_canvas(definition.clone(), vec![curve], canvas.clone());
            let request = GridInspectRequest {
                canvas: canvas.clone(),
                density: ResolvedDensityMetric2D {
                    across_x: canvas.width / 8.0,
                    across_y: canvas.height / 8.0,
                },
                rotation_degrees,
                translation_x: 0.0,
                translation_y: 0.0,
                guard_steps: 2,
                support_radius: 4.5,
                max_family_candidates: 100_000,
            };
            let output = evaluate_document_typed_family_cancellable(
                &document,
                &definition,
                &request,
                &|| false,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{repetition_name} evaluates on {}x{} at {rotation_degrees} degrees: {error}",
                    canvas.width, canvas.height
                )
            });
            let canvas_sites = output
                .site_set()
                .sites()
                .iter()
                .filter(|site| site.scope == SiteScope::Canvas)
                .collect::<Vec<_>>();
            for tile_y in 0..4 {
                for tile_x in 0..4 {
                    let minimum_x = canvas.width * f64::from(tile_x) / 4.0;
                    let maximum_x = canvas.width * f64::from(tile_x + 1) / 4.0;
                    let minimum_y = canvas.height * f64::from(tile_y) / 4.0;
                    let maximum_y = canvas.height * f64::from(tile_y + 1) / 4.0;
                    assert!(
                        canvas_sites.iter().any(|site| {
                            site.position.x >= minimum_x
                                && site.position.x <= maximum_x
                                && site.position.y >= minimum_y
                                && site.position.y <= maximum_y
                        }),
                        "{repetition_name} left tile ({tile_x}, {tile_y}) empty on {}x{} at {rotation_degrees} degrees",
                        canvas.width,
                        canvas.height
                    );
                }
            }
        }
    }
}

/// Proves Stacked curved copies fill wide, tall, and rotated final canvases.
///
/// # Panics
///
/// Panics when finite stacked endpoints leave any final-canvas tile uncovered.
#[test]
fn curved_transform_stack_covers_wide_tall_and_rotated_canvases() {
    assert_curved_repetition_covers_canvas(
        "Stacked",
        GuideRepetition::TransformStack {
            direction_degrees: 90.0,
            spacing_multiplier: 1.0,
        },
    );
}

/// Proves clipped Stacked output retains the preset-owned nominal pitch after longitudinal scale.
///
/// # Panics
///
/// Panics when the source and adjacent rank do not publish, lose the authoritative eight-unit
/// nominal basis, or fail to retain a longitudinally scaled source span.
#[test]
fn stacked_curve_scales_once_without_scaling_its_repetition_pitch() {
    let canvas = CanvasSpec {
        width: 320.0,
        height: 120.0,
    };
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(92),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(92),
            },
            repetition: GuideRepetition::TransformStack {
                direction_degrees: 90.0,
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(92)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let curve = AuthoredStructure::new(
        AuthoredStructureId(92),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: AuthoredPoint2 { x: -60.0, y: 0.0 },
            control_1: AuthoredPoint2 { x: -20.0, y: -24.0 },
            control_2: AuthoredPoint2 { x: 20.0, y: 16.0 },
            end: AuthoredPoint2 { x: 60.0, y: 0.0 },
        }],
    )
    .expect("stacked coverage curve validates");
    let document = document_with_canvas(definition.clone(), vec![curve], canvas.clone());
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &GridInspectRequest {
            canvas: canvas.clone(),
            density: ResolvedDensityMetric2D {
                across_x: canvas.width / 8.0,
                across_y: canvas.height / 8.0,
            },
            rotation_degrees: 37.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 2,
            support_radius: 4.5,
            max_family_candidates: 100_000,
        },
        &|| false,
    )
    .expect("stacked curve evaluates");
    let paths = output
        .structural_path_set()
        .expect("stacked paths publish")
        .paths();
    let source = paths
        .iter()
        .find(|path| path.id.repetition_index == 0)
        .expect("stacked source index publishes");
    let next = paths
        .iter()
        .find(|path| path.id.repetition_index == 1)
        .expect("adjacent stacked index publishes");
    assert_eq!(output.guide_nominal_basis(source.id), Some(8.0));
    assert_eq!(output.guide_nominal_basis(next.id), Some(8.0));
    assert!(
        source
            .path
            .measure_arc_length()
            .expect("scaled source length remains finite")
            .total_length()
            > 120.0
    );
    assert_ne!(source.path, next.path);
}

/// Proves Constant-gap curved copies fill wide, tall, and rotated final canvases.
///
/// # Panics
///
/// Panics when normal-offset cleanup leaves any final-canvas tile uncovered or cannot prove its
/// requested bilateral coverage.
#[test]
fn curved_normal_offset_covers_wide_tall_and_rotated_canvases() {
    assert_curved_repetition_covers_canvas(
        "Constant-gap",
        GuideRepetition::NormalOffset {
            spacing: 8.0,
            cleanup: OffsetCleanup::DissolveCrossings,
        },
    );
}

/// Proves normal offsets use independent signed document-space gaps and retain source index zero.
#[test]
fn normal_offsets_publish_signed_constant_gap_centerlines() {
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(31),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(31),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 12.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(31)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let document = document(
        definition.clone(),
        vec![line(
            31,
            AuthoredPoint2 { x: -30.0, y: 0.0 },
            AuthoredPoint2 { x: 30.0, y: 0.0 },
        )],
    );
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request_at_pattern_size(1_000_000, 1.0),
        &|| false,
    )
    .expect("normal-offset family evaluates");
    let guides = output
        .structural_path_set()
        .expect("guide paths publish")
        .paths();
    let indices = guides
        .iter()
        .map(|guide| guide.id.repetition_index)
        .collect::<Vec<_>>();
    assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
    let prior = guides
        .iter()
        .find(|guide| guide.id.repetition_index == -1)
        .expect("right offset publishes");
    let source = guides
        .iter()
        .find(|guide| guide.id.repetition_index == 0)
        .expect("source index zero publishes");
    let next = guides
        .iter()
        .find(|guide| guide.id.repetition_index == 1)
        .expect("left offset publishes");
    let source_normal = source.path.segments()[0]
        .unit_normal_at(0.5)
        .expect("straight source has a finite normal");
    let transverse_gap = |first: Point2, second: Point2| {
        Vector2::new(second.x - first.x, second.y - first.y)
            .dot(source_normal)
            .abs()
    };
    assert!((transverse_gap(prior.path.start(), source.path.start()) - 12.0).abs() < 1.0e-9);
    assert!((transverse_gap(source.path.start(), next.path.start()) - 12.0).abs() < 1.0e-9);
    assert_eq!(output.guide_nominal_basis(source.id), Some(12.0));
    assert!(
        source
            .path
            .segments()
            .iter()
            .all(|segment| matches!(segment, toniator_geometry::CurveSegment::Line(_))),
        "coverage scaling and end-to-end tiling preserve authored segment kind"
    );
    let final_canvas =
        toniator_geometry::Bounds::new(Point2::new(0.0, 0.0), Point2::new(100.0, 100.0))
            .expect("fixed final canvas bounds");
    assert!(!final_canvas.contains(source.path.start()));
    assert!(!final_canvas.contains(source.path.end()));

    let larger = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request_at_pattern_size(1_000_000, 2.0),
        &|| false,
    )
    .expect("larger Pattern size evaluates the same constant-gap recipe");
    let larger_guides = larger
        .structural_path_set()
        .expect("larger Pattern size publishes guide paths")
        .paths();
    let larger_source = larger_guides
        .iter()
        .find(|guide| guide.id.repetition_index == 0)
        .expect("larger Pattern size retains source index zero");
    let larger_next = larger_guides
        .iter()
        .find(|guide| guide.id.repetition_index == 1)
        .expect("larger Pattern size publishes the next offset");
    assert!(
        (transverse_gap(larger_source.path.start(), larger_next.path.start()) - 24.0).abs()
            < 1.0e-9,
        "doubling Pattern size doubles the authored constant gap"
    );
    assert_eq!(larger.guide_nominal_basis(larger_source.id), Some(24.0));
}

/// Proves an authored Constant-gap source with unpaired interior nodes fails atomically.
///
/// # Panics
///
/// Panics when the invalid finite source publishes partial guide components or reports a diagnostic
/// outside Constant-gap coverage authority.
#[test]
fn normal_offset_self_crossing_source_with_unpaired_nodes_fails_atomically() {
    let structure = AuthoredStructure::new(
        AuthoredStructureId(41),
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -40.0, y: -40.0 },
                end: AuthoredPoint2 { x: 40.0, y: 40.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 40.0, y: 40.0 },
                end: AuthoredPoint2 { x: -40.0, y: 40.0 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: -40.0, y: 40.0 },
                end: AuthoredPoint2 { x: 40.0, y: -40.0 },
            },
        ],
    )
    .expect("finite crossing structure");
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(41),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(41),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 12.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(41)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let document = document(definition.clone(), vec![structure]);
    let error = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request_at_pattern_size(100_000, 1.0),
        &|| false,
    )
    .expect_err("unpaired Constant-gap endpoints cannot publish a partial frontier");
    assert_eq!(error.path(), "coverage.curved_guides.normal_offset");
}

/// Proves a rotated Constant-gap guide tiles its finite ends beyond the visible canvas.
///
/// # Panics
///
/// Panics when a visible offset terminates inside the canvas instead of continuing through an
/// end-to-end copy of the authored guide.
#[test]
fn constant_gap_tiles_curve_ends_beyond_rotated_canvas() {
    let dimension_id = GuideDimensionId(61);
    let definition = definition(
        vec![GuideDimension {
            id: dimension_id,
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(61),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 12.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![dimension_id],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let diagnostic_canvas = CanvasSpec {
        width: 320.0,
        height: 320.0,
    };
    let structure = AuthoredStructure::new(
        AuthoredStructureId(61),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: AuthoredPoint2 { x: -140.0, y: 0.0 },
            control_1: AuthoredPoint2 { x: -64.0, y: -48.0 },
            control_2: AuthoredPoint2 { x: 64.0, y: 24.0 },
            end: AuthoredPoint2 { x: 140.0, y: 0.0 },
        }],
    )
    .expect("diagnostic cubic validates");
    let document = document_with_canvas(definition.clone(), vec![structure], diagnostic_canvas);
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &GridInspectRequest {
            canvas: CanvasSpec {
                width: 320.0,
                height: 320.0,
            },
            density: DensityMetric2D::default_for_canvas(&CanvasSpec {
                width: 320.0,
                height: 320.0,
            })
            .expect("diagnostic canvas has a default density")
            .resolve(&CanvasSpec {
                width: 320.0,
                height: 320.0,
            })
            .expect("diagnostic default density resolves"),
            rotation_degrees: 37.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: 4.5,
            max_family_candidates: 1_000_000,
        },
        &|| false,
    )
    .expect("rotated tiled normal-offset family evaluates");
    let paths = output
        .structural_path_set()
        .expect("tiled paths publish")
        .paths();
    let canvas = toniator_geometry::Bounds::new(Point2::new(0.0, 0.0), Point2::new(320.0, 320.0))
        .expect("fixed canvas bounds");
    let visible = paths
        .iter()
        .filter(|instance| {
            let bounds = instance.path.bounds().expect("tiled path bounds");
            bounds.max.x >= canvas.min.x
                && bounds.min.x <= canvas.max.x
                && bounds.max.y >= canvas.min.y
                && bounds.min.y <= canvas.max.y
        })
        .collect::<Vec<_>>();
    assert!(
        visible.len() > 2,
        "both offset sides cross the final canvas"
    );
    assert!(visible.iter().all(|instance| {
        !canvas.contains(instance.path.start()) && !canvas.contains(instance.path.end())
    }));
}

/// Reproduces the user-authored Constant-gap curve with complete canvas coverage and solved nodes.
///
/// # Panics
///
/// Panics when the exact two-cubic witness cannot evaluate, leaves a component endpoint without a
/// shared vector node in the open canvas, retains an in-canvas backtracking fold, or leaves a
/// canvas coverage tile empty.
#[test]
fn user_reported_constant_gap_curve_dissolves_visible_crossings() {
    let dimension_id = GuideDimensionId(62);
    let definition = definition(
        vec![GuideDimension {
            id: dimension_id,
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(62),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 16.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![dimension_id],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let structure = AuthoredStructure::new(
        AuthoredStructureId(62),
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: -450.0,
                    y: -46.302_273_048_281_165,
                },
                control_1: AuthoredPoint2 {
                    x: -224.216_966_331_868_2,
                    y: -220.394_722_504_916_44,
                },
                control_2: AuthoredPoint2 {
                    x: -216.385_253_516_574_38,
                    y: -226.458_861_247_314_08,
                },
                end: AuthoredPoint2 {
                    x: 0.210_554_031_395_076_9,
                    y: -51.754_559_226_983_986,
                },
            },
            AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: 0.210_554_031_395_076_9,
                    y: -51.754_559_226_983_986,
                },
                control_1: AuthoredPoint2 {
                    x: 148.700_372_878_311_1,
                    y: -158.434_452_888_086_78,
                },
                control_2: AuthoredPoint2 {
                    x: 179.347_387_957_603_25,
                    y: 182.054_700_446_266_62,
                },
                end: AuthoredPoint2 { x: 450.0, y: 0.0 },
            },
        ],
    )
    .expect("user-authored two-cubic guide validates");
    let canvas = CanvasSpec {
        width: 900.0,
        height: 620.0,
    };
    let document = document_with_canvas(definition.clone(), vec![structure], canvas.clone());
    for rotation_degrees in [
        0.0, 45.0, 89.999, 90.0, 135.0, 179.999, 180.0, 270.0, 359.999,
    ] {
        let output = evaluate_document_typed_family_cancellable(
            &document,
            &definition,
            &GridInspectRequest {
                canvas: canvas.clone(),
                density: DensityMetric2D {
                    density: 82.340_605_806_803_78,
                    aspect: 1.0,
                }
                .resolve(&canvas)
                .expect("saved density resolves"),
                rotation_degrees,
                translation_x: 0.0,
                translation_y: 0.0,
                guard_steps: 1,
                support_radius: 4.5,
                max_family_candidates: 1_000_000,
            },
            &|| false,
        )
        .unwrap_or_else(|error| {
            panic!(
                "user-reported Constant-gap guide evaluates at {rotation_degrees} degrees: {error}"
            )
        });
        let paths = output
            .structural_path_set()
            .expect("user-reported guide paths publish")
            .paths();
        assert!(
            paths
                .iter()
                .all(|path| path.path.closure() == toniator_geometry::PathClosure::Open),
            "Dissolve crossings publishes only the exterior Constant-gap frontier at {rotation_degrees} degrees"
        );
        let canvas_bounds = toniator_geometry::Bounds::new(
            Point2::new(0.0, 0.0),
            Point2::new(canvas.width, canvas.height),
        )
        .expect("canvas bounds validate");
        let component_endpoints = paths
            .iter()
            .filter(|path| path.path.closure() == toniator_geometry::PathClosure::Open)
            .flat_map(|path| [(path.id, path.path.start()), (path.id, path.path.end())])
            .collect::<Vec<_>>();
        let path_nodes = paths
            .iter()
            .flat_map(|path| {
                path.path
                    .segments()
                    .iter()
                    .flat_map(move |segment| [(path.id, segment.start()), (path.id, segment.end())])
            })
            .collect::<Vec<_>>();
        let unpaired_endpoints = component_endpoints
            .iter()
            .copied()
            .filter(|(_, endpoint)| {
                endpoint.x > canvas_bounds.min.x
                    && endpoint.x < canvas_bounds.max.x
                    && endpoint.y > canvas_bounds.min.y
                    && endpoint.y < canvas_bounds.max.y
            })
            .filter_map(|(path_id, endpoint)| {
                let paired_node_count = path_nodes
                    .iter()
                    .filter(|(_, candidate)| {
                        (candidate.x - endpoint.x).hypot(candidate.y - endpoint.y) <= 1.0e-6
                    })
                    .count();
                let paired = paired_node_count > 1;
                (!paired).then(|| {
                    let nearest_other = path_nodes
                        .iter()
                        .filter(|(candidate_id, candidate)| {
                            *candidate_id != path_id
                                || (candidate.x - endpoint.x).hypot(candidate.y - endpoint.y)
                                    > 1.0e-6
                        })
                        .map(|(_, candidate)| {
                            (candidate.x - endpoint.x).hypot(candidate.y - endpoint.y)
                        })
                        .fold(f64::INFINITY, f64::min);
                    (path_id, endpoint, nearest_other)
                })
            })
            .collect::<Vec<_>>();
        assert!(
            unpaired_endpoints.is_empty(),
            "every in-canvas cleanup endpoint meets another path at one exact vector node at {rotation_degrees} degrees: {unpaired_endpoints:#?}"
        );
        let terminal_curls =
            paths
                .iter()
                .flat_map(|path| {
                    path.path.segments().windows(2).enumerate().filter_map(
                        |(segment_index, pair)| {
                            let node = pair[0].end();
                            if !canvas_bounds.contains(node) {
                                return None;
                            }
                            let incoming_start = pair[0]
                                .limiting_unit_tangent_at(0.0)
                                .expect("published segment start tangent remains finite");
                            let incoming_end = pair[0]
                                .limiting_unit_tangent_at(1.0)
                                .expect("published incoming tangent remains finite");
                            let outgoing = pair[1]
                                .limiting_unit_tangent_at(0.0)
                                .expect("published outgoing tangent remains finite");
                            let start_agreement = incoming_start.dot(outgoing);
                            let end_agreement = incoming_end.dot(outgoing);
                            (start_agreement > 1.0e-9 && end_agreement < -1.0e-9).then_some((
                                path.id,
                                segment_index,
                                node,
                                start_agreement,
                                end_agreement,
                            ))
                        },
                    )
                })
                .collect::<Vec<_>>();
        assert!(
            terminal_curls.is_empty(),
            "Dissolve crossings retains no in-canvas terminal curl at {rotation_degrees} degrees: {terminal_curls:#?}"
        );
        let source = paths
            .iter()
            .find(|path| path.id.repetition_index == 0)
            .expect("chained source guide publishes");
        assert!(
            !canvas_bounds.contains(source.path.start())
                && !canvas_bounds.contains(source.path.end()),
            "chained base guide supplies off-canvas runway at {rotation_degrees} degrees"
        );
        let canvas_sites = output
            .site_set()
            .sites()
            .iter()
            .filter(|site| site.scope == SiteScope::Canvas)
            .collect::<Vec<_>>();
        let published_rank_range = paths.iter().fold((i64::MAX, i64::MIN), |range, path| {
            (
                range.0.min(path.id.repetition_index),
                range.1.max(path.id.repetition_index),
            )
        });
        for tile_y in 0..4 {
            for tile_x in 0..4 {
                let minimum_x = canvas.width * f64::from(tile_x) / 4.0;
                let maximum_x = canvas.width * f64::from(tile_x + 1) / 4.0;
                let minimum_y = canvas.height * f64::from(tile_y) / 4.0;
                let maximum_y = canvas.height * f64::from(tile_y + 1) / 4.0;
                assert!(
                    canvas_sites.iter().any(|site| {
                        site.position.x >= minimum_x
                            && site.position.x <= maximum_x
                            && site.position.y >= minimum_y
                            && site.position.y <= maximum_y
                    }),
                    "user-reported Constant-gap curve left tile ({tile_x}, {tile_y}) empty at {rotation_degrees} degrees after ranks {published_rank_range:?}"
                );
            }
        }
    }
}

/// Proves a bilateral family reports a stable coverage failure when even its source cannot survive.
#[test]
fn normal_offset_bilateral_collapse_fails_coverage_atomically() {
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(51),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(51),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 12.0,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(51)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let stationary = line(
        51,
        AuthoredPoint2 { x: 0.0, y: 0.0 },
        AuthoredPoint2 { x: 0.0, y: 0.0 },
    );
    let error = evaluate_document_typed_family_cancellable(
        &document(definition.clone(), vec![stationary]),
        &definition,
        &request(128),
        &|| false,
    )
    .expect_err("stationary one-sided guide cannot prove coverage");
    assert_eq!(error.path(), "coverage.curved_guides.normal_offset");
}

/// Proves a 77.5-unit NormalOffset transverse basis expands the positive
/// AlongGuide support bound and realizes maximum-fill marks on an anisotropic canvas.
#[test]
fn normal_offset_along_guides_bound_their_transverse_basis_before_mark_realization() {
    let canvas = CanvasSpec {
        width: 225.0,
        height: 155.0,
    };
    let density = ResolvedDensityMetric2D {
        across_x: 16.0,
        across_y: 16.0 * 155.0 / 225.0,
    };
    let definition = definition(
        vec![GuideDimension {
            id: GuideDimensionId(77),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(77),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing: 77.5,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(77)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
    );
    let curve = AuthoredStructure::new(
        AuthoredStructureId(77),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start: AuthoredPoint2 {
                x: -0.25 * canvas.width,
                y: 17.0 / 24.0 * canvas.height,
            },
            control_1: AuthoredPoint2 {
                x: 0.125 * canvas.width,
                y: 5.0 / 24.0 * canvas.height,
            },
            control_2: AuthoredPoint2 {
                x: 0.75 * canvas.width,
                y: 5.0 / 24.0 * canvas.height,
            },
            end: AuthoredPoint2 {
                x: 1.25 * canvas.width,
                y: 17.0 / 24.0 * canvas.height,
            },
        }],
    )
    .expect("shallow authored cubic validates");
    let document = document_with_canvas(definition.clone(), vec![curve], canvas.clone());
    let plan = resolve_document_pattern_pipeline(&document, &definition)
        .expect("normal-offset pipeline resolves");
    let maximum = maximum_nominal_cell_diameter(&plan.family, &canvas, &density)
        .expect("normal-offset bound remains finite");
    let pattern_size = DensityMetric2D::default_for_canvas(&canvas)
        .expect("fixed anisotropic canvas has a default density")
        .density
        / DensityMetric2D::from_resolved(&canvas, &density)
            .expect("fixed resolved density round-trips")
            .density;
    assert!((maximum - (225.0 / 16.0 + 77.5 * pattern_size)).abs() <= 1e-12);
    let request = GridInspectRequest {
        canvas: canvas.clone(),
        density,
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: maximum,
        max_family_candidates: 1_000_000,
    };
    let family =
        evaluate_document_typed_family_cancellable(&document, &definition, &request, &|| false)
            .expect("normal-offset sites fit the planned positive support");
    assert!(
        family
            .site_set()
            .sites()
            .iter()
            .all(|site| { site.nominal_cell_basis.diameter() <= maximum })
    );
    let source = decode_source(
        &std::fs::read("../../assets/raster-sample.png").expect("fixture reads"),
        SourceFormatHint::Png,
    )
    .expect("fixture decodes");
    let marks = realize_typed_diagnostic_outputs(
        &family,
        &plan,
        &source,
        &canvas,
        SourcePlacement::StretchToCanvas,
        SourceComponent::Luminance,
        MarkResponse {
            minimum_fill: 2.0,
            maximum_fill: 2.0,
            rotation_offset_degrees: 0.0,
        },
    )
    .expect("maximum-fill marks realize within the planned support");
    assert!(!marks.output.marks.is_empty());
}
