//! Presentation-only viewport transforms; canonical canvas and evaluator geometry stay unchanged.

use toniator_domain::CanvasSpec;
use toniator_engine::PreviewRasterTarget;

/// Resolves contain layout and a scrolled Picture origin into physical raster intent.
///
/// Picture coordinates include GTK's current scroll translation. Fit retains the complete fitted
/// scene; manual zoom requests only the visible page, including transparent canvas margins.
pub(crate) fn target(
    canvas: &CanvasSpec,
    page: (f64, f64),
    picture: (f64, f64),
    picture_origin: (f64, f64),
    device_scale: i32,
    fit: bool,
) -> Option<PreviewRasterTarget> {
    let scale = f64::from(device_scale);
    if device_scale <= 0
        || [
            page.0,
            page.1,
            picture.0,
            picture.1,
            canvas.width,
            canvas.height,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || !picture_origin.0.is_finite()
        || !picture_origin.1.is_finite()
    {
        return None;
    }
    let width = (page.0 * scale).round();
    let height = (page.1 * scale).round();
    if width > f64::from(u32::MAX) || height > f64::from(u32::MAX) {
        return None;
    }
    if fit {
        return PreviewRasterTarget::new(width as u32, height as u32).ok();
    }
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
    .ok()
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
