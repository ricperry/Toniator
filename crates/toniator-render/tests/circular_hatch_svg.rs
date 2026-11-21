//! Current curved circular stroke primitive and SVG editability contracts.
use toniator_domain::{
    AuthoredPoint2, CanvasSpec, ChannelId, ColorValue, GuideDimensionId, GuidePrototype,
    PathStrokeStyle, PatternOutputLayerId,
};
use toniator_geometry::{
    CanonicalStroke, CubicBezierSegment, CurvePath, CurveSegment, PathClosure, PathLocation,
    Point2, StrokeProfileSample, StructuralPathInstanceId, VariableWidthOutlineLimits,
    build_circular_stroke_outline_cancellable, resolve_guide_prototype,
};
use toniator_render::{
    GeometryOutput, RenderLayer, RenderOutputLayer, RenderScene, SvgSerializationLimits, rasterize,
    write_svg, write_svg_cancellable_with_limits,
};

/// Builds a real geometry-certified annulus or near-full open ribbon, optionally crossing the canvas.
/// # Panics
/// Panics if current circle, closed-seam, profile or scene validation rejects finite test geometry.
fn scene(closed: bool, clipped: bool) -> RenderScene {
    let center = Point2::new(if clipped { 4. } else { 64. }, 64.);
    let path = resolve_guide_prototype(
        &GuidePrototype::CircularArc {
            center: AuthoredPoint2 {
                x: center.x,
                y: center.y,
            },
            radius: 40.,
            start_angle_degrees: 0.,
            sweep_angle_degrees: 360.,
        },
        None,
    )
    .unwrap();
    let mut segments = path.segments().to_vec();
    let CurveSegment::CubicBezier(last) = *segments.last().unwrap() else {
        unreachable!()
    };
    *segments.last_mut().unwrap() = CurveSegment::CubicBezier(if closed {
        CubicBezierSegment::new(
            last.start(),
            last.control_1(),
            last.control_2(),
            segments[0].start(),
        )
        .unwrap()
    } else {
        last.split(0.9999).unwrap().0
    });
    let path = CurvePath::new(
        segments,
        if closed {
            PathClosure::Closed
        } else {
            PathClosure::Open
        },
    )
    .unwrap();
    let outline = build_circular_stroke_outline_cancellable(
        &path,
        center,
        40.,
        1.,
        2.,
        0.,
        0.04,
        VariableWidthOutlineLimits::new(64).unwrap(),
        &|| false,
    )
    .unwrap()
    .unwrap();
    let stroke = CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        path.clone(),
        2.,
        PathStrokeStyle::default(),
        vec![StrokeProfileSample {
            location: PathLocation::new(0, 0.).unwrap(),
            center: path.start(),
            normalized_thickness: 1.,
            width: 2.,
        }],
        outline,
    )
    .unwrap();
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 0.,
            green: 0.,
            blue: 0.,
            alpha: 1.,
        },
        1.,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalStrokes(vec![stroke]),
            None,
        )],
    )
    .unwrap();
    RenderScene::new(
        CanvasSpec {
            width: 128.,
            height: 128.,
        },
        "curved-family".into(),
        "curved-output".into(),
        vec![layer],
    )
    .unwrap()
}

/// Retains curved body/half-caps and opposite-winding holes through opaque joining and final clipping.
/// # Panics
/// Panics if serialization polygonizes a certified primitive, fills its center or drops work authority.
#[test]
fn certified_circular_svg_keeps_low_node_curves_and_holes() {
    for closed in [false, true] {
        for clipped in [false, true] {
            let scene = scene(closed, clipped);
            let svg = write_svg(&scene).unwrap();
            let data = svg
                .split(" d=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let nodes = data
                .chars()
                .filter(|c| matches!(c, 'M' | 'L' | 'C'))
                .count();
            assert!(data.matches('C').count() >= 4);
            assert!(nodes < 64, "curved primitive has {nodes} nodes");
            assert!(svg.contains("fill-rule=\"nonzero\""));
            let image = rasterize(&scene, toniator_render::RasterBackground::Transparent).unwrap();
            let x = if clipped { 4 } else { 64 };
            assert_eq!(
                image.pixels()[(64 * 128 + x) * 4 + 3],
                0,
                "annular center stays unpainted"
            );
            assert_eq!(
                write_svg_cancellable_with_limits(
                    &scene,
                    SvgSerializationLimits::new(1).unwrap(),
                    &|| false
                )
                .unwrap_err()
                .path(),
                "svg.limits.edges"
            );
        }
    }
}
