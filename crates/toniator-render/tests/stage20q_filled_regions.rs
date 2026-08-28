//! Focused Stage 20Q filled-region renderer coverage.

use std::{fs, path::Path, process::Command};

use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel, PatternMechanismId,
    PatternOutputLayerId,
};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CanonicalRegionSourceId, CurvePath, FamilySiteId, PathClosure, Point2, build_canonical_regions,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RenderLayer, RenderOutputLayer, RenderScene, encode_png,
    rasterize, write_svg,
};

/// Builds one closed positive rectangle without entrusting closure or winding repair to rendering.
fn rectangle(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> CurvePath {
    CurvePath::polyline(
        vec![
            Point2::new(min_x, min_y),
            Point2::new(max_x, min_y),
            Point2::new(max_x, max_y),
            Point2::new(min_x, max_y),
        ],
        PathClosure::Closed,
    )
    .expect("focused rectangle is finite, connected, and closed")
}

/// Builds canonical region authority for ordered positive rectangles before the final consumer.
fn region_set(rectangles: Vec<CurvePath>) -> CanonicalRegionSet {
    build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 0,
            }]),
            components: rectangles,
        }],
    })
    .expect("focused regions are canonical positive geometry")
}

/// Builds the single SourceColorAlpha scene that owns sampled region paint and channel opacity.
fn sampled_region_scene(
    regions: CanonicalRegionSet,
    paints: Vec<ColorValue>,
    opacity: f64,
) -> RenderScene {
    let output = RenderOutputLayer::new(
        PatternOutputLayerId(20),
        GeometryOutput::CanonicalRegions(regions),
        Some(paints),
    );
    let layer = RenderLayer::new_outputs(
        ChannelId(20),
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        opacity,
        vec![output],
    )
    .expect("source-colored region output validates before scene assembly");
    RenderScene::new_modeled(
        CanvasSpec {
            width: 8.0,
            height: 4.0,
        },
        "stage20q-region-family".into(),
        "stage20q-region-realization".into(),
        HalftoneChannelModel::SourceColorAlpha,
        vec![layer],
    )
    .expect("SourceColorAlpha accepts region-aligned sampled paint")
}

/// Returns one exact straight-RGBA pixel from a native raster at its canonical row and column.
fn pixel(raster: &toniator_render::RasterSurface, x: usize, y: usize) -> [u8; 4] {
    let start = (y * raster.width() as usize + x) * 4;
    raster.pixels()[start..start + 4]
        .try_into()
        .expect("focused pixel has exactly four components")
}

/// Accepts an empty treated region set and its exactly empty sampled-paint table without output.
#[test]
fn empty_treated_regions_accept_zero_paints_and_render_transparent() {
    let scene = sampled_region_scene(CanonicalRegionSet::empty(), Vec::new(), 1.0);
    assert_eq!(scene.circular_mark_count(), 0);
    let raster = rasterize(&scene, RasterBackground::Transparent)
        .expect("empty treated output rasterizes without allocating primitives");
    assert!(
        raster
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 0]),
        "empty treated geometry cannot leave coverage or hidden RGB"
    );
    let svg = write_svg(&scene);
    assert!(svg.contains("clip-path=\"url(#canvas-clip)\""));
    assert!(!svg.contains("channel-20-region-"));
}

/// Rejects sampled paint whose cardinality does not exactly match treated canonical-region order.
#[test]
fn sampled_region_paint_cardinality_is_exact() {
    let error = RenderLayer::new_outputs(
        ChannelId(20),
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(20),
            GeometryOutput::CanonicalRegions(region_set(vec![rectangle(0.0, 0.0, 4.0, 4.0)])),
            Some(Vec::new()),
        )],
    )
    .expect_err("one canonical region requires one sampled paint");
    assert_eq!(error.path(), "scene.layer.source_color");
    assert_eq!(
        error.message(),
        "source-colored paint count must match canonical primitive count"
    );
}

/// Keeps strokes solid-only even when their empty geometry would otherwise match an empty table.
#[test]
fn strokes_reject_sampled_paint_even_for_empty_geometry() {
    let error = RenderLayer::new_outputs(
        ChannelId(20),
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(20),
            GeometryOutput::CanonicalStrokes(Vec::new()),
            Some(Vec::new()),
        )],
    )
    .expect_err("canonical strokes never accept sampled region paint");
    assert_eq!(error.path(), "scene.layer.source_color");
    assert_eq!(
        error.message(),
        "canonical strokes require solid channel paint"
    );
}

/// Renders sampled paint in canonical region order and applies channel opacity at final composition.
#[test]
fn sampled_region_paint_order_and_opacity_are_preserved_in_png_and_svg() {
    let scene = sampled_region_scene(
        region_set(vec![
            rectangle(0.0, 0.0, 3.0, 4.0),
            rectangle(5.0, 0.0, 8.0, 4.0),
        ]),
        vec![
            ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0,
            },
        ],
        0.5,
    );
    let raster = rasterize(&scene, RasterBackground::Transparent)
        .expect("sampled regions rasterize in canonical order");
    let left = pixel(&raster, 1, 1);
    let right = pixel(&raster, 6, 1);
    assert!(
        left[0] > 240 && left[2] < 2,
        "first region retains red paint"
    );
    assert!(
        right[2] > 240 && right[0] < 2,
        "second region retains blue paint"
    );
    assert_eq!(left[3], 128, "channel opacity halves first sampled alpha");
    assert_eq!(right[3], 128, "channel opacity halves second sampled alpha");

    let svg = write_svg(&scene);
    assert!(svg.contains("id=\"channel-20-region-0\""));
    assert!(svg.contains("id=\"channel-20-region-1\""));
    assert!(svg.contains("fill=\"#ff0000\" fill-opacity=\"0.5\""));
    assert!(svg.contains("fill=\"#0000ff\" fill-opacity=\"0.5\""));
    assert_eq!(svg.matches("fill-rule=\"nonzero\"").count(), 2);
}

/// Suppresses visible and hidden raster RGB when a sampled region paint has exact zero alpha.
#[test]
fn zero_alpha_sampled_region_has_no_raster_rgb_or_coverage() {
    let scene = sampled_region_scene(
        region_set(vec![rectangle(0.0, 0.0, 4.0, 4.0)]),
        vec![ColorValue {
            red: 1.0,
            green: 0.5,
            blue: 0.25,
            alpha: 0.0,
        }],
        1.0,
    );
    let raster = rasterize(&scene, RasterBackground::Transparent)
        .expect("transparent sampled paint has a valid final raster");
    assert_eq!(pixel(&raster, 1, 1), [0, 0, 0, 0]);
    assert!(write_svg(&scene).contains("fill-opacity=\"0\""));
}

/// Keeps off-canvas treated geometry intact until the renderer's one final canvas clipping boundary.
#[test]
fn sampled_regions_use_final_canvas_clip_without_renderer_topology_repair() {
    let scene = sampled_region_scene(
        region_set(vec![rectangle(-4.0, -4.0, 12.0, 8.0)]),
        vec![ColorValue {
            red: 0.25,
            green: 0.5,
            blue: 0.75,
            alpha: 1.0,
        }],
        1.0,
    );
    let raster = rasterize(&scene, RasterBackground::Transparent)
        .expect("unclipped canonical region rasterizes through final canvas consumer");
    assert!(raster.pixels().chunks_exact(4).all(|pixel| pixel[3] == 255));
    let svg = write_svg(&scene);
    assert!(svg.contains("clip-path=\"url(#canvas-clip)\""));
    assert!(svg.contains("M 12 8 L -4 8 L -4 -4 L 12 -4 L 12 8 Z"));
    assert!(svg.contains(" Z\" fill-rule=\"nonzero\" fill=\"#89bce1\""));
}

/// Proves malformed open rings are rejected by geometry authority rather than repaired by a renderer.
#[test]
fn malformed_region_geometry_is_rejected_upstream() {
    let open = CurvePath::polyline(
        vec![Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)],
        PathClosure::Open,
    )
    .expect("focused malformed path remains finite construction geometry");
    let error = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 0,
            }]),
            components: vec![open],
        }],
    })
    .expect_err("the producer-owned canonical boundary rejects open input");
    assert_eq!(error.path(), "region.geometry.closure");
}

/// Rasterizes the raw sampled-region SVG with Inkscape and bounds its native-PNG channel error.
///
/// This validation-only witness intentionally depends on the Fedora-native Inkscape and
/// ImageMagick command-line consumers. It writes only disposable Stage 20Q evidence below
/// `target/validation` and never changes canonical geometry, SVG, or PNG production behavior.
#[test]
#[ignore = "validation artifact generator requires Inkscape and ImageMagick"]
fn native_png_and_raw_svg_raster_have_bounded_channel_error() {
    let scene = sampled_region_scene(
        region_set(vec![
            rectangle(0.0, 0.0, 3.0, 4.0),
            rectangle(5.0, 0.0, 8.0, 4.0),
        ]),
        vec![
            ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0,
            },
        ],
        0.5,
    );
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage20q/render");
    fs::create_dir_all(&root).expect("Stage 20Q render validation directory exists");
    let native_png = root.join("sampled-regions-native.png");
    let raw_svg = root.join("sampled-regions-raw.svg");
    let svg_png = root.join("sampled-regions-svg-raster.png");
    fs::write(
        &native_png,
        encode_png(
            &rasterize(&scene, RasterBackground::Transparent)
                .expect("sampled Region native rasterizes"),
        )
        .expect("sampled Region native PNG encodes"),
    )
    .expect("native validation PNG writes below target");
    fs::write(&raw_svg, write_svg(&scene)).expect("raw validation SVG writes below target");
    let inkscape = Command::new("inkscape")
        .arg(&raw_svg)
        .arg("--export-type=png")
        .arg(format!("--export-filename={}", svg_png.display()))
        .arg("--export-width=8")
        .arg("--export-height=4")
        .status()
        .expect("Fedora Inkscape is available for validation")
        .success();
    assert!(inkscape, "Inkscape rasterizes the raw canonical SVG");
    let comparison = Command::new("compare")
        .args(["-metric", "RMSE"])
        .arg(&native_png)
        .arg(&svg_png)
        .arg("null:")
        .output()
        .expect("Fedora ImageMagick is available for validation");
    let metric = String::from_utf8_lossy(&comparison.stderr);
    let normalized = metric
        .split('(')
        .nth(1)
        .and_then(|tail| tail.split(')').next())
        .and_then(|value| value.trim().parse::<f64>().ok())
        .expect("ImageMagick reports normalized RMSE in parentheses");
    assert!(
        normalized <= 0.02,
        "native PNG and raw SVG raster differ by normalized RMSE {normalized}: {metric}"
    );
}
