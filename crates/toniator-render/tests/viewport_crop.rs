//! Focused final-consumer crop, canvas clipping and antialiasing witnesses.

use toniator_domain::{CanvasSpec, ChannelId, ColorValue, PathStrokeStyle, PatternMechanismId};
use toniator_geometry::{
    CanonicalStroke, CurvePath, PathClosure, PathLocation, Point2, StrokeProfileSample,
    StructuralPathInstanceId, StructuralPathSourceId, VariableWidthOutlineLimits,
    VariableWidthPathSample, build_variable_width_outline_cancellable,
};
use toniator_render::{
    GeometryOutput, PreviewRasterTarget, RasterBackground, RenderLayer, RenderScene, rasterize,
    rasterize_preview,
};

/// Builds an unclipped diagonal stroke crossing both canvas boundaries for consumer-only tests.
fn crossing_scene() -> RenderScene {
    let path = CurvePath::polyline(
        vec![Point2::new(-3.0, -1.0), Point2::new(13.0, 11.0)],
        PathClosure::Open,
    )
    .unwrap();
    let locations = [
        PathLocation::new(0, 0.0).unwrap(),
        PathLocation::new(0, 1.0).unwrap(),
    ];
    let samples = locations.map(|location| VariableWidthPathSample {
        location,
        width: 1.6,
    });
    let outline = build_variable_width_outline_cancellable(
        &path,
        &samples,
        PathStrokeStyle::default(),
        0.0,
        0.01,
        VariableWidthOutlineLimits::new(4096).unwrap(),
        &|| false,
    )
    .unwrap();
    let profile = locations
        .map(|location| StrokeProfileSample {
            location,
            center: path.point_at(location).unwrap(),
            normalized_thickness: 1.0,
            width: 1.6,
        })
        .to_vec();
    let stroke = CanonicalStroke::new(
        StructuralPathInstanceId {
            source: StructuralPathSourceId::ParametricCurve(PatternMechanismId(1)),
            repetition_index: 0,
            component_ordinal: 0,
        },
        None,
        path,
        1.6,
        PathStrokeStyle::default(),
        profile,
        outline,
    )
    .unwrap();
    RenderScene::new(
        CanvasSpec {
            width: 10.0,
            height: 10.0,
        },
        "crop-family".into(),
        "crop-realization".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                ColorValue {
                    red: 0.3,
                    green: 0.6,
                    blue: 0.9,
                    alpha: 0.7,
                },
                0.8,
                GeometryOutput::CanonicalStrokes(vec![stroke]),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

/// Proves cropped stroke pixels equal the same high-resolution full raster, including AA and alpha.
/// It also keeps native pixels and canonical identities unchanged across the derived consumer.
#[test]
fn viewport_crop_matches_complete_raster_slice() {
    let scene = crossing_scene();
    let native = rasterize(&scene, RasterBackground::Transparent).unwrap();
    let identity = scene.identity().clone();
    let full = rasterize_preview(&scene, PreviewRasterTarget::new(80, 80).unwrap()).unwrap();
    for (x, y, width, height) in [(0, 0, 24, 32), (19, 25, 43, 29), (60, 60, 20, 20)] {
        let crop = rasterize_preview(
            &scene,
            PreviewRasterTarget::for_viewport(
                width,
                height,
                f64::from(x) / 8.0,
                f64::from(y) / 8.0,
                8.0,
            )
            .unwrap(),
        )
        .unwrap();
        for row in 0..height as usize {
            let full_start = ((y as usize + row) * 80 + x as usize) * 4;
            let crop_start = row * width as usize * 4;
            assert_eq!(
                &crop.pixels()[crop_start..crop_start + width as usize * 4],
                &full.pixels()[full_start..full_start + width as usize * 4]
            );
        }
    }
    assert_eq!(identity, *scene.identity());
    assert_eq!(
        native,
        rasterize(&scene, RasterBackground::Transparent).unwrap()
    );
    assert!(
        full.pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[3] < 143)
    );
}

/// Proves fractional transformed canvas edges clip stroke samples before coverage accumulation.
#[test]
fn viewport_crop_letterbox_and_overflow_are_bounded() {
    let scene = crossing_scene();
    let crop = rasterize_preview(
        &scene,
        PreviewRasterTarget::for_viewport(90, 90, -0.3, -0.3, 8.0).unwrap(),
    )
    .unwrap();
    assert!(
        crop.pixels()
            .chunks_exact(4)
            .take(90 * 2)
            .all(|pixel| pixel[3] == 0)
    );
    for y in 0..90_usize {
        for x in 0..90_usize {
            if x < 2 || y < 2 || x >= 83 || y >= 83 {
                assert_eq!(crop.pixels()[(y * 90 + x) * 4 + 3], 0);
            }
        }
    }
    let overflow = PreviewRasterTarget::for_viewport(10, 10, 0.0, 0.0, f64::MAX).unwrap();
    assert_eq!(
        rasterize_preview(&scene, overflow).unwrap_err().path(),
        "preview.target"
    );
    assert!(PreviewRasterTarget::for_viewport(10, 10, f64::NAN, 0.0, 1.0).is_err());
}
