use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    CanvasSpec, Document, MarkGeometryResponse, PatternGeometryResponse, SourceReference,
    SourceReferenceId,
};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save,
};

/// Creates a unique disposable archive path for a current-schema persistence assertion.
fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage20e1-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time advances")
            .as_nanos()
    ))
}

/// Builds the smallest authoritative document and source pair accepted by the current container writer.
fn current_document() -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("stage20e1-source").expect("fixed source id is valid");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default document is valid");
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Png,
        b"stage20e1 source bytes".to_vec(),
        Some("stage20e1.png".into()),
    )
    .expect("embedded source is valid");
    (
        document,
        SourceBundle::new([source]).expect("one source is valid"),
    )
}

/// Resolves the first document-owned mark response through the current output-settings authority.
fn first_mark_response(document: &Document) -> &MarkGeometryResponse {
    match &document.pattern_definition_bundles()[0].output_settings[0].response {
        PatternGeometryResponse::Marks(response) => response,
        _ => panic!("fixture owns one mark response"),
    }
}

/// Proves current v7 round-trips deterministic normalized fill fields without a migration path.
#[test]
fn current_v7_round_trips_normalized_fill_deterministically() {
    let (document, sources) = current_document();
    let first = temporary("first.toniator");
    let second = temporary("second.toniator");
    save(&first, &document, &sources).expect("current document saves");
    save(&second, &document, &sources).expect("current document saves deterministically");
    assert_eq!(
        fs::read(&first).expect("first bytes"),
        fs::read(&second).expect("second bytes")
    );

    let loaded = load(&first).expect("current v7 archive loads");
    assert_eq!(loaded.versions().document(), DOCUMENT_SCHEMA_VERSION);
    let response = first_mark_response(loaded.document());
    assert_eq!(
        (
            response.minimum_fill,
            response.maximum_fill,
            loaded.document().pattern_settings().shape_rotation_degrees
        ),
        (0.0, 1.0, 0.0)
    );
    fs::remove_file(first).expect("remove first archive");
    fs::remove_file(second).expect("remove second archive");
}

/// Proves a v2 envelope is rejected deterministically instead of activating a compatibility decoder.
#[test]
fn v2_document_schema_is_rejected_without_migration() {
    let (document, sources) = current_document();
    let current = temporary("current.toniator");
    let obsolete = temporary("obsolete-v2.toniator");
    save(&current, &document, &sources).expect("current document saves");
    let input = fs::File::open(&current).expect("current archive opens");
    let mut archive = zip::ZipArchive::new(input).expect("current archive is zip");
    let mut document_json = Vec::new();
    archive
        .by_name("document.json")
        .expect("document entry")
        .read_to_end(&mut document_json)
        .expect("document reads");
    let source_name = archive
        .file_names()
        .find(|name| name.starts_with("sources/") && !name.ends_with('/'))
        .expect("source entry")
        .to_owned();
    let mut source = Vec::new();
    archive
        .by_name(&source_name)
        .expect("source entry")
        .read_to_end(&mut source)
        .expect("source reads");
    let mut json: serde_json::Value = serde_json::from_slice(&document_json).expect("current JSON");
    json["document_schema_version"] = serde_json::json!(2);
    let output = fs::File::create(&obsolete).expect("obsolete archive creates");
    let mut writer = zip::ZipWriter::new(output);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer
        .start_file("document.json", options)
        .expect("document entry writes");
    writer
        .write_all(&serde_json::to_vec(&json).expect("obsolete JSON"))
        .expect("document bytes write");
    writer
        .start_file(&source_name, options)
        .expect("source entry writes");
    writer.write_all(&source).expect("source bytes write");
    writer.finish().expect("archive finishes");

    let error = load(&obsolete).expect_err("v2 archive must reject");
    assert_eq!(error.path(), "version");
    assert!(
        error
            .context()
            .contains("unsupported document schema version 2")
    );
    fs::remove_file(current).expect("remove current archive");
    fs::remove_file(obsolete).expect("remove obsolete archive");
}

/// Proves every current baseline fixture retains its authored normalized mark-fill endpoints.
#[test]
fn current_baseline_fixtures_retain_authoritative_normalized_fill() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    for (name, expected) in [
        (
            "raster-sample.toniator",
            (0.141_421_356_237_309_5, 0.636_396_103_067_892_7),
        ),
        (
            "vector-sample.toniator",
            (0.141_421_356_237_309_5, 0.636_396_103_067_892_7),
        ),
    ] {
        let loaded = load(&assets.join(name)).expect("current fixture loads");
        let response = first_mark_response(loaded.document());
        assert_eq!(
            (response.minimum_fill, response.maximum_fill),
            expected,
            "{name} retains current normalized intent"
        );
    }
}
