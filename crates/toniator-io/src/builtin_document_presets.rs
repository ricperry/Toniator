//! Immutable complete Document Presets, separate from the structural Pattern registry.

use crate::{DocumentPresetError, StagedDocumentPreset};
use std::io::Cursor;
use toniator_domain::{Document, DocumentConfiguration};

/// Identifies one immutable source-free configuration by product ID rather than filesystem location.
/// Metadata never selects rendering behavior; loading uses the ordinary strict archive decoder.
pub struct BuiltinDocumentPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub archive_sha256: &'static str,
    archive: &'static [u8],
}

impl BuiltinDocumentPreset {
    /// Returns the original embedded archive for inspection or making an independent personal copy.
    pub fn archive_bytes(&self) -> &'static [u8] {
        self.archive
    }

    /// Decodes the complete embedded configuration against the destination without changing it.
    /// Catalog integrity tests require current embedded archives; the shared decoder retains
    /// the supported personal-file conversions and rejects unresolved former raw intent here.
    ///
    /// # Errors
    /// Returns the shared strict ZIP, schema, calibration, or destination-validation diagnostic.
    pub fn load(
        &self,
        destination: &Document,
    ) -> Result<DocumentConfiguration, DocumentPresetError> {
        match super::document_presets::load_source_free_preset(
            Cursor::new(self.archive),
            self.archive.len() as u64,
            destination,
        )? {
            StagedDocumentPreset::Current(configuration) => Ok(configuration),
            StagedDocumentPreset::PendingRaw(_) => Err(DocumentPresetError::Version {
                context: "built-in Document Preset requires a current configuration".into(),
            }),
        }
    }
}

/// Returns the stable, display-ordered complete Document Preset catalog without filesystem access.
pub fn builtin_document_presets() -> &'static [BuiltinDocumentPreset] {
    BUILTIN_DOCUMENT_PRESETS
}

/// Resolves a stable built-in identity without interpreting it as a personal file path.
pub fn builtin_document_preset(id: &str) -> Option<&'static BuiltinDocumentPreset> {
    BUILTIN_DOCUMENT_PRESETS
        .iter()
        .find(|preset| preset.id == id)
}

static BUILTIN_DOCUMENT_PRESETS: &[BuiltinDocumentPreset] = &[
    BuiltinDocumentPreset {
        id: "chromatic-aberration-cmyk",
        name: "Chromatic Aberration CMYK",
        archive_sha256: "0890af9ee8188d7de467450aeca6af597d3b4dd65b65e64bee8376e42fb4478a",
        archive: include_bytes!(
            "../../../assets/document-presets/chromatic-aberration-cmyk.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "confetti-stained-glass-cmyk",
        name: "Confetti Stained Glass CMYK",
        archive_sha256: "08bf6139878184f2b96a6ba8a0fba707eec37f395ccd8b8ecbd8fe0cc27aaa59",
        archive: include_bytes!(
            "../../../assets/document-presets/confetti-stained-glass-cmyk.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "controlled-splatter-cmyk",
        name: "Controlled Splatter CMYK",
        archive_sha256: "04715f540ab67eeb46ce22481e7093d6412ddfab225d3ae89803b5f6e66e3477",
        archive: include_bytes!(
            "../../../assets/document-presets/controlled-splatter-cmyk.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "diamond-diamond-rgb",
        name: "Diamond Diamond RGB",
        archive_sha256: "d48e42f5bcae43270898f9574f92f54947d8b31f403e3abb02863842772b70a8",
        archive: include_bytes!(
            "../../../assets/document-presets/diamond-diamond-rgb.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "hex-grid-dots-aligned-rgb",
        name: "Hex Grid Dots Aligned RGB",
        archive_sha256: "621ce79befd37d6cb429de96a45e6ab5ce2a9d4757a49b73a420be2496e34bc2",
        archive: include_bytes!(
            "../../../assets/document-presets/hex-grid-dots-aligned-rgb.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "magazine-cmyk",
        name: "Magazine CMYK",
        archive_sha256: "6f24d8a01836f65b1db89bda19ab4030c5859ce10b714c526d5da9f346b32b8b",
        archive: include_bytes!("../../../assets/document-presets/magazine-cmyk.toniator-preset"),
    },
    BuiltinDocumentPreset {
        id: "magazine-cmyk-alternate",
        name: "Magazine CMYK — Alternate",
        archive_sha256: "8b5581e2ff5498f45e857ddc2b87fc8bcf360c1eacbbacd084369f93721da740",
        archive: include_bytes!(
            "../../../assets/document-presets/magazine-cmyk-alternate.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "magazine-rgb",
        name: "Magazine RGB",
        archive_sha256: "52dea4f8c9afbe833c3d954351e5089fa9f79a805f30116d06060f90484ea4aa",
        archive: include_bytes!("../../../assets/document-presets/magazine-rgb.toniator-preset"),
    },
    BuiltinDocumentPreset {
        id: "magazine-rgb-alternate",
        name: "Magazine RGB — Alternate",
        archive_sha256: "a93ecd4bda8697b26288c2b0a72c6b1a686e45d74b49cc0a4ae35d64f9056214",
        archive: include_bytes!(
            "../../../assets/document-presets/magazine-rgb-alternate.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "moire-grayscale",
        name: "Moire Grayscale",
        archive_sha256: "8bde512c4f6a49d0895cf7ef9462a9fdbde1d485c793ef56fe4b71571db44df3",
        archive: include_bytes!("../../../assets/document-presets/moire-grayscale.toniator-preset"),
    },
    BuiltinDocumentPreset {
        id: "noisy-stained-glass-rgb",
        name: "Noisy Stained Glass RGB",
        archive_sha256: "484fb6345b52b31c9acb5f0661aec0ff7209d717666456603e0cf47d5c20bedc",
        archive: include_bytes!(
            "../../../assets/document-presets/noisy-stained-glass-rgb.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "skinny-curve",
        name: "Skinny Curve",
        archive_sha256: "d7011afebb47ac9abba0e92f7728c697c9b50172e7928f9cb610bfd396ebefb7",
        archive: include_bytes!("../../../assets/document-presets/skinny-curve.toniator-preset"),
    },
    BuiltinDocumentPreset {
        id: "skinny-wave-cmyk",
        name: "Skinny Wave CMYK",
        archive_sha256: "661ffac952ee9c1061087afd60b8a7b3df2e601d85ff1cd739773cee942bc58d",
        archive: include_bytes!(
            "../../../assets/document-presets/skinny-wave-cmyk.toniator-preset"
        ),
    },
    BuiltinDocumentPreset {
        id: "skinny-wave-rgb",
        name: "Skinny Wave RGB",
        archive_sha256: "d4e8e9607ce2ac3f87b40dd905407b6e84b2cbe3326aada54f857f24c0b3c097",
        archive: include_bytes!("../../../assets/document-presets/skinny-wave-rgb.toniator-preset"),
    },
];
