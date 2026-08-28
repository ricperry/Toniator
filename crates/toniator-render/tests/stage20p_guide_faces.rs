//! Stage 20P renderer integration for canonical guide-arrangement faces.

use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, GuideDimensionId, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    Bounds, CubicBezierSegment, CurvePath, CurveSegment, GuideFaceLimits, GuideFaceRequest,
    PathClosure, Point2, StructuralPathInstance, StructuralPathInstanceId, StructuralPathSet,
    StructuralPathSourceId, build_guide_faces_cancellable,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RenderLayer, RenderScene, encode_png, rasterize, write_svg,
};

/// Builds a final-clipped two-guide face scene without asking the renderer to construct topology.
fn guide_face_scene(kind: &str, width: f64, height: f64) -> RenderScene {
    let line = |a, b| CurvePath::line(a, b).expect("finite guide line");
    let cubic = |start, first, second, end| {
        CurvePath::new(
            vec![CurveSegment::CubicBezier(
                CubicBezierSegment::new(start, first, second, end).expect("finite cubic"),
            )],
            PathClosure::Open,
        )
        .expect("open cubic")
    };
    let paths_for = |entries: Vec<(u64, CurvePath)>| {
        StructuralPathSet::new(
            format!("stage20p-{kind}-family"),
            PatternMechanismId(20),
            entries
                .into_iter()
                .enumerate()
                .map(|(ordinal, (dimension, path))| StructuralPathInstance {
                    id: StructuralPathInstanceId {
                        source: StructuralPathSourceId::GuideDimension(GuideDimensionId(dimension)),
                        repetition_index: ordinal as i64,
                        component_ordinal: 0,
                    },
                    source_structure_id: None,
                    path,
                })
                .collect(),
        )
        .expect("guide paths")
    };
    let long = |a, b| line(a, b);
    let (dimensions, paths) = match kind {
        "two-guide" => (
            vec![1, 2],
            paths_for(vec![
                (
                    1,
                    long(
                        Point2::new(width * 0.2, -height),
                        Point2::new(width * 0.2, height * 2.0),
                    ),
                ),
                (
                    1,
                    long(
                        Point2::new(width * 0.8, -height),
                        Point2::new(width * 0.8, height * 2.0),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(-width, height * 0.2),
                        Point2::new(width * 2.0, height * 0.2),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(-width, height * 0.8),
                        Point2::new(width * 2.0, height * 0.8),
                    ),
                ),
            ]),
        ),
        "authored-cubic" => (
            vec![1, 2],
            paths_for(vec![
                (
                    1,
                    cubic(
                        Point2::new(-width, height * 0.24),
                        Point2::new(width * 0.15, height * 0.05),
                        Point2::new(width * 0.85, height * 0.05),
                        Point2::new(width * 2.0, height * 0.24),
                    ),
                ),
                (
                    1,
                    cubic(
                        Point2::new(-width, height * 0.76),
                        Point2::new(width * 0.15, height * 0.95),
                        Point2::new(width * 0.85, height * 0.95),
                        Point2::new(width * 2.0, height * 0.76),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(width * 0.24, -height),
                        Point2::new(width * 0.24, height * 2.0),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(width * 0.76, -height),
                        Point2::new(width * 0.76, height * 2.0),
                    ),
                ),
            ]),
        ),
        "curved-off-canvas" => (
            vec![1, 2],
            paths_for(vec![
                (
                    1,
                    cubic(
                        Point2::new(-width * 0.5, height * 0.28),
                        Point2::new(width * 0.15, height * 0.52),
                        Point2::new(width * 0.85, height * 0.52),
                        Point2::new(width * 1.5, height * 0.28),
                    ),
                ),
                (
                    1,
                    cubic(
                        Point2::new(-width * 0.5, height * 0.72),
                        Point2::new(width * 0.15, height * 0.48),
                        Point2::new(width * 0.85, height * 0.48),
                        Point2::new(width * 1.5, height * 0.72),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(-width * 0.15, -height),
                        Point2::new(-width * 0.15, height * 2.0),
                    ),
                ),
                (
                    2,
                    long(
                        Point2::new(width * 1.15, -height),
                        Point2::new(width * 1.15, height * 2.0),
                    ),
                ),
            ]),
        ),
        _ => panic!("unknown Stage 20P guide witness"),
    };
    let guide_faces = build_guide_faces_cancellable(
        GuideFaceRequest {
            output_layer_id: PatternOutputLayerId(20),
            guide_mechanism_id: PatternMechanismId(20),
            dimensions: dimensions.into_iter().map(GuideDimensionId).collect(),
            paths,
            canvas: Bounds::new(Point2::new(0.0, 0.0), Point2::new(width, height)).expect("canvas"),
        },
        GuideFaceLimits::default(),
        || false,
    )
    .expect("guide faces");
    let regions = guide_faces.regions;
    let layer = RenderLayer::new_for_output(
        ChannelId(20),
        true,
        ColorValue {
            red: 0.25,
            green: 0.6,
            blue: 0.9,
            alpha: 0.8,
        },
        0.75,
        PatternOutputLayerId(20),
        GeometryOutput::CanonicalRegions(regions),
    )
    .expect("region layer");
    RenderScene::new(
        CanvasSpec { width, height },
        "stage20p-guide-faces".into(),
        "stage20p-guide-faces-realization".into(),
        vec![layer],
    )
    .expect("scene")
}

/// Proves canonical guide faces use the same fixed nonzero PNG and SVG fill path as other regions.
#[test]
fn guide_faces_render_to_png_and_svg() {
    let scene = guide_face_scene("two-guide", 64.0, 64.0);
    let raster = rasterize(&scene, RasterBackground::Transparent).expect("raster");
    assert!(!encode_png(&raster).expect("png").is_empty());
    assert!(write_svg(&scene).contains("fill-rule=\"nonzero\""));
}

/// Generates intrinsic native PNG, raw SVG, and SVG-rasterized witnesses for every Stage 20P face arrangement.
#[test]
#[ignore = "validation artifact generator"]
fn generate_intrinsic_guide_face_artifacts() {
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage20p");
    std::fs::create_dir_all(&output).expect("validation directory");
    for (kind, name, width, height) in [
        ("two-guide", "two-guide-raster-1024", 1024.0, 1024.0),
        (
            "authored-cubic",
            "authored-cubic-vector-900x620",
            900.0,
            620.0,
        ),
        (
            "curved-off-canvas",
            "curved-off-canvas-raster-1024",
            1024.0,
            1024.0,
        ),
    ] {
        let scene = guide_face_scene(kind, width, height);
        let region_layer = scene
            .layers()
            .first()
            .expect("guide-face scene has one layer")
            .outputs()
            .first()
            .expect("guide-face layer has one output");
        let GeometryOutput::CanonicalRegions(regions) = region_layer.geometry() else {
            panic!("guide-face evidence retains canonical regions");
        };
        let mut identity_record = format!("fingerprint={}\n", regions.fingerprint());
        for region in regions.regions() {
            identity_record.push_str(&format!("{:?}\n", region.id));
        }
        std::fs::write(output.join(format!("{name}-regions.txt")), identity_record)
            .expect("region identity record write");
        let raster = rasterize(&scene, RasterBackground::Transparent).expect("raster");
        std::fs::write(
            output.join(format!("{name}.png")),
            encode_png(&raster).expect("png"),
        )
        .expect("png write");
        let svg_path = output.join(format!("{name}.svg"));
        std::fs::write(&svg_path, write_svg(&scene)).expect("svg write");
        let status = std::process::Command::new("inkscape")
            .arg(&svg_path)
            .arg("--export-type=png")
            .arg(format!(
                "--export-filename={}",
                output.join(format!("{name}-svg-rasterized.png")).display()
            ))
            .status()
            .expect("Inkscape is available for Stage 20P SVG evidence");
        assert!(status.success(), "SVG rasterization succeeds");
    }
}
