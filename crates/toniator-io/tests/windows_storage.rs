//! Native Windows persistence witnesses using immutable project-wide inputs.
#![cfg(windows)]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    CanvasSpec, Document, FrameRange, FrameRate, ProjectTiming, SourceReference, SourceReferenceId,
};
use toniator_io::sequence::{SequenceFormat, SequenceManifest, SequenceWriter};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

/// Retains exact project and frame artifacts for both immutable input formats through native publication.
///
/// This storage test preserves source bytes and tests canonical names; it does not rasterize SVG text.
/// # Panics
/// Panics if native storage loses source bytes, violates sequence publication, or cannot reload current state.
#[test]
fn project_and_sequence_preserve_both_immutable_inputs() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repository
        .join("target/validation/windows-port/io-storage/artifacts")
        .join(format!(
            "inputs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    println!("retained native storage artifacts: {}", root.display());
    for (name, format, sequence_format, width, height) in [
        (
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            SequenceFormat::Png,
            1024,
            1024,
        ),
        (
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            SequenceFormat::Svg,
            900,
            620,
        ),
    ] {
        let bytes = fs::read(repository.join("assets").join(name)).unwrap();
        let id = SourceReferenceId::new(name).unwrap();
        let source =
            EmbeddedSource::new(id.clone(), format, bytes.clone(), Some(name.into())).unwrap();
        let sources = SourceBundle::new([source]).unwrap();
        let document = Document::new_default_document(
            CanvasSpec {
                width: width as f64,
                height: height as f64,
            },
            SourceReference::Assigned(id),
        )
        .unwrap();
        let project = root.join(format!("{name}.toniator"));
        save(&project, &document, &sources).unwrap();
        save(&project, &document, &sources).unwrap();
        let loaded = load(&project).unwrap();
        assert_eq!(loaded.document(), &document);
        assert_eq!(loaded.sources(), &sources);
        let timing = ProjectTiming::new(
            FrameRate::new(30_000, 1001).unwrap(),
            FrameRange::new(0, 1).unwrap(),
        );
        let manifest = SequenceManifest::new(sequence_format, width, height, &timing);
        let frames = root.join(format!("{name}-frames"));
        let mut sequence = SequenceWriter::create(&frames, manifest, bytes.len() as u64).unwrap();
        sequence.write_frame(&bytes).unwrap();
        sequence.finish().unwrap();
        assert_eq!(
            fs::read(frames.join(format!("frame-000000.{}", sequence_format.extension()))).unwrap(),
            bytes
        );
        let manifest: SequenceManifest =
            serde_json::from_slice(&fs::read(frames.join("manifest.json")).unwrap()).unwrap();
        assert!(manifest.complete);
        assert_eq!(manifest.completed_frames, 1);
    }
}
