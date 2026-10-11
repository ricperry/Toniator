//! Viewer-only linear-light alpha presentation for model-backed GTK previews.

use crate::PreviewModel;
use toniator_domain::{Document,HalftoneChannelModel};
use toniator_engine::{RasterBackground, linear_to_srgb, srgb_to_linear};
#[cfg(test)]
use toniator_engine::RasterSurface;

/// Selects preview presentation backing from the actual projected channel model.
///
/// Missing model authority retains the viewer's RGB fallback. Ordinary PNG export has a
/// different unmodeled default and does not use this presentation policy.
const fn raster_background_for_model(model: Option<HalftoneChannelModel>) -> RasterBackground {
    match model {
        Some(HalftoneChannelModel::Rgb)|None => RasterBackground::OpaqueBlack,
        Some(HalftoneChannelModel::Cmyk) => RasterBackground::OpaqueWhite,
        Some(HalftoneChannelModel::SourceColorAlpha) => RasterBackground::Transparent,
    }
}

/// Reads the actual projected document model for a viewer-backed raster request.
pub(crate) fn raster_background_for_document(document:&Document)->RasterBackground {
    raster_background_for_model(document.channel_model())
}

/// Composites straight sRGBA bytes in place against the selected viewer backdrop.
///
/// RGB uses black, CMYK uses white, and SourceColorAlpha remains transparent for its gray CSS
/// backdrop. The input is presentation data only; callers must pass an owned buffer when the
/// bytes originate from canonical renderer output.
///
/// # Errors
/// Returns an alignment diagnostic when the byte slice does not contain complete RGBA pixels.
pub(crate) fn composite_rgba_in_place(rgba: &mut [u8], model: PreviewModel) -> Result<(), String> {
    if !rgba.len().is_multiple_of(4) {
        return Err("Viewer image bytes must contain complete RGBA pixels.".to_owned());
    }
    let backdrop = match model {
        PreviewModel::Rgb => Some(0.0),
        PreviewModel::Cmyk => Some(1.0),
        PreviewModel::SourceColorAlpha => None,
    };
    let Some(backdrop) = backdrop else {
        return Ok(());
    };
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = f64::from(pixel[3]) / 255.0;
        for channel in &mut pixel[..3] {
            let foreground = srgb_to_linear(f64::from(*channel) / 255.0);
            let composited = alpha * foreground + (1.0 - alpha) * backdrop;
            *channel = (linear_to_srgb(composited) * 255.0).round() as u8;
        }
        pixel[3] = 255;
    }
    Ok(())
}

/// Copies canonical straight-sRGBA pixels into the model-aware viewer representation.
///
/// The renderer-owned `RasterSurface` remains immutable and keeps its exact native bytes for
/// export, fallback caching, and later consumers.
///
/// # Errors
/// Returns the same pixel-alignment diagnostic as `composite_rgba_in_place`.
#[cfg(test)]
pub(crate) fn pixels_for_preview_surface(
    surface: &RasterSurface,
    model: PreviewModel,
) -> Result<Vec<u8>, String> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(surface.pixels().len())
        .map_err(|_| "Could not allocate viewer preview pixels.".to_owned())?;
    pixels.extend_from_slice(surface.pixels());
    composite_rgba_in_place(&mut pixels, model)?;
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks preview-only backing selection, including its historical missing-model RGB fallback.
    ///
    /// # Panics
    /// Panics if a projected model selects another presentation backing.
    #[test]
    fn projected_model_selects_preview_background() {
        assert_eq!(raster_background_for_model(None),RasterBackground::OpaqueBlack);
        assert_eq!(raster_background_for_model(Some(HalftoneChannelModel::Rgb)),RasterBackground::OpaqueBlack);
        assert_eq!(raster_background_for_model(Some(HalftoneChannelModel::Cmyk)),RasterBackground::OpaqueWhite);
        assert_eq!(raster_background_for_model(Some(HalftoneChannelModel::SourceColorAlpha)),RasterBackground::Transparent);
    }

    /// Verifies actual alpha selects black in linear light and yields an opaque RGB texture.
    ///
    /// # Panics
    /// Panics if transparent, partial, or opaque input pixels violate the viewer presentation.
    #[test]
    fn rgb_viewer_uses_linear_black_for_each_alpha_boundary() {
        let mut rgba = vec![
            230, 40, 170, 0, // fully transparent hidden color
            188, 128, 64, 128, // partial coverage
            17, 129, 240, 255, // fully opaque color
        ];

        composite_rgba_in_place(&mut rgba, PreviewModel::Rgb)
            .expect("complete RGBA pixels composite");

        assert_eq!(
            rgba,
            [
                0, 0, 0, 255, // alpha zero is exactly the black backdrop
                138, 93, 45, 255, // encoded from the linear-light composite
                17, 129, 240, 255, // opaque foreground remains unchanged
            ]
        );
    }

    /// Verifies actual alpha selects white in linear light and yields an opaque CMYK texture.
    ///
    /// # Panics
    /// Panics if transparent, partial, or opaque input pixels violate the viewer presentation.
    #[test]
    fn cmyk_viewer_uses_linear_white_for_each_alpha_boundary() {
        let mut rgba = vec![
            230, 40, 170, 0, // fully transparent hidden color
            188, 128, 64, 128, // partial coverage
            17, 129, 240, 255, // fully opaque color
        ];

        composite_rgba_in_place(&mut rgba, PreviewModel::Cmyk)
            .expect("complete RGBA pixels composite");

        assert_eq!(
            rgba,
            [
                255, 255, 255, 255, // alpha zero is exactly the white backdrop
                225, 204, 191, 255, // encoded from the linear-light composite
                17, 129, 240, 255, // opaque foreground remains unchanged
            ]
        );
    }

    /// Preserves SourceColorAlpha's native transparent RGBA for presentation over gray.
    ///
    /// # Panics
    /// Panics if any source color, alpha, or hidden RGB byte changes.
    #[test]
    fn source_color_alpha_viewer_preserves_transparent_bytes() {
        let original = [230, 40, 170, 0, 188, 128, 64, 128, 17, 129, 240, 255];
        let mut rgba = original;

        composite_rgba_in_place(&mut rgba, PreviewModel::SourceColorAlpha)
            .expect("complete RGBA pixels remain valid");

        assert_eq!(rgba, original);
    }

    /// Keeps canonical renderer bytes unchanged while constructing a model-aware viewer copy.
    ///
    /// # Panics
    /// Panics if the viewer transformation mutates the canonical RasterSurface or misses its
    /// requested backdrop.
    #[test]
    fn preview_surface_native_bytes_remain_unchanged() {
        let native = RasterSurface::new(1, 1, vec![188, 128, 64, 128])
            .expect("one RGBA pixel has a valid surface layout");
        let original = native.pixels().to_vec();

        let presented = pixels_for_preview_surface(&native, PreviewModel::Rgb)
            .expect("canonical surface converts to viewer bytes");

        assert_eq!(native.pixels(), original);
        assert_eq!(presented, [138, 93, 45, 255]);
    }

    /// Rejects a truncated pixel rather than silently dropping renderer or source bytes.
    ///
    /// # Panics
    /// Panics if malformed RGBA is accepted as a valid viewer image.
    #[test]
    fn viewer_rejects_incomplete_rgba_pixels() {
        let error = composite_rgba_in_place(&mut [10, 20, 30], PreviewModel::Rgb)
            .expect_err("truncated RGB pixel is rejected");

        assert_eq!(
            error,
            "Viewer image bytes must contain complete RGBA pixels."
        );
    }
}
