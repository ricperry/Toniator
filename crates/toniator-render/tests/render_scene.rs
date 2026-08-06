use toniator_domain::{CanvasSpec, ChannelId, ColorValue};
use toniator_geometry::{
    CanonicalCircleMark, GuideInstanceId, GuideIntersectionProvenance, Point2, SiteId, SiteScope,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RasterSurface, RenderLayer, RenderScene, encode_png,
    linear_to_srgb, rasterize, srgb_to_linear, write_svg,
};

fn mark(x: f64, y: f64, radius: f64, scope: SiteScope) -> CanonicalCircleMark {
    CanonicalCircleMark::new(
        SiteId {
            first_dimension_id: 1,
            first_index: 0,
            second_dimension_id: 2,
            second_index: 0,
        },
        Point2::new(x, y),
        radius,
        scope,
        GuideIntersectionProvenance {
            contributors: [
                GuideInstanceId {
                    dimension_id: 1,
                    index: 0,
                },
                GuideInstanceId {
                    dimension_id: 2,
                    index: 0,
                },
            ],
        },
    )
    .unwrap()
}

fn scene(visible: bool, color: ColorValue, opacity: f64) -> RenderScene {
    RenderScene::new(
        CanvasSpec {
            width: 10.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                visible,
                color,
                opacity,
                GeometryOutput::CircularMarks(vec![
                    mark(-0.25, 4.0, 1.0, SiteScope::Guard),
                    mark(5.0, 4.0, 2.0, SiteScope::Canvas),
                ]),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn scene_validates_order_preserves_geometry_and_includes_presentation_in_identity() {
    let blue = ColorValue {
        red: 0.0,
        green: srgb_to_linear(183.0 / 255.0),
        blue: 1.0,
        alpha: 1.0,
    };
    let first = scene(true, blue.clone(), 0.72);
    let color_only = scene(
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        0.72,
    );
    let opacity_only = scene(true, blue.clone(), 0.2);
    let visibility_only = scene(false, blue, 0.72);
    assert_eq!(first.circular_mark_count(), 2);
    for changed in [&color_only, &opacity_only, &visibility_only] {
        assert_eq!(first.layers()[0].geometry(), changed.layers()[0].geometry());
        assert_eq!(
            first.identity().family_fingerprint(),
            changed.identity().family_fingerprint()
        );
        assert_eq!(
            first.identity().realization_fingerprint(),
            changed.identity().realization_fingerprint()
        );
        assert_ne!(
            first.identity().scene_fingerprint(),
            changed.identity().scene_fingerprint()
        );
    }
    let first_svg = write_svg(&first);
    let color_svg = write_svg(&color_only);
    assert!(first_svg.contains("family=family;realization=realization;scene="));
    assert!(color_svg.contains("family=family;realization=realization;scene="));
    assert!(!color_svg.contains(first.identity().scene_fingerprint()));
    let unordered = RenderScene::new(
        CanvasSpec {
            width: 10.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![
            first.layers()[0].clone(),
            RenderLayer::new(
                ChannelId(1),
                first.layers()[0].visible(),
                first.layers()[0].color().clone(),
                first.layers()[0].opacity(),
                first.layers()[0].geometry().clone(),
            )
            .unwrap(),
        ],
    );
    assert!(unordered.is_err());
}

#[test]
fn multi_layer_order_visibility_and_source_over_are_preserved() {
    let marks = GeometryOutput::CircularMarks(vec![mark(5.0, 4.0, 2.0, SiteScope::Canvas)]);
    let red = RenderLayer::new(
        ChannelId(9),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        0.5,
        marks.clone(),
    )
    .unwrap();
    let blue = RenderLayer::new(
        ChannelId(2),
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
            alpha: 1.0,
        },
        0.5,
        marks.clone(),
    )
    .unwrap();
    let invisible = RenderLayer::new(
        ChannelId(3),
        false,
        ColorValue {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        marks,
    )
    .unwrap();
    let first = RenderScene::new(
        CanvasSpec {
            width: 10.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![red.clone(), blue.clone(), invisible.clone()],
    )
    .unwrap();
    let second = RenderScene::new(
        CanvasSpec {
            width: 10.0,
            height: 8.0,
        },
        "family".into(),
        "realization".into(),
        vec![blue, red, invisible],
    )
    .unwrap();
    assert_ne!(
        first.identity().scene_fingerprint(),
        second.identity().scene_fingerprint()
    );
    let raster = rasterize(&first, RasterBackground::Transparent).unwrap();
    let pixel = &raster.pixels()[4 * (4 * 10 + 5)..4 * (4 * 10 + 6)];
    assert!(
        pixel[2] > pixel[0],
        "later blue source-over layer is visible"
    );
    let svg = write_svg(&first);
    assert!(svg.find("channel-9").unwrap() < svg.find("channel-2").unwrap());
    assert!(!svg.contains("channel-3"));
}

#[test]
fn scene_identity_covers_canonical_provenance() {
    let first = scene(
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
    );
    let GeometryOutput::CircularMarks(mut marks) = first.layers()[0].geometry().clone();
    marks[0].provenance.contributors[1].index = 99;
    let second = RenderScene::new(
        first.canvas().clone(),
        first.identity().family_fingerprint().to_owned(),
        first.identity().realization_fingerprint().to_owned(),
        vec![
            RenderLayer::new(
                first.layers()[0].channel_id(),
                first.layers()[0].visible(),
                first.layers()[0].color().clone(),
                first.layers()[0].opacity(),
                GeometryOutput::CircularMarks(marks),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert_ne!(
        first.identity().scene_fingerprint(),
        second.identity().scene_fingerprint()
    );
}

#[test]
fn raster_surface_is_straight_srgba_and_clips_only_at_surface_bounds() {
    let scene = scene(
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.5,
        },
        0.5,
    );
    let transparent = rasterize(&scene, RasterBackground::Transparent).unwrap();
    assert_eq!((transparent.width(), transparent.height()), (10, 8));
    assert_eq!(transparent.pixels().len(), 10 * 8 * 4);
    let edge = &transparent.pixels()[4 * (4 * 10)..4 * (4 * 10 + 1)];
    assert!(
        edge[3] > 0,
        "the overlapping guard circle survives final clipping"
    );
    assert!(
        edge[0] > edge[3] / 2,
        "straight sRGBA unpremultiplies color at output"
    );
    assert_eq!(
        scene.circular_mark_count(),
        2,
        "raster clipping never mutates scene geometry"
    );
    assert!(!encode_png(&transparent).unwrap().is_empty());
    assert!(RasterSurface::new(1, 1, vec![0; 3]).is_err());
}

#[test]
fn backgrounds_visibility_effective_alpha_and_color_boundaries_are_explicit() {
    let blue = ColorValue {
        red: 0.0,
        green: srgb_to_linear(183.0 / 255.0),
        blue: 1.0,
        alpha: 1.0,
    };
    let render_scene = scene(true, blue, 0.72);
    let scene_identity = render_scene.identity().scene_fingerprint().to_owned();
    let black = rasterize(&render_scene, RasterBackground::OpaqueBlack).unwrap();
    let white = rasterize(&render_scene, RasterBackground::OpaqueWhite).unwrap();
    let transparent = rasterize(&render_scene, RasterBackground::Transparent).unwrap();
    assert_eq!(&black.pixels()[..4], &[0, 0, 0, 255]);
    assert_eq!(&white.pixels()[..4], &[255, 255, 255, 255]);
    assert_eq!(transparent.pixels()[3], 0);
    let center_alpha = transparent.pixels()[4 * (4 * 10 + 5) + 3];
    assert!(
        (center_alpha as i32 - 184).abs() <= 1,
        "1.0 * 0.72 becomes straight alpha"
    );
    let half_alpha = rasterize(
        &scene(
            true,
            ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.5,
            },
            0.5,
        ),
        RasterBackground::Transparent,
    )
    .unwrap();
    let fully_covered_alpha = half_alpha.pixels()[4 * (4 * 10 + 5) + 3];
    assert!(
        (fully_covered_alpha as i32 - 64).abs() <= 1,
        "effective alpha is color alpha 0.5 times layer opacity 0.5"
    );
    assert!((linear_to_srgb(srgb_to_linear(183.0 / 255.0)) - 183.0 / 255.0).abs() < 1e-9);
    let invisible = rasterize(
        &scene(
            false,
            ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            1.0,
        ),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert!(
        invisible
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 0])
    );
    assert_eq!(
        render_scene.identity().scene_fingerprint(),
        scene_identity,
        "consumer-only background selection never changes the scene identity"
    );
}

#[test]
fn svg_is_deterministic_clipped_and_uses_linear_to_srgb_color() {
    let svg = write_svg(&scene(
        true,
        ColorValue {
            red: 0.0,
            green: srgb_to_linear(183.0 / 255.0),
            blue: 1.0,
            alpha: 1.0,
        },
        0.72,
    ));
    assert!(svg.contains("width=\"10\" height=\"8\" viewBox=\"0 0 10 8\""));
    assert!(svg.contains("<clipPath id=\"canvas-clip\""));
    assert!(svg.contains("fill=\"#00b7ff\" fill-opacity=\"0.72\""));
    assert_eq!(svg.matches("<circle ").count(), 2);
    assert!(svg.contains("family=family;realization=realization;scene=fnv1a64:"));
}
