use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, DensityMetric2D, Document,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
    MarkOrientation, MarkPrototype, PatternDefinition, PatternDefinitionId, PatternMechanismId,
    PatternOutputLayer, PatternOutputLayerId, SourceMapping, SourceMappingComponent,
    SourceReference,
};
use toniator_geometry::{CanonicalMark, CurveSegment, Point2};
use toniator_patterns::{
    CanonicalMarkRequest, GridInspectRequest, MarkResponse,
    evaluate_document_typed_family_cancellable, realize_typed_canonical_marks,
    resolve_document_pattern_pipeline,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds one explicit line without adding implicit closure or smoothing behavior.
fn line(start: AuthoredPoint2, end: AuthoredPoint2) -> AuthoredCurveSegment {
    AuthoredCurveSegment::Line { start, end }
}

/// Builds a single-site document whose mark shape and orientation are the only varying inputs.
fn shape_document(
    segments: Vec<AuthoredCurveSegment>,
    orientation: MarkOrientation,
) -> (Document, PatternDefinition) {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "shape transform",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(1),
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
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        orientation,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let PatternOutputLayer::MarkPrototype { prototype, .. } = &mut definition.output_layers[0]
    else {
        panic!("generalized guides own one typed mark output")
    };
    *prototype = MarkPrototype::AuthoredClosedShape {
        structure_id: AuthoredStructureId(3),
    };
    let horizontal = AuthoredStructure::new(
        AuthoredStructureId(1),
        AuthoredStructureKind::OpenPath,
        vec![line(
            AuthoredPoint2 { x: 0.0, y: 50.0 },
            AuthoredPoint2 { x: 100.0, y: 50.0 },
        )],
    )
    .unwrap();
    let vertical = AuthoredStructure::new(
        AuthoredStructureId(2),
        AuthoredStructureKind::OpenPath,
        vec![line(
            AuthoredPoint2 { x: 50.0, y: 0.0 },
            AuthoredPoint2 { x: 50.0, y: 100.0 },
        )],
    )
    .unwrap();
    let shape = AuthoredStructure::new(
        AuthoredStructureId(3),
        AuthoredStructureKind::ClosedShape,
        segments,
    )
    .unwrap();
    let document = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition.clone()],
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
        vec![horizontal, vertical, shape],
    )
    .unwrap();
    (document, definition)
}

/// Returns one finite asymmetric closed triangle with bounds center `(2, 1)` and radius `sqrt(5)`.
fn asymmetric_shape() -> Vec<AuthoredCurveSegment> {
    let first = AuthoredPoint2 { x: 0.0, y: 0.0 };
    let second = AuthoredPoint2 { x: 4.0, y: 0.0 };
    let third = AuthoredPoint2 { x: 0.0, y: 2.0 };
    vec![line(first, second), line(second, third), line(third, first)]
}

/// Returns one accepted construction-degenerate closed shape whose reference radius is zero.
fn zero_radius_shape() -> Vec<AuthoredCurveSegment> {
    let point = AuthoredPoint2 { x: 4.0, y: 7.0 };
    vec![line(point, point)]
}

/// Supplies the deterministic single-site family request used for exact transform comparisons.
fn family_request() -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        density: DensityMetric2D {
            across_x: 10.0,
            across_y: 10.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 1,
        support_radius: 100.0,
        max_family_candidates: 64,
    }
}

/// Rotates one normalized authored offset by the expected output-plus-channel orientation.
fn expected_point(offset: Point2, scale: f64, degrees: f64) -> Point2 {
    let radians = degrees.to_radians();
    Point2::new(
        50.0 + scale * (radians.cos() * offset.x - radians.sin() * offset.y),
        50.0 + scale * (radians.sin() * offset.x + radians.cos() * offset.y),
    )
}

/// Asserts two finite construction points are equal within deterministic floating arithmetic noise.
fn assert_point_close(actual: Point2, expected: Point2) {
    assert!(
        (actual.x - expected.x).abs() < 1.0e-10,
        "x: {actual:?} {expected:?}"
    );
    assert!(
        (actual.y - expected.y).abs() < 1.0e-10,
        "y: {actual:?} {expected:?}"
    );
}

/// Encodes one exact RGBA source pixel for focused sampled-paint witnesses.
fn one_pixel_png(pixel: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixel, 1, 1, ColorType::Rgba8.into())
        .unwrap();
    bytes
}

/// Proves bounds-center anchoring, maximum-control/endpoint radius normalization, fixed/tangent/
/// normal orientation, post-orientation channel rotation, and the inclusive segment preflight.
#[test]
fn authored_shape_normalization_and_all_orientations_are_exact() {
    let source = decode_source(
        &std::fs::read("../../assets/raster-sample.png").unwrap(),
        SourceFormatHint::Png,
    )
    .unwrap();
    for (orientation, base_degrees) in [
        (MarkOrientation::Fixed, 0.0),
        (
            MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(2),
            },
            90.0,
        ),
        (
            MarkOrientation::GuideNormal {
                dimension_id: GuideDimensionId(2),
            },
            180.0,
        ),
    ] {
        let (document, definition) = shape_document(asymmetric_shape(), orientation);
        let plan = resolve_document_pattern_pipeline(&document, &definition).unwrap();
        let family = evaluate_document_typed_family_cancellable(
            &document,
            &definition,
            &family_request(),
            &|| false,
        )
        .unwrap();
        assert_eq!(family.site_set().len(), 1);
        let response = MarkResponse {
            minimum_fill: 1.0,
            maximum_fill: 1.0,
            rotation_offset_degrees: 30.0,
        };
        let realized = realize_typed_canonical_marks(
            &document,
            &family,
            &plan,
            &source,
            &family_request().canvas,
            CanonicalMarkRequest {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                sampled_paint: false,
                response,
                max_transformed_curve_segment_instances: 3,
            },
        )
        .unwrap();
        let [CanonicalMark::ClosedPath(mark)] = realized.output.marks.as_slice() else {
            panic!("one authored-shape site must publish one canonical path")
        };
        let CurveSegment::Line(first_segment) = &mark.path.segments()[0] else {
            panic!("the first authored line must retain its construction variant")
        };
        let nominal_radius = family.site_set().sites()[0].nominal_cell_basis.diameter() / 2.0;
        let scale = nominal_radius / 5.0_f64.sqrt();
        assert_point_close(
            first_segment.start(),
            expected_point(Point2::new(-2.0, -1.0), scale, base_degrees + 30.0),
        );
        assert_point_close(
            first_segment.end(),
            expected_point(Point2::new(2.0, -1.0), scale, base_degrees + 30.0),
        );
        let error = realize_typed_canonical_marks(
            &document,
            &family,
            &plan,
            &source,
            &family_request().canvas,
            CanonicalMarkRequest {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                sampled_paint: false,
                response,
                max_transformed_curve_segment_instances: 2,
            },
        )
        .unwrap_err();
        assert_eq!(error.path(), "realization.mark.segment_limit");
    }
}

/// Proves a finite domain-valid closed construction with no reference radius fails before any
/// canonical mark can publish, while retaining its resource and family as separate authorities.
#[test]
fn zero_radius_authored_shape_is_rejected_before_realization_output() {
    let (document, definition) = shape_document(zero_radius_shape(), MarkOrientation::Fixed);
    let plan = resolve_document_pattern_pipeline(&document, &definition).unwrap();
    let family = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &family_request(),
        &|| false,
    )
    .unwrap();
    let source = decode_source(
        &std::fs::read("../../assets/raster-sample.png").unwrap(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let error = realize_typed_canonical_marks(
        &document,
        &family,
        &plan,
        &source,
        &family_request().canvas,
        CanonicalMarkRequest {
            mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
            sampled_paint: false,
            response: MarkResponse {
                minimum_fill: 1.0,
                maximum_fill: 1.0,
                rotation_offset_degrees: 0.0,
            },
            max_transformed_curve_segment_instances: 1,
        },
    )
    .unwrap_err();
    assert_eq!(error.path(), "pattern.output_layers.prototype.radius");
}

/// Proves exact-zero source alpha retains one canonical path per family site with zero scale and
/// neutral invisible paint, while every mapping bit remains an identity discriminator.
#[test]
fn sampled_zero_alpha_retains_sites_and_complete_mapping_identity() {
    let (document, definition) = shape_document(asymmetric_shape(), MarkOrientation::Fixed);
    let plan = resolve_document_pattern_pipeline(&document, &definition).unwrap();
    let family = evaluate_document_typed_family_cancellable(
        &document,
        &definition,
        &family_request(),
        &|| false,
    )
    .unwrap();
    let source = decode_source(&one_pixel_png([255, 0, 255, 0]), SourceFormatHint::Png).unwrap();
    let response = MarkResponse {
        minimum_fill: 1.0,
        maximum_fill: 1.0,
        rotation_offset_degrees: 0.0,
    };
    let realize = |mapping| {
        realize_typed_canonical_marks(
            &document,
            &family,
            &plan,
            &source,
            &family_request().canvas,
            CanonicalMarkRequest {
                mapping,
                sampled_paint: true,
                response,
                max_transformed_curve_segment_instances: 3,
            },
        )
        .unwrap()
        .output
    };
    let alpha = realize(SourceMapping::canonical(SourceMappingComponent::Alpha));
    assert_eq!(alpha.marks.len(), family.site_set().len());
    let [CanonicalMark::ClosedPath(mark)] = alpha.marks.as_slice() else {
        panic!("zero-alpha sampled output retains its authored canonical path")
    };
    assert_eq!(mark.bounds.min, Point2::new(50.0, 50.0));
    assert_eq!(mark.bounds.max, Point2::new(50.0, 50.0));
    let paints = alpha.paints.as_ref().unwrap();
    assert_eq!(paints.len(), alpha.marks.len());
    assert_eq!(paints[0].red, 0.0);
    assert_eq!(paints[0].green, 0.0);
    assert_eq!(paints[0].blue, 0.0);
    assert_eq!(paints[0].alpha, 1.0);

    let luminance = realize(SourceMapping::canonical(SourceMappingComponent::Luminance));
    assert_eq!(alpha.marks, luminance.marks);
    assert_eq!(alpha.paints, luminance.paints);
    assert_ne!(
        alpha.realization_fingerprint,
        luminance.realization_fingerprint
    );
}

/// Proves structurally distinct tangent and normal selections remain distinct realization
/// identities even when they produce exactly equal canonical construction geometry.
#[test]
fn equivalent_geometry_retains_explicit_orientation_identity() {
    let (tangent_document, tangent_definition) = shape_document(
        asymmetric_shape(),
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(2),
        },
    );
    let (normal_document, normal_definition) = shape_document(
        asymmetric_shape(),
        MarkOrientation::GuideNormal {
            dimension_id: GuideDimensionId(1),
        },
    );
    let tangent_plan =
        resolve_document_pattern_pipeline(&tangent_document, &tangent_definition).unwrap();
    let normal_plan =
        resolve_document_pattern_pipeline(&normal_document, &normal_definition).unwrap();
    let request = family_request();
    let tangent_family = evaluate_document_typed_family_cancellable(
        &tangent_document,
        &tangent_definition,
        &request,
        &|| false,
    )
    .unwrap();
    let normal_family = evaluate_document_typed_family_cancellable(
        &normal_document,
        &normal_definition,
        &request,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        tangent_family.family_fingerprint(),
        normal_family.family_fingerprint()
    );
    let source = decode_source(
        &std::fs::read("../../assets/raster-sample.png").unwrap(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let canonical_request = CanonicalMarkRequest {
        mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
        sampled_paint: false,
        response: MarkResponse {
            minimum_fill: 1.0,
            maximum_fill: 1.0,
            rotation_offset_degrees: 0.0,
        },
        max_transformed_curve_segment_instances: 3,
    };
    let tangent = realize_typed_canonical_marks(
        &tangent_document,
        &tangent_family,
        &tangent_plan,
        &source,
        &request.canvas,
        canonical_request,
    )
    .unwrap()
    .output;
    let normal = realize_typed_canonical_marks(
        &normal_document,
        &normal_family,
        &normal_plan,
        &source,
        &request.canvas,
        canonical_request,
    )
    .unwrap()
    .output;
    assert_eq!(tangent.marks, normal.marks);
    assert_ne!(
        tangent.realization_fingerprint,
        normal.realization_fingerprint
    );
}
