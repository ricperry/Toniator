use toniator_domain::{CanvasSpec, ChannelId, ColorValue, DensityMetric2D};
use toniator_engine::{
    GeometryOutput, GridInspectRequest, MarkResponse, MarksInspectRequest, RasterBackground,
    RenderSceneRequest, ScenePresentation, SourceComponent, SourceFormatHint, SourcePlacement,
    inspect_circular_marks, rasterize, render_scene, write_svg,
};

fn request<'a>(bytes: &'a [u8], source_format: SourceFormatHint) -> RenderSceneRequest<'a> {
    RenderSceneRequest {
        marks: MarksInspectRequest {
            grid: GridInspectRequest {
                canvas: CanvasSpec {
                    width: 900.0,
                    height: 600.0,
                },
                density: DensityMetric2D {
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
                },
                rotation_degrees: 17.0,
                translation_x: 3.25,
                translation_y: -4.5,
                guard_steps: 2,
                support_radius: 4.5,
            },
            source_bytes: bytes,
            source_format,
            source_component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
            response: MarkResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
        },
        presentation: ScenePresentation {
            channel_id: ChannelId(1),
            visible: true,
            color: ColorValue {
                red: 0.0,
                green: toniator_engine::srgb_to_linear(183.0 / 255.0),
                blue: 1.0,
                alpha: 1.0,
            },
            opacity: 0.72,
        },
    }
}

#[test]
fn scene_copies_every_stage_4_mark_without_rebuilding_the_realization() {
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let request = request(&bytes, SourceFormatHint::Png);
    let realization = inspect_circular_marks(&request.marks).unwrap();
    let scene = render_scene(&request).unwrap();
    let GeometryOutput::CircularMarks(scene_marks) = scene.layers()[0].geometry();
    assert_eq!(scene_marks, &realization.marks);
    assert_eq!(
        scene.identity().family_fingerprint(),
        realization.family_fingerprint
    );
    assert_eq!(
        scene.identity().realization_fingerprint(),
        realization.realization_fingerprint
    );
    assert_eq!(
        scene_marks
            .iter()
            .map(|mark| (
                &mark.source_site_id,
                mark.center,
                mark.radius,
                mark.scope,
                &mark.provenance
            ))
            .collect::<Vec<_>>(),
        realization
            .marks
            .iter()
            .map(|mark| (
                &mark.source_site_id,
                mark.center,
                mark.radius,
                mark.scope,
                &mark.provenance
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn both_baselines_build_one_scene_consumed_by_raster_and_svg_without_mark_loss() {
    for (path, format) in [
        ("../../assets/raster-sample.png", SourceFormatHint::Png),
        ("../../assets/vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let bytes = std::fs::read(path).unwrap();
        let scene = render_scene(&request(&bytes, format)).unwrap();
        let GeometryOutput::CircularMarks(marks) = scene.layers()[0].geometry();
        assert_eq!(marks.len(), 10_304);
        assert_eq!(
            marks
                .iter()
                .filter(|mark| matches!(mark.scope, toniator_engine::SiteScope::Guard))
                .count(),
            4_902
        );
        assert!(
            marks
                .iter()
                .all(|mark| mark.center.is_finite() && mark.radius.is_finite())
        );
        let raster = rasterize(&scene, RasterBackground::Transparent).unwrap();
        let svg = write_svg(&scene);
        assert_eq!((raster.width(), raster.height()), (900, 600));
        assert_eq!(svg.matches("<circle ").count(), marks.len());
        assert!(svg.contains(scene.identity().family_fingerprint()));
        assert!(svg.contains(scene.identity().realization_fingerprint()));
        assert!(svg.contains(scene.identity().scene_fingerprint()));
    }
}

#[test]
fn component_response_changes_realization_and_scene_not_stage_3_geometry() {
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let luminance_request = request(&bytes, SourceFormatHint::Png);
    let mut alpha_request = luminance_request.clone();
    alpha_request.marks.source_component = SourceComponent::Alpha;

    let luminance_realization = inspect_circular_marks(&luminance_request.marks).unwrap();
    let alpha_realization = inspect_circular_marks(&alpha_request.marks).unwrap();
    assert_eq!(
        luminance_realization.family_fingerprint,
        alpha_realization.family_fingerprint
    );
    assert_ne!(
        luminance_realization.realization_fingerprint,
        alpha_realization.realization_fingerprint
    );
    assert_eq!(luminance_realization.marks.len(), 10_304);
    assert_eq!(alpha_realization.marks.len(), 10_304);
    for (luminance, alpha) in luminance_realization
        .marks
        .iter()
        .zip(&alpha_realization.marks)
    {
        assert_eq!(luminance.source_site_id, alpha.source_site_id);
        assert_eq!(luminance.center, alpha.center);
        assert_eq!(luminance.scope, alpha.scope);
        assert_eq!(luminance.provenance, alpha.provenance);
    }
    assert!(
        luminance_realization
            .marks
            .iter()
            .zip(&alpha_realization.marks)
            .any(|(luminance, alpha)| luminance.radius != alpha.radius)
    );

    let luminance_scene = render_scene(&luminance_request).unwrap();
    let alpha_scene = render_scene(&alpha_request).unwrap();
    assert_eq!(
        luminance_scene.identity().family_fingerprint(),
        alpha_scene.identity().family_fingerprint()
    );
    assert_ne!(
        luminance_scene.identity().realization_fingerprint(),
        alpha_scene.identity().realization_fingerprint()
    );
    assert_ne!(
        luminance_scene.identity().scene_fingerprint(),
        alpha_scene.identity().scene_fingerprint()
    );
}
