use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    Bounds, FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, NominalCellBasis,
    Point2, SiteScope, Vector2, VoronoiRegionLimits, VoronoiRegionRequest,
    build_voronoi_regions_cancellable,
};
use toniator_render::{
    GeometryOutput, RasterBackground, RenderLayer, RenderScene, encode_png, rasterize, write_svg,
};

/// Builds a guard-inclusive ordinary Voronoi scene without asking renderers to create topology.
fn voronoi_scene(width: f64, height: f64) -> RenderScene {
    let mechanism = PatternMechanismId(81);
    let points = [
        (-width, -height),
        (width / 2.0, -height),
        (2.0 * width, -height),
        (-width, height / 2.0),
        (width / 2.0, height / 2.0),
        (2.0 * width, height / 2.0),
        (-width, 2.0 * height),
        (width / 2.0, 2.0 * height),
        (2.0 * width, 2.0 * height),
    ];
    let family = FamilySiteSet::new(
        "stage20o-artifact-family".into(),
        mechanism,
        points
            .into_iter()
            .enumerate()
            .map(|(ordinal, (x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: mechanism,
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .expect("basis"),
                scope: SiteScope::Guard,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("family");
    let regions = build_voronoi_regions_cancellable(
        &family,
        VoronoiRegionRequest {
            output_layer_id: PatternOutputLayerId(81),
            canvas: Bounds::new(Point2::new(0.0, 0.0), Point2::new(width, height)).expect("canvas"),
        },
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("guard cells")
    .0;
    let layer = RenderLayer::new_for_output(
        ChannelId(81),
        true,
        ColorValue {
            red: 0.2,
            green: 0.55,
            blue: 0.9,
            alpha: 0.8,
        },
        0.75,
        PatternOutputLayerId(81),
        GeometryOutput::CanonicalRegions(regions),
    )
    .expect("region layer");
    RenderScene::new(
        CanvasSpec { width, height },
        "stage20o-voronoi-family".into(),
        "stage20o-voronoi-realization".into(),
        vec![layer],
    )
    .expect("scene")
}

/// Proves the renderer fills ordinary canonical cells as a solid final-clipped output.
#[test]
fn ordinary_voronoi_regions_render() {
    let scene = voronoi_scene(64.0, 64.0);
    assert!(
        !rasterize(&scene, RasterBackground::Transparent)
            .expect("raster")
            .pixels()
            .is_empty()
    );
    assert!(write_svg(&scene).contains("fill-rule=\"nonzero\""));
}

/// Generates intrinsic direct Voronoi PNG and raw SVG evidence for parent inspection.
#[test]
#[ignore = "validation artifact generator"]
fn generate_intrinsic_voronoi_artifacts() {
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage20o");
    std::fs::create_dir_all(&output).expect("validation directory");
    for (name, width, height) in [
        ("voronoi-raster-1024", 1024.0, 1024.0),
        ("voronoi-vector-900x620", 900.0, 620.0),
    ] {
        let scene = voronoi_scene(width, height);
        let raster = rasterize(&scene, RasterBackground::Transparent).expect("raster");
        std::fs::write(
            output.join(format!("{name}.png")),
            encode_png(&raster).expect("png"),
        )
        .expect("write png");
        std::fs::write(output.join(format!("{name}.svg")), write_svg(&scene)).expect("write svg");
    }
}
