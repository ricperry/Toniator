use toniator_domain::{CanvasSpec, ChannelId, ColorValue, GuideDimensionId, PathStrokeStyle};
use toniator_geometry::{
    CanonicalStroke, CurvePath, PathLocation, Point2, StrokeProfileSample,
    StructuralPathInstanceId, VariableWidthOutlineLimits, VariableWidthPathSample,
    build_variable_width_outline_cancellable,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RenderLayer, RenderScene, rasterize, write_svg,
};

/// Builds one finite canonical outline stroke for renderer structural witnesses.
fn stroke(y: f64) -> CanonicalStroke {
    let path = CurvePath::line(Point2::new(-2.0, y), Point2::new(10.0, y)).expect("line");
    let profile = vec![
        StrokeProfileSample {
            location: PathLocation::new(0, 0.0).expect("location"),
            center: Point2::new(-2.0, y),
            normalized_thickness: 1.0,
            width: 4.0,
        },
        StrokeProfileSample {
            location: PathLocation::new(0, 1.0).expect("location"),
            center: Point2::new(10.0, y),
            normalized_thickness: 1.0,
            width: 4.0,
        },
    ];
    let outline = build_variable_width_outline_cancellable(
        &path,
        &[
            VariableWidthPathSample {
                location: profile[0].location,
                width: 4.0,
            },
            VariableWidthPathSample {
                location: profile[1].location,
                width: 4.0,
            },
        ],
        PathStrokeStyle::default(),
        1.0 / 8.0,
        VariableWidthOutlineLimits::new(32).expect("limit"),
        &|| false,
    )
    .expect("outline");
    CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), y as i64, 0),
        None,
        path,
        4.0,
        PathStrokeStyle::default(),
        profile,
        outline,
    )
    .expect("stroke")
}

/// Proves SVG writes direct nonzero outline paths and keeps off-canvas canonical geometry intact.
#[test]
fn stroke_outline_is_directly_painted_and_canonical_geometry_remains_offcanvas() {
    let canonical_stroke = stroke(4.0);
    let original = canonical_stroke.clone();
    let layer = RenderLayer::new(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        0.5,
        GeometryOutput::CanonicalStrokes(vec![canonical_stroke]),
    )
    .expect("layer");
    let scene = RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![layer],
    )
    .expect("scene");
    let svg = write_svg(&scene);
    assert!(svg.contains("fill-rule=\"nonzero\""));
    assert!(!svg.contains("stroke-0-clip"));
    assert!(!svg.contains("<rect id=\"channel-1-stroke"));
    assert!(!svg.contains("<circle"));
    let raster = rasterize(&scene, RasterBackground::Transparent).expect("native raster");
    assert_eq!(raster.pixels().len(), 8 * 8 * 4);
    assert!(original.profile[0].center.x < 0.0);
    let offcanvas = RenderLayer::new(
        ChannelId(2),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        GeometryOutput::CanonicalStrokes(vec![stroke(20.0)]),
    )
    .expect("offcanvas layer");
    let partial = RenderLayer::new(
        ChannelId(3),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        GeometryOutput::CanonicalStrokes(vec![stroke(0.0)]),
    )
    .expect("partial layer");
    let culled = RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![offcanvas, partial],
    )
    .expect("scene");
    let culled_svg = write_svg(&culled);
    assert!(!culled_svg.contains("channel-2-stroke-0"));
    assert!(culled_svg.contains("channel-3-stroke-0"));
}

/// Proves ordered strokes contribute distinct scene identities and source-over compositing.
#[test]
fn stroke_order_changes_scene_identity_and_two_layers_darken() {
    let layer = |id, y| {
        RenderLayer::new(
            ChannelId(id),
            true,
            ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            0.5,
            GeometryOutput::CanonicalStrokes(vec![stroke(y)]),
        )
        .expect("layer")
    };
    let first = RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "f".into(),
        "r".into(),
        vec![layer(1, 4.0), layer(2, 4.0)],
    )
    .expect("scene");
    let second = RenderScene::new(
        CanvasSpec {
            width: 8.0,
            height: 8.0,
        },
        "f".into(),
        "r".into(),
        vec![layer(2, 4.0), layer(1, 4.0)],
    )
    .expect("scene");
    assert_ne!(
        first.identity().scene_fingerprint(),
        second.identity().scene_fingerprint()
    );
    assert!(
        rasterize(&first, RasterBackground::Transparent)
            .expect("raster")
            .pixels()
            .iter()
            .any(|value| *value > 127)
    );
}
