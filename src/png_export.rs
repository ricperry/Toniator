use crate::CancellationToken;
use crate::model::{Document, Ink, RenderVariant};
use crate::pattern::CanonicalPatternOutput;
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
    let document = document.projected_for_render()?;
    let source = source_dimensions(&document.source)?;
    let long_edge = match &document.render {
        RenderVariant::NativeBasicV1 => return Ok(source),
        RenderVariant::WebShapeV1 { settings } => settings.output_width.max(settings.output_height),
        RenderVariant::WebCurveV1 { settings } => settings.output_width.max(settings.output_height),
        RenderVariant::WeightedVoronoiCanonicalV1 => return Ok(source),
    };
    Ok(crate::model::aspect_locked_dimensions(
        source.0, source.1, long_edge,
    ))
}

pub fn png_bytes(document: &Document, options: PngExportOptions) -> Result<Vec<u8>> {
    png_bytes_cancellable(document, options, &CancellationToken::new())
}

/// Encode an already-generated canonical output without regenerating the
/// pattern. Document export uses the same renderer through
/// `render_document_output_cancellable`; this seam also lets synthetic
/// region/network fixtures prove PNG parity directly.
pub fn canonical_pattern_png_bytes(
    output: &CanonicalPatternOutput,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
) -> Result<Vec<u8>> {
    canonical_pattern_png_bytes_cancellable(
        output,
        width,
        height,
        white_background,
        channel,
        &CancellationToken::new(),
    )
}

pub fn canonical_pattern_png_bytes_cancellable(
    output: &CanonicalPatternOutput,
    width: u32,
    height: u32,
    white_background: bool,
    channel: Option<Ink>,
    token: &CancellationToken,
) -> Result<Vec<u8>> {
    token.checkpoint()?;
    let image = crate::render::render_canonical_pattern_output_cancellable(
        output,
        width,
        height,
        white_background,
        channel,
        token,
    )?;
    token.checkpoint()?;
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .context("could not encode canonical PNG output")?;
    token.checkpoint()?;
    Ok(encoded.into_inner())
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
        ClosedShapePath, DocumentAppearance, DocumentEditor, ExportBackground, PreviewSurface,
        RgbaColor, ShapePoint, SourceArtwork, ValueMode, WebCurveSettings, WebShape,
        WebShapeSettings,
    };
    use crate::pattern::PatternId;
    use crate::preset::parse_treatment;
    use crate::render::{composite_export_background, composite_preview, render_document_preview};
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
            .pattern_state
            .select_pattern(crate::pattern::PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
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
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        document
    }

    fn c1_fixture_source() -> SourceArtwork {
        // Match the fixtures' 900 × 620 aspect ratio so parity checks only
        // exercise current pattern output, not an aspect-normalization edit.
        let image = RgbaImage::from_fn(45, 31, |x, y| {
            Rgba([
                (20 + x * 4) as u8,
                (40 + y * 3) as u8,
                (180 - ((x + y) % 20) * 7) as u8,
                255,
            ])
        });
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        SourceArtwork {
            name: "c1-fixture-source.png".into(),
            media_type: "image/png".into(),
            bytes: bytes.into_inner().into(),
        }
    }

    fn contradictory_adapter_for(pattern: PatternId) -> RenderVariant {
        match pattern {
            PatternId::COMPATIBILITY_SHAPES_V1 => RenderVariant::WebCurveV1 {
                settings: Box::new(WebCurveSettings {
                    output_width: 19,
                    output_height: 13,
                    long_edge_cells: 2.0,
                    max_mark: 91.0,
                    ..Default::default()
                }),
            },
            PatternId::COMPATIBILITY_CURVES_V1 => RenderVariant::WebShapeV1 {
                settings: Box::new(WebShapeSettings {
                    output_width: 17,
                    output_height: 11,
                    long_edge_cells: 2.0,
                    grid_scale: 77.0,
                    polygon_sides: 3,
                    ..Default::default()
                }),
            },
            PatternId::WEIGHTED_VORONOI_V1 => RenderVariant::NativeBasicV1,
        }
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
    fn c3a_c1_fixtures_preview_and_png_share_authoritative_pattern_output() {
        for (name, bytes, selected) in [
            (
                "polygon-six",
                include_bytes!("../assets/presets/Polygon Six.tntr").as_slice(),
                PatternId::COMPATIBILITY_SHAPES_V1,
            ),
            (
                "motif-ladder",
                include_bytes!("../assets/presets/Motif Ladder.tntr").as_slice(),
                PatternId::COMPATIBILITY_CURVES_V1,
            ),
        ] {
            let mut fixture_editor = DocumentEditor::new(Document::new(c1_fixture_source()));
            let candidate = parse_treatment(bytes, (900, 620))
                .unwrap_or_else(|error| panic!("{name} did not parse: {error:#}"))
                .candidate_for(fixture_editor.document())
                .unwrap_or_else(|error| panic!("{name} did not apply: {error:#}"));
            assert!(fixture_editor.replace_with_preset_candidate(candidate));
            assert_eq!(
                fixture_editor
                    .document()
                    .pattern_state
                    .selected_pattern_id(),
                Some(selected)
            );
            assert!(fixture_editor.set_appearance(DocumentAppearance {
                preview_surface: PreviewSurface::Color {
                    color: RgbaColor::opaque(237, 241, 248),
                },
                export_background: ExportBackground::None,
            }));
            let canonical = fixture_editor.document().clone();
            let options = PngExportOptions::document_size(&canonical).unwrap();
            assert_eq!((options.width, options.height), (900, 620));

            // The active facade is deliberately the wrong family with
            // incompatible dimensions and parameters. Preview and every PNG
            // route must still derive the same raw pattern from pattern_state.
            let mut contradictory = canonical.clone();
            contradictory.render = contradictory_adapter_for(selected);
            let before_render = contradictory.clone();
            let raw =
                render_document_output(&contradictory, options.width, options.height, false, None)
                    .unwrap();
            assert_eq!(
                contradictory, before_render,
                "{name} rendering is read-only"
            );
            assert_eq!(
                raw,
                render_document_output(&canonical, options.width, options.height, false, None,)
                    .unwrap(),
                "{name} active adapter cannot alter raw pattern output"
            );

            let preview = render_document_preview(&contradictory, options.width, 1)
                .unwrap()
                .image;
            assert_eq!(
                preview,
                composite_preview(raw.clone(), contradictory.appearance),
                "{name} preview must compose the authoritative raw pattern"
            );
            let transparent_options = PngExportOptions {
                background: PngBackground::Transparent,
                ..options
            };
            let transparent_png = png_bytes(&contradictory, transparent_options).unwrap();
            assert_eq!(
                transparent_png,
                png_bytes(&contradictory, transparent_options).unwrap(),
                "{name} transparent PNG encoding is deterministic"
            );
            let transparent = image::load_from_memory(&transparent_png)
                .unwrap()
                .to_rgba8();
            assert_eq!(transparent.dimensions(), (options.width, options.height));
            assert_eq!(transparent, raw, "{name} PNG shares preview's raw pattern");
            assert!(
                transparent.pixels().any(|pixel| pixel[3] < 255),
                "{name} transparent PNG retains transparent artwork gaps"
            );

            let document_options = PngExportOptions {
                background: PngBackground::Document,
                ..options
            };
            let document_none = png_bytes(&contradictory, document_options).unwrap();
            assert_eq!(
                image::load_from_memory(&document_none).unwrap().to_rgba8(),
                raw,
                "{name} document PNG remains transparent when Export Background is None"
            );
            let preview_before_export_change = preview.clone();
            contradictory.appearance.preview_surface = PreviewSurface::Checkerboard;
            assert_eq!(
                png_bytes(&contradictory, document_options).unwrap(),
                document_none,
                "{name} Preview Surface cannot enter document PNG bytes"
            );
            assert_ne!(
                render_document_preview(&contradictory, options.width, 2)
                    .unwrap()
                    .image,
                preview_before_export_change,
                "{name} preview surface remains a visible preview-only choice"
            );

            let export_background = ExportBackground::Color {
                color: RgbaColor::opaque(12, 34, 56),
            };
            contradictory.appearance.export_background = export_background;
            let preview_before_document_background =
                render_document_preview(&contradictory, options.width, 3)
                    .unwrap()
                    .image;
            let document_color = png_bytes(&contradictory, document_options).unwrap();
            assert_eq!(
                document_color,
                png_bytes(&contradictory, document_options).unwrap(),
                "{name} document-background PNG encoding is deterministic"
            );
            let exported = image::load_from_memory(&document_color).unwrap().to_rgba8();
            assert_eq!(
                exported,
                composite_export_background(raw.clone(), export_background),
                "{name} document PNG composes its saved Export Background over the same raw pattern"
            );
            assert!(exported.pixels().all(|pixel| pixel[3] == 255));
            assert_eq!(
                render_document_preview(&contradictory, options.width, 4)
                    .unwrap()
                    .image,
                preview_before_document_background,
                "{name} Export Background cannot enter the preview"
            );

            // Create the CMYK cache from the contradictory active facade,
            // then corrupt that inactive cache too. Returning to CMYK must
            // rebuild it from cached typed authority before preview/PNG use it.
            let mut cache_editor = DocumentEditor::new(contradictory);
            assert!(cache_editor.set_output_mode(crate::model::OutputMode::RgbScreen));
            let mut inactive_contradiction = cache_editor.document().clone();
            inactive_contradiction
                .inactive_cmyk
                .as_mut()
                .expect("CMYK treatment is cached while RGB is active")
                .render = contradictory_adapter_for(selected);
            let mut cache_editor = DocumentEditor::new(inactive_contradiction);
            assert!(cache_editor.set_output_mode(crate::model::OutputMode::CmykInks));
            let restored = cache_editor.document().clone();
            assert_eq!(restored.pattern_state, canonical.pattern_state);
            assert_eq!(
                render_document_output(&restored, options.width, options.height, false, None,)
                    .unwrap(),
                raw,
                "{name} inactive adapter cannot alter restored raw pattern output"
            );
            assert_eq!(
                image::load_from_memory(&png_bytes(&restored, document_options).unwrap())
                    .unwrap()
                    .to_rgba8(),
                exported,
                "{name} inactive adapter cannot alter restored document PNG"
            );
            assert_eq!(
                render_document_preview(&restored, options.width, 5)
                    .unwrap()
                    .image,
                preview_before_document_background,
                "{name} inactive adapter cannot alter restored preview"
            );
        }
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
    fn document_png_uses_saved_export_background_and_none_remains_transparent() {
        let mut document = curve_document();
        document.appearance.export_background = crate::model::ExportBackground::None;
        document.appearance.preview_surface = crate::model::PreviewSurface::Color {
            color: crate::model::RgbaColor::opaque(18, 28, 42),
        };
        let options = PngExportOptions::document_size(&document).unwrap();
        let transparent_before = png_bytes(
            &document,
            PngExportOptions {
                background: PngBackground::Document,
                ..options
            },
        )
        .unwrap();
        document.appearance.preview_surface = crate::model::PreviewSurface::Checkerboard;
        let transparent_after = png_bytes(
            &document,
            PngExportOptions {
                background: PngBackground::Document,
                ..options
            },
        )
        .unwrap();
        assert_eq!(transparent_before, transparent_after);

        let transparent = image::load_from_memory(&transparent_after)
            .unwrap()
            .to_rgba8();
        assert!(transparent.pixels().any(|pixel| pixel[3] < 255));

        document.appearance.export_background = crate::model::ExportBackground::Color {
            color: crate::model::RgbaColor::opaque(12, 34, 56),
        };
        let flattened = image::load_from_memory(
            &png_bytes(
                &document,
                PngExportOptions {
                    background: PngBackground::Document,
                    ..options
                },
            )
            .unwrap(),
        )
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
