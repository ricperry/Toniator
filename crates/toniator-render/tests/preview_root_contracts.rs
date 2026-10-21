//! Headless retained-root whole-view certification and crop parity contracts.
use std::sync::Arc;
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel, PatternMechanismId,
};
use toniator_geometry::*;
use toniator_render::{
    preview_tiles::{PreviewRoot, Region},
    *,
};
/// Supplies finite original linear paint; tiny positive alpha intentionally preserves hidden RGB.
fn color(r: f64, g: f64, b: f64, a: f64) -> ColorValue {
    ColorValue {
        red: r,
        green: g,
        blue: b,
        alpha: a,
    }
}

/// Builds exact line-only geometry with a bridged hole, reversed winding and narrow boundary slivers.
/// # Panics
/// Panics only when finite synthetic canonical paths fail current admission.
fn marks(rule: CanonicalFillRule, reverse: bool) -> Vec<CanonicalMark> {
    let hole = vec![
        (-2., -2.),
        (12., -2.),
        (12., 10.),
        (-2., 10.),
        (-2., -2.),
        (2., 2.),
        (2., 6.),
        (6., 6.),
        (6., 2.),
        (2., 2.),
        (-2., -2.),
    ];
    let sliver = vec![(0.01, -1.), (0.04, -1.), (8.98, 7.01), (8.95, 7.01)];
    let touch = vec![(9., 0.), (10., 0.), (10., 7.), (9., 7.)];
    [hole, sliver, touch]
        .into_iter()
        .enumerate()
        .map(|(ordinal, mut points)| {
            if reverse {
                points.reverse();
            }
            CanonicalMark::ClosedPath(
                CanonicalPathMark::new(
                    FamilySiteId {
                        mechanism_id: PatternMechanismId(1),
                        ordinal,
                    },
                    CurvePath::polyline(
                        points.into_iter().map(|(x, y)| Point2::new(x, y)).collect(),
                        PathClosure::Closed,
                    )
                    .unwrap(),
                    SiteScope::Canvas,
                    FamilySiteProvenance::Random {
                        candidate_ordinal: ordinal,
                        accepted_ordinal: ordinal,
                        exclusion_neighbor_ordinal: None,
                    },
                    rule,
                )
                .unwrap(),
            )
        })
        .collect()
}

/// Creates all composition authorities without sampling or calibration; color/order remain original.
/// # Panics
/// Panics on invalid synthetic model/paint admission.
fn scene(
    model: Option<HalftoneChannelModel>,
    rule: CanonicalFillRule,
    reverse: bool,
) -> RenderScene {
    let geometry = marks(rule, reverse);
    let layers = if model == Some(HalftoneChannelModel::SourceColorAlpha) {
        vec![
            RenderLayer::new_source_color_geometry(
                ChannelId(1),
                true,
                if reverse { 0.0001 } else { 0.41 },
                geometry,
                vec![
                    color(1., 0.1, 0., 1.),
                    color(0., 1., 0., 1.),
                    color(0., 0., 1., 1.),
                ],
            )
            .unwrap(),
        ]
    } else {
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                color(1., 0., 0., 1.),
                0.41,
                GeometryOutput::CanonicalMarks(geometry.clone()),
            )
            .unwrap(),
            RenderLayer::new(
                ChannelId(2),
                true,
                color(0., 1., 1., 1.),
                1.,
                GeometryOutput::CanonicalMarks(geometry),
            )
            .unwrap(),
            RenderLayer::new(
                ChannelId(3),
                false,
                color(0., 0., 1., 1.),
                1.,
                GeometryOutput::CanonicalMarks(marks(rule, !reverse)),
            )
            .unwrap(),
        ]
    };
    let canvas = CanvasSpec {
        width: 9.,
        height: 7.,
    };
    match model {
        Some(m) => {
            RenderScene::new_modeled(canvas, "crop-fixture".into(), "line-hole".into(), m, layers)
        }
        None => RenderScene::new(canvas, "crop-fixture".into(), "line-hole".into(), layers),
    }
    .unwrap()
}

/// Compares unequal crops against one fixed-root preparation for every model/backing/fill rule.
/// # Panics
/// Panics on changed RGBA (including hidden RGB), work consumption, namespace or cancellation.
#[test]
fn retained_root_stitches_all_models_without_preparing_tiles() {
    for reverse in [false, true] {
        for model in [
            None,
            Some(HalftoneChannelModel::Rgb),
            Some(HalftoneChannelModel::Cmyk),
            Some(HalftoneChannelModel::SourceColorAlpha),
        ] {
            for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
                for background in [
                    RasterBackground::Transparent,
                    RasterBackground::OpaqueBlack,
                    RasterBackground::OpaqueWhite,
                ] {
                    let scene = Arc::new(scene(model, rule, reverse));
                    let mut root = PreviewRoot::new(
                        scene,
                        1.,
                        background,
                        RasterizationLimits::default(),
                        &|| false,
                    )
                    .unwrap();
                    let whole = Region {
                        x: 0,
                        y: 0,
                        width: 9,
                        height: 7,
                    };
                    let certificate = root.certify(whole, &|| false).unwrap();
                    let snapshot = root.snapshot(&certificate).unwrap();
                    let reference = snapshot.paint(whole, &certificate, &|| false).unwrap();
                    let mut stitched = vec![0u8; 9 * 7 * 4];
                    let before = root.facts().unwrap();
                    for y in 0..7 {
                        for (x, width) in [(0, 1), (1, 3), (4, 5)] {
                            let tile = snapshot
                                .paint(
                                    Region {
                                        x,
                                        y,
                                        width,
                                        height: 1,
                                    },
                                    &certificate,
                                    &|| false,
                                )
                                .unwrap();
                            let bytes = tile.surface(&root, &certificate).unwrap().pixels();
                            let offset = ((y * 9 + x) * 4) as usize;
                            stitched[offset..offset + bytes.len()].copy_from_slice(bytes);
                        }
                    }
                    assert_eq!(
                        stitched,
                        reference.surface(&root, &certificate).unwrap().pixels()
                    );
                    let after = root.facts().unwrap();
                    assert_eq!(before.consumed_edges, after.consumed_edges);
                    assert_eq!(
                        before.consumed_boolean_operations,
                        after.consumed_boolean_operations
                    );
                    assert!(snapshot.paint(whole, &certificate, &|| true).is_err());
                    let other = PreviewRoot::new(
                        Arc::new(scene_for_namespace()),
                        1.,
                        background,
                        RasterizationLimits::default(),
                        &|| false,
                    )
                    .unwrap();
                    assert!(reference.surface(&other, &certificate).is_err());
                }
            }
        }
    }
}

/// Supplies an independent valid root identity without persisting or cloning canonical payload.
/// # Panics
/// Panics only on invalid synthetic scene construction.
fn scene_for_namespace() -> RenderScene {
    scene(None, CanonicalFillRule::EvenOdd, false)
}

/// Checks fractional root coverage, later-pan circles, invalid regions and initial work exhaustion.
/// # Panics
/// Panics on narrowing sampled fetch bounds or admitting uncertified/invalid crops.
#[test]
fn retained_root_frame_and_resource_admission_are_independent_of_first_view() {
    let source = Arc::new(scene_for_namespace());
    let mut root = PreviewRoot::new(
        source.clone(),
        1.25,
        RasterBackground::Transparent,
        RasterizationLimits::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!((root.frame().width, root.frame().height), (12, 9));
    assert_eq!(
        root.sampled_region(0.25, 0.5, 3, 2).unwrap(),
        Region {
            x: 0,
            y: 0,
            width: 4,
            height: 3
        }
    );
    let region = Region {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let cert = root.certify(region, &|| false).unwrap();
    assert!(
        root.snapshot(&cert)
            .unwrap()
            .paint(Region { x: 1, ..region }, &cert, &|| false)
            .is_err()
    );
    assert!(
        root.certify(
            Region {
                x: u32::MAX,
                ..region
            },
            &|| false
        )
        .is_err()
    );
    assert!(
        PreviewRoot::new(
            source.clone(),
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::new(1).unwrap(),
            &|| false
        )
        .is_err()
    );
    assert!(
        PreviewRoot::new(
            source,
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| true
        )
        .is_err()
    );
}

/// Retains an initially unseen circle and fractional canvas clip for a later certified pan.
/// # Panics
/// Panics when first-view visibility culls geometry or fractional canvas backing is lost.
#[test]
fn later_pan_retains_circle_and_fractional_canvas_edge() {
    let circle = CanonicalMark::Circle {
        source_site_id: FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        },
        center: Point2::new(8.5, 3.5),
        radius: 1.,
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: 0,
            accepted_ordinal: 0,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule: CanonicalFillRule::NonZero,
    };
    let source = Arc::new(
        RenderScene::new(
            CanvasSpec {
                width: 9.25,
                height: 7.25,
            },
            "circle".into(),
            "later".into(),
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    color(1., 0., 0., 1.),
                    1.,
                    GeometryOutput::CanonicalMarks(vec![circle]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let mut root = PreviewRoot::new(
        source.clone(),
        1.,
        RasterBackground::Transparent,
        RasterizationLimits::default(),
        &|| false,
    )
    .unwrap();
    let first = Region {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    let certificate = root.certify(first, &|| false).unwrap();
    let tile = root
        .snapshot(&certificate)
        .unwrap()
        .paint(first, &certificate, &|| false)
        .unwrap();
    assert!(
        tile.surface(&root, &certificate)
            .unwrap()
            .pixels()
            .iter()
            .all(|v| *v == 0)
    );
    let later = Region {
        x: 7,
        y: 2,
        width: 3,
        height: 3,
    };
    let certificate = root.certify(later, &|| false).unwrap();
    let tile = root
        .snapshot(&certificate)
        .unwrap()
        .paint(later, &certificate, &|| false)
        .unwrap();
    assert!(
        tile.surface(&root, &certificate)
            .unwrap()
            .pixels()
            .chunks_exact(4)
            .any(|p| p[3] > 0)
    );
    let target = PreviewRasterTarget::for_viewport(3, 3, 7., 2., 1.)
        .unwrap()
        .with_background(RasterBackground::Transparent);
    let ordinary =
        rasterize_preview_cancellable(&source, target, RasterizationLimits::default(), &|| false)
            .unwrap();
    assert_eq!(
        tile.surface(&root, &certificate).unwrap().pixels(),
        ordinary.pixels()
    );
}
