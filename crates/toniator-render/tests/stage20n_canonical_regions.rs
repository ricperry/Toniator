//! Focused Stage 20N canonical-region consumer coverage.

use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    CanonicalRegionProposal, CanonicalRegionSourceGroup, CanonicalRegionSourceId, CurvePath,
    FamilySiteId, PathClosure, Point2, build_canonical_regions,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RenderLayer, RenderOutputLayer, RenderScene, encode_png,
    rasterize, write_svg,
};

/// Builds one direct deterministic canonical-region scene for intrinsic artifact generation.
fn direct_region_scene(width: f64, height: f64) -> RenderScene {
    let ring = CurvePath::polyline(
        vec![
            Point2::new(-width * 0.1, height * 0.1),
            Point2::new(width * 0.9, -height * 0.1),
            Point2::new(width * 1.1, height * 0.8),
            Point2::new(width * 0.25, height * 1.1),
        ],
        PathClosure::Closed,
    )
    .expect("finite direct region ring");
    let regions = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(20),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(20),
                ordinal: 0,
            }]),
            components: vec![ring],
        }],
    })
    .expect("direct canonical region");
    let layer = RenderLayer::new_for_output(
        ChannelId(20),
        true,
        ColorValue {
            red: 0.85,
            green: 0.2,
            blue: 0.5,
            alpha: 0.75,
        },
        0.6,
        PatternOutputLayerId(20),
        GeometryOutput::CanonicalRegions(regions),
    )
    .expect("direct region layer");
    RenderScene::new(
        CanvasSpec { width, height },
        "stage20n-direct-region-family".into(),
        "stage20n-direct-region-realization".into(),
        vec![layer],
    )
    .expect("direct region scene")
}

/// Proves raster and SVG consumers fill an already-closed canonical region without topology repair.
#[test]
fn canonical_regions_fill_with_final_canvas_clipping_only() {
    let ring = CurvePath::polyline(
        vec![
            Point2::new(-2.0, -2.0),
            Point2::new(6.0, -2.0),
            Point2::new(6.0, 6.0),
            Point2::new(-2.0, 6.0),
        ],
        PathClosure::Closed,
    )
    .expect("finite closed ring");
    let regions = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(3),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(2),
                ordinal: 0,
            }]),
            components: vec![ring],
        }],
    })
    .expect("canonical region");
    let layer = RenderLayer::new(
        ChannelId(1),
        true,
        ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        GeometryOutput::CanonicalRegions(regions),
    )
    .expect("valid layer");
    let scene = RenderScene::new(
        CanvasSpec {
            width: 4.0,
            height: 4.0,
        },
        "family".into(),
        "realization".into(),
        vec![layer],
    )
    .expect("valid scene");
    let raster = rasterize(&scene, RasterBackground::Transparent).expect("rasterized region");
    assert_eq!(raster.width(), 4);
    assert_eq!(raster.height(), 4);
    assert!(raster.pixels().chunks_exact(4).all(|pixel| pixel[3] > 0));
    assert!(write_svg(&scene).contains("fill-rule=\"nonzero\""));
}

/// Proves ordered output IDs are validated and every region output participates in native raster filling.
#[test]
fn ordered_region_outputs_validate_identity_and_raster_in_painter_order() {
    let ring = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ],
        PathClosure::Closed,
    )
    .expect("finite closed ring");
    let regions = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(7),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
                mechanism_id: PatternMechanismId(2),
                ordinal: 1,
            }]),
            components: vec![ring],
        }],
    })
    .expect("canonical region");
    let output = RenderOutputLayer {
        output_layer_id: PatternOutputLayerId(7),
        geometry: GeometryOutput::CanonicalRegions(regions.clone()),
        primitive_paints: None,
    };
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        },
        0.5,
        vec![
            output.clone(),
            RenderOutputLayer {
                output_layer_id: PatternOutputLayerId(8),
                geometry: GeometryOutput::CanonicalRegions(regions),
                primitive_paints: None,
            },
        ],
    )
    .expect("ordered outputs");
    let scene = RenderScene::new(
        CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        "family".into(),
        "realization".into(),
        vec![layer],
    )
    .expect("scene");
    assert_eq!(scene.layers()[0].outputs().len(), 2);
    assert!(
        rasterize(&scene, RasterBackground::Transparent)
            .expect("plural outputs rasterize")
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel[3] > 0)
    );
    assert!(
        RenderLayer::new_outputs(
            ChannelId(1),
            true,
            ColorValue {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0
            },
            1.0,
            vec![output.clone(), output],
        )
        .is_err()
    );
}

/// Generates the deterministic Stage 20N direct-region native and SVG artifacts under `target`.
#[test]
#[ignore = "validation artifact generator"]
fn generate_direct_region_intrinsic_artifacts() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage20n/intrinsic");
    std::fs::create_dir_all(&output).expect("validation directory");
    for (name, width, height) in [
        ("direct-region-1024", 1024.0, 1024.0),
        ("direct-region-900x620", 900.0, 620.0),
    ] {
        let scene = direct_region_scene(width, height);
        let raster =
            rasterize(&scene, RasterBackground::Transparent).expect("native region raster");
        std::fs::write(
            output.join(format!("{name}.png")),
            encode_png(&raster).expect("PNG"),
        )
        .expect("native region PNG");
        std::fs::write(output.join(format!("{name}.svg")), write_svg(&scene))
            .expect("direct region SVG");
    }
}
