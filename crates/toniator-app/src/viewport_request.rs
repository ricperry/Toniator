//! Presentation-only viewport transforms; canonical canvas and evaluator geometry stay unchanged.

use toniator_domain::CanvasSpec;
use toniator_engine::PreviewRasterTarget;

/// Keys the exact attempted viewport without rounding away allocation or pan distinctions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attempt {
    pub(crate) canvas: [u64; 2],
    pub(crate) page: [u64; 2],
    pub(crate) picture: [u64; 2],
    pub(crate) origin: [u64; 2],
    pub(crate) device_scale: i32,
    pub(crate) fit: bool,
}

/// Distinguishes absent GTK allocation from a concrete target or a genuine checked rejection.
pub(crate) enum Resolution {
    Unallocated,
    Ready(PreviewRasterTarget),
    Rejected {
        error: toniator_engine::RenderError,
        attempt: Attempt,
    },
}

/// Resolves checked viewport intent while leaving nonpositive, not-yet-mapped allocation quiet.
/// Positive malformed/extreme allocations retain exact producer errors without changing pixel limits.
pub(crate) fn resolve(
    canvas: &CanvasSpec,
    page: (f64, f64),
    picture: (f64, f64),
    picture_origin: (f64, f64),
    device_scale: i32,
    fit: bool,
) -> Resolution {
    if device_scale <= 0
        || [page.0, page.1, picture.0, picture.1]
            .iter()
            .any(|value| *value <= 0.0)
    {
        return Resolution::Unallocated;
    }
    let attempt = Attempt {
        canvas: [canvas.width.to_bits(), canvas.height.to_bits()],
        page: [page.0.to_bits(), page.1.to_bits()],
        picture: [picture.0.to_bits(), picture.1.to_bits()],
        origin: [picture_origin.0.to_bits(), picture_origin.1.to_bits()],
        device_scale,
        fit,
    };
    let reject = |error| Resolution::Rejected { error, attempt };
    if [
        page.0,
        page.1,
        picture.0,
        picture.1,
        canvas.width,
        canvas.height,
        picture_origin.0,
        picture_origin.1,
    ]
    .iter()
    .any(|value| !value.is_finite())
        || canvas.width <= 0.0
        || canvas.height <= 0.0
    {
        return reject(toniator_engine::RenderError::new(
            "preview.target",
            "viewport allocation and canvas must be finite with positive dimensions",
        ));
    }
    let scale = f64::from(device_scale);
    let width = (page.0 * scale).round();
    let height = (page.1 * scale).round();
    if !width.is_finite()
        || !height.is_finite()
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return reject(toniator_engine::RenderError::new(
            "preview.target.addressability",
            "physical viewport dimensions exceed representable integer extent",
        ));
    }
    let result = if fit {
        PreviewRasterTarget::new(width as u32, height as u32)
    } else {
        let logical_scale = (picture.0 / canvas.width).min(picture.1 / canvas.height);
        let canvas_x = picture_origin.0 + (picture.0 - canvas.width * logical_scale) * 0.5;
        let canvas_y = picture_origin.1 + (picture.1 - canvas.height * logical_scale) * 0.5;
        PreviewRasterTarget::for_viewport(
            width as u32,
            height as u32,
            -canvas_x / logical_scale,
            -canvas_y / logical_scale,
            logical_scale * scale,
        )
    };
    match result {
        Ok(target) => Resolution::Ready(target),
        Err(error) => reject(error),
    }
}

/// Projects checked intent for read-only consumers without logging or retry side effects.
/// Fit/crop arithmetic remains in `resolve`; callers needing failure authority inspect its typed result.
#[cfg(test)]
pub(crate) fn target(
    canvas: &CanvasSpec,
    page: (f64, f64),
    picture: (f64, f64),
    picture_origin: (f64, f64),
    device_scale: i32,
    fit: bool,
) -> Option<PreviewRasterTarget> {
    match resolve(canvas, page, picture, picture_origin, device_scale, fit) {
        Resolution::Ready(target) => Some(target),
        Resolution::Unallocated | Resolution::Rejected { .. } => None,
    }
}

/// Places a crop texture in the document-sized Paintable, independently of current pan or scale.
pub(crate) fn crop_bounds(
    target: PreviewRasterTarget,
    canvas: &CanvasSpec,
    view: (f64, f64),
) -> Option<(f32, f32, f32, f32)> {
    let (x, y, scale) = target.viewport()?;
    Some((
        (x / canvas.width * view.0) as f32,
        (y / canvas.height * view.1) as f32,
        (f64::from(target.width()) / scale / canvas.width * view.0) as f32,
        (f64::from(target.height()) / scale / canvas.height * view.1) as f32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves centering, scroll translation and device scale retain one document-space crop.
    /// # Panics
    /// Panics if valid Fit/crop mapping or device-scale placement changes.
    #[test]
    fn viewport_crop_pan_center_and_hidpi() {
        let canvas = CanvasSpec {
            width: 900.0,
            height: 620.0,
        };
        let crop = target(
            &canvas,
            (800.0, 500.0),
            (1800.0, 1240.0),
            (-200.0, -150.0),
            2,
            false,
        )
        .unwrap();
        assert_eq!((crop.width(), crop.height()), (1600, 1000));
        assert_eq!(crop.viewport(), Some((100.0, 75.0, 4.0)));
        assert_eq!(
            crop_bounds(crop, &canvas, (1800.0, 1240.0)),
            Some((200.0, 150.0, 800.0, 500.0))
        );
        let centered = target(
            &canvas,
            (1000.0, 800.0),
            (450.0, 310.0),
            (275.0, 245.0),
            1,
            false,
        )
        .unwrap();
        assert_eq!(centered.viewport(), Some((-550.0, -490.0, 0.5)));
        let fit = target(
            &canvas,
            (1000.0, 800.0),
            (1000.0, 800.0),
            (0.0, 0.0),
            2,
            true,
        )
        .unwrap();
        assert_eq!(
            (fit.width(), fit.height(), fit.viewport()),
            (2000, 1600, None)
        );
    }

    /// Proves fractional zoom contains the exact canvas aspect inside rounded widget dimensions.
    /// Raster placement uses the same uniform scale and subpixel centering at both device scales.
    /// # Panics
    /// Panics if valid fractional scale loses exact contained mapping.
    #[test]
    fn viewport_crop_fractional_zoom_preserves_uniform_physical_scale() {
        for (width, height) in [(900.0, 620.0), (1.0, 100.0), (100.0, 1.0)] {
            let canvas = CanvasSpec { width, height };
            for zoom in [0.33, 0.67] {
                let view = (canvas.width * zoom, canvas.height * zoom);
                let picture = (view.0.round().max(1.0), view.1.round().max(1.0));
                for device_scale in [1, 2] {
                    let crop = target(
                        &canvas,
                        (800.0, 500.0),
                        picture,
                        (100.0, 40.0),
                        device_scale,
                        false,
                    )
                    .unwrap();
                    let (_, _, physical_scale) = crop.viewport().unwrap();
                    let logical_scale = physical_scale / f64::from(device_scale);
                    let bounds = crop_bounds(crop, &canvas, view).unwrap();
                    // Snapshot receives contained canvas dimensions, so one crop pixel maps to
                    // exactly one device pixel on both axes, even with rounded widget padding.
                    let snapshot = (canvas.width * logical_scale, canvas.height * logical_scale);
                    let displayed_width = f64::from(bounds.2) * snapshot.0 / view.0;
                    let displayed_height = f64::from(bounds.3) * snapshot.1 / view.1;
                    assert!(
                        (displayed_width * f64::from(device_scale) - f64::from(crop.width())).abs()
                            < 0.001
                    );
                    assert!(
                        (displayed_height * f64::from(device_scale) - f64::from(crop.height()))
                            .abs()
                            < 0.001
                    );
                }
            }
        }
    }
}
