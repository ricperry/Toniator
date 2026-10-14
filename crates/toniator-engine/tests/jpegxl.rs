use std::path::{Path, PathBuf};
use toniator_domain::{CanvasSpec, Document, FrameRate, RationalTime, SourceReference};
use toniator_engine::import_source_media;
use toniator_engine::open_source_media;
use toniator_io::{EmbeddedSourceFormat, SourceMediaManifest};
use toniator_sampling::SourceField;
use toniator_sampling::media::{FrameSource, MediaTools, SourceMediaKind};

/// Resolves a durable synthetic JPEG XL fixture from the workspace without modifying it.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../toniator-sampling/tests/fixtures/jpegxl")
        .join(name)
}

/// Checks importer authority, animation/sequence routing, portable original bytes, and baseline inputs.
///
/// # Panics
/// Writes roundtrip archives and native RGBA frame witnesses under target/validation/jpegxl-043.
/// Panics if current native imports, exact byte persistence, or finite frame selection regress.
#[test]
fn jpegxl_import_persists_original_bytes_and_media_kinds() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/jpegxl-043/roundtrip");
    std::fs::create_dir_all(&output).unwrap();
    for (name, kind) in [
        ("rgba-still.jxl", SourceMediaKind::StillImage),
        ("rgba-animation.jxl", SourceMediaKind::AnimatedImage),
    ] {
        let path = fixture(name);
        let imported = import_source_media(
            std::slice::from_ref(&path),
            None,
            MediaTools::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(imported.metadata.kind, kind);
        let entry = imported.sources.get(&imported.source_id).unwrap();
        assert_eq!(entry.format(), EmbeddedSourceFormat::JpegXl);
        assert_eq!(entry.bytes(), std::fs::read(&path).unwrap());
        let document = Document::new_default_document(
            CanvasSpec {
                width: 3.,
                height: 2.,
            },
            SourceReference::Assigned(imported.source_id.clone()),
        )
        .unwrap();
        let archive = output.join(format!("{name}.toniator"));
        toniator_io::save(&archive, &document, &imported.sources).unwrap();
        let reopened = toniator_io::load(&archive).unwrap();
        assert_eq!(
            reopened.sources().get(&imported.source_id).unwrap().bytes(),
            entry.bytes()
        );
        let mut media =
            open_source_media(reopened.sources(), MediaTools::default(), &|| false).unwrap();
        assert_eq!(media.metadata().kind, kind);
        let time = if kind == SourceMediaKind::AnimatedImage {
            RationalTime::new(1, 2).unwrap()
        } else {
            RationalTime::default()
        };
        let selected = media.frame_at(time, &|| false).unwrap();
        write_native(
            &selected.field,
            &output.join(format!("{name}-selected.png")),
        )
        .unwrap();
        assert_eq!(
            selected.index,
            if kind == SourceMediaKind::AnimatedImage {
                2
            } else {
                0
            }
        );
    }
    let sequence = import_source_media(
        &[
            fixture("rgba-still.jxl"),
            fixture("rgba16.jxl"),
            fixture("rgba-still.jxl"),
        ],
        Some(FrameRate::new(4, 1).unwrap()),
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequence.metadata.kind, SourceMediaKind::ImageSequence);
    assert!(matches!(
        sequence.sources.media(),
        Some(SourceMediaManifest::ImageSequence { .. })
    ));
    assert_eq!(sequence.metadata.frame_count, 3);
    assert!(
        import_source_media(
            &[fixture("rgba-animation.jxl")],
            Some(FrameRate::new(4, 1).unwrap()),
            MediaTools::default(),
            &|| false
        )
        .is_err()
    );
    for (name, size) in [
        ("raster-sample.png", (1024, 1024)),
        ("vector-sample.svg", (900, 620)),
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(name);
        let imported =
            import_source_media(&[path], None, MediaTools::default(), &|| false).unwrap();
        assert_eq!((imported.metadata.width, imported.metadata.height), size);
        let mut media =
            open_source_media(&imported.sources, MediaTools::default(), &|| false).unwrap();
        let frame = media.frame_at(RationalTime::default(), &|| false).unwrap();
        assert_eq!(frame.field.identity().width, size.0);
        write_native(&frame.field, &output.join(format!("{name}-decoded.png"))).unwrap();
    }
}

/// Writes the canonical straight samples as a native RGBA PNG without compositing.
///
/// # Errors
/// Returns PNG/filesystem errors at the derived validation artifact path.
///
/// # Panics
/// Panics only if a source field violates its dimension/pixel-count invariant.
fn write_native(field: &SourceField, path: &Path) -> Result<(), image::ImageError> {
    let native =
        image::RgbaImage::from_fn(field.identity().width, field.identity().height, |x, y| {
            let p = field
                .pixel(x, y)
                .expect("field dimensions bound its pixels");
            image::Rgba([p.red, p.green, p.blue, p.alpha].map(|v| (v * 255.).round() as u8))
        });
    native.save(path)
}
