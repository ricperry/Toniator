use std::{
    fs,
    process::Command,
    sync::{Mutex, OnceLock},
};

use toniator_domain::{CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel};
use toniator_geometry::{
    CanonicalCircleMark, GuideInstanceId, GuideIntersectionProvenance, Point2, SiteId, SiteScope,
};
use toniator_render::{
    GeometryOutput, OutputRasterTarget, PreviewRasterTarget, RasterAntialiasing, RasterBackground,
    RasterSurface, RenderLayer, RenderScene, SourceColorCircle, encode_png, linear_to_srgb,
    raster_output_identity, rasterize, rasterize_output, rasterize_preview, srgb_to_linear,
    write_svg,
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
            contributors: vec![
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

fn modeled(model: HalftoneChannelModel, layers: Vec<RenderLayer>) -> RenderScene {
    modeled_with_canvas(model, 6.0, 6.0, layers)
}

fn modeled_with_canvas(
    model: HalftoneChannelModel,
    width: f64,
    height: f64,
    layers: Vec<RenderLayer>,
) -> RenderScene {
    RenderScene::new_modeled(
        CanvasSpec { width, height },
        "stage-9c-family".into(),
        "stage-9c-realization".into(),
        model,
        layers,
    )
    .unwrap()
}

fn overlap_layer(channel_id: u64, color: ColorValue, opacity: f64, center_x: f64) -> RenderLayer {
    overlap_layer_with_visibility(channel_id, true, color, opacity, center_x)
}

fn overlap_layer_with_visibility(
    channel_id: u64,
    visible: bool,
    color: ColorValue,
    opacity: f64,
    center_x: f64,
) -> RenderLayer {
    RenderLayer::new(
        ChannelId(channel_id),
        visible,
        color,
        opacity,
        GeometryOutput::CircularMarks(vec![mark(center_x, 80.0, 66.0, SiteScope::Canvas)]),
    )
    .unwrap()
}

fn full_mark() -> GeometryOutput {
    GeometryOutput::CircularMarks(vec![mark(3.5, 3.5, 1.0, SiteScope::Canvas)])
}

fn solid(channel_id: u64, color: ColorValue, opacity: f64) -> RenderLayer {
    RenderLayer::new(ChannelId(channel_id), true, color, opacity, full_mark()).unwrap()
}

fn center(surface: &RasterSurface) -> [u8; 4] {
    let offset = 4 * (3 * surface.width() as usize + 3);
    surface.pixels()[offset..offset + 4].try_into().unwrap()
}

fn color(red: f64, green: f64, blue: f64, alpha: f64) -> ColorValue {
    ColorValue {
        red,
        green,
        blue,
        alpha,
    }
}

#[test]
fn preview_target_rerasterizes_geometry_without_changing_native_raster() {
    let scene = scene(true, color(1.0, 0.0, 0.0, 1.0), 1.0);
    let native_before = rasterize(&scene, RasterBackground::Transparent).unwrap();
    let target = PreviewRasterTarget::new(100, 100).unwrap();
    let preview = rasterize_preview(&scene, target).unwrap();
    let native_after = rasterize(&scene, RasterBackground::Transparent).unwrap();
    assert_eq!((preview.width(), preview.height()), (100, 100));
    assert_eq!(native_before, native_after);
    assert_eq!(preview.pixels().len(), 100 * 100 * 4);
    assert_eq!(&preview.pixels()[0..4], &[0, 0, 0, 0]);
    let alpha = preview
        .pixels()
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .collect::<Vec<_>>();
    let fractional = alpha
        .iter()
        .filter(|&&value| value > 0 && value < 255)
        .count();
    assert!(alpha.contains(&255));
    assert!(
        fractional > 0 && fractional < 600,
        "output-space AA must remain a narrow edge band"
    );
}

#[test]
fn png_antialiasing_and_output_target_are_final_consumer_only() {
    let scene = scene(true, color(1.0, 0.0, 0.0, 1.0), 1.0);
    let identity = scene.identity().clone();
    let svg = write_svg(&scene);
    let accepted = rasterize(&scene, RasterBackground::Transparent).unwrap();
    let explicit_on = rasterize_output(
        &scene,
        RasterBackground::Transparent,
        None,
        RasterAntialiasing::On,
    )
    .unwrap();
    let hard_edges = rasterize_output(
        &scene,
        RasterBackground::Transparent,
        None,
        RasterAntialiasing::Off,
    )
    .unwrap();
    let resized = rasterize_output(
        &scene,
        RasterBackground::Transparent,
        Some(OutputRasterTarget::new(20, 16).unwrap()),
        RasterAntialiasing::On,
    )
    .unwrap();
    assert_eq!(accepted, explicit_on, "on is the accepted native result");
    assert_ne!(accepted, hard_edges, "off changes only the final PNG bytes");
    assert_ne!(
        raster_output_identity(
            &scene,
            RasterBackground::Transparent,
            None,
            RasterAntialiasing::On,
        ),
        raster_output_identity(
            &scene,
            RasterBackground::Transparent,
            None,
            RasterAntialiasing::Off,
        ),
        "antialiasing is isolated to raster-output cache identity"
    );
    assert!(
        hard_edges
            .pixels()
            .chunks_exact(4)
            .all(|pixel| matches!(pixel[3], 0 | 255)),
        "off has no fractional edge coverage"
    );
    assert_eq!((resized.width(), resized.height()), (20, 16));
    assert_eq!(scene.identity(), &identity);
    assert_eq!(write_svg(&scene), svg);
    assert_eq!(
        rasterize(&scene, RasterBackground::Transparent).unwrap(),
        accepted
    );
    assert_eq!(
        OutputRasterTarget::new(0, 1).unwrap_err().path(),
        "output.target"
    );
    let unsafe_native = RenderScene::new(
        CanvasSpec {
            width: 67_108_865.0,
            height: 1.0,
        },
        "family".into(),
        "realization".into(),
        vec![scene.layers()[0].clone()],
    )
    .unwrap();
    assert_eq!(
        rasterize(&unsafe_native, RasterBackground::Transparent)
            .unwrap_err()
            .path(),
        "output.target",
        "native document-canvas rasterization is checked before allocation"
    );
}

#[test]
fn preview_target_rejects_invalid_or_unsafe_extents() {
    assert_eq!(
        PreviewRasterTarget::new(0, 1).unwrap_err().path(),
        "preview.target"
    );
    assert_eq!(
        PreviewRasterTarget::new(8193, 8193).unwrap_err().path(),
        "preview.target"
    );
}

#[test]
fn preview_letterbox_clips_crossing_guard_marks_to_transformed_canvas_for_all_models() {
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let geometry = GeometryOutput::CircularMarks(vec![mark(5.0, -0.5, 1.5, SiteScope::Guard)]);
        let layer = match model {
            HalftoneChannelModel::SourceColorAlpha => RenderLayer::new_source_color(
                ChannelId(8),
                true,
                1.0,
                vec![SourceColorCircle {
                    mark: mark(5.0, -0.5, 1.5, SiteScope::Guard),
                    paint: color(1.0, 0.0, 0.0, 1.0),
                }],
            )
            .unwrap(),
            _ => RenderLayer::new(ChannelId(1), true, color(1.0, 0.0, 0.0, 1.0), 1.0, geometry)
                .unwrap(),
        };
        let scene = modeled_with_canvas(model, 10.0, 5.0, vec![layer]);
        let native = rasterize(&scene, RasterBackground::Transparent).unwrap();
        let preview = rasterize_preview(&scene, PreviewRasterTarget::new(10, 10).unwrap()).unwrap();
        assert_eq!(
            native,
            rasterize(&scene, RasterBackground::Transparent).unwrap()
        );
        for y in 0..2 {
            assert!(
                preview.pixels()[y * 10 * 4..(y + 1) * 10 * 4]
                    .chunks_exact(4)
                    .all(|p| p[3] == 0)
            );
        }
        assert!(
            preview.pixels()[2 * 10 * 4..5 * 10 * 4]
                .chunks_exact(4)
                .any(|p| p[3] > 0)
        );
    }
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

#[test]
fn rgb_plus_uses_linear_primaries_secondaries_neutrals_and_saturation() {
    let red = color(1.0, 0.0, 0.0, 1.0);
    let green = color(0.0, 1.0, 0.0, 1.0);
    let blue = color(0.0, 0.0, 1.0, 1.0);
    for (layers, expected) in [
        (vec![solid(1, red.clone(), 1.0)], [255, 0, 0, 255]),
        (
            vec![solid(1, red.clone(), 1.0), solid(2, green.clone(), 1.0)],
            [255, 255, 0, 255],
        ),
        (
            vec![solid(1, red.clone(), 1.0), solid(3, blue.clone(), 1.0)],
            [255, 0, 255, 255],
        ),
        (
            vec![solid(2, green.clone(), 1.0), solid(3, blue.clone(), 1.0)],
            [0, 255, 255, 255],
        ),
        (
            vec![
                solid(1, red.clone(), 1.0),
                solid(2, green.clone(), 1.0),
                solid(3, blue.clone(), 1.0),
            ],
            [255, 255, 255, 255],
        ),
    ] {
        let surface = rasterize(
            &modeled(HalftoneChannelModel::Rgb, layers),
            RasterBackground::Transparent,
        )
        .unwrap();
        assert_eq!(center(&surface), expected);
    }
    let neutral = rasterize(
        &modeled(
            HalftoneChannelModel::Rgb,
            vec![solid(1, color(0.25, 0.25, 0.25, 1.0), 1.0)],
        ),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert_eq!(center(&neutral)[0], center(&neutral)[1]);
    assert_eq!(center(&neutral)[1], center(&neutral)[2]);
    let saturated = rasterize(
        &modeled(
            HalftoneChannelModel::Rgb,
            vec![solid(1, red.clone(), 1.0), solid(2, red, 1.0)],
        ),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert_eq!(center(&saturated), [255, 0, 0, 255]);
}

#[test]
fn cmyk_transmittance_has_exact_secondaries_k_and_white_recovery() {
    let cyan = color(0.0, 1.0, 1.0, 1.0);
    let magenta = color(1.0, 0.0, 1.0, 1.0);
    let yellow = color(1.0, 1.0, 0.0, 1.0);
    let black = color(0.0, 0.0, 0.0, 1.0);
    for (layers, expected) in [
        (
            vec![
                solid(5, magenta.clone(), 1.0),
                solid(6, yellow.clone(), 1.0),
            ],
            [255, 0, 0, 255],
        ),
        (
            vec![solid(4, cyan.clone(), 1.0), solid(6, yellow.clone(), 1.0)],
            [0, 255, 0, 255],
        ),
        (
            vec![solid(4, cyan.clone(), 1.0), solid(5, magenta.clone(), 1.0)],
            [0, 0, 255, 255],
        ),
        (
            vec![
                solid(4, cyan.clone(), 1.0),
                solid(5, magenta.clone(), 1.0),
                solid(6, yellow.clone(), 1.0),
            ],
            [0, 0, 0, 255],
        ),
        (vec![solid(7, black, 1.0)], [0, 0, 0, 255]),
    ] {
        let surface = rasterize(
            &modeled(HalftoneChannelModel::Cmyk, layers),
            RasterBackground::Transparent,
        )
        .unwrap();
        assert_eq!(center(&surface), expected);
    }
    let half_cyan = rasterize(
        &modeled(HalftoneChannelModel::Cmyk, vec![solid(4, cyan, 0.5)]),
        RasterBackground::Transparent,
    )
    .unwrap();
    let transparent = center(&half_cyan);
    assert_eq!(transparent[3], 128);
    let over_white = rasterize(
        &modeled(
            HalftoneChannelModel::Cmyk,
            vec![solid(4, color(0.0, 1.0, 1.0, 1.0), 0.5)],
        ),
        RasterBackground::OpaqueWhite,
    )
    .unwrap();
    assert_eq!(
        center(&over_white),
        [188, 255, 255, 255],
        "T is recovered over white"
    );
    let over_black = rasterize(
        &modeled(
            HalftoneChannelModel::Cmyk,
            vec![solid(4, color(0.0, 1.0, 1.0, 1.0), 0.5)],
        ),
        RasterBackground::OpaqueBlack,
    )
    .unwrap();
    assert_eq!(center(&over_black), [0, 188, 188, 255]);
    let neutral = rasterize(
        &modeled(
            HalftoneChannelModel::Cmyk,
            vec![solid(7, color(0.0, 0.0, 0.0, 1.0), 0.5)],
        ),
        RasterBackground::OpaqueWhite,
    )
    .unwrap();
    assert_eq!(center(&neutral), [188, 188, 188, 255]);
}

#[test]
fn modeled_layers_keep_per_mark_coverage_opacity_visibility_and_straight_output() {
    let half = solid(1, color(1.0, 0.0, 0.0, 0.5), 0.5);
    let surface = rasterize(
        &modeled(HalftoneChannelModel::Rgb, vec![half]),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert_eq!(center(&surface), [255, 0, 0, 64]);
    let invisible = RenderLayer::new(
        ChannelId(1),
        false,
        color(1.0, 0.0, 0.0, 1.0),
        1.0,
        full_mark(),
    )
    .unwrap();
    let transparent = rasterize(
        &modeled(HalftoneChannelModel::Rgb, vec![invisible]),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert!(
        transparent
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 0])
    );
    let fractional = RenderLayer::new(
        ChannelId(1),
        true,
        color(1.0, 0.0, 0.0, 1.0),
        1.0,
        GeometryOutput::CircularMarks(vec![mark(0.5, 0.5, 0.5, SiteScope::Canvas)]),
    )
    .unwrap();
    let fractional = rasterize(
        &modeled(HalftoneChannelModel::Rgb, vec![fractional]),
        RasterBackground::Transparent,
    )
    .unwrap();
    let edge = &fractional.pixels()[..4];
    assert!(edge[3] > 0 && edge[3] < 255);
    assert_eq!(edge[0], 255, "unassociation remains straight sRGBA");
}

#[test]
fn source_color_alpha_uses_stable_mark_source_over() {
    let source_marks = vec![
        SourceColorCircle {
            mark: mark(3.5, 3.5, 1.0, SiteScope::Canvas),
            paint: color(1.0, 0.0, 0.0, 1.0),
        },
        SourceColorCircle {
            mark: mark(3.5, 3.5, 1.0, SiteScope::Canvas),
            paint: color(0.0, 0.0, 1.0, 1.0),
        },
    ];
    let layer = RenderLayer::new_source_color(ChannelId(8), true, 0.5, source_marks).unwrap();
    let surface = rasterize(
        &modeled(HalftoneChannelModel::SourceColorAlpha, vec![layer]),
        RasterBackground::Transparent,
    )
    .unwrap();
    assert_eq!(center(&surface), [156, 0, 213, 191]);
    assert!(
        RenderScene::new_modeled(
            CanvasSpec {
                width: 6.0,
                height: 6.0
            },
            "source".into(),
            "source".into(),
            HalftoneChannelModel::SourceColorAlpha,
            vec![
                solid(8, color(1.0, 0.0, 0.0, 1.0), 1.0),
                solid(9, color(0.0, 0.0, 1.0, 1.0), 1.0)
            ],
        )
        .is_err()
    );
}

#[test]
fn modeled_scene_validates_fixed_paint_kinds_and_opaque_source_paint() {
    let source_marks = vec![SourceColorCircle {
        mark: mark(3.5, 3.5, 1.0, SiteScope::Canvas),
        paint: color(0.2, 0.4, 0.6, 1.0),
    }];
    let sampled = RenderLayer::new_source_color(ChannelId(8), true, 1.0, source_marks).unwrap();
    let canvas = CanvasSpec {
        width: 6.0,
        height: 6.0,
    };
    for model in [HalftoneChannelModel::Rgb, HalftoneChannelModel::Cmyk] {
        assert_eq!(
            RenderScene::new_modeled(
                canvas.clone(),
                "sampled".into(),
                "sampled".into(),
                model,
                vec![sampled.clone()],
            )
            .unwrap_err()
            .path(),
            "scene.layers"
        );
    }
    assert_eq!(
        RenderScene::new(
            canvas.clone(),
            "legacy".into(),
            "legacy".into(),
            vec![sampled.clone()],
        )
        .unwrap_err()
        .path(),
        "scene.layers"
    );
    assert_eq!(
        RenderScene::new_modeled(
            canvas,
            "solid".into(),
            "solid".into(),
            HalftoneChannelModel::SourceColorAlpha,
            vec![solid(8, color(0.2, 0.4, 0.6, 1.0), 1.0)],
        )
        .unwrap_err()
        .path(),
        "scene.layers"
    );
    assert_eq!(
        RenderLayer::new_source_color(
            ChannelId(8),
            true,
            1.0,
            vec![SourceColorCircle {
                mark: mark(3.5, 3.5, 1.0, SiteScope::Canvas),
                paint: color(0.2, 0.4, 0.6, 0.5),
            }],
        )
        .unwrap_err()
        .path(),
        "scene.layer.source_color"
    );
}

#[test]
fn modeled_single_layer_matches_accepted_stage5_transparent_pixels() {
    let accepted = RenderScene::new(
        CanvasSpec {
            width: 6.0,
            height: 6.0,
        },
        "family".into(),
        "realization".into(),
        vec![solid(1, color(0.2, 0.6, 0.9, 0.7), 0.8)],
    )
    .unwrap();
    let modeled = modeled(
        HalftoneChannelModel::Rgb,
        vec![solid(1, color(0.2, 0.6, 0.9, 0.7), 0.8)],
    );
    assert_eq!(
        rasterize(&accepted, RasterBackground::Transparent)
            .unwrap()
            .pixels(),
        rasterize(&modeled, RasterBackground::Transparent)
            .unwrap()
            .pixels(),
    );
}

#[test]
fn modeled_svg_is_one_editable_canvas_with_ordered_vector_channel_groups() {
    let rgb = write_svg(&modeled(
        HalftoneChannelModel::Rgb,
        vec![
            solid(1, color(1.0, 0.0, 0.0, 1.0), 1.0),
            solid(2, color(0.0, 1.0, 0.0, 1.0), 1.0),
        ],
    ));
    assert!(rgb.contains("<title>Toniator RGB halftone</title>"));
    assert_eq!(rgb.matches("clip-path=").count(), 1);
    assert!(
        rgb.contains(
            "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">"
        )
    );
    let defs_end = rgb.find("</defs>").unwrap();
    let canvas = rgb.find("<g id=\"canvas\"").unwrap();
    let channel_one = rgb
        .find("<g id=\"channel-1\" style=\"mix-blend-mode:screen\">")
        .unwrap();
    let channel_two = rgb
        .find("<g id=\"channel-2\" style=\"mix-blend-mode:screen\">")
        .unwrap();
    assert!(defs_end < canvas && canvas < channel_one && channel_one < channel_two);
    assert_eq!(rgb.matches("<g id=\"channel-").count(), 2);
    assert_eq!(rgb.matches("<circle ").count(), 2);
    assert!(rgb.contains("id=\"channel-1-mark-0\""));
    assert!(rgb.contains("id=\"channel-2-mark-0\""));
    assert!(!rgb.contains("<feImage") && !rgb.contains("<filter") && !rgb.contains("data:image"));
    assert!(!rgb.contains("fill-opacity=\"0\" filter="));
    let cmyk = write_svg(&modeled(
        HalftoneChannelModel::Cmyk,
        vec![
            solid(4, color(0.0, 1.0, 1.0, 1.0), 1.0),
            solid(5, color(1.0, 0.0, 1.0, 1.0), 1.0),
        ],
    ));
    assert!(cmyk.contains("<title>Toniator CMYK halftone</title>"));
    assert!(cmyk.contains("<g id=\"channel-4\" style=\"mix-blend-mode:multiply\">"));
    assert!(cmyk.contains("<g id=\"channel-5\" style=\"mix-blend-mode:multiply\">"));
    assert_eq!(cmyk.matches("<g id=\"channel-").count(), 2);
    let source_color = write_svg(&modeled(
        HalftoneChannelModel::SourceColorAlpha,
        vec![
            RenderLayer::new_source_color(
                ChannelId(8),
                true,
                1.0,
                vec![SourceColorCircle {
                    mark: mark(3.5, 3.5, 1.0, SiteScope::Canvas),
                    paint: color(0.2, 0.4, 0.6, 1.0),
                }],
            )
            .unwrap(),
        ],
    ));
    assert!(source_color.contains("<title>Toniator source-colored halftone</title>"));
    assert_eq!(source_color.matches("clip-path=").count(), 1);
    assert!(
        source_color.contains(
            "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">"
        )
    );
    assert!(source_color.contains("<g id=\"channel-8\">"));
    assert!(source_color.contains("id=\"channel-8-mark-0\""));
    assert_eq!(source_color.matches("<g id=\"channel-").count(), 1);
    assert!(!source_color.contains("<filter"));
    assert!(!source_color.contains("mix-blend-mode"));
}

#[test]
fn invisible_modeled_channels_keep_editable_geometry_but_do_not_render() {
    let output = validation_output();
    let rgb = modeled_with_canvas(
        HalftoneChannelModel::Rgb,
        200.0,
        160.0,
        vec![
            overlap_layer(1, color(1.0, 0.0, 0.0, 1.0), 1.0, 80.0),
            overlap_layer_with_visibility(2, false, color(0.0, 1.0, 0.0, 1.0), 1.0, 120.0),
        ],
    );
    assert_invisible_channel_is_editable_but_hidden(
        &rgb,
        &output.join("synthetic-rgb-hidden-channel.svg"),
        &output.join("synthetic-rgb-hidden-channel-inkscape.png"),
        "<g id=\"channel-2\" style=\"mix-blend-mode:screen;display:none\">",
        "channel-2-mark-0",
        [255, 0, 0, 255],
    );

    let cmyk = modeled_with_canvas(
        HalftoneChannelModel::Cmyk,
        200.0,
        160.0,
        vec![
            overlap_layer(4, color(0.0, 1.0, 1.0, 1.0), 1.0, 80.0),
            overlap_layer_with_visibility(5, false, color(1.0, 0.0, 1.0, 1.0), 1.0, 120.0),
        ],
    );
    assert_invisible_channel_is_editable_but_hidden(
        &cmyk,
        &output.join("synthetic-cmyk-hidden-channel.svg"),
        &output.join("synthetic-cmyk-hidden-channel-inkscape.png"),
        "<g id=\"channel-5\" style=\"mix-blend-mode:multiply;display:none\">",
        "channel-5-mark-0",
        [0, 255, 255, 255],
    );

    let source = modeled_with_canvas(
        HalftoneChannelModel::SourceColorAlpha,
        200.0,
        160.0,
        vec![
            RenderLayer::new_source_color(
                ChannelId(8),
                false,
                1.0,
                vec![SourceColorCircle {
                    mark: mark(80.0, 80.0, 66.0, SiteScope::Canvas),
                    paint: color(0.2, 0.4, 0.6, 1.0),
                }],
            )
            .unwrap(),
        ],
    );
    assert_invisible_channel_is_editable_but_hidden(
        &source,
        &output.join("synthetic-source-hidden-channel.svg"),
        &output.join("synthetic-source-hidden-channel-inkscape.png"),
        "<g id=\"channel-8\" style=\"display:none\">",
        "channel-8-mark-0",
        [0, 0, 0, 0],
    );
}

#[test]
fn editable_rgb_svg_has_opaque_semantic_correspondence_in_both_renderers_and_characterized_fractional_difference()
 {
    let output = validation_output();
    let opaque = modeled_with_canvas(
        HalftoneChannelModel::Rgb,
        200.0,
        160.0,
        vec![
            overlap_layer(1, color(1.0, 0.0, 0.0, 1.0), 1.0, 80.0),
            overlap_layer(2, color(0.0, 1.0, 0.0, 1.0), 1.0, 120.0),
        ],
    );
    let opaque_svg = output.join("synthetic-rgb-screen-overlap.svg");
    let opaque_native = output.join("synthetic-rgb-screen-overlap-native.png");
    let opaque_inkscape = output.join("synthetic-rgb-screen-overlap-inkscape.png");
    assert_opaque_editable_secondary(
        &opaque,
        &opaque_svg,
        &opaque_native,
        &opaque_inkscape,
        [255, 255, 0, 255],
        &[
            "channel-1",
            "channel-1-mark-0",
            "channel-2",
            "channel-2-mark-0",
        ],
    );

    let fractional = modeled_with_canvas(
        HalftoneChannelModel::Rgb,
        200.0,
        160.0,
        vec![
            overlap_layer(1, color(1.0, 0.0, 0.0, 1.0), 0.5, 80.0),
            overlap_layer(2, color(0.0, 1.0, 0.0, 1.0), 0.5, 120.0),
        ],
    );
    assert_fractional_svg_difference(
        &fractional,
        output.join("synthetic-rgb-screen-fractional.svg"),
        output.join("synthetic-rgb-screen-fractional-native.png"),
        output.join("synthetic-rgb-screen-fractional-inkscape.png"),
        [188, 188, 0, 255],
        [170, 170, 0, 192],
    );
}

#[test]
fn editable_cmyk_svg_has_opaque_semantic_correspondence_in_both_renderers_and_characterized_fractional_difference()
 {
    let output = validation_output();
    let opaque = modeled_with_canvas(
        HalftoneChannelModel::Cmyk,
        200.0,
        160.0,
        vec![
            overlap_layer(4, color(0.0, 1.0, 1.0, 1.0), 1.0, 80.0),
            overlap_layer(5, color(1.0, 0.0, 1.0, 1.0), 1.0, 120.0),
        ],
    );
    let opaque_svg = output.join("synthetic-cmyk-multiply-overlap.svg");
    let opaque_native = output.join("synthetic-cmyk-multiply-overlap-native.png");
    let opaque_inkscape = output.join("synthetic-cmyk-multiply-overlap-inkscape.png");
    assert_opaque_editable_secondary(
        &opaque,
        &opaque_svg,
        &opaque_native,
        &opaque_inkscape,
        [0, 0, 255, 255],
        &[
            "channel-4",
            "channel-4-mark-0",
            "channel-5",
            "channel-5-mark-0",
        ],
    );

    let fractional = modeled_with_canvas(
        HalftoneChannelModel::Cmyk,
        200.0,
        160.0,
        vec![
            overlap_layer(4, color(0.0, 1.0, 1.0, 1.0), 0.5, 80.0),
            overlap_layer(5, color(1.0, 0.0, 1.0, 1.0), 0.5, 120.0),
        ],
    );
    assert_fractional_svg_difference(
        &fractional,
        output.join("synthetic-cmyk-multiply-fractional.svg"),
        output.join("synthetic-cmyk-multiply-fractional-native.png"),
        output.join("synthetic-cmyk-multiply-fractional-inkscape.png"),
        [156, 156, 255, 191],
        [85, 85, 255, 192],
    );
}

#[test]
fn immutable_project_assets_decode_and_vector_rasterizes_without_claiming_model_evaluation() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let raster = fs::read(assets.join("raster-sample.png")).unwrap();
    assert!(raster.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = image::load_from_memory(&raster).unwrap().to_rgba8();
    assert_eq!(decoded.dimensions(), (1024, 1024));
    assert!(decoded.pixels().any(|pixel| pixel[3] == 0));
    assert!(decoded.pixels().any(|pixel| pixel[3] == 255));

    let vector_path = assets.join("vector-sample.svg");
    let vector = fs::read_to_string(&vector_path).unwrap();
    assert!(vector.contains("<text"));
    let rasterized = validation_output().join("asset-vector-sample-inkscape.png");
    export_inkscape(&vector_path, &rasterized);
    let rasterized = image::open(rasterized).unwrap().to_rgba8();
    assert_eq!(rasterized.dimensions(), (900, 620));
    assert!(rasterized.pixels().any(|pixel| pixel[3] != 0));
}

fn assert_opaque_editable_secondary(
    scene: &RenderScene,
    svg_path: &std::path::Path,
    native_path: &std::path::Path,
    inkscape_path: &std::path::Path,
    expected_center: [u8; 4],
    visible_ids: &[&str],
) {
    fs::write(svg_path, write_svg(scene)).unwrap();
    let native = rasterize(scene, RasterBackground::Transparent).unwrap();
    fs::write(native_path, encode_png(&native).unwrap()).unwrap();
    export_inkscape(svg_path, inkscape_path);
    let inkscape = image::open(inkscape_path).unwrap().to_rgba8();
    let native_center = surface_pixel(&native, 100, 80);
    assert_eq!(native_center, expected_center);
    assert_eq!(inkscape.get_pixel(100, 80).0, expected_center);
    for id in visible_ids {
        assert!(
            inkscape_query_width(svg_path, id) > 0.0,
            "visible SVG ID {id}"
        );
    }
}

fn assert_invisible_channel_is_editable_but_hidden(
    scene: &RenderScene,
    svg_path: &std::path::Path,
    inkscape_path: &std::path::Path,
    hidden_group: &str,
    hidden_mark_id: &str,
    expected_center: [u8; 4],
) {
    let svg = write_svg(scene);
    assert_eq!(svg.matches("clip-path=").count(), 1);
    assert!(svg.contains(hidden_group));
    assert!(svg.contains(&format!("id=\"{hidden_mark_id}\"")));
    fs::write(svg_path, svg).unwrap();
    let native = rasterize(scene, RasterBackground::Transparent).unwrap();
    assert_eq!(surface_pixel(&native, 100, 80), expected_center);
    export_inkscape(svg_path, inkscape_path);
    let exported_center = image::open(inkscape_path)
        .unwrap()
        .to_rgba8()
        .get_pixel(100, 80)
        .0;
    if expected_center[3] == 0 {
        assert_eq!(
            exported_center[3], 0,
            "hidden SVG group has no visible coverage"
        );
    } else {
        assert_eq!(exported_center, expected_center);
    }
}

fn assert_fractional_svg_difference(
    scene: &RenderScene,
    svg_path: std::path::PathBuf,
    native_path: std::path::PathBuf,
    inkscape_path: std::path::PathBuf,
    expected_native: [u8; 4],
    expected_inkscape: [u8; 4],
) {
    fs::write(&svg_path, write_svg(scene)).unwrap();
    let native = rasterize(scene, RasterBackground::Transparent).unwrap();
    fs::write(&native_path, encode_png(&native).unwrap()).unwrap();
    export_inkscape(&svg_path, &inkscape_path);
    let native_center = surface_pixel(&native, 100, 80);
    let inkscape_center = image::open(&inkscape_path)
        .unwrap()
        .to_rgba8()
        .get_pixel(100, 80)
        .0;
    assert_eq!(native_center, expected_native);
    assert_eq!(inkscape_center, expected_inkscape);
    assert_ne!(
        native_center, inkscape_center,
        "fractional correspondence is intentionally not pixel parity"
    );
}

fn validation_output() -> std::path::PathBuf {
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-9c");
    fs::create_dir_all(&output).unwrap();
    output
}

fn export_inkscape(svg_path: &std::path::Path, png_path: &std::path::Path) {
    let inkscape_guard = inkscape_lock().lock().unwrap();
    let status = Command::new("inkscape")
        .arg(svg_path)
        .arg("--export-type=png")
        .arg(format!("--export-filename={}", png_path.display()))
        .status()
        .unwrap();
    drop(inkscape_guard);
    assert!(
        status.success(),
        "Inkscape export for {}",
        svg_path.display()
    );
}

fn inkscape_query_width(svg_path: &std::path::Path, id: &str) -> f64 {
    let inkscape_guard = inkscape_lock().lock().unwrap();
    let output = Command::new("inkscape")
        .arg(svg_path)
        .arg(format!("--query-id={id}"))
        .arg("--query-width")
        .output()
        .unwrap();
    drop(inkscape_guard);
    assert!(output.status.success(), "Inkscape query for SVG ID {id}");
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

fn surface_pixel(surface: &RasterSurface, x: usize, y: usize) -> [u8; 4] {
    let offset = 4 * (y * surface.width() as usize + x);
    surface.pixels()[offset..offset + 4].try_into().unwrap()
}

fn inkscape_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
