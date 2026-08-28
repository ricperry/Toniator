//! Opt-in native artifact generation for the Stage 21A rotated One Guide Lines raster witness.

use std::{env, fs, path::Path};

use toniator_domain::{
    CanvasSpec, ChannelId, DensityEditedField, Document, DocumentHistory, DocumentSession,
    PatternCapabilityScope, PatternMechanism, PropertyFieldId, SiteDensityModulation,
    SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, encode_png, evaluate, write_svg,
};
use toniator_patterns::PresetRegistry;
use toniator_render::GeometryOutput;

const ARTIFACT_ENVIRONMENT: &str = "TONIATOR_STAGE21A_ROTATED_STROKE_ARTIFACTS";

/// Builds one current authoritative history with One Guide Lines as the document base and a
/// rotated eligible red channel, leaving source decoding and rendering to the shared engine path.
fn rotated_one_guide_history(source_id: &str, width: f64, height: f64) -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(SourceReferenceId::new(source_id).expect("source ID validates")),
    )
    .expect("artifact document validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("session validates"));
    PresetRegistry::bundled()
        .apply_to_document_base(&mut history, "one-guide-lines")
        .expect("One Guide Lines applies to document base");
    let rotation = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 17.0)
        .expect("eligible channel rotation command builds");
    history
        .apply(&rotation)
        .expect("eligible channel rotation command applies");
    history
}

/// Builds one immutable-source request from the supplied current document history.
///
/// The helper preserves exact source bytes and format classification; it does not synthesize a
/// sampling field, mutate the document, or bypass the engine's canonical evaluation authority.
fn artifact_request(
    history: &DocumentHistory,
    source_id: &str,
    source_path: &Path,
    format: SourceFormatHint,
) -> EvaluationRequest {
    EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new(source_id).expect("source ID validates"),
            fs::read(source_path).expect("immutable source reads"),
            format,
        )
        .expect("resolved source validates"),
    )
}

/// Builds a bounded source-backed history for the complete bundled-preset evaluation matrix.
///
/// The compact canvas and explicit density limit test cost only; every recipe still traverses the
/// complete authoritative source, family, realization, scene, and raster pipeline.
fn bounded_matrix_history(source_id: &str) -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 48.0,
            height: 32.0,
        },
        SourceReference::Assigned(SourceReferenceId::new(source_id).expect("source ID validates")),
    )
    .expect("bounded matrix document validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("matrix session validates"));
    let density = history
        .document()
        .set_document_density_field(DensityEditedField::Density, 12.0)
        .expect("bounded matrix density command builds");
    history
        .apply(&density)
        .expect("bounded matrix density command applies");
    history
}

/// Builds one immutable PNG evaluation request for the bounded bundled-preset matrix.
///
/// The fixture remains byte-identical and the returned request retains the ordinary decoder and
/// source-identity authority rather than synthesizing a sampled field.
fn bounded_matrix_request(
    history: &DocumentHistory,
    source_id: &str,
    source_bytes: Vec<u8>,
) -> EvaluationRequest {
    EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new(source_id).expect("source ID validates"),
            source_bytes,
            SourceFormatHint::Png,
        )
        .expect("bounded matrix source validates"),
    )
}

/// Reports whether the selected definition uses artwork-weighted placement, the only rotation
/// exception authorized by the domain capability projection.
fn selected_definition_uses_artwork_weighted_density(history: &DocumentHistory) -> bool {
    history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("selected definition resolves")
        .mechanisms
        .iter()
        .any(|mechanism| {
            matches!(
                mechanism,
                PatternMechanism::SiteDensityModulation {
                    modulation: SiteDensityModulation::ArtworkWeighted { .. },
                    ..
                }
            )
        })
}

/// Proves One Guide Lines with a 17-degree eligible channel rotation evaluates through the complete
/// authoritative pipeline at both immutable sources' intrinsic dimensions without writing files.
#[test]
fn rotated_one_guide_lines_evaluate_at_immutable_intrinsic_sizes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (source_id, asset, format, width, height) in [
        (
            "stage21a-rotated-stroke-png",
            "raster-sample.png",
            SourceFormatHint::Png,
            1024.0,
            1024.0,
        ),
        (
            "stage21a-rotated-stroke-svg",
            "vector-sample.svg",
            SourceFormatHint::Svg,
            900.0,
            620.0,
        ),
    ] {
        let history = rotated_one_guide_history(source_id, width, height);
        let result = evaluate(artifact_request(
            &history,
            source_id,
            &root.join("assets").join(asset),
            format,
        ))
        .unwrap_or_else(|error| panic!("{asset} rotated One Guide Lines evaluates: {error}"));
        assert_eq!(
            (result.raster().width(), result.raster().height()),
            (width as u32, height as u32),
            "{asset} remains intrinsic"
        );
        assert!(
            result
                .scene()
                .layers()
                .iter()
                .find(|layer| layer.channel_id() == ChannelId(1))
                .is_some_and(|layer| {
                    layer.outputs().iter().any(|output| {
                        matches!(output.geometry(), GeometryOutput::CanonicalStrokes(_))
                    })
                }),
            "{asset} retains a rotated One Guide Lines canonical-stroke output"
        );
    }
}

/// Proves every bundled preset completes source decoding, realization, scene construction, and
/// rasterization with the ordinary channel rotation where capabilities permit it.
///
/// Artwork-weighted placement is the sole exception: it omits pattern rotation, rejects both
/// channel-rotation command construction and evaluation input rotation, and resolves zero degrees.
#[test]
fn every_bundled_preset_completes_bounded_rotated_engine_evaluation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_bytes = fs::read(root.join("assets/raster-sample.png"))
        .expect("immutable bounded matrix source reads");
    let registry = PresetRegistry::bundled();
    assert_eq!(
        registry.entries().len(),
        16,
        "the complete bundled-preset matrix retains every catalog recipe"
    );
    for (ordinal, entry) in registry.entries().iter().enumerate() {
        let source_id = format!("stage21a-bundled-matrix-{ordinal}");
        let mut history = bounded_matrix_history(&source_id);
        registry
            .apply_to_selected(&mut history, ChannelId(1), &entry.metadata.id)
            .unwrap_or_else(|error| {
                panic!(
                    "{} applies to the selected channel: {error}",
                    entry.metadata.id
                )
            });
        let projection = history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .unwrap_or_else(|error| {
                panic!(
                    "{} projects selected capabilities: {error}",
                    entry.metadata.id
                )
            });
        let pattern_rotation_is_available = projection
            .active_controls
            .iter()
            .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees);
        let artwork_weighted = selected_definition_uses_artwork_weighted_density(&history);
        assert_eq!(
            pattern_rotation_is_available, !artwork_weighted,
            "{} makes rotation available exactly when placement is not artwork-weighted",
            entry.metadata.id
        );
        if pattern_rotation_is_available {
            let rotation = history
                .document()
                .set_channel_pattern_rotation_for_effective(ChannelId(1), 17.0)
                .unwrap_or_else(|error| {
                    panic!("{} accepts channel rotation: {error}", entry.metadata.id)
                });
            history.apply(&rotation).unwrap_or_else(|error| {
                panic!("{} applies channel rotation: {error}", entry.metadata.id)
            });
            assert_eq!(
                history
                    .document()
                    .effective_channel_pattern(ChannelId(1))
                    .expect("rotated selected pattern resolves")
                    .pattern_rotation_degrees,
                17.0,
                "{} retains its eligible rotation",
                entry.metadata.id
            );
        } else {
            assert_eq!(
                history
                    .document()
                    .set_channel_pattern_rotation_for_effective(ChannelId(1), 17.0)
                    .expect_err("artwork-weighted placement rejects rotation")
                    .path(),
                "channel.pattern.rotation",
                "{} omits only artwork-weighted rotation",
                entry.metadata.id
            );
            assert_eq!(
                history
                    .document()
                    .effective_channel_pattern(ChannelId(1))
                    .expect("artwork-weighted selected pattern resolves")
                    .pattern_rotation_degrees,
                0.0,
                "{} supplies zero evaluator rotation",
                entry.metadata.id
            );
        }
        let result = evaluate(bounded_matrix_request(
            &history,
            &source_id,
            source_bytes.clone(),
        ))
        .unwrap_or_else(|error| {
            panic!(
                "{} completes bounded rotated engine evaluation through rasterization: {error}",
                entry.metadata.id
            )
        });
        assert_eq!(
            (result.raster().width(), result.raster().height()),
            (48, 32),
            "{} preserves the bounded canvas target",
            entry.metadata.id
        );
        assert_eq!(
            result.raster().pixels().len(),
            48 * 32 * 4,
            "{} produces one complete RGBA raster",
            entry.metadata.id
        );
        assert!(
            !result.scene().layers().is_empty(),
            "{} publishes a complete canonical scene",
            entry.metadata.id
        );
    }
}

/// Generates intrinsic native PNG and raw SVG outputs for both immutable sources only when the
/// explicit artifact environment is enabled, so ordinary tests never write validation artifacts.
///
/// # Panics
///
/// Panics with a deterministic instruction if directly invoked without the opt-in environment, or
/// when current authoritative One Guide Lines evaluation, canonical rendering, or artifact writes
/// fail. It writes exclusively beneath the Stage 21A validation directory.
#[test]
#[ignore = "opt-in validation artifact generator"]
fn generate_rotated_one_guide_intrinsic_artifacts() {
    assert_eq!(
        env::var(ARTIFACT_ENVIRONMENT).as_deref(),
        Ok("1"),
        "set {ARTIFACT_ENVIRONMENT}=1 to generate Stage 21A rotated stroke artifacts"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage21a-rotated-stroke-raster");
    fs::create_dir_all(&output).expect("stage-specific validation directory creates");
    for (source_id, asset, format, width, height, stem) in [
        (
            "stage21a-rotated-stroke-raster-png",
            "raster-sample.png",
            SourceFormatHint::Png,
            1024.0,
            1024.0,
            "raster-sample-one-guide-channel-1-rotation-17",
        ),
        (
            "stage21a-rotated-stroke-raster-svg",
            "vector-sample.svg",
            SourceFormatHint::Svg,
            900.0,
            620.0,
            "vector-sample-one-guide-channel-1-rotation-17",
        ),
    ] {
        let history = rotated_one_guide_history(source_id, width, height);
        let result = evaluate(artifact_request(
            &history,
            source_id,
            &root.join("assets").join(asset),
            format,
        ))
        .unwrap_or_else(|error| panic!("{asset} rotated One Guide Lines evaluates: {error}"));
        assert_eq!(
            (result.raster().width(), result.raster().height()),
            (width as u32, height as u32),
            "{asset} remains intrinsic"
        );
        fs::write(
            output.join(format!("{stem}.png")),
            encode_png(result.raster()).expect("native raster PNG encodes"),
        )
        .expect("native raster PNG writes");
        fs::write(
            output.join(format!("{stem}.svg")),
            write_svg(result.scene()),
        )
        .expect("raw canonical SVG writes");
    }
}
