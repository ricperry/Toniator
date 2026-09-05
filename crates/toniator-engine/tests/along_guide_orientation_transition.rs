//! Current transition-to-evaluation regression for guide-relative Along Guides marks.

use std::{fs, path::Path};

use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentHistory, DocumentSession,
    PatternRecipeSiteGenerationKind, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, encode_png, evaluate, write_svg,
};
use toniator_patterns::PresetRegistry;
use toniator_render::GeometryOutput;

/// Builds and evaluates the canonical Straight Grid transition against one immutable source.
///
/// # Panics
///
/// Panics when the current catalog recipe, domain command, immutable source decoder, canonical
/// realization, or validation-artifact write fails.
fn evaluate_transition(
    source_name: &str,
    format: SourceFormatHint,
    artifact_stem: &str,
    canvas: CanvasSpec,
) {
    let source_id = SourceReferenceId::new(format!("along-guide-orientation-{artifact_stem}"))
        .expect("source ID validates");
    let document =
        Document::new_default_document(canvas, SourceReference::Assigned(source_id.clone()))
            .expect("default document validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    let recipe = PresetRegistry::bundled()
        .entries()
        .iter()
        .find(|record| record.metadata.id == "straight-grid-circles")
        .expect("current Straight Grid Circles recipe exists")
        .recipe
        .with_site_generation_kind(PatternRecipeSiteGenerationKind::AlongGuides)
        .expect("tangent marks transition to compatible Along Guides provenance");
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("base definition exists")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .expect("transitioned recipe materializes through history");

    let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(source_name);
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            source_id,
            fs::read(source_path).expect("immutable source reads"),
            format,
        )
        .expect("immutable source resolves"),
    ))
    .expect("Along Guides tangent marks evaluate without missing provenance");
    let mark_count = result
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
        .map(|output| match output.geometry() {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
            GeometryOutput::CanonicalStrokes(_) | GeometryOutput::CanonicalRegions(_) => 0,
        })
        .sum::<usize>();
    assert!(
        mark_count > 0,
        "transition publishes visible canonical marks"
    );

    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage21b4");
    fs::create_dir_all(&output).expect("validation output directory creates");
    fs::write(
        output.join(format!("along-guide-orientation-{artifact_stem}.png")),
        encode_png(result.raster()).expect("canonical raster encodes"),
    )
    .expect("validation PNG writes");
    fs::write(
        output.join(format!("along-guide-orientation-{artifact_stem}.svg")),
        write_svg(result.scene()),
    )
    .expect("validation SVG writes");
}

/// Proves the canonical oriented Along Guides transition evaluates both immutable source formats.
///
/// # Panics
///
/// Panics when either source fails the shared transition/evaluation assertions.
#[test]
fn oriented_along_guides_transition_evaluates_both_immutable_sources() {
    evaluate_transition(
        "raster-sample.png",
        SourceFormatHint::Png,
        "raster",
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
    );
    evaluate_transition(
        "vector-sample.svg",
        SourceFormatHint::Svg,
        "vector",
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
    );
}
