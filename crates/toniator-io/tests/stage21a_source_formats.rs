use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{CanvasSpec, Document, SourceReference, SourceReferenceId};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save,
};

/// Returns a collision-resistant path for one bounded Stage 21A archive witness.
///
/// # Panics
///
/// Panics only if the host clock predates the Unix epoch.
fn temporary(format: EmbeddedSourceFormat) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage21a-source-{}-{}-v{DOCUMENT_SCHEMA_VERSION}.toniator",
        format.extension(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows the Unix epoch")
            .as_nanos(),
    ))
}

/// Verifies every Stage 21A persisted source format keeps its explicit tag and exact bytes.
#[test]
fn schema_v6_round_trips_every_supported_still_source_without_reencoding() {
    for (index, format) in [
        EmbeddedSourceFormat::Png,
        EmbeddedSourceFormat::Svg,
        EmbeddedSourceFormat::Jpeg,
        EmbeddedSourceFormat::Webp,
        EmbeddedSourceFormat::Bmp,
        EmbeddedSourceFormat::Tiff,
        EmbeddedSourceFormat::OpenExr,
        EmbeddedSourceFormat::Avif,
    ]
    .into_iter()
    .enumerate()
    {
        let id = SourceReferenceId::new(format!("source-stage21a-{index}"))
            .expect("test source ID is valid");
        let document = Document::new_default_document(
            CanvasSpec {
                width: 64.0,
                height: 48.0,
            },
            SourceReference::Assigned(id.clone()),
        )
        .expect("current source-backed document is valid");
        let bytes = vec![index as u8 + 1, 0, 255, 17, index as u8];
        let sources = SourceBundle::new([EmbeddedSource::new(
            id.clone(),
            format,
            bytes.clone(),
            Some(format!("source.{}", format.extension())),
        )
        .expect("explicit source format and bytes are valid")])
        .expect("one-source bundle is valid");
        let path = temporary(format);
        save(&path, &document, &sources).expect("schema-v6 container saves atomically");
        let loaded = load(&path).expect("schema-v6 container reopens");
        let source = loaded
            .sources()
            .get(&id)
            .expect("reopened bundle retains the exact source ID");
        assert_eq!(source.format(), format);
        assert_eq!(source.bytes(), bytes.as_slice());
        fs::remove_file(path).expect("bounded temporary witness is removable");
    }
}
