use crate::CancellationToken;
use crate::model::{Document, Ink, RenderVariant};
use crate::persistence::{atomic_write, atomic_write_cancellable};
#[cfg(test)]
use crate::render::render_document_output;
use crate::render::{render_document_output_cancellable, source_dimensions};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngExportOptions {
    pub width: u32,
    pub height: u32,
    /// `Document` is the normal path: PNG follows the saved Export
    /// Background. Overrides are explicit and never mutate the document.
    pub background: PngBackground,
    pub channel: Option<Ink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngBackground {
    Document,
    Transparent,
    White,
}

impl PngExportOptions {
    pub fn document_size(document: &Document) -> Result<Self> {
        let (width, height) = document_artboard(document)?;
        Ok(Self {
            width,
            height,
            background: PngBackground::Document,
            channel: None,
        })
    }
}

pub fn document_artboard(document: &Document) -> Result<(u32, u32)> {
    let source = source_dimensions(&document.source)?;
    let long_edge = match &document.render {
        RenderVariant::NativeBasicV1 => return Ok(source),
        RenderVariant::WebShapeV1 { settings } => settings.output_width.max(settings.output_height),
        RenderVariant::WebCurveV1 { settings } => settings.output_width.max(settings.output_height),
    };
    Ok(crate::model::aspect_locked_dimensions(
        source.0, source.1, long_edge,
    ))
}

pub fn png_bytes(document: &Document, options: PngExportOptions) -> Result<Vec<u8>> {
    png_bytes_cancellable(document, options, &CancellationToken::new())
}

pub fn png_bytes_cancellable(
    document: &Document,
    options: PngExportOptions,
    token: &CancellationToken,
) -> Result<Vec<u8>> {
    token.checkpoint()?;
    let (document_width, document_height) = document_artboard(document)?;
    let expected = crate::model::aspect_locked_dimensions(
        document_width,
        document_height,
        options.width.max(options.height),
    );
    anyhow::ensure!(
        (options.width, options.height) == expected,
        "PNG dimensions must preserve the source artwork aspect ratio"
    );
    let image = match options.background {
        PngBackground::Document => crate::render::render_document_export_cancellable(
            document,
            options.width,
            options.height,
            options.channel,
            token,
        )?,
        PngBackground::Transparent => render_document_output_cancellable(
            document,
            options.width,
            options.height,
            false,
            options.channel,
            token,
        )?,
        PngBackground::White => render_document_output_cancellable(
            document,
            options.width,
            options.height,
            true,
            options.channel,
            token,
        )?,
    };
    token.checkpoint()?;
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .context("could not encode PNG output")?;
    token.checkpoint()?;
    Ok(encoded.into_inner())
}

pub fn export_png(path: &Path, document: &Document, options: PngExportOptions) -> Result<()> {
    let bytes = png_bytes(document, options)?;
    atomic_write(path, &bytes)
}

pub fn export_png_cancellable(
    path: &Path,
    document: &Document,
    options: PngExportOptions,
    token: &CancellationToken,
) -> Result<()> {
    let bytes = png_bytes_cancellable(document, options, token)?;
    atomic_write_cancellable(path, &bytes, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ClosedShapePath, ShapePoint, SourceArtwork, ValueMode, WebCurveSettings, WebShape,
        WebShapeSettings,
    };
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

    fn cubic_shape_document() -> Document {
        let black = RgbaImage::from_pixel(8, 6, Rgba([0, 0, 0, 255]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(black)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let mut document = Document::new(SourceArtwork {
            name: "black.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from(bytes.into_inner()),
        });
        let mut settings = WebShapeSettings {
            output_width: 120,
            output_height: 80,
            value_mode: ValueMode::SingleChannel,
            single_channel: Ink::Black,
            shared_shape: WebShape::UserDefined,
            ..Default::default()
        };
        let mut path = ClosedShapePath::from_polygon(&settings.custom_nodes);
        path.anchors[0].outgoing = ShapePoint { x: 0.1, y: -0.7 };
        path.anchors[1].incoming = ShapePoint { x: 0.2, y: -0.1 };
        settings.custom_shape_path = Some(path);
        for ink in Ink::ALL {
            settings.channels.get_mut(ink).enabled = ink == Ink::Black;
        }
        document.render = RenderVariant::WebShapeV1 {
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
                height: 180,
                background: PngBackground::Transparent,
                channel: None,
            },
        )
        .unwrap();
        let image = ImageReader::new(Cursor::new(transparent))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(image.dimensions(), (240, 180));
        let decoded = image.to_rgba8();
        assert!(decoded.pixels().any(|pixel| pixel.0[3] == 0));
        assert_eq!(
            decoded,
            render_document_output(&document, 240, 180, false, None).unwrap()
        );

        let opaque = png_bytes(
            &document,
            PngExportOptions {
                width: 120,
                height: 90,
                background: PngBackground::White,
                channel: Some(Ink::Black),
            },
        )
        .unwrap();
        let image = image::load_from_memory(&opaque).unwrap().to_rgba8();
        assert!(image.pixels().all(|pixel| pixel.0[3] == 255));
    }

    #[test]
    fn nonstraight_cubic_png_decodes_to_canonical_preview_pixels() {
        let document = cubic_shape_document();
        let options = PngExportOptions {
            background: PngBackground::White,
            ..PngExportOptions::document_size(&document).unwrap()
        };
        let decoded = image::load_from_memory(&png_bytes(&document, options).unwrap())
            .unwrap()
            .to_rgba8();
        let canonical = render_document_output(&document, 120, 90, true, None).unwrap();
        assert_eq!(decoded, canonical);
        assert!(
            decoded
                .pixels()
                .any(|pixel| pixel.0 != [255, 255, 255, 255])
        );
    }

    #[test]
    fn unsafe_pixel_count_is_rejected_before_allocation() {
        let error = png_bytes(
            &curve_document(),
            PngExportOptions {
                width: 32_000,
                height: 24_000,
                background: PngBackground::White,
                channel: None,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("64 megapixel"));
    }

    #[test]
    fn mismatched_custom_dimensions_are_rejected_before_destination_mutation() {
        let document = curve_document();
        let error = png_bytes(
            &document,
            PngExportOptions {
                width: 120,
                height: 80,
                background: PngBackground::White,
                channel: None,
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("preserve the source artwork aspect ratio")
        );
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
                    background: PngBackground::White,
                    channel: None,
                },
            )
            .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), b"keep me");
    }

    #[test]
    fn document_background_controls_default_png_without_mutating_document() {
        let mut document = curve_document();
        document.appearance.export_background = crate::model::ExportBackground::None;
        let options = PngExportOptions::document_size(&document).unwrap();
        let transparent = image::load_from_memory(&png_bytes(&document, options).unwrap())
            .unwrap()
            .to_rgba8();
        assert!(transparent.pixels().any(|pixel| pixel[3] < 255));

        document.appearance.export_background = crate::model::ExportBackground::Color {
            color: crate::model::RgbaColor::opaque(12, 34, 56),
        };
        let flattened = image::load_from_memory(&png_bytes(&document, options).unwrap())
            .unwrap()
            .to_rgba8();
        assert!(flattened.pixels().all(|pixel| pixel[3] == 255));
        assert!(flattened.pixels().any(|pixel| pixel.0 == [12, 34, 56, 255]));
        let appearance = document.appearance;
        let override_png = png_bytes(
            &document,
            PngExportOptions {
                background: PngBackground::Transparent,
                ..options
            },
        )
        .unwrap();
        assert!(
            image::load_from_memory(&override_png)
                .unwrap()
                .to_rgba8()
                .pixels()
                .any(|pixel| pixel[3] < 255)
        );
        assert_eq!(
            document.appearance, appearance,
            "export overrides do not mutate the document"
        );
    }
}
