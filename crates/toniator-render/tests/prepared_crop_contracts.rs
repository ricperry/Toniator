//! Default-off prepared-area crop parity and bounded-run failure contracts.
#![cfg(feature = "prepared-crop-pilot")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel, PatternMechanismId,
};
use toniator_geometry::*;
use toniator_render::{
    prepared_crop_pilot::{self as pilot, Crop, Options, Summary},
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
        vec![RenderLayer::new_source_color_geometry(
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
        .unwrap()]
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

/// Partitions every root pixel once using odd/uneven widths and heights, optionally reversing delivery.
/// # Panics
/// Panics only if positive synthetic bounds cease to form valid crops.
fn partition(w: u32, h: u32, reverse: bool) -> Vec<Crop> {
    let mut out = Vec::new();
    let mut y = 0;
    let mut row = 0;
    while y < h {
        let height = [1, 3, 2][row % 3].min(h - y);
        let mut x = 0;
        let mut col = 0;
        while x < w {
            let width = [3, 1, 5, 2][col % 4].min(w - x);
            out.push(Crop::new(x, y, width, height).unwrap());
            x += width;
            col += 1;
        }
        y += height;
        row += 1;
    }
    if reverse {
        out.reverse();
    }
    out
}

/// Stitches ordered caller-thread deliveries while accounting for actual retained root buffers.
/// # Panics
/// Panics on unexpected order/thread, overlap, missing pixel or a diagnostic run failure.
fn stitched(
    scene: &RenderScene,
    target: PreviewRasterTarget,
    crops: &[Crop],
    workers: usize,
) -> (Vec<u8>, Summary) {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    let mut pixels = vec![0; target.width() as usize * target.height() as usize * 4];
    let mut seen = vec![false; pixels.len() / 4];
    let caller = std::thread::current().id();
    let mut index = 0;
    let summary = pilot::run(
        scene,
        target,
        RasterizationLimits::default(),
        crops,
        Options {
            profiling: false,
            maximum_estimated_payload_bytes: 256 * 1024 * 1024,
            caller_retained_bytes: pixels.capacity() * 2 + seen.capacity(),
            maximum_in_flight: workers,
        },
        &pool,
        &|| false,
        |crop, surface| {
            assert_eq!(std::thread::current().id(), caller);
            assert_eq!(crop, crops[index]);
            index += 1;
            for y in 0..crop.height as usize {
                for x in 0..crop.width as usize {
                    let global =
                        (crop.y as usize + y) * target.width() as usize + crop.x as usize + x;
                    let local = y * crop.width as usize + x;
                    assert!(!seen[global]);
                    seen[global] = true;
                    pixels[global * 4..global * 4 + 4]
                        .copy_from_slice(&surface.pixels()[local * 4..local * 4 + 4]);
                }
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(seen.into_iter().all(|x| x));
    assert_eq!(index, crops.len());
    assert!(summary.maximum_batch_tiles <= workers);
    (pixels, summary)
}

/// Proves full native RGBA equality at tiny/thin/odd targets and fractional positive/negative phases.
/// Models, paint order, holes/rules/reversed paths, backing and shuffled crop order retain authority.
/// # Panics
/// Panics on any differing component, admission failure or out-of-order/cross-thread sink call.
#[test]
fn arbitrary_crops_match_root_rgba_for_models_phases_and_workers() {
    for model in [
        None,
        Some(HalftoneChannelModel::Rgb),
        Some(HalftoneChannelModel::Cmyk),
        Some(HalftoneChannelModel::SourceColorAlpha),
    ] {
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            for reverse in [false, true] {
                let scene = scene(model, rule, reverse);
                for (w, h, origin, scale, background) in [
                    (1, 1, [-0.25, 0.125], 1.25, RasterBackground::Transparent),
                    (1, 9, [0.125, -0.375], 1.25, RasterBackground::OpaqueWhite),
                    (13, 1, [0., 0.], 1., RasterBackground::OpaqueBlack),
                    (13, 11, [-0.25, -0.125], 1.25, RasterBackground::Transparent),
                ] {
                    let target =
                        PreviewRasterTarget::for_viewport(w, h, origin[0], origin[1], scale)
                            .unwrap()
                            .with_background(background);
                    let root = rasterize_preview(&scene, target).unwrap();
                    if model == Some(HalftoneChannelModel::SourceColorAlpha)
                        && reverse
                        && w == 13
                        && h == 11
                    {
                        assert!(
                            root.pixels()
                                .chunks_exact(4)
                                .any(|p| p[3] == 0 && p[..3].iter().any(|&v| v != 0)),
                            "fixture must exercise hidden RGB"
                        );
                    }

                    let crops = partition(w, h, reverse);
                    for workers in [1, 2, 4] {
                        let (actual, _) = stitched(&scene, target, &crops, workers);
                        assert_eq!(actual,root.pixels(),"{model:?} {rule:?} reverse={reverse} target={target:?} workers={workers}");
                    }
                }
            }
        }
    }
}

/// Proves geometry work/cardinality is root-only, while tiny crops retain global-X scratch admission.
/// # Panics
/// Panics on crop-dependent preparation work or a missed payload admission failure.
#[test]
fn root_work_is_not_repeated_and_payload_admission_is_checked() {
    let scene = scene(None, CanonicalFillRule::NonZero, false);
    let target = PreviewRasterTarget::new(13, 11).unwrap();
    let full = [Crop::new(0, 0, 13, 11).unwrap()];
    let many = partition(13, 11, true);
    let (_, a) = stitched(&scene, target, &full, 1);
    let (_, b) = stitched(&scene, target, &many, 4);
    assert!(a.consumed_flattened_edges > 0 && a.consumed_boolean_operations > 0);
    assert_eq!(
        (
            a.consumed_flattened_edges,
            a.consumed_boolean_operations,
            a.normalized_edges,
            a.prepared_paints,
            a.root_primitives
        ),
        (
            b.consumed_flattened_edges,
            b.consumed_boolean_operations,
            b.normalized_edges,
            b.prepared_paints,
            b.root_primitives
        )
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let calls = AtomicUsize::new(0);
    for bytes in [0, a.prepared_capacity_bytes] {
        let e = pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &full,
            Options {
                profiling: false,
                maximum_estimated_payload_bytes: bytes,
                caller_retained_bytes: 0,
                maximum_in_flight: 1,
            },
            &pool,
            &|| false,
            |_, _| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(e.path(), "raster.crop.memory");
    }
    let e = pilot::run(
        &scene,
        target,
        RasterizationLimits::default(),
        &full,
        Options {
            profiling: false,
            maximum_estimated_payload_bytes: usize::MAX,
            caller_retained_bytes: usize::MAX,
            maximum_in_flight: 1,
        },
        &pool,
        &|| false,
        |_, _| Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.path(), "raster.crop.memory");
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Rejects malformed crop/options and preserves exact production root errors under tiny tiles.
/// # Panics
/// Panics on invalid admission, partial sink delivery before root failure or changed error authority.
#[test]
fn invalid_regions_and_root_budgets_never_reset_per_crop() {
    let scene = scene(None, CanonicalFillRule::EvenOdd, false);
    let target = PreviewRasterTarget::new(13, 11).unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let options = Options {
        profiling: false,
        maximum_estimated_payload_bytes: usize::MAX,
        caller_retained_bytes: 0,
        maximum_in_flight: 2,
    };
    assert!(Crop::new(0, 0, 0, 1).is_err());
    assert!(Crop::new(u32::MAX, 0, 2, 1).is_err());
    for crop in [
        Crop {
            x: 13,
            y: 0,
            width: 1,
            height: 1,
        },
        Crop {
            x: u32::MAX,
            y: 0,
            width: 2,
            height: 1,
        },
        Crop {
            x: 0,
            y: 0,
            width: 0,
            height: 1,
        },
    ] {
        assert_eq!(
            pilot::run(
                &scene,
                target,
                RasterizationLimits::default(),
                &[crop],
                options,
                &pool,
                &|| false,
                |_, _| Ok(())
            )
            .unwrap_err()
            .path(),
            "raster.crop.region"
        );
    }
    let crops = partition(13, 11, false);
    for limits in [
        RasterizationLimits::new(1).unwrap(),
        RasterizationLimits::default().with_max_boolean_operations(0),
    ] {
        let expected =
            rasterize_preview_cancellable(&scene, target, limits, &|| false).unwrap_err();
        let mut delivered = 0;
        let actual = pilot::run(
            &scene,
            target,
            limits,
            &crops,
            options,
            &pool,
            &|| false,
            |_, _| {
                delivered += 1;
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(delivered, 0);
    }
    for n in [0, 3] {
        assert_eq!(
            pilot::run(
                &scene,
                target,
                RasterizationLimits::default(),
                &crops,
                Options {
                    maximum_in_flight: n,
                    ..options
                },
                &pool,
                &|| false,
                |_, _| Ok(())
            )
            .unwrap_err()
            .path(),
            "raster.crop.options"
        );
    }
    assert!(pilot::run(
        &scene,
        target,
        RasterizationLimits::default(),
        &[],
        options,
        &pool,
        &|| false,
        |_, _| Ok(())
    )
    .is_err());
}

/// Returns no completed root summary on initial/ongoing cancellation or caller sink failure.
/// # Panics
/// Panics if cancellation or consumer failure is hidden by successful crop completion.
#[test]
fn cancellation_and_sink_failure_never_return_complete() {
    let scene = scene(None, CanonicalFillRule::NonZero, false);
    let target = PreviewRasterTarget::new(13, 11).unwrap();
    let crops = partition(13, 11, false);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let options = Options {
        profiling: false,
        maximum_estimated_payload_bytes: 256 * 1024 * 1024,
        caller_retained_bytes: 0,
        maximum_in_flight: 2,
    };
    assert_eq!(
        pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &crops,
            options,
            &pool,
            &|| true,
            |_, _| Ok(())
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    let cancelled = AtomicBool::new(false);
    let mut delivered = 0;
    assert_eq!(
        pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &crops,
            options,
            &pool,
            &|| cancelled.load(Ordering::Acquire),
            |_, _| {
                delivered += 1;
                cancelled.store(true, Ordering::Release);
                Ok(())
            }
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
    assert_eq!(delivered, 1);
    let sentinel = RenderError::new("pilot.sink", "owned sink failure");
    assert_eq!(
        pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &crops,
            options,
            &pool,
            &|| false,
            |_, _| Err(sentinel.clone())
        )
        .unwrap_err(),
        sentinel
    );
    let checks = AtomicUsize::new(0);
    assert_eq!(
        pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &crops,
            options,
            &pool,
            &|| checks.fetch_add(1, Ordering::Relaxed) > crops.len() + 3,
            |_, _| Ok(())
        )
        .unwrap_err()
        .path(),
        "evaluation.cancelled"
    );
}

/// Includes original global-X scratch even for1x1 crops and rejects a batch before dispatch.
/// # Panics
/// Panics if admission substitutes tile-width scratch or ignores completed/result payload.
#[test]
fn batch_admission_keeps_global_x_scratch_for_one_pixel_crops() {
    let scene = scene(None, CanonicalFillRule::NonZero, false);
    let target = PreviewRasterTarget::for_viewport(1003, 3, 0., 0., 100.).unwrap();
    let crops = [
        Crop::new(500, 0, 1, 1).unwrap(),
        Crop::new(501, 1, 1, 1).unwrap(),
    ];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let options = Options {
        profiling: false,
        maximum_estimated_payload_bytes: usize::MAX,
        caller_retained_bytes: 0,
        maximum_in_flight: 2,
    };
    let summary = pilot::run(
        &scene,
        target,
        RasterizationLimits::default(),
        &crops,
        options,
        &pool,
        &|| false,
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(summary.maximum_estimated_payload_bytes - summary.prepared_capacity_bytes > 20_000);
    let mut delivered = 0;
    let error = pilot::run(
        &scene,
        target,
        RasterizationLimits::default(),
        &crops,
        Options {
            profiling: false,
            maximum_estimated_payload_bytes: summary.maximum_estimated_payload_bytes - 1,
            ..options
        },
        &pool,
        &|| false,
        |_, _| {
            delivered += 1;
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.path(), "raster.crop.memory");
    assert_eq!(delivered, 0);
}

/// Reads arbitrary overlapping/duplicate subsets in the original root frame without mutable sharing.
/// # Panics
/// Panics on a differing root component or changed ordered subset delivery.
#[test]
fn overlapping_subsets_preserve_root_pixels_and_request_order() {
    let scene = scene(
        Some(HalftoneChannelModel::SourceColorAlpha),
        CanonicalFillRule::NonZero,
        false,
    );
    let target = PreviewRasterTarget::for_viewport(13, 11, 0.125, -0.25, 1.25).unwrap();
    let root = rasterize_preview(&scene, target).unwrap();
    let crops = [
        Crop::new(2, 3, 4, 5).unwrap(),
        Crop::new(0, 0, 1, 11).unwrap(),
        Crop::new(2, 3, 4, 5).unwrap(),
        Crop::new(0, 10, 13, 1).unwrap(),
    ];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let mut delivered = 0;
    let result = pilot::run(
        &scene,
        target,
        RasterizationLimits::default(),
        &crops,
        Options {
            profiling: false,
            maximum_estimated_payload_bytes: 64 * 1024 * 1024,
            caller_retained_bytes: root.pixels().len(),
            maximum_in_flight: 4,
        },
        &pool,
        &|| false,
        |crop, tile| {
            assert_eq!(crop, crops[delivered]);
            delivered += 1;
            for y in 0..crop.height as usize {
                let start = ((crop.y as usize + y) * 13 + crop.x as usize) * 4;
                let local = y * crop.width as usize * 4;
                let len = crop.width as usize * 4;
                assert_eq!(
                    &tile.pixels()[local..local + len],
                    &root.pixels()[start..start + len]
                );
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result.delivered_tiles, 4);
    assert_eq!(delivered, 4);
}

/// Keeps opt-in coarse profiling independent of RGBA/work authority, including quantized hidden RGB.
/// # Panics
/// Panics on changed pixels/work or profiling enabled/disabled state that contradicts the request.
#[test]
fn optional_coarse_timings_preserve_hidden_rgba_and_work() {
    let scene = scene(
        Some(HalftoneChannelModel::SourceColorAlpha),
        CanonicalFillRule::NonZero,
        true,
    );
    let target = PreviewRasterTarget::for_viewport(13, 11, -0.25, -0.125, 1.25).unwrap();
    let root = rasterize_preview(&scene, target).unwrap();
    let crops = partition(13, 11, true);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let mut previous = None;
    for profiling in [false, true] {
        let mut output = vec![0; root.pixels().len()];
        let summary = pilot::run(
            &scene,
            target,
            RasterizationLimits::default(),
            &crops,
            Options {
                maximum_estimated_payload_bytes: 256 * 1024 * 1024,
                caller_retained_bytes: root.pixels().len() + output.capacity(),
                maximum_in_flight: 4,
                profiling,
            },
            &pool,
            &|| false,
            |crop, tile| {
                for y in 0..crop.height as usize {
                    let dst = ((crop.y as usize + y) * 13 + crop.x as usize) * 4;
                    let src = y * crop.width as usize * 4;
                    let n = crop.width as usize * 4;
                    output[dst..dst + n].copy_from_slice(&tile.pixels()[src..src + n]);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(output, root.pixels());
        assert_eq!(summary.timings.is_some(), profiling);
        let work = (
            summary.consumed_flattened_edges,
            summary.consumed_boolean_operations,
            summary.normalized_edges,
            summary.prepared_paints,
        );
        if let Some(expected) = previous {
            assert_eq!(work, expected);
        }
        previous = Some(work);
    }
}
