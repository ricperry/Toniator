use crate::model::{Document, Ink, RenderVariant};
use crate::persistence::atomic_write;
use crate::render::{render_document_output, source_dimensions};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngExportOptions {
    pub width: u32,
    pub height: u32,
    pub white_background: bool,
    pub channel: Option<Ink>,
}

impl PngExportOptions {
    pub fn document_size(document: &Document) -> Result<Self> {
        let (width, height) = document_artboard(document)?;
        Ok(Self {
            width,
            height,
            white_background: true,
            channel: None,
        })
    }
}

pub fn document_artboard(document: &Document) -> Result<(u32, u32)> {
    match &document.render {
        RenderVariant::NativeBasicV1 => source_dimensions(&document.source),
        RenderVariant::WebShapeV1 { settings } => {
            Ok((settings.output_width, settings.output_height))
        }
        RenderVariant::WebCurveV1 { settings } => {
            Ok((settings.output_width, settings.output_height))
        }
    }
}

pub fn png_bytes(document: &Document, options: PngExportOptions) -> Result<Vec<u8>> {
    let image = render_document_output(
        document,
        options.width,
        options.height,
        options.white_background,
        options.channel,
    )?;
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .context("could not encode PNG output")?;
    Ok(encoded.into_inner())
}

pub fn export_png(path: &Path, document: &Document, options: PngExportOptions) -> Result<()> {
    let bytes = png_bytes(document, options)?;
    atomic_write(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SourceArtwork, WebCurveSettings};
    use image::{GenericImageView, ImageReader, Rgba, RgbaImage};

    fn source_png() -> Vec<u8> {
        let image = RgbaImage::from_pixel(8, 6, Rgba([255, 255, 255, 255]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn curve_document() -> Document {
        let mut document = Document::new(SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from(source_png()),
        });
        let mut settings = WebCurveSettings {
            output_width: 120,
            output_height: 80,
            ..Default::default()
        };
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = ink == Ink::Black;
        }
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(settings),
        };
        document
    }

    #[test]
    fn png_has_exact_custom_dimensions_and_real_transparency() {
        let document = curve_document();
        let transparent = png_bytes(
            &document,
            PngExportOptions {
                width: 240,
                height: 100,
                white_background: false,
                channel: None,
            },
        )
        .unwrap();
        let image = ImageReader::new(Cursor::new(transparent))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(image.dimensions(), (240, 100));
        let decoded = image.to_rgba8();
        assert!(decoded.pixels().any(|pixel| pixel.0[3] == 0));
        assert_eq!(
            decoded,
            render_document_output(&document, 240, 100, false, None).unwrap()
        );

        let opaque = png_bytes(
            &document,
            PngExportOptions {
                width: 120,
                height: 80,
                white_background: true,
                channel: Some(Ink::Black),
            },
        )
        .unwrap();
        let image = image::load_from_memory(&opaque).unwrap().to_rgba8();
        assert!(image.pixels().all(|pixel| pixel.0[3] == 255));
    }

    #[test]
    fn unsafe_pixel_count_is_rejected_before_allocation() {
        let error = png_bytes(
            &curve_document(),
            PngExportOptions {
                width: 32_000,
                height: 32_000,
                white_background: true,
                channel: None,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("64 megapixel"));
    }

    #[test]
    fn failed_export_preserves_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.png");
        std::fs::write(&path, b"keep me").unwrap();
        assert!(
            export_png(
                &path,
                &curve_document(),
                PngExportOptions {
                    width: 32_000,
                    height: 32_000,
                    white_background: true,
                    channel: None,
                },
            )
            .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), b"keep me");
    }
}
