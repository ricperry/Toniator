//! Gate 21B-5 witnesses configuration reuse through the canonical native output pipeline.

use std::{fs, path::PathBuf, sync::Arc};
use toniator_domain::{
    CanvasSpec, ChannelTopology, ChannelTopologyTemplate, Document, DocumentCommand,
    DocumentConfiguration, DocumentHistory, DocumentSession, HalftoneChannelModel, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, encode_png, evaluate, write_svg,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, capture_document_preset_destination,
    load_document_preset, save, save_document_preset,
};
use toniator_patterns::PresetRegistry;

/// Builds an actual heterogeneous CMYK design with embedded Curve Motif resources and source settings.
///
/// # Panics
/// Panics if current built-in recipes or the modeled domain fail their accepted contracts.
fn heterogeneous_document(source: SourceReferenceId) -> Document {
    let initial = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(source),
    )
    .unwrap();
    let mut channels = ChannelTopology::canonical(
        HalftoneChannelModel::Cmyk,
        ChannelTopologyTemplate {
            pattern_instance: initial.channel_topology().unwrap().channels()[0]
                .pattern_instance
                .clone(),
        },
    )
    .unwrap()
    .channels()
    .to_vec();
    channels[1].mapping.gain = 0.85;
    channels[2].opacity = 0.8;
    let mut settings = initial.pattern_settings().clone();
    settings.density.density = 16.0;
    let document = Document::with_source_topology_and_authored_structures(
        initial.id(),
        initial.canvas().clone(),
        initial.source().clone(),
        initial.pattern_definition_bundles().to_vec(),
        settings,
        HalftoneChannelModel::Cmyk,
        ChannelTopology::new(channels),
        vec![],
    )
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let registry = PresetRegistry::bundled();
    let ids = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for (channel, pattern) in ids.iter().zip([
        "even-random-circles",
        "one-guide-lines",
        "curve-motif-rows",
        "grid-voronoi-scale",
    ]) {
        registry
            .apply_to_selected(&mut history, *channel, pattern)
            .unwrap();
    }
    assert!(!history.document().authored_structures().is_empty());
    history.document().clone()
}

/// Loads both accepted file kinds onto both immutable sources and inspects equivalent authored state.
/// Writes only current-gate native PNG/SVG/project/Presets for direct visual and frontend review.
///
/// # Panics
/// Panics on a persistence, destination-retention, resource, evaluation or artifact-write regression.
#[test]
fn document_presets_render_both_intrinsic_sources_from_both_input_kinds() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage21b-gate5");
    fs::create_dir_all(&output).unwrap();
    let donor_id = SourceReferenceId::new("gate5-donor-artwork").unwrap();
    let donor = heterogeneous_document(donor_id.clone());
    let donor_sources = SourceBundle::new([EmbeddedSource::new(
        donor_id,
        EmbeddedSourceFormat::Png,
        fs::read(root.join("assets/raster-sample.png")).unwrap(),
        Some("Donor artwork.png".into()),
    )
    .unwrap()])
    .unwrap();
    let project = output.join("heterogeneous-cmyk.toniator");
    save(&project, &donor, &donor_sources).unwrap();
    let preset = output.join("heterogeneous-cmyk.toniator-preset");
    save_document_preset(
        &preset,
        &DocumentConfiguration::capture(&donor),
        &capture_document_preset_destination(&preset).unwrap(),
    )
    .unwrap();

    for (label, suffix, hint, format, width, height) in [
        (
            "raster",
            "png",
            SourceFormatHint::Png,
            EmbeddedSourceFormat::Png,
            1024,
            1024,
        ),
        (
            "vector",
            "svg",
            SourceFormatHint::Svg,
            EmbeddedSourceFormat::Svg,
            900,
            620,
        ),
    ] {
        let bytes: Arc<[u8]> = fs::read(root.join(format!("assets/{label}-sample.{suffix}")))
            .unwrap()
            .into();
        let id = SourceReferenceId::new(format!("gate5-destination-{label}")).unwrap();
        let destination = Document::new_default_document(
            CanvasSpec {
                width: f64::from(width),
                height: f64::from(height),
            },
            SourceReference::Assigned(id.clone()),
        )
        .unwrap();
        let from_preset = load_document_preset(&preset, &destination).unwrap();
        let from_project = load_document_preset(&project, &destination).unwrap();
        assert_eq!(from_preset, from_project);
        let mut history = DocumentHistory::new(DocumentSession::new(destination.clone()).unwrap());
        history
            .apply_document_configuration(&destination, history.revision(), &from_project)
            .unwrap();
        assert_eq!(history.document().source(), destination.source());
        assert_eq!(history.document().canvas(), destination.canvas());
        assert_eq!(
            history.document().channel_model(),
            Some(HalftoneChannelModel::Cmyk)
        );
        let scene = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(id.clone(), bytes.clone(), hint).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            (scene.raster().width(), scene.raster().height()),
            (width, height)
        );
        fs::write(
            output.join(format!("{label}-loaded.png")),
            encode_png(scene.raster()).unwrap(),
        )
        .unwrap();
        fs::write(
            output.join(format!("{label}-loaded.svg")),
            write_svg(scene.scene()),
        )
        .unwrap();
        let sources = SourceBundle::new([EmbeddedSource::new(
            id,
            format,
            bytes,
            Some(format!("Destination {label}")),
        )
        .unwrap()])
        .unwrap();
        save(
            &output.join(format!("{label}-loaded.toniator")),
            history.document(),
            &sources,
        )
        .unwrap();
        history.undo().unwrap();
        assert_eq!(history.document(), &destination);
        history.redo().unwrap();
        assert_eq!(
            DocumentConfiguration::capture(history.document()),
            from_preset
        );

        // A subsequent ordinary edit proves the loaded configuration is the editable authority.
        let base = history.document().pattern_settings().clone();
        let mut settings = base.clone();
        settings.pattern_rotation_degrees += 5.0;
        history
            .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
            .unwrap();
    }
}
