use crate::cancel::CancellationToken;
use crate::model::{DOCUMENT_FORMAT, DOCUMENT_VERSION, Document};
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
    canonical.canonicalize_pipeline_facades()?;
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
    Ok(load_document_with_adjustments(path)?.document)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoadAdjustments {
    pub canvas_aspect: bool,
    pub crosshatch_geometry: bool,
}

pub struct LoadedDocument {
    pub document: Document,
    pub adjustments: LoadAdjustments,
}

pub fn load_document_with_adjustments(path: &Path) -> Result<LoadedDocument> {
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let header: DocumentHeader = serde_json::from_slice(&bytes)
        .with_context(|| format!("could not parse {}", path.display()))?;
    anyhow::ensure!(header.format == DOCUMENT_FORMAT, "not a Toniator document");
    anyhow::ensure!(
        header.version == DOCUMENT_VERSION,
        "This project was created with an unsupported pre-release Toniator format."
    );
    let mut document: Document = serde_json::from_slice(&bytes)
        .with_context(|| format!("could not parse {}", path.display()))?;

    // Current schema fields are required by serde. Legacy facade values are
    // deliberately overwritten from the semantic pipeline at this boundary.
    document.canonicalize_pipeline_facades()?;
    let canvas_aspect =
        if let Ok((width, height)) = crate::render::source_dimensions(&document.source) {
            document.normalize_canvas_aspect(width, height)
        } else {
            false
        };
    let crosshatch_geometry = document.normalize_crosshatch_treatment();
    document.validate()?;
    document.settings = document.settings.sanitized();
    Ok(LoadedDocument {
        document,
        adjustments: LoadAdjustments {
            canvas_aspect,
            crosshatch_geometry,
        },
    })
}

#[derive(serde::Deserialize)]
struct DocumentHeader {
    format: String,
    version: u32,
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
    use crate::model::{
        DocumentEditor, RenderVariant, SettingKey, SourceArtwork, WebCurveSettings,
        WebShapeSettings,
    };

    fn source(bytes: impl Into<std::sync::Arc<[u8]>>) -> SourceArtwork {
        SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: bytes.into(),
        }
    }

    #[test]
    fn current_project_roundtrips_and_rejects_pre_release_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current.toniator");
        let document = Document::new(source([1]));
        save_document_atomic(&path, &document).unwrap();
        assert_eq!(load_document(&path).unwrap(), document);

        let mut editor = DocumentEditor::new(document.clone());
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        assert!(editor.apply_legacy_mapping_action(crate::model::ValueMode::CrosshatchLuminance));
        let crosshatch_path = directory.path().join("crosshatch.toniator");
        save_document_atomic(&crosshatch_path, editor.document()).unwrap();
        assert_eq!(load_document(&crosshatch_path).unwrap(), *editor.document());

        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["version"] = serde_json::json!(5);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            load_document(&path)
                .unwrap_err()
                .to_string()
                .contains("unsupported pre-release")
        );
    }

    #[test]
    fn current_v6_requires_valid_pipeline_state_everywhere() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("strict.toniator");
        let mut editor = DocumentEditor::new(Document::new(source([1])));
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        let document = editor.document();
        let bytes = document_json(document).unwrap();

        let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        missing.as_object_mut().unwrap().remove("artwork_pipeline");
        std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        unknown["artwork_pipeline"]["source"] = serde_json::json!("source.unknown");
        std::fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut missing_cache: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        missing_cache["inactive_cmyk"]
            .as_object_mut()
            .unwrap()
            .remove("artwork_pipeline");
        std::fs::write(&path, serde_json::to_vec(&missing_cache).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut wrong_cache_owner: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let rgb_pipeline = wrong_cache_owner["artwork_pipeline"].clone();
        wrong_cache_owner["inactive_cmyk"]["artwork_pipeline"] = rgb_pipeline;
        std::fs::write(&path, serde_json::to_vec(&wrong_cache_owner).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut mismatched_saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        mismatched_saved["saved_web_shape"] =
            serde_json::to_value(WebShapeSettings::default()).unwrap();
        std::fs::write(&path, serde_json::to_vec(&mismatched_saved).unwrap()).unwrap();
        assert!(load_document(&path).is_err());
    }

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
    fn current_curve_project_roundtrips_source_saved_and_inactive_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("curve.toniator");
        let mut document = Document::new(source([0, 1, 2, 3, 254, 255]));
        document.settings.treatment = crate::model::Treatment::Lines;
        document.saved_web_shape = Some(Box::new(WebShapeSettings::default()));
        document.saved_web_shape_pipeline = Some(document.artwork_pipeline.clone());
        document.render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        document.sync_legacy_projection().unwrap();
        let mut editor = DocumentEditor::new(document);
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        save_document_atomic(&path, editor.document()).unwrap();
        assert_eq!(load_document(&path).unwrap(), *editor.document());
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"format\": \"toniator-document\""));
        assert!(text.contains("AAECA/7/"));
    }

    #[test]
    fn appearance_roundtrips_with_output_treatment_preview_snapshots() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("appearance-caches.toniator");
        let mut editor = DocumentEditor::new(Document::new(source([1])));
        assert!(editor.set_appearance(crate::model::DocumentAppearance {
            preview_surface: crate::model::PreviewSurface::Color {
                color: crate::model::RgbaColor::opaque(243, 236, 219),
            },
            export_background: crate::model::ExportBackground::None,
        }));
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        assert!(editor.set_appearance(crate::model::DocumentAppearance {
            preview_surface: crate::model::PreviewSurface::Color {
                color: crate::model::RgbaColor::opaque(9, 17, 29),
            },
            export_background: crate::model::ExportBackground::Color {
                color: crate::model::RgbaColor::opaque(70, 80, 90),
            },
        }));

        save_document_atomic(&path, editor.document()).unwrap();
        assert_eq!(load_document(&path).unwrap(), *editor.document());
        let serialized = std::fs::read_to_string(path).unwrap();
        assert!(serialized.contains("\"preview_surface\""));
    }

    #[test]
    fn old_v6_treatment_caches_without_preview_snapshots_use_model_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old-cache.toniator");
        let mut editor = DocumentEditor::new(Document::new(source([1])));
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        let mut value: serde_json::Value =
            serde_json::from_slice(&document_json(editor.document()).unwrap()).unwrap();
        value["inactive_cmyk"]
            .as_object_mut()
            .unwrap()
            .remove("preview_surface");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut loaded = DocumentEditor::new(load_document(&path).unwrap());
        assert!(loaded.set_output_mode(crate::model::OutputMode::CmykInks));
        assert_eq!(
            loaded.document().appearance.preview_surface,
            crate::model::PreviewSurface::Color {
                color: crate::model::RgbaColor::WHITE,
            }
        );
    }

    #[test]
    fn recovery_is_removed_only_for_the_matching_clean_document() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recovery.toniator");
        let document = Document::new(source([1]));
        save_document_atomic(&path, &document).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(!clear_recovery_if_matches(&path, "another-document").unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), before);

        let mut editor = DocumentEditor::new(document);
        let mut changed = editor.document().settings;
        changed.coverage = 125.0;
        assert!(editor.set_settings(SettingKey::Coverage, changed));
        assert!(editor.undo());
        assert!(!editor.is_dirty());
        assert!(clear_recovery_if_matches(&path, &editor.document().document_id).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn current_load_canvas_adjustment_is_dirty_until_canonical_save() {
        use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
        use std::io::Cursor;

        let image = RgbImage::from_pixel(16, 9, Rgb([80, 120, 160]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let mut document = Document::new(source(png.into_inner()));
        let RenderVariant::WebShapeV1 { settings } = &mut document.render else {
            panic!()
        };
        settings.output_width = 1000;
        settings.output_height = 1000;
        document.saved_web_curve = Some(Box::new(WebCurveSettings {
            output_width: 700,
            output_height: 700,
            ..Default::default()
        }));
        document.saved_web_curve_pipeline = Some(document.artwork_pipeline.clone());

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current.toniator");
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let loaded = load_document_with_adjustments(&path).unwrap();
        assert!(loaded.adjustments.canvas_aspect);
        let mut editor = DocumentEditor::new_with_load_adjustment(loaded.document, true);
        assert!(editor.is_dirty());
        let RenderVariant::WebShapeV1 { settings } = &editor.document().render else {
            panic!()
        };
        assert_eq!((settings.output_width, settings.output_height), (1000, 563));
        assert_eq!(
            editor
                .document()
                .saved_web_curve
                .as_ref()
                .map(|settings| (settings.output_width, settings.output_height)),
            Some((700, 394))
        );

        save_document_atomic(&path, editor.document()).unwrap();
        editor.mark_clean();
        assert!(!editor.is_dirty());
        let reopened = load_document_with_adjustments(&path).unwrap();
        assert_eq!(reopened.adjustments, LoadAdjustments::default());
        assert_eq!(reopened.document, *editor.document());
    }
}
