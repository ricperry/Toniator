use crate::cancel::CancellationToken;
use crate::model::{
    DOCUMENT_FORMAT, DOCUMENT_VERSION, Document, DocumentAppearance, ExportBackground, OutputMode,
    PreviewSurface, RenderVariant, RgbaColor, Settings, SourceArtwork, new_document_id,
};
use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn document_json(document: &Document) -> Result<Vec<u8>> {
    let mut canonical = document.clone();
    if let Ok((width, height)) = crate::render::source_dimensions(&canonical.source) {
        canonical.normalize_canvas_aspect(width, height);
    }
    canonical.validate()?;
    let mut bytes =
        serde_json::to_vec_pretty(&canonical).context("could not serialize document")?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn save_document_atomic(path: &Path, document: &Document) -> Result<()> {
    let bytes = document_json(document)?;
    atomic_write(path, &bytes)
}

pub fn save_document_atomic_cancellable(
    path: &Path,
    document: &Document,
    token: &CancellationToken,
) -> Result<()> {
    let bytes = document_json(document)?;
    atomic_write_cancellable(path, &bytes, token)
}

pub fn load_document(path: &Path) -> Result<Document> {
    Ok(load_document_with_migration(path)?.document)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocumentMigration {
    pub canvas_aspect: bool,
    pub crosshatch_treatment: bool,
    pub appearance: bool,
    pub output_mode: bool,
}

pub struct LoadedDocument {
    pub document: Document,
    pub migration: DocumentMigration,
}

pub fn load_document_with_migration(path: &Path) -> Result<LoadedDocument> {
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let header: DocumentHeader = serde_json::from_slice(&bytes)
        .with_context(|| format!("could not parse {}", path.display()))?;
    anyhow::ensure!(header.format == DOCUMENT_FORMAT, "not a Toniator document");
    let mut document = match header.version {
        1 => {
            let legacy: DocumentV1 = serde_json::from_slice(&bytes)
                .with_context(|| format!("could not parse {}", path.display()))?;
            Document {
                format: DOCUMENT_FORMAT.into(),
                version: DOCUMENT_VERSION,
                document_id: legacy.document_id,
                source: legacy.source,
                settings: legacy.settings.sanitized(),
                appearance: legacy_appearance(),
                output_mode: OutputMode::CmykInks,
                render: RenderVariant::NativeBasicV1,
                saved_web_shape: None,
                saved_web_curve: None,
                inactive_cmyk: None,
                inactive_rgb: None,
            }
        }
        2 => {
            let legacy: DocumentV2 = serde_json::from_slice(&bytes)
                .with_context(|| format!("could not parse {}", path.display()))?;
            Document {
                format: DOCUMENT_FORMAT.into(),
                version: DOCUMENT_VERSION,
                document_id: legacy.document_id,
                source: legacy.source,
                settings: legacy.settings.sanitized(),
                appearance: legacy_appearance(),
                output_mode: OutputMode::CmykInks,
                render: legacy.render,
                saved_web_shape: None,
                saved_web_curve: None,
                inactive_cmyk: None,
                inactive_rgb: None,
            }
        }
        3 => {
            let legacy: DocumentV3 = serde_json::from_slice(&bytes)
                .with_context(|| format!("could not parse {}", path.display()))?;
            Document {
                format: legacy.format,
                version: DOCUMENT_VERSION,
                document_id: legacy.document_id,
                source: legacy.source,
                settings: legacy.settings,
                appearance: legacy_appearance(),
                output_mode: OutputMode::CmykInks,
                render: legacy.render,
                saved_web_shape: legacy.saved_web_shape,
                saved_web_curve: legacy.saved_web_curve,
                inactive_cmyk: None,
                inactive_rgb: None,
            }
        }
        4 => {
            let mut legacy: Document = serde_json::from_slice(&bytes)
                .with_context(|| format!("could not parse {}", path.display()))?;
            legacy.version = DOCUMENT_VERSION;
            legacy.output_mode = OutputMode::CmykInks;
            legacy.inactive_cmyk = None;
            legacy.inactive_rgb = None;
            legacy
        }
        DOCUMENT_VERSION => serde_json::from_slice(&bytes)
            .with_context(|| format!("could not parse {}", path.display()))?,
        version => anyhow::bail!("unsupported Toniator document version {version}"),
    };
    let canvas_aspect =
        if let Ok((width, height)) = crate::render::source_dimensions(&document.source) {
            document.normalize_canvas_aspect(width, height)
        } else {
            false
        };
    let crosshatch_treatment = document.normalize_crosshatch_treatment();
    document.validate()?;
    document.settings = document.settings.sanitized();
    Ok(LoadedDocument {
        document,
        migration: DocumentMigration {
            canvas_aspect,
            crosshatch_treatment,
            appearance: header.version < 4,
            output_mode: header.version < 5,
        },
    })
}

#[derive(serde::Deserialize)]
struct DocumentHeader {
    format: String,
    version: u32,
}

#[derive(serde::Deserialize)]
struct DocumentV1 {
    #[serde(default = "legacy_document_id")]
    document_id: String,
    source: SourceArtwork,
    settings: Settings,
}

#[derive(serde::Deserialize)]
struct DocumentV2 {
    #[serde(default = "legacy_document_id")]
    document_id: String,
    source: SourceArtwork,
    settings: Settings,
    #[serde(default)]
    render: RenderVariant,
}

#[derive(serde::Deserialize)]
struct DocumentV3 {
    format: String,
    #[allow(dead_code)]
    version: u32,
    #[serde(default = "legacy_document_id")]
    document_id: String,
    source: SourceArtwork,
    settings: Settings,
    #[serde(default)]
    render: RenderVariant,
    #[serde(default)]
    saved_web_shape: Option<Box<crate::model::WebShapeSettings>>,
    #[serde(default)]
    saved_web_curve: Option<Box<crate::model::WebCurveSettings>>,
}

fn legacy_appearance() -> DocumentAppearance {
    // v1–v3 always composited preview and exports on white paper. Keep that
    // visible and editable rather than hiding it in compatibility code.
    DocumentAppearance {
        preview_surface: PreviewSurface::Color {
            color: RgbaColor::WHITE,
        },
        export_background: ExportBackground::Color {
            color: RgbaColor::WHITE,
        },
    }
}

fn legacy_document_id() -> String {
    new_document_id()
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_cancellable(path, bytes, &CancellationToken::new())
}

pub fn atomic_write_cancellable(
    path: &Path,
    bytes: &[u8],
    token: &CancellationToken,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("could not create {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("destination has no valid file name")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));

    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .with_context(|| format!("could not create {}", temporary.display()))?;
        for chunk in bytes.chunks(64 * 1024) {
            token.checkpoint()?;
            file.write_all(chunk)
                .with_context(|| format!("could not write {}", temporary.display()))?;
        }
        token.checkpoint()?;
        file.flush()?;
        file.sync_all()?;
        token.begin_commit()?;
        fs::rename(&temporary, path)
            .with_context(|| format!("could not replace {}", path.display()))?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

pub fn recovery_path() -> PathBuf {
    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    state_home.join("toniator").join("recovery.toniator")
}

/// Removes recovery only when it belongs to the document being saved or discarded.
pub fn clear_recovery_if_matches(path: &Path, document_id: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let recovery = load_document(path)?;
    if recovery.document_id != document_id {
        return Ok(false);
    }
    fs::remove_file(path).with_context(|| format!("could not remove {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancellationToken;
    use crate::model::{Document, SourceArtwork};

    #[test]
    fn cancelled_atomic_write_preserves_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("destination.bin");
        std::fs::write(&path, b"keep").unwrap();
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(atomic_write_cancellable(&path, &[7; 200_000], &token).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn v1_document_migrates_to_native_basic_without_changing_marks() {
        use crate::render::{decode_source, generate_document_marks, generate_marks};
        use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
        use std::io::Cursor;

        let image = RgbImage::from_fn(12, 8, |x, y| Rgb([(x * 17) as u8, (y * 29) as u8, 80]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png.get_ref());
        let legacy = format!(
            r#"{{"format":"toniator-document","version":1,"document_id":"legacy","source":{{"name":"old.png","media_type":"image/png","bytes":"{encoded}"}},"settings":{{"treatment":"squares","detail":73.0,"coverage":101.0,"contrast":123.0,"angle":22.0}}}}"#
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.toniator");
        std::fs::write(&path, legacy).unwrap();
        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded.version, DOCUMENT_VERSION);
        assert_eq!(loaded.render, RenderVariant::NativeBasicV1);
        assert_eq!(loaded.appearance, legacy_appearance());
        let decoded = decode_source(&loaded.source, 2400).unwrap();
        assert_eq!(
            generate_document_marks(&loaded).unwrap(),
            generate_marks(&decoded, loaded.settings)
        );
    }

    #[test]
    fn atomic_document_roundtrip_embeds_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("roundtrip.toniator");
        let document = Document::new(SourceArtwork {
            name: "source.webp".into(),
            media_type: "image/webp".into(),
            bytes: std::sync::Arc::from([0, 1, 2, 3, 254, 255]),
        });
        let mut document = document;
        document.settings.treatment = crate::model::Treatment::Lines;
        document.settings.detail = 73.0;
        document.settings.angle = 120.0;
        save_document_atomic(&path, &document).unwrap();
        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded, document);
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"format\": \"toniator-document\""));
        assert!(text.contains("AAECA/7/"));
    }

    #[test]
    fn legacy_shape_crosshatch_migrates_to_curve_without_losing_output_state() {
        use crate::model::{
            ClosedShapePath, RenderVariant, ShapePoint, ValueMode, WebShape, WebShapeSettings,
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("web-shape.toniator");
        let mut document = Document::new(SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([9, 8, 7]),
        });
        let mut settings = WebShapeSettings {
            output_width: 900,
            output_height: 620,
            long_edge_cells: 92.0,
            grid_scale: 76.0,
            min_mark: 4.0,
            max_mark: 90.0,
            value_mode: ValueMode::CrosshatchLuminance,
            use_shared_mark: false,
            ..Default::default()
        };
        settings.channels.c.enabled = false;
        let mut custom = ClosedShapePath::from_polygon(&settings.custom_nodes);
        custom.anchors[0].outgoing = ShapePoint { x: 0.1, y: -0.7 };
        custom.anchors[1].incoming = ShapePoint { x: 0.2, y: -0.1 };
        settings.custom_shape_path = Some(custom);
        settings.channels.m.color = "#123456".into();
        settings.channels.m.grid_rotation = 73.5;
        settings.channels.m.grid_pivot_x = -155.49;
        settings.channels.m.offset_y = -19.49;
        settings.channels.m.resolution_scale = 2.0;
        settings.channels.m.opacity = 0.37;
        settings.channels.m.shape = WebShape::Pentagon;
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(settings),
        };
        save_document_atomic(&path, &document).unwrap();
        let loaded = load_document_with_migration(&path).unwrap();
        assert!(loaded.migration.crosshatch_treatment);
        let RenderVariant::WebCurveV1 { settings } = loaded.document.render else {
            panic!("legacy dot crosshatch must become curve geometry")
        };
        assert_eq!(settings.value_mode, ValueMode::CrosshatchLuminance);
        assert_eq!(settings.channels.m.color, "#123456");
        assert_eq!(settings.channels.m.offset_y, -19.49);
        assert_eq!(settings.channels.m.resolution_scale, 2.0);
        assert_eq!(settings.channels.m.opacity, 0.37);
    }

    #[test]
    fn v2_document_migrates_to_v3_without_losing_web_shape_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v2.toniator");
        let mut document = Document::new(SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([9, 8, 7]),
        });
        document.render = RenderVariant::WebShapeV1 {
            settings: Box::new(crate::model::WebShapeSettings::default()),
        };
        let mut value = serde_json::to_value(&document).unwrap();
        value["version"] = serde_json::json!(2);
        value.as_object_mut().unwrap().remove("saved_web_shape");
        value.as_object_mut().unwrap().remove("saved_web_curve");
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded.version, DOCUMENT_VERSION);
        assert_eq!(loaded.render, document.render);
        assert_eq!(loaded.appearance, legacy_appearance());
        assert!(loaded.saved_web_shape.is_none());
        assert!(loaded.saved_web_curve.is_none());
    }

    #[test]
    fn web_curve_document_roundtrips_with_inactive_treatment_cache() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("curve.toniator");
        let mut document = Document::new(SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([9, 8, 7]),
        });
        let mut curve = crate::model::WebCurveSettings {
            layout: crate::model::CurveLayout::MotifPattern,
            ..Default::default()
        };
        curve.channels.c.curve_scale = 47.0;
        curve.channels.c.motif_coverage = crate::model::MotifCoverage::Manual;
        curve.channels.c.tile_count = 3;
        curve.channels.c.stack_count = 2;
        curve.channels.c.stack_spacing = 18.0;
        curve.channels.c.alternate_tile_transform = crate::model::AlternateTileTransform::Rotate180;
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(curve),
        };
        document.saved_web_shape = Some(Box::new(crate::model::WebShapeSettings::default()));
        save_document_atomic(&path, &document).unwrap();
        assert_eq!(load_document(&path).unwrap(), document);
    }

    #[test]
    fn v3_migrates_to_visible_white_appearance_and_v4_roundtrips_rgba() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v3.toniator");
        let document = Document::new(SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([9, 8, 7]),
        });
        let mut legacy = serde_json::to_value(&document).unwrap();
        legacy["version"] = serde_json::json!(3);
        legacy.as_object_mut().unwrap().remove("appearance");
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let migrated = load_document_with_migration(&path).unwrap();
        assert!(migrated.migration.appearance);
        assert_eq!(migrated.document.appearance, legacy_appearance());

        let mut current = migrated.document;
        current.appearance = DocumentAppearance {
            preview_surface: PreviewSurface::Checkerboard,
            export_background: ExportBackground::Color {
                color: RgbaColor {
                    red: 3,
                    green: 4,
                    blue: 5,
                    alpha: 99,
                },
            },
        };
        save_document_atomic(&path, &current).unwrap();
        let reopened = load_document_with_migration(&path).unwrap();
        assert_eq!(reopened.document, current);
        assert_eq!(reopened.migration, DocumentMigration::default());
    }

    #[test]
    fn mismatched_recovery_is_never_removed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recovery.toniator");
        let document = Document::new(SourceArtwork {
            name: "original.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1]),
        });
        save_document_atomic(&path, &document).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(!clear_recovery_if_matches(&path, "another-document").unwrap());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn discard_removes_only_matching_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recovery.toniator");
        let document = Document::new(SourceArtwork {
            name: "current.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1]),
        });
        save_document_atomic(&path, &document).unwrap();
        assert!(clear_recovery_if_matches(&path, &document.document_id).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn undo_to_clean_saved_state_reconciles_matching_recovery() {
        use crate::model::{DocumentEditor, SettingKey};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recovery.toniator");
        let document = Document::new(SourceArtwork {
            name: "saved.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1]),
        });
        let mut editor = DocumentEditor::new(document);
        let mut changed = editor.document().settings;
        changed.coverage = 125.0;
        editor.set_settings(SettingKey::Coverage, changed);
        save_document_atomic(&path, editor.document()).unwrap();
        assert!(path.exists());
        assert!(editor.undo());
        assert!(!editor.is_dirty());
        assert!(clear_recovery_if_matches(&path, &editor.document().document_id).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn migrated_aspect_is_dirty_until_canonical_save_and_covers_all_caches() {
        use crate::model::{DocumentEditor, WebCurveSettings, WebShapeSettings};
        use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
        use std::io::Cursor;

        let image = RgbImage::from_pixel(16, 9, Rgb([80, 120, 160]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let mut document = Document::new(SourceArtwork {
            name: "wide.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from(png.into_inner()),
        });
        let RenderVariant::WebShapeV1 { settings } = &mut document.render else {
            panic!()
        };
        settings.output_width = 1000;
        settings.output_height = 1000;
        document.saved_web_shape = Some(Box::new(WebShapeSettings {
            output_width: 800,
            output_height: 800,
            ..Default::default()
        }));
        document.saved_web_curve = Some(Box::new(WebCurveSettings {
            output_width: 700,
            output_height: 700,
            ..Default::default()
        }));
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("legacy.toniator");
        std::fs::write(&legacy_path, serde_json::to_vec(&document).unwrap()).unwrap();

        let loaded = load_document_with_migration(&legacy_path).unwrap();
        assert!(loaded.migration.canvas_aspect);
        let mut editor = DocumentEditor::new_with_migration(loaded.document, true);
        assert!(editor.is_dirty());
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!()
        };
        assert_eq!((settings.output_width, settings.output_height), (1000, 563));
        assert_eq!(
            (
                editor
                    .document()
                    .saved_web_shape
                    .as_ref()
                    .unwrap()
                    .output_width,
                editor
                    .document()
                    .saved_web_shape
                    .as_ref()
                    .unwrap()
                    .output_height,
            ),
            (800, 450)
        );
        assert_eq!(
            (
                editor
                    .document()
                    .saved_web_curve
                    .as_ref()
                    .unwrap()
                    .output_width,
                editor
                    .document()
                    .saved_web_curve
                    .as_ref()
                    .unwrap()
                    .output_height,
            ),
            (700, 394)
        );

        let canonical_path = directory.path().join("canonical.toniator");
        save_document_atomic(&canonical_path, editor.document()).unwrap();
        editor.mark_clean();
        assert!(!editor.is_dirty());
        let reopened = load_document_with_migration(&canonical_path).unwrap();
        assert_eq!(reopened.migration, DocumentMigration::default());
        assert_eq!(reopened.document, *editor.document());
    }
}
