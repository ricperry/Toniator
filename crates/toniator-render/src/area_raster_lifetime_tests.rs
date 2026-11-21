//! Frozen pre-change normalization oracle and focused storage-lifetime witnesses.

use super::*;

/// Normalizes every primitive independently, then unions their already-positive fills.
///
/// Independently grid-collapsed contours consume the layer's absolute fill-error budget even
/// beside retained paint. Checked split-mutation certificates share that same cumulative budget.
/// A preview rectangle intersects the positive subjects in one checked operation on the same
/// adapter, avoiding an intermediate global union and float-grid roundtrip. Unclipped consumers
/// retain their original global union. Both branches charge actual request-wide Boolean work.
/// Optional visible bounds scope only normalization certificates proven irrelevant to every pixel;
/// the adapter and final global union/clip certificate remain unchanged.
///
/// # Errors
/// Returns cancellation, precision, allocation or Boolean diagnostics without painting partial pixels.
/// Already incurred local recovery debt is charged once for every primitive, independently of
/// healthy geometry or holes; potential circle/subgrid loss retains its existing separate semantics.
fn reference_normalize_and_union(
    raw: &[RawAreaPrimitive],
    clip: Option<[f64; 4]>,
    work: &mut RasterWork<'_>,
    omissions: &mut AreaOmissionBudget,
    visible: Option<[f64; 4]>,
) -> Result<AreaShapes, RenderError> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    for primitive in raw {
        omissions.record_incurred(primitive.incurred_area_error_bound)?;
    }
    let mut omitted = AreaSum::default();
    for primitive in raw.iter().filter(|primitive| primitive.shape.is_empty()) {
        omitted.add(primitive.omitted_area_bound);
    }
    if !raw.iter().any(|primitive| !primitive.shape.is_empty()) {
        omissions.record(omitted.value())?;
        return Ok(Vec::new());
    }
    let rect = area_adapter(raw, clip, work)?;
    let mut positive = Vec::new();
    for primitive in raw.iter().filter(|primitive| !primitive.shape.is_empty()) {
        work.check()?;
        let mut bound =
            area_collapsed_contour_bound(&primitive.shape, primitive.fill_rule, rect, work)?;
        let normalized = area_overlay(
            std::slice::from_ref(&primitive.shape),
            rect,
            primitive.fill_rule,
            work,
            omissions,
            visible,
        )?;
        let exact_empty = normalized.is_empty()
            && primitive.shape.len() == 2
            && exact_contours_cancel(
                &primitive.shape[0],
                &primitive.shape[1],
                primitive.fill_rule,
                work,
            )?;
        if normalized.is_empty() && !exact_empty {
            bound = bound
                .max(primitive.omitted_area_bound)
                .max(area_perturbation_bound(&primitive.shape, work)?);
        }
        omitted.add(bound);
        positive.try_reserve(normalized.len()).map_err(|_| {
            area_error(
                "raster.allocation",
                "normalized area shape allocation failed",
            )
        })?;
        positive.extend(normalized);
    }
    omissions.record(omitted.value())?;
    work.check()?;
    if let Some([left, top, right, bottom]) = clip {
        work.check()?;
        let rectangle = vec![vec![vec![
            [left, top],
            [right, top],
            [right, bottom],
            [left, bottom],
        ]]];
        let adapter = FloatPointAdapter::<AreaPoint, i64>::try_with_scale_conservative(
            rect,
            AREA_BOOLEAN_SCALE,
        )
        .map_err(|_| {
            area_error(
                "raster.area.precision",
                "clip grid cannot represent area bounds",
            )
        })?;
        let mut options = OverlayOptions::<f64, i64>::default();
        options.min_output_area = 0.0;
        options.clean_result = false;
        let mut overlay = FloatOverlay::<AreaPoint, i64>::new_custom(
            adapter,
            options,
            Solver {
                precision: Precision::ABSOLUTE,
                ..Solver::AUTO
            },
            4,
        )
        .unsafe_add_source(&positive, ShapeType::Subject)
        .unsafe_add_source(&rectangle, ShapeType::Clip);
        area_checked_overlay(
            &mut overlay,
            OverlayRule::Intersect,
            FillRule::NonZero,
            work,
            omissions,
            AreaOverlayPurpose::Final,
            None,
        )
    } else {
        area_overlay(&positive, rect, FillRule::NonZero, work, omissions, None)
    }
}

/// Supplies mixed fills, overlap, a hole, self-intersection, a tiny sliver and explicit omission debt.
/// These derived fixtures never change canonical scene or source authority.
fn primitives() -> Vec<RawAreaPrimitive> {
    let rectangle =
        |left, top, right, bottom| vec![[left, top], [right, top], [right, bottom], [left, bottom]];
    vec![
        RawAreaPrimitive {
            shape: vec![
                rectangle(-1.0, -1.0, 12.0, 12.0),
                rectangle(2.0, 2.0, 6.0, 6.0),
            ],
            fill_rule: FillRule::EvenOdd,
            omitted_area_bound: 0.0,
            incurred_area_error_bound: 1.0e-12,
        },
        RawAreaPrimitive {
            shape: vec![rectangle(4.5, 3.5, 14.0, 8.2)],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
            incurred_area_error_bound: 0.0,
        },
        RawAreaPrimitive {
            shape: vec![vec![[1.0, 1.0], [9.0, 9.0], [1.0, 9.0], [9.0, 1.0]]],
            fill_rule: FillRule::EvenOdd,
            omitted_area_bound: 0.0,
            incurred_area_error_bound: 0.0,
        },
        RawAreaPrimitive {
            shape: vec![rectangle(0.25, 0.2, 0.2500001, 10.0)],
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 0.0,
            incurred_area_error_bound: 0.0,
        },
        RawAreaPrimitive {
            shape: Vec::new(),
            fill_rule: FillRule::NonZero,
            omitted_area_bound: 2.0e-12,
            incurred_area_error_bound: 0.0,
        },
    ]
}

/// Paints actual normalized geometry through the unchanged linear composition and native RGBA consumer.
/// # Panics
/// Panics if finite focused geometry fails normal pixel integration or quantization.
fn pixels(shapes: &AreaShapes) -> Vec<u8> {
    let mut work = RasterWork::new(
        RasterizationLimits::default(),
        RasterAntialiasing::On,
        &|| false,
    );
    let mut pixels =
        allocate_linear_pixels(16, 16, background_pixel(RasterBackground::Transparent)).unwrap();
    let color = ColorValue {
        red: 0.2,
        green: 0.4,
        blue: 0.8,
        alpha: 1.0,
    };
    for_each_area_pixel(shapes, 16, 16, &mut work, |index, coverage| {
        pixels.paint_shape(index, coverage, &color, 1.0)
    })
    .unwrap();
    pixels_from_linear(16, 16, pixels, &|| false)
        .unwrap()
        .pixels()
        .to_vec()
}

/// Proves released/streamed temporary storage preserves the frozen global Boolean and pixel contracts.
///
/// Both clipped and unclipped consumers retain every vertex bit, error diagnostic, cumulative
/// certificate, consumed work and cancellation boundary; native RGBA uses the real consumer.
/// # Panics
/// Panics on geometry/pixel drift, changed diagnostics, reset work/debt, or lost cancellation.
#[test]
fn owned_normalization_matches_prechange_geometry_pixels_work_and_failures() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let clip = Some([0.3, 0.7, 11.2, 10.6]);
    for (clip, boolean_limit, precision_failure, cancel_at) in [
        (None, usize::MAX, false, usize::MAX),
        (clip, usize::MAX, false, usize::MAX),
        (None, 0, false, usize::MAX),
        (clip, 20, false, usize::MAX),
        (None, usize::MAX, true, usize::MAX),
        (clip, usize::MAX, true, usize::MAX),
        (None, usize::MAX, false, 3),
        (clip, usize::MAX, false, 80),
    ] {
        let mut facts = Vec::new();
        for reference in [true, false] {
            let calls = AtomicUsize::new(0);
            let cancelled = || calls.fetch_add(1, Ordering::Relaxed) + 1 >= cancel_at;
            let mut work = RasterWork::new(
                RasterizationLimits::new(1_000_000)
                    .unwrap()
                    .with_max_boolean_operations(boolean_limit),
                RasterAntialiasing::On,
                &cancelled,
            );
            let mut omissions = AreaOmissionBudget::default();
            let mut raw = primitives();
            if precision_failure {
                raw[0].incurred_area_error_bound = 2.0e-6;
            }
            let result = if reference {
                reference_normalize_and_union(
                    &raw,
                    clip,
                    &mut work,
                    &mut omissions,
                    Some([0.0, 0.0, 16.0, 16.0]),
                )
            } else {
                normalize_owned_and_union(
                    raw,
                    clip,
                    &mut work,
                    &mut omissions,
                    Some([0.0, 0.0, 16.0, 16.0]),
                )
            };
            let result =
                result.map_err(|error| (error.path().to_owned(), error.message().to_owned()));
            facts.push((
                result,
                work.remaining_edges,
                work.remaining_boolean_operations,
                work.completed_units,
                omissions.total_bound().to_bits(),
                calls.load(Ordering::Relaxed),
            ));
        }
        assert_eq!(facts[0], facts[1]);
        if boolean_limit == usize::MAX && !precision_failure && cancel_at == usize::MAX {
            let expected = facts[0].0.as_ref().unwrap();
            let actual = facts[1].0.as_ref().unwrap();
            let bits = |shapes: &AreaShapes| {
                shapes
                    .iter()
                    .flatten()
                    .flatten()
                    .map(|point| [point[0].to_bits(), point[1].to_bits()])
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(actual), bits(expected));
            assert_eq!(pixels(actual), pixels(expected));
        }
    }
}
