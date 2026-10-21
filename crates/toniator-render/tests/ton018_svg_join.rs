//! Current TON-018 final SVG consumer checks.

use std::sync::atomic::{AtomicUsize, Ordering};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, GuideDimensionId, HalftoneChannelModel, PathStrokeStyle,
    PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    Bounds, CanonicalFillRule, CanonicalFilledOutline, CanonicalMark, CanonicalOutlineContour,
    CanonicalPathMark, CanonicalStroke, CubicBezierSegment, CurvePath, CurveSegment, FamilySiteId,
    FamilySiteProvenance, PathClosure, PathLocation, Point2, SiteScope, StrokeProfileSample,
    StructuralPathInstanceId,
};
use toniator_render::{
    GeometryOutput, OutputRasterTarget, RasterAntialiasing, RasterBackground, RenderLayer,
    RenderOutputLayer, RenderScene, SvgSerializationLimits, encode_png, rasterize,
    rasterize_output, write_svg, write_svg_cancellable_with_limits,
};

/// Builds an identified filled circle for one final-consumer scene.
///
/// # Panics
/// Panics only if this test's finite circle fixture becomes invalid.
fn circle(ordinal: usize, x: f64, y: f64, radius: f64) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal,
        },
        center: Point2::new(x, y),
        radius,
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: ordinal,
            accepted_ordinal: ordinal,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule: CanonicalFillRule::EvenOdd,
    }
}

/// Builds one opaque single-channel scene from ordered canonical mark outputs.
///
/// # Panics
/// Panics only if the finite test fixture violates scene construction rules.
fn scene(outputs: Vec<RenderOutputLayer>) -> RenderScene {
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        outputs,
    )
    .expect("test layer");
    RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "ton018-family".into(),
        "ton018-realization".into(),
        vec![layer],
    )
    .expect("test scene")
}

/// Proves touching and overlapping opaque marks become one exact editable path.
///
/// # Panics
/// Panics on failed fixture construction or serialization.
#[test]
fn opaque_overlap_and_touching_share_one_nonzero_path() {
    let scene = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![
            circle(0, 2.0, 4.0, 1.0),
            circle(1, 3.0, 4.0, 1.0),
            circle(2, 5.0, 4.0, 1.0),
        ]),
        None,
    )]);
    let svg = write_svg(&scene).expect("SVG serialization");
    assert_eq!(
        svg.matches("<path id=\"channel-1-opaque-join-0\"").count(),
        1
    );
    assert_eq!(svg.matches("a1 1 0 0 1").count(), 12);
    assert!(!svg.contains("<circle"));
    assert!(svg.contains("fill-rule=\"nonzero\""));
}

/// Proves an unsafe EvenOdd bowtie is normalized before an exact circle joins it.
///
/// # Panics
/// Panics on invalid test path or serialization.
#[test]
fn unsafe_evenodd_fill_is_local_to_its_primitive() {
    let bowtie = CurvePath::polyline(
        vec![
            Point2::new(1.0, 1.0),
            Point2::new(4.0, 4.0),
            Point2::new(1.0, 4.0),
            Point2::new(4.0, 1.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed bowtie");
    let path = CanonicalPathMark::new(
        FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        },
        bowtie,
        SiteScope::Canvas,
        FamilySiteProvenance::Random {
            candidate_ordinal: 0,
            accepted_ordinal: 0,
            exclusion_neighbor_ordinal: None,
        },
        CanonicalFillRule::EvenOdd,
    )
    .expect("canonical bowtie");
    let scene = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![
            CanonicalMark::ClosedPath(path),
            circle(1, 2.5, 2.5, 0.5),
        ]),
        None,
    )]);
    let svg = write_svg(&scene).expect("normalized compound SVG");
    assert_eq!(
        svg.matches("<path id=\"channel-1-opaque-join-0\"").count(),
        1
    );
    assert!(svg.contains("a0.5 0.5"), "circle stays an exact vector arc");
}

/// Proves no partial SVG escapes cancellation or a caller's explicit edge bound.
///
/// # Panics
/// Panics on invalid fixture or an unexpected successful serialization.
#[test]
fn cancellation_and_limits_leave_scene_and_raster_unchanged() {
    let scene = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![circle(0, 2.0, 4.0, 1.0), circle(1, 3.0, 4.0, 1.0)]),
        None,
    )]);
    let original = scene.clone();
    let before = rasterize(&scene, RasterBackground::Transparent).expect("native raster");
    let calls = AtomicUsize::new(0);
    let error =
        write_svg_cancellable_with_limits(&scene, SvgSerializationLimits::default(), &|| {
            calls.fetch_add(1, Ordering::Relaxed) >= 2
        })
        .expect_err("cancelled serialization cannot return SVG");
    assert_eq!(error.path(), "svg.cancelled");
    let error = write_svg_cancellable_with_limits(
        &scene,
        SvgSerializationLimits::new(4).expect("nonzero bound"),
        &|| false,
    )
    .expect_err("second circle exceeds the explicit edge budget");
    assert_eq!(error.path(), "svg.limits.edges");
    assert_eq!(scene, original);
    assert_eq!(
        rasterize(&scene, RasterBackground::Transparent).expect("native raster"),
        before
    );
}

/// Proves a positive sub-micro radius survives exact arc serialization.
///
/// # Panics
/// Panics on invalid fixture or serialization failure.
#[test]
fn tiny_positive_circle_keeps_roundtrip_radius() {
    let scene = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![circle(0, 2.0, 4.0, 1.0e-9)]),
        None,
    )]);
    let svg = write_svg(&scene).expect("tiny circle SVG");
    assert!(svg.contains("a0.000000001 0.000000001"));
}

/// Proves opaque joining stops at the channel boundary and translucent paint keeps order.
///
/// # Panics
/// Panics on invalid fixture or failed serialization.
#[test]
fn channel_boundary_keeps_translucent_painter_order() {
    let opaque = RenderLayer::new(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        GeometryOutput::CanonicalMarks(vec![circle(0, 2.0, 4.0, 1.0), circle(1, 3.0, 4.0, 1.0)]),
    )
    .expect("opaque layer");
    let translucent = RenderLayer::new(
        ChannelId(2),
        true,
        ColorValue {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        },
        0.5,
        GeometryOutput::CanonicalMarks(vec![circle(2, 3.0, 4.0, 1.0)]),
    )
    .expect("translucent layer");
    let scene = RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "layer-order-family".into(),
        "layer-order-realization".into(),
        vec![opaque, translucent],
    )
    .expect("ordered scene");
    let svg = write_svg(&scene).expect("ordered SVG");
    let joined = svg.find("channel-1-opaque-join-0").expect("opaque run");
    let translucent_group = svg.find("id=\"channel-2\"").expect("translucent group");
    assert!(joined < translucent_group);
    assert!(!svg.contains("channel-2-opaque-join"));
    assert!(svg.contains("fill-opacity=\"0.5\""));
}

/// Proves a narrow rotated tapered outline retains its original canonical cubic controls.
///
/// Axis-aligned bounds of both rails overlap at their common tip and across
/// nonadjacent pieces. Certifying a genuinely separating plane avoids converting
/// ordinary guide outlines into huge flattened polygons.
///
/// # Panics
/// Panics on invalid finite fixture construction or loss of the exact curves.
#[test]
fn rotated_tapered_curve_keeps_exact_cubics() {
    let vertices = [
        Point2::new(1.0, 1.0),
        Point2::new(3.0, 4.0),
        Point2::new(5.0, 7.0),
        Point2::new(5.01, 7.0),
        Point2::new(3.005, 4.0),
        Point2::new(1.0, 1.0),
    ];
    let segments = vertices
        .windows(2)
        .map(|pair| {
            let start = pair[0];
            let end = pair[1];
            CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    start,
                    Point2::new(
                        start.x + (end.x - start.x) * (1.0 / 3.0),
                        start.y + (end.y - start.y) * (1.0 / 3.0),
                    ),
                    Point2::new(
                        start.x + (end.x - start.x) * (2.0 / 3.0),
                        start.y + (end.y - start.y) * (2.0 / 3.0),
                    ),
                    end,
                )
                .expect("finite rail cubic"),
            )
        })
        .collect();
    let path = CurvePath::new(segments, PathClosure::Closed).expect("tapered contour");
    let mark = CanonicalPathMark::new(
        FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        },
        path,
        SiteScope::Canvas,
        FamilySiteProvenance::Random {
            candidate_ordinal: 0,
            accepted_ordinal: 0,
            exclusion_neighbor_ordinal: None,
        },
        CanonicalFillRule::NonZero,
    )
    .expect("canonical tapered outline");
    let scene = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(mark)]),
        None,
    )]);
    let svg = write_svg(&scene).expect("exact tapered outline SVG");
    assert_eq!(svg.matches('C').count(), 5);
    assert!(!svg.contains('L'), "canonical cubic rails remain unchanged");
    assert!(svg.contains("5.01 7"));
}

/// Proves invisible circles are omitted without changing surviving primitive paint indices.
///
/// # Panics
/// Panics on an invalid fixture or if culling changes original enumeration/paint.
#[test]
fn invisible_painted_circle_preserves_surviving_id_and_paint() {
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalMarks(vec![
                circle(0, -4.0, 4.0, 1.0),
                circle(1, 0.0, 4.0, 2.0),
            ]),
            Some(vec![
                ColorValue {
                    red: 0.0,
                    green: 1.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
            ]),
        )],
    )
    .expect("painted circle layer");
    let scene = RenderScene::new_modeled(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "cull-family".into(),
        "cull-realization".into(),
        HalftoneChannelModel::SourceColorAlpha,
        vec![layer],
    )
    .expect("painted cull scene");
    let svg = write_svg(&scene).expect("culled SVG");
    assert!(!svg.contains("channel-1-mark-0"));
    assert!(svg.contains("channel-1-mark-1"));
    assert!(svg.contains("fill=\"#0000ff\""));
    assert!(!svg.contains("fill=\"#00ff00\""));
    assert!(svg.contains("<path id=\"channel-1-mark-1\""));
    assert!(svg.contains("a2 2"));
}

/// Proves an ordered sampled/translucent EvenOdd primitive keeps its rule and paint after clipping.
/// # Panics
/// Panics on invalid fixture, changed scene/raster, or lost ordered-paint attributes.
#[test]
fn painted_evenodd_path_keeps_fill_rule_after_closed_intersection() {
    let path = CurvePath::polyline(
        vec![
            Point2::new(-2.0, 1.0),
            Point2::new(10.0, 7.0),
            Point2::new(-2.0, 7.0),
            Point2::new(10.0, 1.0),
        ],
        PathClosure::Closed,
    )
    .expect("EvenOdd crossing contour");
    let mark = CanonicalPathMark::new(
        FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        },
        path,
        SiteScope::Canvas,
        FamilySiteProvenance::Random {
            candidate_ordinal: 0,
            accepted_ordinal: 0,
            exclusion_neighbor_ordinal: None,
        },
        CanonicalFillRule::EvenOdd,
    )
    .expect("sampled EvenOdd mark");
    let paint = ColorValue {
        red: 0.0,
        green: 0.0,
        blue: 1.0,
        alpha: 1.0,
    };
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        paint.clone(),
        0.2,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(mark)]),
            Some(vec![paint]),
        )],
    )
    .expect("sampled/translucent layer");
    let scene = RenderScene::new_modeled(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "paint-clip-family".into(),
        "paint-clip-realization".into(),
        HalftoneChannelModel::SourceColorAlpha,
        vec![layer],
    )
    .expect("painted clip scene");
    let original = scene.clone();
    let raster = rasterize(&scene, RasterBackground::Transparent).expect("native raster");
    let svg = write_svg(&scene).expect("ordered painted clip");
    assert!(!svg.contains("opaque-join"));
    assert!(svg.contains("channel-1-mark-0"));
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("fill-opacity=\"0.2\""));
    assert!(!svg.contains("-2 "));
    assert!(!svg.contains("L10 "));
    assert_eq!(scene, original);
    assert_eq!(
        rasterize(&scene, RasterBackground::Transparent).expect("unchanged raster"),
        raster
    );
}

/// Proves sampled paint keeps original primitives and shares cancellation and work limits.
///
/// # Panics
/// Panics on invalid fixtures, reordered paint, or ignored bounded serialization.
#[test]
fn sampled_paint_keeps_original_primitives_and_request_bounds() {
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(2),
            GeometryOutput::CanonicalMarks(vec![
                circle(1, 3.0, 4.0, 1.0),
                circle(2, 4.0, 4.0, 1.0),
            ]),
            Some(vec![
                ColorValue {
                    red: 0.0,
                    green: 1.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
            ]),
        )],
    )
    .expect("sampled-paint layer");
    let scene = RenderScene::new_modeled(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "paint-family".into(),
        "paint-realization".into(),
        HalftoneChannelModel::SourceColorAlpha,
        vec![layer],
    )
    .expect("source-colored scene");
    let svg = write_svg(&scene).expect("painted output SVG");
    assert_eq!(svg.matches("<circle").count(), 2);
    assert!(!svg.contains("opaque-join"));
    assert!(svg.contains("fill=\"#00ff00\""));
    assert!(svg.contains("fill=\"#0000ff\""));
    let error = write_svg_cancellable_with_limits(
        &scene,
        SvgSerializationLimits::new(4).expect("explicit edge limit"),
        &|| false,
    )
    .expect_err("painted circles must charge the same edge budget");
    assert_eq!(error.path(), "svg.limits.edges");
    let calls = AtomicUsize::new(0);
    let error =
        write_svg_cancellable_with_limits(&scene, SvgSerializationLimits::default(), &|| {
            calls.fetch_add(1, Ordering::Relaxed) >= 5
        })
        .expect_err("sampled-paint serialization must poll within the output");
    assert_eq!(error.path(), "svg.cancelled");
}

/// Builds a renderer-only stroke fixture with explicitly supplied closed contours.
///
/// The harmless centerline/profile supplies provenance; only its immutable
/// derived outline participates in SVG fill normalization.
///
/// # Panics
/// Panics on invalid finite test geometry or scene construction.
fn outline_scene(contours: Vec<Vec<CurveSegment>>) -> RenderScene {
    let path =
        CurvePath::line(Point2::new(1.0, 1.0), Point2::new(7.0, 7.0)).expect("fixture centerline");
    let stroke = CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        path,
        1.0,
        PathStrokeStyle::default(),
        vec![StrokeProfileSample {
            location: PathLocation::new(0, 0.0).expect("fixture location"),
            center: Point2::new(1.0, 1.0),
            normalized_thickness: 1.0,
            width: 1.0,
        }],
        CanonicalFilledOutline {
            full_segment_count: None, prepared: None, coverage: Default::default(), contours: contours
                .into_iter()
                .map(|segments| CanonicalOutlineContour { segments })
                .collect(),
            fill_rule: CanonicalFillRule::NonZero,
            bounds: Some(Bounds {
                min: Point2::new(0.0, 0.0),
                max: Point2::new(8.0, 8.0),
            }),
        },
    )
    .expect("fixture outlined stroke");
    scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalStrokes(vec![stroke]),
        None,
    )])
}

/// Builds an authored diamond, including its exact zero-size four-edge limit.
///
/// # Panics
/// Panics if finite fixture controls cannot form a closed canonical mark.
fn authored_diamond(radius: f64, angle: f64, rule: CanonicalFillRule) -> CanonicalMark {
    let (sin, cos) = angle.to_radians().sin_cos();
    let points = [(0.0, -radius), (radius, 0.0), (0.0, radius), (-radius, 0.0)]
        .map(|(x, y)| Point2::new(3.0 + cos * x - sin * y, 3.0 + sin * x + cos * y));
    let segments = (0..4)
        .map(|i| {
            CurveSegment::Line(
                toniator_geometry::LineSegment::new(points[i], points[(i + 1) % 4])
                    .expect("authored diamond edge"),
            )
        })
        .collect();
    CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            CurvePath::new(segments, PathClosure::Closed).expect("closed authored diamond"),
            SiteScope::Canvas,
            FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            rule,
        )
        .expect("canonical authored diamond"),
    )
}

/// Returns serialized SVG content after intentional canonical-scene identity metadata.
///
/// # Panics
/// Panics on failed serialization or missing current SVG metadata.
fn svg_body(scene: &RenderScene) -> String {
    write_svg(scene)
        .expect("fixture SVG")
        .split_once("</metadata>")
        .expect("current SVG scene metadata")
        .1
        .to_owned()
}

/// Proves exact zero-size authored marks omit no visible geometry at either fill rule or rotation.
///
/// The canonical scene and native raster remain unchanged by final serialization.
/// # Panics
/// Panics on serialization, raster mismatch, or ignored cancellation/work limits.
#[test]
fn collapsed_authored_diamonds_preserve_visible_marks_and_request_bounds() {
    for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
        for angle in [0.0, 45.0, 137.0, 360.0] {
            let visible = authored_diamond(1.0, angle, rule);
            let make_scene = |marks| {
                scene(vec![RenderOutputLayer::new(
                    PatternOutputLayerId(1),
                    GeometryOutput::CanonicalMarks(marks),
                    None,
                )])
            };
            let reference = make_scene(vec![visible.clone()]);
            let mixed = make_scene(vec![authored_diamond(0.0, angle, rule), visible]);
            let original = mixed.clone();
            let raster = rasterize(&mixed, RasterBackground::Transparent).expect("native mixed");
            assert_eq!(svg_body(&mixed), svg_body(&reference));
            assert_eq!(mixed, original);
            assert_eq!(
                raster,
                rasterize(&reference, RasterBackground::Transparent).unwrap()
            );
            let error = write_svg_cancellable_with_limits(
                &mixed,
                SvgSerializationLimits::new(7).unwrap(),
                &|| false,
            )
            .expect_err("collapsed edges still consume canonical work");
            assert_eq!(error.path(), "svg.limits.edges");
            let calls = AtomicUsize::new(0);
            let error = write_svg_cancellable_with_limits(
                &mixed,
                SvgSerializationLimits::default(),
                &|| calls.fetch_add(1, Ordering::Relaxed) >= 6,
            )
            .expect_err("empty classification polls cancellation");
            assert_eq!(error.path(), "svg.cancelled");
            assert!(
                !write_svg(&make_scene(vec![authored_diamond(0.0, angle, rule)]))
                    .expect("entirely empty mark succeeds")
                    .contains("opaque-join")
            );
        }
    }
}

/// Proves exact empty compound members preserve winding, original cubics, and real holes.
///
/// Cubics with coincident endpoints and distinct controls are never classified
/// as empty; their uncertain positive extent still fails closed.
/// # Panics
/// Panics on changed fill semantics, lost exact curves, or an omitted positive contour.
#[test]
fn collapsed_compound_contours_preserve_fill_and_fail_closed() {
    let point = Point2::new(4.0, 4.0);
    let empty = vec![CurveSegment::CubicBezier(
        CubicBezierSegment::new(point, point, point, point).unwrap(),
    )];
    for reverse in [false, true] {
        let positive = square_contour(Point2::new(1.0, 1.0), 6.0, 0.1, reverse);
        let reference = outline_scene(vec![positive.clone()]);
        let mixed = outline_scene(vec![empty.clone(), positive.clone(), empty.clone()]);
        assert_eq!(svg_body(&mixed), svg_body(&reference));
        assert!(write_svg(&mixed).unwrap().contains('C'));
        let inner = square_contour(Point2::new(3.0, 3.0), 2.0, 0.0, !reverse);
        let reference = outline_scene(vec![positive.clone(), inner.clone()]);
        let mixed = outline_scene(vec![empty.clone(), positive.clone(), inner]);
        assert_eq!(svg_body(&mixed), svg_body(&reference));
        let tiny = square_contour(Point2::new(4.0, 4.0), 1.0e-13, 0.0, !reverse);
        let error = write_svg(&outline_scene(vec![empty.clone(), positive, tiny]))
            .expect_err("empty member cannot hide a real subgrid hole");
        assert_eq!(error.path(), "svg.precision.contour");
    }
    assert!(
        !write_svg(&outline_scene(vec![empty.clone(), empty.clone()]))
            .unwrap()
            .contains("opaque-join")
    );
    let curved = vec![CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            point,
            Point2::new(4.0 + 1.0e-13, 4.0),
            Point2::new(4.0, 4.0 + 1.0e-13),
            point,
        )
        .unwrap(),
    )];
    assert_eq!(
        write_svg(&outline_scene(vec![empty.clone(), curved]))
            .expect_err("coincident endpoints do not prove an empty cubic")
            .path(),
        "vector.contour"
    );
}

/// Builds a square contour with an optional exact cubic bulge on its first edge.
///
/// # Panics
/// Panics only on invalid finite fixture geometry.
fn square_contour(origin: Point2, size: f64, bulge: f64, reverse: bool) -> Vec<CurveSegment> {
    let path = CurvePath::polyline(
        vec![
            origin,
            Point2::new(origin.x + size, origin.y),
            Point2::new(origin.x + size, origin.y + size),
            Point2::new(origin.x, origin.y + size),
        ],
        PathClosure::Closed,
    )
    .expect("square contour");
    let mut segments = path.segments().to_vec();
    segments[0] = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            origin,
            Point2::new(origin.x + size / 3.0, origin.y - bulge),
            Point2::new(origin.x + 2.0 * size / 3.0, origin.y - bulge),
            Point2::new(origin.x + size, origin.y),
        )
        .expect("bulged square edge"),
    );
    if reverse {
        segments.reverse();
        for segment in &mut segments {
            *segment = match *segment {
                CurveSegment::Line(line) => CurveSegment::Line(
                    toniator_geometry::LineSegment::new(line.end(), line.start())
                        .expect("reverse line"),
                ),
                CurveSegment::CubicBezier(cubic) => CurveSegment::CubicBezier(
                    CubicBezierSegment::new(
                        cubic.end(),
                        cubic.control_2(),
                        cubic.control_1(),
                        cubic.start(),
                    )
                    .expect("reverse cubic"),
                ),
            };
        }
    }
    segments
}

/// Proves exact opposite contours cancel while a sub-tolerance real sliver fails closed.
///
/// # Panics
/// Panics if flattening equality is mistaken for canonical coincidence.
#[test]
fn exact_cancellation_does_not_erase_a_real_curved_sliver() {
    let outer = square_contour(Point2::new(0.0, 0.0), 1.0e-6, 0.0, false);
    let exact = outline_scene(vec![
        outer.clone(),
        square_contour(Point2::new(0.0, 0.0), 1.0e-6, 0.0, true),
    ]);
    assert!(
        !write_svg(&exact)
            .expect("exact cancellation SVG")
            .contains("opaque-join")
    );
    let sliver = outline_scene(vec![
        outer,
        square_contour(Point2::new(0.0, 0.0), 1.0e-6, 1.0e-10, true),
    ]);
    let error =
        write_svg(&sliver).expect_err("genuine sub-tolerance sliver cannot silently disappear");
    assert_eq!(error.path(), "svg.precision.omitted");
    let visible = outline_scene(vec![
        square_contour(Point2::new(1.0, 1.0), 1.0, 0.0, false),
        square_contour(Point2::new(1.0, 1.0), 1.0, 0.01, true),
    ]);
    assert!(
        write_svg(&visible)
            .expect("resolvable curved sliver SVG")
            .contains("opaque-join")
    );
}

/// Proves a tiny hole fails closed while a certified positive component stays exact.
///
/// # Panics
/// Panics if a real component disappears or a collapsed hole reaches normalization.
#[test]
fn subgrid_hole_fails_and_exact_component_survives() {
    for reverse in [false, true] {
        let scene = outline_scene(vec![
            square_contour(Point2::new(1.0, 1.0), 6.0, 0.0, false),
            square_contour(
                if reverse {
                    Point2::new(4.0, 4.0)
                } else {
                    Point2::new(0.5, 0.5)
                },
                1.0e-13,
                0.0,
                reverse,
            ),
        ]);
        if reverse {
            let error = write_svg(&scene).expect_err("tiny real hole cannot disappear silently");
            assert_eq!(error.path(), "svg.precision.contour");
        } else {
            let svg = write_svg(&scene).expect("certified tiny positive component stays exact");
            assert_eq!(svg.matches('M').count(), 2);
            assert!(svg.contains("0.5000000000001"));
        }
    }
}

/// Proves overlapping same-winding NonZero contours retain their exact cubic edges.
///
/// A primitive with several disconnected positive stroke runs needs no Boolean
/// flattening when every simple contour contributes winding of the same sign.
///
/// # Panics
/// Panics if multi-contour exact geometry is unnecessarily flattened.
#[test]
fn same_winding_multicontour_stroke_keeps_exact_curves() {
    let scene = outline_scene(vec![
        square_contour(Point2::new(1.0, 1.0), 3.0, 0.1, false),
        square_contour(Point2::new(3.0, 3.0), 3.0, 0.1, false),
    ]);
    let svg = write_svg(&scene).expect("same-winding exact compound");
    assert_eq!(svg.matches('C').count(), 2);
    assert_eq!(svg.matches('L').count(), 6);
}

/// Writes current native RGBA and SVG overlap/hole witnesses for an external consumer.
///
/// # Panics
/// Panics on fixture, rendering, serialization, or stage-owned artifact I/O failure.
#[test]
#[ignore = "writes current TON-018 native consumer witnesses"]
fn native_overlap_and_hole_consumer_witnesses() {
    let directory = std::path::Path::new("../../target/validation/ton018-svg-join-20261013");
    std::fs::create_dir_all(directory).expect("stage artifact directory");
    let overlap = scene(vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(vec![circle(0, 3.0, 4.0, 2.0), circle(1, 5.0, 4.0, 2.0)]),
        None,
    )]);
    let hole = outline_scene(vec![
        square_contour(Point2::new(1.0, 1.0), 6.0, 0.0, false),
        square_contour(Point2::new(3.0, 3.0), 2.0, 0.0, true),
    ]);
    for (name, scene) in [("synthetic-overlap", overlap), ("synthetic-hole", hole)] {
        let raster = rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(256, 256).expect("witness size")),
            RasterAntialiasing::On,
        )
        .expect("native witness raster");
        std::fs::write(
            directory.join(format!("{name}.png")),
            encode_png(&raster).expect("native RGBA PNG"),
        )
        .expect("native witness writes");
        std::fs::write(
            directory.join(format!("{name}.svg")),
            write_svg(&scene).expect("native vector SVG"),
        )
        .expect("SVG witness writes");
    }
}

/// Builds current canvas-intersection witnesses from immutable renderer-only geometry.
/// # Panics
/// Panics only if a finite fixture violates canonical or scene construction.
fn canvas_intersection_fixtures() -> Vec<(&'static str, RenderScene)> {
    let marks = |values: Vec<CanonicalMark>| {
        scene(vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalMarks(values),
            None,
        )])
    };
    let disconnected = CurvePath::polyline(
        vec![
            Point2::new(1.0, 5.0),
            Point2::new(3.0, 5.0),
            Point2::new(3.0, -2.0),
            Point2::new(5.0, -2.0),
            Point2::new(5.0, 5.0),
            Point2::new(7.0, 5.0),
            Point2::new(7.0, -3.0),
            Point2::new(1.0, -3.0),
        ],
        PathClosure::Closed,
    )
    .expect("disconnected clip contour");
    let mut curve = CurvePath::polyline(
        vec![
            Point2::new(-2.0, 1.0),
            Point2::new(10.0, 6.0),
            Point2::new(10.0, 7.0),
            Point2::new(-2.0, 7.0),
        ],
        PathClosure::Closed,
    )
    .expect("curve fixture")
    .segments()
    .to_vec();
    curve[0] = CurveSegment::CubicBezier(
        CubicBezierSegment::new(
            Point2::new(-2.0, 1.0),
            Point2::new(1.0, 2.0),
            Point2::new(4.0, 5.0),
            Point2::new(10.0, 6.0),
        )
        .expect("genuine cubic edge"),
    );
    vec![
        ("circle-edge", marks(vec![circle(0, 0.0, 4.0, 2.0)])),
        ("circle-corner", marks(vec![circle(0, 0.0, 0.0, 3.0)])),
        ("circle-enclosing", marks(vec![circle(0, 4.0, 4.0, 10.0)])),
        ("circle-tangent", marks(vec![circle(0, -2.0, 4.0, 2.0)])),
        (
            "hole-clipped",
            outline_scene(vec![
                square_contour(Point2::new(-2.0, -2.0), 12.0, 0.0, false),
                square_contour(Point2::new(-1.0, 2.0), 4.0, 0.0, true),
            ]),
        ),
        (
            "disconnected",
            outline_scene(vec![disconnected.segments().to_vec()]),
        ),
        ("true-cubic", outline_scene(vec![curve])),
    ]
}

/// Proves circles retain radii, tangent-only fills disappear, and genuine curves survive intersection.
/// # Panics
/// Panics on failed serialization or lost analytic geometry/fill structure.
#[test]
fn closed_canvas_intersection_keeps_circle_and_curve_geometry() {
    for (name, scene) in canvas_intersection_fixtures() {
        let before = scene.clone();
        let svg = write_svg(&scene).expect("canvas intersection SVG");
        assert_eq!(scene, before, "SVG does not mutate {name}");
        match name {
            "circle-edge" => {
                assert!(svg.contains("a2 2"));
                assert!(svg.contains('l'));
                assert!(!svg.contains('C'));
            }
            "circle-corner" => {
                assert!(svg.contains("a3 3"));
                assert!(!svg.contains('C'));
            }
            "circle-tangent" => assert!(!svg.contains("opaque-join")),
            "true-cubic" => assert!(svg.contains('C')),
            _ => assert!(svg.contains("opaque-join")),
        }
    }
    for (center, radius) in [
        (Point2::new(0.0, 0.0), 1.0e-15),
        (Point2::new(8.0, 4.0), 1.0e-20),
    ] {
        let svg = write_svg(&scene(vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalMarks(vec![circle(0, center.x, center.y, radius)]),
            None,
        )]))
        .expect("tiny boundary circle");
        assert!(
            svg.contains(&format!("a{radius} {radius}")),
            "original tiny radius survives"
        );
        assert!(
            svg.contains("l0 "),
            "boundary circle is genuinely intersected even below the center ULP"
        );
    }
}

/// Writes native RGBA and SVG canvas witnesses for actual Inkscape fill/alpha verification.
/// # Panics
/// Panics on fixture, native render, serialization, or stage-owned artifact I/O failure.
#[test]
#[ignore = "writes current canvas-intersection consumer witnesses"]
fn native_canvas_intersection_consumer_witnesses() {
    let directory = std::path::Path::new("../../target/validation/ton018-svg-canvas-20261013");
    std::fs::create_dir_all(directory).expect("canvas artifact directory");
    for (name, scene) in canvas_intersection_fixtures() {
        let raster = rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(256, 256).expect("witness size")),
            RasterAntialiasing::On,
        )
        .expect("native canvas witness raster");
        std::fs::write(
            directory.join(format!("{name}.png")),
            encode_png(&raster).expect("native RGBA PNG"),
        )
        .expect("native witness writes");
        std::fs::write(
            directory.join(format!("{name}.svg")),
            write_svg(&scene).expect("native canvas SVG"),
        )
        .expect("SVG witness writes");
    }
}
