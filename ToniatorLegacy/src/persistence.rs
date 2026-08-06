use crate::cancel::CancellationToken;
use crate::model::{DOCUMENT_FORMAT, DOCUMENT_VERSION, Document, MissingSelectedPatternDefinition};
use crate::{
    MissingPatternDefinitionDiagnostic, PatternDefinitionLifecycleResolver,
    PatternDefinitionSource, PatternId, native_user_pattern_library_dir, parse_tnpattern,
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

/// The result of opening one current-schema document.  A missing-definition
/// candidate is deliberately not a `Document`: callers must finish one
/// explicit recovery action before they receive a valid editable value.
pub enum DocumentOpenCandidate {
    Valid(Box<LoadedDocument>),
    MissingPatternDefinition(Box<MissingPatternDefinitionOpenCandidate>),
}

/// Presentation-ready replacement metadata. Stable IDs remain the only
/// selection authority; name and provenance are creator-facing context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingPatternDefinitionReplacementCandidate {
    pub id: PatternId,
    pub name: String,
    pub provenance: PatternDefinitionSource,
}

/// Strictly decoded but invalid document state retained only for the narrow
/// selected-stable-ID recovery flow.  All public recovery methods clone the
/// candidate, validate the complete result, and leave the source file alone.
pub struct MissingPatternDefinitionOpenCandidate {
    document: Document,
    missing: MissingSelectedPatternDefinition,
    resolver: Option<PatternDefinitionLifecycleResolver>,
    diagnostic: MissingPatternDefinitionDiagnostic,
    replacements: Vec<MissingPatternDefinitionReplacementCandidate>,
    replacement_discovery_error: Option<String>,
}

impl MissingPatternDefinitionOpenCandidate {
    pub fn diagnostic(&self) -> &MissingPatternDefinitionDiagnostic {
        &self.diagnostic
    }

    pub fn replacement_candidates(&self) -> &[MissingPatternDefinitionReplacementCandidate] {
        &self.replacements
    }

    pub fn replacement_discovery_error(&self) -> Option<&str> {
        self.replacement_discovery_error.as_deref()
    }

    /// Strictly reads an external current `.tnpattern`, requiring its stable
    /// ID to match the missing document selection exactly before preserved
    /// values (when present) can be installed and validated.
    pub fn recover_with_matching_file(&self, path: &Path) -> Result<LoadedDocument> {
        anyhow::ensure!(
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tnpattern")),
            "recovery definitions must use the .tnpattern extension"
        );
        let bytes = fs::read(path)
            .with_context(|| format!("could not read recovery definition {}", path.display()))?;
        let definition = parse_tnpattern(&bytes)
            .map_err(|error| anyhow::anyhow!("invalid current recovery definition: {error}"))?;
        anyhow::ensure!(
            definition.id == self.missing.requested_id,
            "recovery definition {} does not match requested pattern {}",
            definition.id,
            self.missing.requested_id
        );
        let instance = match &self.missing.retained_instance {
            Some(instance) => instance.clone(),
            None => definition
                .default_instance_parameters(
                    crate::OutputChannelId::CMYK
                        .into_iter()
                        .chain(crate::OutputChannelId::RGB),
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?,
        };
        let mut recovered = self.document.clone();
        recovered
            .pattern_state
            .recover_missing_selected_definition(&self.missing, definition, instance)?;
        finalize_loaded_document(recovered)
    }

    /// Creates a fresh default instance for one explicitly chosen, currently
    /// resolved stable ID.  The incompatible old structural values are never
    /// mapped to the replacement definition.
    pub fn recover_with_replacement(&self, replacement_id: &PatternId) -> Result<LoadedDocument> {
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "replacement candidates are unavailable: {}",
                self.replacement_discovery_error
                    .as_deref()
                    .unwrap_or("definition discovery did not complete")
            )
        })?;
        let resolved = resolver
            .resolve(replacement_id)
            .map_err(|diagnostic| anyhow::anyhow!(format_missing_diagnostic(&diagnostic)))?;
        let mut recovered = self.document.clone();
        recovered
            .pattern_state
            .replace_missing_selected_definition(
                &self.missing,
                resolved.definition.clone(),
                resolved.authoritative_source,
            )?;
        finalize_loaded_document(recovered)
    }

    pub fn replacement_source(&self, id: &PatternId) -> Option<PatternDefinitionSource> {
        self.resolver
            .as_ref()?
            .resolve(id)
            .ok()
            .map(|resolved| resolved.authoritative_source)
    }
}

pub fn load_document_with_adjustments(path: &Path) -> Result<LoadedDocument> {
    let document = parse_current_document(path)?;
    finalize_loaded_document(document)
}

/// Opens a current document without allowing a partially valid value into the
/// editor.  Only a selected missing stable-ID definition receives a typed
/// recovery candidate; every other parse or validation failure stays hard.
pub fn load_document_open_candidate(path: &Path) -> Result<DocumentOpenCandidate> {
    load_document_open_candidate_with_library(path, native_user_pattern_library_dir())
}

pub fn load_document_open_candidate_with_library(
    path: &Path,
    user_library_directory: impl Into<PathBuf>,
) -> Result<DocumentOpenCandidate> {
    let document = parse_current_document(path)?;
    match finalize_loaded_document(document.clone()) {
        Ok(loaded) => Ok(DocumentOpenCandidate::Valid(Box::new(loaded))),
        Err(original_error) => {
            let missing = match document.missing_selected_pattern_definition() {
                Ok(missing) => missing,
                Err(_) => return Err(original_error),
            };
            let (resolver, diagnostic, replacements, replacement_discovery_error) =
                match PatternDefinitionLifecycleResolver::load_for_document(
                    user_library_directory,
                    &document.pattern_state,
                ) {
                    Ok(resolver) => {
                        let diagnostic =
                            resolver.missing_definition_diagnostic(&missing.requested_id);
                        let replacements = resolver
                            .registry()
                            .definitions()
                            .map(|resolved| MissingPatternDefinitionReplacementCandidate {
                                id: resolved.definition.id.clone(),
                                name: resolved.definition.display.name.clone(),
                                provenance: resolved.authoritative_source,
                            })
                            .collect();
                        (Some(resolver), diagnostic, replacements, None)
                    }
                    Err(error) => (
                        None,
                        MissingPatternDefinitionDiagnostic {
                            requested_id: missing.requested_id.clone(),
                            available_ids: Vec::new(),
                        },
                        Vec::new(),
                        Some(error.to_string()),
                    ),
                };
            Ok(DocumentOpenCandidate::MissingPatternDefinition(Box::new(
                MissingPatternDefinitionOpenCandidate {
                    document,
                    missing,
                    resolver,
                    diagnostic,
                    replacements,
                    replacement_discovery_error,
                },
            )))
        }
    }
}

fn parse_current_document(path: &Path) -> Result<Document> {
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let header: DocumentHeader = serde_json::from_slice(&bytes)
        .with_context(|| format!("could not parse {}", path.display()))?;
    anyhow::ensure!(header.format == DOCUMENT_FORMAT, "not a Toniator document");
    anyhow::ensure!(
        header.version == DOCUMENT_VERSION,
        "This project was created with an unsupported pre-release Toniator format."
    );
    serde_json::from_slice(&bytes).with_context(|| format!("could not parse {}", path.display()))
}

fn finalize_loaded_document(mut document: Document) -> Result<LoadedDocument> {
    // Current schema fields are required by serde. The transient legacy
    // adapter is rebuilt only from authoritative pattern state at this boundary.
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

fn format_missing_diagnostic(diagnostic: &MissingPatternDefinitionDiagnostic) -> String {
    format!(
        "pattern definition {} is not an available replacement; candidates: {}",
        diagnostic.requested_id,
        diagnostic
            .available_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
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
    use crate::pattern::PatternId;
    use crate::preset::parse_treatment;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn source(bytes: impl Into<std::sync::Arc<[u8]>>) -> SourceArtwork {
        SourceArtwork {
            name: "source.png".into(),
            media_type: "image/png".into(),
            bytes: bytes.into(),
        }
    }

    fn rendered_source() -> SourceArtwork {
        // Match the C1 fixtures' 900 × 620 artboard ratio so current save/load
        // normalization is a no-op and this test isolates adapter authority.
        let image = RgbaImage::from_fn(45, 31, |x, y| {
            Rgba([
                (20 + x * 4) as u8,
                (40 + y * 3) as u8,
                (180 - ((x + y) % 20) * 7) as u8,
                255,
            ])
        });
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        source(png.into_inner())
    }

    fn custom_shapes_recipe() -> (
        crate::pattern_definition::PatternDefinition,
        crate::pattern_definition::PatternInstanceParameters,
    ) {
        let mut definition = crate::load_bundled_shapes_definition().unwrap();
        definition.id = PatternId::new("custom.persistence-dots.v1").unwrap();
        definition.display.name = "Persistent Dots".into();
        definition.display.summary = "A portable project Shapes recipe.".into();
        let instance = definition
            .default_instance_parameters(
                crate::OutputChannelId::CMYK
                    .into_iter()
                    .chain(crate::OutputChannelId::RGB),
            )
            .unwrap();
        (definition, instance)
    }

    fn write_missing_definition_fixture(
        directory: &tempfile::TempDir,
    ) -> (
        PathBuf,
        crate::pattern_definition::PatternDefinition,
        crate::pattern_definition::PatternInstanceParameters,
    ) {
        let path = directory.path().join("missing-definition.toniator");
        let (definition, instance) = custom_shapes_recipe();
        let bytes = document_json(&Document::new(source([1]))).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["pattern_state"]["selected"] = serde_json::json!({
            "registered": definition.id.as_str(),
        });
        value["pattern_state"]["bundled_definition_instances"][definition.id.as_str()] =
            serde_json::to_value(&instance).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        (path, definition, instance)
    }

    fn missing_candidate(path: &Path, library: &Path) -> MissingPatternDefinitionOpenCandidate {
        match load_document_open_candidate_with_library(path, library).unwrap() {
            DocumentOpenCandidate::MissingPatternDefinition(candidate) => *candidate,
            DocumentOpenCandidate::Valid(_) => panic!("fixture must require explicit recovery"),
        }
    }

    #[test]
    fn missing_selected_definition_exact_recovery_preserves_instance_and_roundtrips_portably() {
        let directory = tempfile::tempdir().unwrap();
        let (path, definition, instance) = write_missing_definition_fixture(&directory);
        let source_before = std::fs::read(&path).unwrap();
        assert!(
            load_document(&path).is_err(),
            "invalid authority never opens live"
        );
        let candidate = missing_candidate(&path, directory.path().join("library").as_path());
        assert_eq!(candidate.diagnostic().requested_id, definition.id);
        assert!(
            candidate
                .diagnostic()
                .available_ids
                .windows(2)
                .all(|ids| ids[0] <= ids[1])
        );

        let matching = directory.path().join("matching.tnpattern");
        std::fs::write(&matching, crate::serialize_tnpattern(&definition).unwrap()).unwrap();
        let matching_before = std::fs::read(&matching).unwrap();
        let recovered = candidate.recover_with_matching_file(&matching).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), source_before);
        assert_eq!(std::fs::read(&matching).unwrap(), matching_before);
        let embedded = recovered
            .document
            .pattern_state
            .selected_embedded_pattern()
            .unwrap();
        assert_eq!(embedded.definition, definition);
        assert_eq!(embedded.instance, instance);

        save_document_atomic(&path, &recovered.document).unwrap();
        let reopened = load_document(&path).unwrap();
        assert_eq!(
            reopened.pattern_state.selected_embedded_pattern(),
            Some(embedded)
        );
    }

    #[test]
    fn missing_definition_replacement_uses_fresh_user_library_defaults_and_embeds_authority() {
        let directory = tempfile::tempdir().unwrap();
        let library = directory.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let (path, _missing_definition, retained_instance) =
            write_missing_definition_fixture(&directory);
        let mut replacement = crate::load_bundled_shapes_definition().unwrap();
        replacement.id = PatternId::new("custom.recovery-replacement.v1").unwrap();
        replacement.display.name = "Recovery Replacement".into();
        replacement.display.summary = "Fresh defaults for explicit recovery.".into();
        std::fs::write(
            library.join("replacement.tnpattern"),
            crate::serialize_tnpattern(&replacement).unwrap(),
        )
        .unwrap();

        let candidate = missing_candidate(&path, &library);
        assert!(
            candidate
                .diagnostic()
                .available_ids
                .contains(&replacement.id)
        );
        let recovered = candidate.recover_with_replacement(&replacement.id).unwrap();
        let embedded = recovered
            .document
            .pattern_state
            .selected_embedded_pattern()
            .unwrap();
        let expected = replacement
            .default_instance_parameters(
                crate::OutputChannelId::CMYK
                    .into_iter()
                    .chain(crate::OutputChannelId::RGB),
            )
            .unwrap();
        assert_eq!(embedded.definition, replacement);
        assert_eq!(embedded.instance, expected);
        assert_ne!(embedded.instance.pattern_id, retained_instance.pattern_id);
        assert!(recovered.document.validate().is_ok());
        let reopened_path = directory.path().join("replacement-recovered.toniator");
        save_document_atomic(&reopened_path, &recovered.document).unwrap();
        assert_eq!(
            load_document(&reopened_path)
                .unwrap()
                .pattern_state
                .selected_embedded_pattern()
                .unwrap()
                .definition,
            replacement
        );
    }

    #[test]
    fn missing_definition_recovery_rejects_wrong_malformed_and_obsolete_files_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let (path, definition, _instance) = write_missing_definition_fixture(&directory);
        let source_before = std::fs::read(&path).unwrap();
        let current_editor = DocumentEditor::new(Document::new(source([9, 8, 7])));
        let current_before = current_editor.document().clone();
        let candidate = missing_candidate(&path, directory.path().join("library").as_path());

        let mut wrong = definition.clone();
        wrong.id = PatternId::new("custom.wrong-recovery-id.v1").unwrap();
        let wrong_path = directory.path().join("wrong.tnpattern");
        std::fs::write(&wrong_path, crate::serialize_tnpattern(&wrong).unwrap()).unwrap();
        assert!(candidate.recover_with_matching_file(&wrong_path).is_err());

        let malformed = directory.path().join("malformed.tnpattern");
        std::fs::write(&malformed, b"not JSON").unwrap();
        assert!(candidate.recover_with_matching_file(&malformed).is_err());
        let obsolete = directory.path().join("obsolete.tnpattern");
        std::fs::write(&obsolete, br#"{"version":0}"#).unwrap();
        assert!(candidate.recover_with_matching_file(&obsolete).is_err());
        assert!(
            candidate
                .recover_with_replacement(&PatternId::new("custom.not-a-candidate.v1").unwrap())
                .is_err()
        );
        // Cancel is intentionally inert: merely retaining the candidate and
        // rejecting every invalid choice never changes either source file.
        drop(candidate);
        assert_eq!(std::fs::read(&path).unwrap(), source_before);
        assert_eq!(current_editor.document(), &current_before);
        assert_eq!(
            std::fs::read(&wrong_path).unwrap(),
            crate::serialize_tnpattern(&wrong).unwrap()
        );
    }

    #[test]
    fn malformed_retained_instance_is_a_hard_open_failure_not_a_replacement_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let (path, definition, _instance) = write_missing_definition_fixture(&directory);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["pattern_state"]["bundled_definition_instances"][definition.id.as_str()]["format_version"] =
            serde_json::json!(99);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        assert!(load_document(&path).is_err());
        assert!(
            load_document_open_candidate_with_library(&path, directory.path().join("library"))
                .is_err(),
            "a second retained-instance defect must not be discarded through replacement"
        );
    }

    #[test]
    fn conflicting_user_library_keeps_exact_recovery_available_without_replacements() {
        let directory = tempfile::tempdir().unwrap();
        let library = directory.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let (path, definition, instance) = write_missing_definition_fixture(&directory);
        let mut first = crate::load_bundled_shapes_definition().unwrap();
        first.id = PatternId::new("custom.conflicting-library.v1").unwrap();
        first.display.name = "First conflicting library definition".into();
        first.display.summary = "Valid but intentionally conflicting library input.".into();
        let mut second = first.clone();
        second.display.name = "Second conflicting library definition".into();
        std::fs::write(
            library.join("first.tnpattern"),
            crate::serialize_tnpattern(&first).unwrap(),
        )
        .unwrap();
        std::fs::write(
            library.join("second.tnpattern"),
            crate::serialize_tnpattern(&second).unwrap(),
        )
        .unwrap();

        let candidate = missing_candidate(&path, &library);
        assert!(candidate.replacement_candidates().is_empty());
        assert!(candidate.replacement_discovery_error().is_some());
        let replacement_error = candidate.recover_with_replacement(&first.id);
        assert!(replacement_error.is_err());
        assert!(
            replacement_error
                .err()
                .expect("checked error")
                .to_string()
                .contains("unavailable")
        );

        let exact = directory.path().join("exact.tnpattern");
        std::fs::write(&exact, crate::serialize_tnpattern(&definition).unwrap()).unwrap();
        let recovered = candidate.recover_with_matching_file(&exact).unwrap();
        let embedded = recovered
            .document
            .pattern_state
            .selected_embedded_pattern()
            .unwrap();
        assert_eq!(embedded.definition, definition);
        assert_eq!(embedded.instance, instance);
    }

    #[test]
    fn current_project_roundtrips_and_rejects_pre_release_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current.toniator");
        let document = Document::new(source([1]));
        save_document_atomic(&path, &document).unwrap();
        assert_eq!(load_document(&path).unwrap(), document);
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], crate::model::DOCUMENT_VERSION);
        assert!(saved["pattern_state"].is_object());
        assert!(saved["render"].is_null());
        assert!(saved["saved_web_shape"].is_null());
        assert!(saved["saved_web_curve"].is_null());
        assert!(saved["compatibility_pattern"].is_null());

        let mut obsolete_projection = saved.clone();
        obsolete_projection["render"] = serde_json::json!({ "variant": "native-basic-v1" });
        std::fs::write(&path, serde_json::to_vec(&obsolete_projection).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut obsolete_selector = saved;
        obsolete_selector["compatibility_pattern"] = serde_json::json!("shapes");
        std::fs::write(&path, serde_json::to_vec(&obsolete_selector).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut editor = DocumentEditor::new(document.clone());
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        assert!(editor.apply_legacy_mapping_action(crate::model::ValueMode::CrosshatchLuminance));
        let crosshatch_path = directory.path().join("crosshatch.toniator");
        save_document_atomic(&crosshatch_path, editor.document()).unwrap();
        let loaded = load_document(&crosshatch_path).unwrap();
        assert_eq!(loaded.pattern_state, editor.document().pattern_state);
        assert_eq!(loaded.artwork_pipeline, editor.document().artwork_pipeline);

        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["version"] = serde_json::json!(8);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            load_document(&path)
                .unwrap_err()
                .to_string()
                .contains("unsupported pre-release")
        );
    }

    #[test]
    fn current_v10_rejects_missing_mismatched_or_unsupported_authoritative_patterns() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("invalid-compatibility-pattern.toniator");
        let bytes = document_json(&Document::new(source([1]))).unwrap();

        let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        missing.as_object_mut().unwrap().remove("pattern_state");
        std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        assert!(load_document(&path).is_err());

        let mut missing_bundled_instances: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap();
        missing_bundled_instances["pattern_state"]
            .as_object_mut()
            .unwrap()
            .remove("bundled_definition_instances");
        std::fs::write(
            &path,
            serde_json::to_vec(&missing_bundled_instances).unwrap(),
        )
        .unwrap();
        assert!(load_document(&path).is_err());

        let mut mismatched: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        mismatched["pattern_state"]["instances"]["compat.shapes.v1"]["pattern_id"] =
            serde_json::json!("compat.curves.v1");
        std::fs::write(&path, serde_json::to_vec(&mismatched).unwrap()).unwrap();
        assert!(
            load_document(&path)
                .unwrap_err()
                .to_string()
                .contains("contradicts")
        );

        let mut unsupported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        unsupported["pattern_state"]["instances"]["compat.shapes.v1"]["schema_version"] =
            serde_json::json!(2);
        std::fs::write(&path, serde_json::to_vec(&unsupported).unwrap()).unwrap();
        assert!(
            load_document(&path)
                .unwrap_err()
                .to_string()
                .contains("does not support parameter schema version")
        );
    }

    #[test]
    fn current_v10_roundtrips_embedded_custom_recipe_and_rejects_missing_selection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("custom.toniator");
        let mut editor = DocumentEditor::new(Document::new(rendered_source()));
        let (definition, instance) = custom_shapes_recipe();
        let id = definition.id.clone();
        assert!(editor.install_and_select_embedded_pattern(definition, instance));
        save_document_atomic(&path, editor.document()).unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(saved["pattern_state"]["embedded_patterns"][id.as_str()].is_object());
        let reopened = load_document(&path).unwrap();
        assert_eq!(reopened.pattern_state, editor.document().pattern_state);
        assert_eq!(
            reopened.pattern_state.selected_pattern_id(),
            Some(id.clone())
        );

        let mut missing = saved;
        missing["pattern_state"]["embedded_patterns"] = serde_json::json!({});
        std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        assert!(
            load_document(&path)
                .unwrap_err()
                .to_string()
                .contains("embedded definition")
        );
    }

    #[test]
    fn current_v10_roundtrips_document_local_parametric_recipe_without_loss() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("custom-parametric.toniator");
        let mut definition = crate::load_bundled_quadratic_radial_spiral_definition().unwrap();
        let id = PatternId::new("custom.persisted-spiral.1").unwrap();
        definition.id = id.clone();
        let instance = definition
            .default_instance_parameters(crate::OutputChannelId::CMYK)
            .unwrap();
        let mut editor = DocumentEditor::new(Document::new(rendered_source()));
        assert!(editor.install_and_select_embedded_pattern(definition.clone(), instance.clone()));
        save_document_atomic(&path, editor.document()).unwrap();

        let reopened = load_document(&path).unwrap();
        let embedded = reopened.pattern_state.selected_embedded_pattern().unwrap();
        assert_eq!(embedded.definition, definition);
        assert_eq!(embedded.instance, instance);
        assert_eq!(reopened.pattern_state.selected_pattern_id(), Some(id));
    }

    #[test]
    fn wave_channel_width_values_roundtrip_for_bundled_and_embedded_instances() {
        let directory = tempfile::tempdir().unwrap();
        for embedded in [false, true] {
            let path = directory.path().join(if embedded {
                "wave-embedded.toniator"
            } else {
                "wave-bundled.toniator"
            });
            let mut editor = DocumentEditor::new(Document::new(rendered_source()));
            if embedded {
                let mut definition = crate::load_bundled_wave_line_field_definition().unwrap();
                definition.id = PatternId::new("custom.persisted-wave.v1").unwrap();
                let mut instance = definition
                    .default_instance_parameters(crate::OutputChannelId::CMYK)
                    .unwrap();
                instance.pattern_id = definition.id.clone();
                assert!(editor.install_and_select_embedded_pattern(definition, instance));
            } else {
                assert!(editor.select_pattern(PatternId::WAVE_LINE_FIELD_V1));
            }
            assert!(editor.set_selected_output_channel_parameter(
                crate::OutputChannelId::CmykCyan,
                "line-width-min",
                crate::LiteralValue::Number(0.8),
            ));
            let expected = editor
                .document()
                .pattern_state
                .resolve_selected_definition()
                .unwrap()
                .unwrap()
                .instance;
            save_document_atomic(&path, editor.document()).unwrap();
            assert_eq!(
                load_document(&path)
                    .unwrap()
                    .pattern_state
                    .resolve_selected_definition()
                    .unwrap()
                    .unwrap()
                    .instance,
                expected
            );
        }
    }

    #[test]
    fn c2a_c1_fixtures_save_reopen_and_undo_redo_authoritative_pattern_edits() {
        let directory = tempfile::tempdir().unwrap();
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
            let mut editor = DocumentEditor::new(Document::new(source([1])));
            let candidate = parse_treatment(bytes, (900, 620))
                .unwrap_or_else(|error| panic!("{name} did not parse: {error:#}"))
                .candidate_for(editor.document())
                .unwrap_or_else(|error| panic!("{name} did not apply: {error:#}"));
            assert!(editor.replace_with_preset_candidate(candidate));
            assert_eq!(
                editor.document().pattern_state.selected_pattern_id(),
                Some(selected.clone())
            );
            let fixture_state = editor.document().pattern_state.clone();

            match selected.as_str() {
                "compat.shapes.v1" => {
                    let mut settings = editor.document().pattern_state.shape_settings().unwrap();
                    settings.polygon_sides = 3;
                    settings.base_channel.rotation = 27.0;
                    assert!(editor.set_shape_settings(settings));
                }
                "compat.curves.v1" => {
                    let mut settings = editor.document().pattern_state.curve_settings().unwrap();
                    settings.base_channel.curve_scale = 52.0;
                    settings.base_channel.tile_count = 6;
                    settings.base_channel.stack_count = 4;
                    assert!(editor.set_curve_settings(settings));
                }
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            }

            let edited_state = editor.document().pattern_state.clone();
            let path = directory.path().join(format!("{name}.toniator"));
            save_document_atomic(&path, editor.document()).unwrap();
            let saved: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert!(saved["pattern_state"].is_object());
            assert!(saved["render"].is_null());
            assert_eq!(
                saved["pattern_state"]["selected"]["registered"],
                selected.as_str()
            );

            let reopened = load_document(&path).unwrap();
            assert_eq!(reopened.pattern_state, edited_state);
            assert_eq!(
                reopened.pattern_state.selected_pattern_id(),
                Some(selected.clone())
            );
            match selected.as_str() {
                "compat.shapes.v1" => {
                    let settings = reopened.pattern_state.shape_settings().unwrap();
                    assert_eq!(settings.polygon_sides, 3);
                    assert_eq!(settings.base_channel.rotation, 27.0);
                }
                "compat.curves.v1" => {
                    let settings = reopened.pattern_state.curve_settings().unwrap();
                    assert_eq!(settings.base_channel.curve_scale, 52.0);
                    assert_eq!(settings.base_channel.tile_count, 6);
                    assert_eq!(settings.base_channel.stack_count, 4);
                }
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            }

            assert!(editor.undo());
            assert_eq!(editor.document().pattern_state, fixture_state);
            assert!(editor.redo());
            assert_eq!(editor.document().pattern_state, edited_state);
        }
    }

    #[test]
    fn c2b1_c1_fixtures_ignore_contradictory_transient_adapters_across_render_save_and_history() {
        let directory = tempfile::tempdir().unwrap();
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
            let mut fixture_editor = DocumentEditor::new(Document::new(rendered_source()));
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
                Some(selected.clone())
            );

            let authoritative_before = fixture_editor.document().pattern_state.clone();
            let rendered_before = crate::render::render_document_output(
                fixture_editor.document(),
                120,
                80,
                false,
                None,
            )
            .unwrap();

            // This can never be produced by the current selected pattern. It
            // deliberately reverses the adapter kind and uses incompatible
            // parameter values, while retaining a real production Document,
            // preset candidate, renderer, serializer, and editor history.
            let mut contradictory = fixture_editor.document().clone();
            contradictory.render = match selected.as_str() {
                "compat.shapes.v1" => RenderVariant::WebCurveV1 {
                    settings: Box::new(WebCurveSettings {
                        output_width: 17,
                        output_height: 13,
                        long_edge_cells: 2.0,
                        max_mark: 91.0,
                        ..Default::default()
                    }),
                },
                "compat.curves.v1" => RenderVariant::WebShapeV1 {
                    settings: Box::new(WebShapeSettings {
                        output_width: 19,
                        output_height: 11,
                        long_edge_cells: 2.0,
                        grid_scale: 77.0,
                        ..Default::default()
                    }),
                },
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            };
            assert!(matches!(
                &contradictory.render,
                RenderVariant::WebCurveV1 { .. } | RenderVariant::WebShapeV1 { .. }
            ));
            assert_eq!(
                crate::render::render_document_output(&contradictory, 120, 80, false, None)
                    .unwrap(),
                rendered_before,
                "{name} render must rebuild the adapter from pattern_state"
            );

            let mut editor = DocumentEditor::new(contradictory);
            match selected.as_str() {
                "compat.shapes.v1" => {
                    let mut settings = editor.document().pattern_state.shape_settings().unwrap();
                    settings.polygon_sides = 3;
                    settings.base_channel.rotation = 27.0;
                    assert!(editor.set_shape_settings(settings));
                }
                "compat.curves.v1" => {
                    let mut settings = editor.document().pattern_state.curve_settings().unwrap();
                    settings.base_channel.curve_scale = 52.0;
                    settings.base_channel.tile_count = 6;
                    assert!(editor.set_curve_settings(settings));
                }
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            }
            let authoritative_after = editor.document().pattern_state.clone();
            let rendered_after =
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap();

            let path = directory.path().join(format!("{name}.toniator"));
            save_document_atomic(&path, editor.document()).unwrap();
            let serialized = std::fs::read_to_string(&path).unwrap();
            assert!(serialized.contains("\"pattern_state\""));
            assert!(!serialized.contains("\"render\""));
            let reopened = load_document(&path).unwrap();
            assert_eq!(reopened.pattern_state, authoritative_after);
            assert_eq!(
                crate::render::render_document_output(&reopened, 120, 80, false, None).unwrap(),
                rendered_after,
                "{name} reopen must render the saved pattern authority"
            );
            match selected.as_str() {
                "compat.shapes.v1" => assert_eq!(
                    reopened.pattern_state.shape_settings().unwrap(),
                    authoritative_after.shape_settings().unwrap()
                ),
                "compat.curves.v1" => assert_eq!(
                    reopened.pattern_state.curve_settings().unwrap(),
                    authoritative_after.curve_settings().unwrap()
                ),
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            }

            assert!(editor.undo());
            assert_eq!(editor.document().pattern_state, authoritative_before);
            assert_eq!(
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap(),
                rendered_before,
                "{name} undo must ignore the restored contradictory adapter"
            );
            assert!(editor.redo());
            assert_eq!(editor.document().pattern_state, authoritative_after);
            assert_eq!(
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap(),
                rendered_after,
                "{name} redo must restore the authoritative edit"
            );
        }
    }

    #[test]
    fn c2b2a_c1_fixtures_keep_pattern_authority_across_output_caches_and_roundtrips() {
        let directory = tempfile::tempdir().unwrap();
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
            let mut fixture_editor = DocumentEditor::new(Document::new(rendered_source()));
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
                Some(selected.clone())
            );
            let cmyk_state = fixture_editor.document().pattern_state.clone();
            let cmyk_preview = crate::model::PreviewSurface::Color {
                color: crate::model::RgbaColor::opaque(241, 236, 225),
            };
            let export_background = crate::model::ExportBackground::Color {
                color: crate::model::RgbaColor::opaque(11, 22, 33),
            };
            assert!(
                fixture_editor.set_appearance(crate::model::DocumentAppearance {
                    preview_surface: cmyk_preview,
                    export_background,
                })
            );
            let cmyk_rendered = crate::render::render_document_output(
                fixture_editor.document(),
                120,
                80,
                false,
                None,
            )
            .unwrap();

            // Begin with the opposite adapter kind and incompatible settings.
            // The transition must snapshot typed authority, not this facade.
            let mut active_contradiction = fixture_editor.document().clone();
            active_contradiction.render = match selected.as_str() {
                "compat.shapes.v1" => RenderVariant::WebCurveV1 {
                    settings: Box::new(WebCurveSettings {
                        output_width: 17,
                        output_height: 13,
                        long_edge_cells: 2.0,
                        max_mark: 91.0,
                        ..Default::default()
                    }),
                },
                "compat.curves.v1" => RenderVariant::WebShapeV1 {
                    settings: Box::new(WebShapeSettings {
                        output_width: 19,
                        output_height: 11,
                        long_edge_cells: 2.0,
                        grid_scale: 77.0,
                        ..Default::default()
                    }),
                },
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            };
            assert_eq!(
                crate::render::render_document_output(&active_contradiction, 120, 80, false, None)
                    .unwrap(),
                cmyk_rendered,
                "{name} active adapter cannot override CMYK authority"
            );
            let mut editor = DocumentEditor::new(active_contradiction);
            assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
            assert_eq!(
                editor.document().pattern_state,
                cmyk_state,
                "{name} first RGB cache must clone pattern authority"
            );
            assert_eq!(
                editor.document().appearance.export_background,
                export_background
            );
            assert_eq!(
                editor.document().appearance.preview_surface,
                crate::model::PreviewSurface::Color {
                    color: crate::model::RgbaColor::opaque(0, 0, 0)
                }
            );

            // Corrupt only the inactive CMYK adapter after its cache was
            // created. Re-entering CMYK must rebuild it from cached authority.
            let mut inactive_contradiction = editor.document().clone();
            let cmyk_cache = inactive_contradiction
                .inactive_cmyk
                .as_mut()
                .expect("CMYK treatment is cached while RGB is active");
            cmyk_cache.render = match selected.as_str() {
                "compat.shapes.v1" => RenderVariant::WebCurveV1 {
                    settings: Box::new(WebCurveSettings {
                        output_width: 23,
                        output_height: 17,
                        long_edge_cells: 2.0,
                        max_mark: 88.0,
                        ..Default::default()
                    }),
                },
                "compat.curves.v1" => RenderVariant::WebShapeV1 {
                    settings: Box::new(WebShapeSettings {
                        output_width: 29,
                        output_height: 19,
                        long_edge_cells: 2.0,
                        grid_scale: 71.0,
                        ..Default::default()
                    }),
                },
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            };
            let mut editor = DocumentEditor::new(inactive_contradiction);

            match selected.as_str() {
                "compat.shapes.v1" => {
                    let mut settings = editor.document().pattern_state.shape_settings().unwrap();
                    settings.base_channel.rotation = 37.0;
                    settings.polygon_sides = 3;
                    assert!(editor.set_shape_settings(settings));
                }
                "compat.curves.v1" => {
                    let mut settings = editor.document().pattern_state.curve_settings().unwrap();
                    settings.base_channel.curve_scale = 52.0;
                    settings.base_channel.tile_count = 6;
                    assert!(editor.set_curve_settings(settings));
                }
                "weighted-voronoi.v1" => unreachable!("compatibility fixture"),
                _ => unreachable!("compatibility fixture"),
            }
            let rgb_state = editor.document().pattern_state.clone();
            let rgb_rendered =
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap();

            assert!(editor.set_output_mode(crate::model::OutputMode::CmykInks));
            assert_eq!(editor.document().pattern_state, cmyk_state);
            assert_eq!(
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap(),
                cmyk_rendered,
                "{name} restored CMYK cache must ignore its contradictory adapter"
            );
            assert_eq!(editor.document().appearance.preview_surface, cmyk_preview);
            assert_eq!(
                editor.document().appearance.export_background,
                export_background
            );

            assert!(editor.undo());
            assert_eq!(
                editor.document().artwork_pipeline.output_model,
                crate::artwork_pipeline::OutputModel::RgbScreen
            );
            assert_eq!(editor.document().pattern_state, rgb_state);
            assert_eq!(
                crate::render::render_document_output(editor.document(), 120, 80, false, None)
                    .unwrap(),
                rgb_rendered
            );
            assert!(editor.redo());
            assert_eq!(editor.document().pattern_state, cmyk_state);

            let path = directory
                .path()
                .join(format!("{name}-output-cache.toniator"));
            save_document_atomic(&path, editor.document()).unwrap();
            let serialized = std::fs::read_to_string(&path).unwrap();
            assert!(serialized.contains("\"inactive_rgb\""));
            assert!(serialized.contains("\"pattern_state\""));
            assert!(!serialized.contains("\"render\""));
            let reopened = load_document(&path).unwrap();
            assert_eq!(reopened.pattern_state, cmyk_state);
            assert_eq!(reopened.appearance.preview_surface, cmyk_preview);
            assert_eq!(reopened.appearance.export_background, export_background);

            let mut reopened_editor = DocumentEditor::new(reopened);
            assert!(reopened_editor.set_output_mode(crate::model::OutputMode::RgbScreen));
            assert_eq!(reopened_editor.document().pattern_state, rgb_state);
            assert_eq!(
                crate::render::render_document_output(
                    reopened_editor.document(),
                    120,
                    80,
                    false,
                    None,
                )
                .unwrap(),
                rgb_rendered,
                "{name} reopened RGB cache must project typed authority"
            );
            assert_eq!(
                reopened_editor.document().appearance.export_background,
                export_background
            );
        }
    }

    #[test]
    fn current_v10_requires_valid_pipeline_and_cached_pattern_state_everywhere() {
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

        let mut missing_cached_pattern: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        missing_cached_pattern["inactive_cmyk"]
            .as_object_mut()
            .unwrap()
            .remove("pattern_state");
        std::fs::write(&path, serde_json::to_vec(&missing_cached_pattern).unwrap()).unwrap();
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
        document
            .pattern_state
            .select_pattern(crate::pattern::PatternId::COMPATIBILITY_CURVES_V1)
            .unwrap();
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
        document.sync_legacy_projection().unwrap();
        let mut editor = DocumentEditor::new(document);
        assert!(editor.set_output_mode(crate::model::OutputMode::RgbScreen));
        save_document_atomic(&path, editor.document()).unwrap();
        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded.pattern_state, editor.document().pattern_state);
        assert_eq!(loaded.artwork_pipeline, editor.document().artwork_pipeline);
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
    fn current_cache_without_optional_preview_uses_model_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current-cache.toniator");
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
        document
            .pattern_state
            .set_selected_parameters_for_test(&document.render);
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
        assert!(editor.document().saved_web_curve.is_none());

        save_document_atomic(&path, editor.document()).unwrap();
        editor.mark_clean();
        assert!(!editor.is_dirty());
        let reopened = load_document_with_adjustments(&path).unwrap();
        assert_eq!(reopened.adjustments, LoadAdjustments::default());
        assert_eq!(reopened.document, *editor.document());
    }
}
