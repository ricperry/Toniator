use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, DensityMetric2D, Document, DocumentId,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
    MarkOrientation, OffsetCleanup, OffsetSides, PatternDefinition, PatternDefinitionBundle,
    PatternDefinitionId, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputSettings, SourceComponent, SourcePlacement,
    SourceReference,
};
use toniator_geometry::{AffineTransform2D, FamilySiteProvenance, Point2, SiteScope, Vector2};
use toniator_patterns::{
    GridInspectRequest, MarkResponse, directional_spacing,
    evaluate_document_typed_family_cancellable, maximum_nominal_cell_diameter,
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
        density: DensityMetric2D {
            across_x: 10.0,
            across_y: 20.0,
            aspect_locked: false,
        },
        rotation_degrees: 17.0,
        translation_x: 4.0,
        translation_y: -3.0,
        guard_steps: 1,
        support_radius: 4.5,
        max_family_candidates,
    }
}

/// Proves resolved authored guides merge ordered selected contributors without sorting their dimension IDs.
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
    let output =
        evaluate_document_typed_family_cancellable(&document, &definition, &request(64), &|| false)
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
            phase: 27.0,
            prototype: GuidePrototype::CircularArc {
                center: AuthoredPoint2 { x: 0.0, y: 0.0 },
                radius: 55.0,
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
    assert!(
        guides
            .windows(2)
            .all(|pair| pair[0].id.repetition_index < pair[1].id.repetition_index)
    );
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
    .apply_point(Point2::new(55.0 + 27.0, 0.0));
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
    .apply_point(Point2::new(55.0 + 27.0 + 5.0, 0.0));
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
    let density = DensityMetric2D {
        across_x: 10.0,
        across_y: 20.0,
        aspect_locked: false,
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
                sides: OffsetSides::Both,
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
        &request(5_000),
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
    let distance = |first: Point2, second: Point2| (first.x - second.x).hypot(first.y - second.y);
    assert!((distance(prior.path.start(), source.path.start()) - 12.0).abs() < 1.0e-9);
    assert!((distance(source.path.start(), next.path.start()) - 12.0).abs() < 1.0e-9);
    assert_eq!(output.guide_nominal_basis(source.id), Some(12.0));
    let authored_points = [Point2::new(-30.0, 0.0), Point2::new(30.0, 0.0)].map(|point| {
        AffineTransform2D::rotate_about_then_translate(
            Point2::new(0.0, 0.0),
            17.0,
            Vector2::new(54.0, 47.0),
        )
        .unwrap()
        .apply_point(point)
    });
    assert!(authored_points.into_iter().all(|authored| {
        source.path.segments().iter().any(|segment| {
            [segment.start(), segment.end()].into_iter().any(|point| {
                (point.x - authored.x).abs() < 1.0e-9 && (point.y - authored.y).abs() < 1.0e-9
            })
        })
    }));
}

/// Proves crossing cleanup components publish through the family boundary with distinct full identities.
#[test]
fn normal_offset_cleanup_components_publish_without_identity_collision() {
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
                sides: OffsetSides::Left,
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
    let output = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &request(10_000),
        &|| false,
    )
    .expect("split normal-offset family publishes atomically");
    let guides = output
        .structural_path_set()
        .expect("guide paths publish")
        .paths();
    let split_index = guides
        .iter()
        .find(|guide| guide.id.component_ordinal > 0)
        .expect("at least one offset crossing becomes multiple components")
        .id
        .repetition_index;
    let components = guides
        .iter()
        .filter(|guide| guide.id.repetition_index == split_index)
        .collect::<Vec<_>>();
    assert!(components.len() >= 2);
    assert!(components.windows(2).all(|pair| {
        pair[0].id.component_ordinal < pair[1].id.component_ordinal && pair[0].id != pair[1].id
    }));
    let samples = output
        .site_set()
        .iter()
        .filter_map(|site| match &site.provenance {
            FamilySiteProvenance::CurveAlongGuide {
                location,
                sequence,
                absolute_arc_position_bits,
                ..
            } if location.path.repetition_index == split_index => Some((
                location.path.component_ordinal,
                *sequence,
                f64::from_bits(*absolute_arc_position_bits),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(samples.iter().any(|sample| sample.0 == 0));
    assert!(samples.iter().any(|sample| sample.0 > 0));
    assert!(
        samples
            .windows(2)
            .all(|pair| { pair[0].1 < pair[1].1 && pair[0].2 < pair[1].2 })
    );
}

/// Proves the 320x320 diagnostic propagates both ways and stops at terminal-extension collapse.
#[test]
fn diagnostic_cubic_stops_when_terminal_extensions_cross() {
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
                sides: OffsetSides::Both,
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
            control_1: AuthoredPoint2 { x: -64.0, y: -96.0 },
            control_2: AuthoredPoint2 { x: 64.0, y: -96.0 },
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
            density: DensityMetric2D {
                across_x: 32.0,
                across_y: 32.0,
                aspect_locked: false,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: 4.5,
            max_family_candidates: 50_000,
        },
        &|| false,
    )
    .expect("diagnostic normal-offset family evaluates");
    let paths = output
        .structural_path_set()
        .expect("diagnostic paths publish")
        .paths();
    assert!(
        paths.iter().any(|path| path.id.repetition_index < 0),
        "diagnostic publishes the right-side offset ladder"
    );
    assert!(
        paths.iter().any(|path| path.id.repetition_index > 0),
        "diagnostic publishes the left-side offset ladder"
    );
    let mut positive_indices = paths
        .iter()
        .filter_map(|path| (path.id.repetition_index > 0).then_some(path.id.repetition_index))
        .collect::<Vec<_>>();
    positive_indices.sort_unstable();
    positive_indices.dedup();
    let mut left_canvas = false;
    for repetition_index in positive_indices {
        let intersects_canvas = paths
            .iter()
            .filter(|path| path.id.repetition_index == repetition_index)
            .any(|path| {
                let bounds = path.path.bounds().expect("diagnostic path bounds");
                bounds.max.x >= 0.0
                    && bounds.min.x <= 320.0
                    && bounds.max.y >= 0.0
                    && bounds.min.y <= 320.0
            });
        if intersects_canvas {
            assert!(
                !left_canvas,
                "repetition {repetition_index} must not re-enter the canvas after the cusp family leaves it"
            );
        } else {
            left_canvas = true;
        }
    }
    for (first_index, first) in paths.iter().enumerate() {
        for second in paths.iter().skip(first_index + 1) {
            let intersections = first
                .path
                .intersections(&second.path)
                .expect("diagnostic component intersections remain bounded");
            assert!(
                intersections.iter().all(|intersection| {
                    intersection.kind() != toniator_geometry::IntersectionKind::Crossing
                }),
                "diagnostic paths {:?} and {:?} must not form a cross-hatched offset family: {intersections:?}",
                first.id,
                second.id
            );
        }
    }
    let last_visible = paths
        .iter()
        .filter(|path| path.id.repetition_index == 14)
        .collect::<Vec<_>>();
    assert_eq!(last_visible.len(), 2);
    assert_eq!(last_visible[0].id.component_ordinal, 0);
    assert_eq!(last_visible[1].id.component_ordinal, 1);
    assert!(last_visible.iter().all(|component| {
        let bounds = component.path.bounds().expect("last visible cusp bounds");
        bounds.max.x - bounds.min.x > 40.0
            && bounds.max.y - bounds.min.y > 50.0
            && component
                .path
                .unit_tangent_at(
                    toniator_geometry::PathLocation::new(0, 0.0).expect("component start location"),
                )
                .is_ok()
            && component
                .path
                .unit_tangent_at(
                    toniator_geometry::PathLocation::new(component.path.segments().len() - 1, 1.0)
                        .expect("component end location"),
                )
                .is_ok()
    }));
    assert!(
        paths.iter().all(|path| path.id.repetition_index < 15),
        "the family stops when both tangential endpoint extensions cross instead of publishing floating cusp fragments"
    );
}

/// Proves a one-sided family reports a stable coverage failure when even its source cannot survive.
#[test]
fn normal_offset_one_sided_collapse_fails_coverage_atomically() {
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
                sides: OffsetSides::Left,
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
    let density = DensityMetric2D {
        across_x: 16.0,
        across_y: 16.0 * 155.0 / 225.0,
        aspect_locked: true,
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
                sides: OffsetSides::Both,
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
    assert!((maximum - (225.0 / 16.0 + 77.5)).abs() <= 1e-12);
    let request = GridInspectRequest {
        canvas: canvas.clone(),
        density: density.clone(),
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: maximum,
        max_family_candidates: 100_000,
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
