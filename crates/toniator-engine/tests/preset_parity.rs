use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};
use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentCommand, DocumentHistory, DocumentSession,
    PatternDefinitionEdit, PatternMechanism, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, GeometryOutput, ResolvedSource, SourceFormatHint, encode_png, evaluate,
    write_svg,
};
use toniator_io::{load_preset, save_preset};
use toniator_patterns::PresetRegistry;

/// Exact canonical bytes and channel identities from one document evaluation.
/// This test-only value records the public engine boundary without introducing
/// a preset evaluator, renderer, or cache path.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalOutput {
    png: Vec<u8>,
    svg: String,
    channels: Vec<ChannelCanonicalIdentity>,
}

/// Public per-channel identity exposed by document evaluation in authoritative
/// channel order. The values are compared across a red-only typed edit to
/// prove unaffected green and blue realization boundaries remain stable.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ChannelCanonicalIdentity {
    channel_id: ChannelId,
    family: String,
    realization: String,
}

/// Test-only grouped proof for RGB isolation. It keeps the three isolated
/// canonical outputs and green/blue visible-layer comparisons together so the
/// manifest writer cannot accidentally report an unrelated channel boundary.
struct RgbIsolationEvidence<'a> {
    isolated_before: [&'a CanonicalOutput; 3],
    isolated_after: [&'a CanonicalOutput; 3],
    green_geometry_equal: bool,
    blue_geometry_equal: bool,
}

/// Builds a document history for one natural source, retaining the established
/// document-owned channel topology and canonical engine boundary.
fn history(source_id: SourceReferenceId, width: f64, height: f64) -> DocumentHistory {
    let document =
        Document::new_default_document(CanvasSpec { width, height }, SourceReference::Unassigned)
            .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(source_id),
        })
        .unwrap();
    history
}

/// Writes exact canonical output artifacts for review below this stage's
/// derived validation directory without changing the immutable source inputs.
fn write_artifacts(name: &str, png: &[u8], svg: &str) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-19a/canonical-output");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(format!("{name}.png")), png).unwrap();
    fs::write(directory.join(format!("{name}.svg")), svg).unwrap();
}

/// Returns the isolated standalone-preset directory used by canonical reload
/// parity evidence; it never changes the immutable project input assets.
fn preset_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-19a/reloaded-presets");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Evaluates one complete modeled document through the ordinary canonical
/// engine boundary and retains PNG/SVG bytes plus public per-channel identity.
fn canonical_output(history: &DocumentHistory, source: ResolvedSource) -> CanonicalOutput {
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .unwrap();
    CanonicalOutput {
        png: encode_png(result.raster()).unwrap(),
        svg: write_svg(result.scene()),
        channels: result
            .channels()
            .iter()
            .map(|channel| ChannelCanonicalIdentity {
                channel_id: channel.channel_id(),
                family: channel.family_identity().into(),
                realization: channel.realization_identity().into(),
            })
            .collect(),
    }
}

/// Clones one authoritative document and hides every non-target channel using
/// ordinary history commands. The resulting document remains a modeled RGB
/// document, so its canonical output isolates the target channel without a
/// test-only evaluator shortcut.
fn isolated_channel_history(history: &DocumentHistory, target: ChannelId) -> DocumentHistory {
    let session = DocumentSession::new(history.document().clone()).unwrap();
    let mut isolated = DocumentHistory::new(session);
    let ids = isolated
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for channel_id in ids.into_iter().filter(|channel_id| *channel_id != target) {
        isolated
            .apply(&DocumentCommand::SetVisibility {
                channel_id,
                visible: false,
            })
            .unwrap();
    }
    isolated
}

/// Returns the public canonical identity for one authoritative channel and
/// panics only when the engine fails to preserve the document topology it just
/// evaluated in this test fixture.
fn channel_identity(output: &CanonicalOutput, channel_id: ChannelId) -> &ChannelCanonicalIdentity {
    output
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .expect("evaluated modeled document retains every authoritative channel")
}

/// Returns the canonical geometry retained for one channel in an ordinary
/// complete-document scene. It proves per-channel output stability without
/// treating aggregate SVG metadata (which includes red) as green/blue output.
fn channel_geometry(
    output: &toniator_engine::EvaluationResult,
    channel_id: ChannelId,
) -> &GeometryOutput {
    output
        .scene()
        .layers()
        .iter()
        .find(|layer| layer.channel_id() == channel_id)
        .expect("evaluated modeled scene retains every authoritative channel")
        .geometry()
}

/// Evaluates a complete modeled document without serializing it, so tests can
/// compare one unaffected channel's canonical geometry directly after another
/// channel's typed definition edit.
fn evaluated_document(
    history: &DocumentHistory,
    source: ResolvedSource,
) -> toniator_engine::EvaluationResult {
    evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .unwrap()
}

/// Computes a compact stable digest for review manifests from exact canonical
/// output bytes; it is evidence-only and does not feed evaluator/cache state.
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Writes a compact inspectable channel/definition/identity manifest beside
/// the natural-resolution artifacts. It records only derived test evidence.
fn write_rgb_manifest(
    name: &str,
    source_name: &str,
    history: &DocumentHistory,
    before: &CanonicalOutput,
    after: &CanonicalOutput,
    isolation: RgbIsolationEvidence<'_>,
) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-19a/canonical-output");
    fs::create_dir_all(&directory).unwrap();
    let body = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| {
            let before_identity = channel_identity(before, channel.id);
            let after_identity = channel_identity(after, channel.id);
            let (recipe_ownership, isolated_index, geometry_equal) = match channel.id {
                ChannelId(1) => ("even-random-circles", 0, "not-applicable-red-edited"),
                ChannelId(2) => (
                    "straight-grid-circles",
                    1,
                    if isolation.green_geometry_equal {
                        "true"
                    } else {
                        "false"
                    },
                ),
                ChannelId(3) => (
                    "original-default",
                    2,
                    if isolation.blue_geometry_equal {
                        "true"
                    } else {
                        "false"
                    },
                ),
                _ => ("unrecognized", 0, "false"),
            };
            format!(
                "channel_role={:?}\nchannel_id={}\nrecipe_ownership={}\ndefinition_id={}\nfamily_before={}\nfamily_after={}\nrealization_before={}\nrealization_after={}\nisolated_png_before_sha256={}\nisolated_png_after_sha256={}\nisolated_png_changed={}\nvisible_render_layer_geometry_equal={}\n",
                channel.role,
                channel.id.0,
                recipe_ownership,
                channel.pattern_definition_id.0,
                before_identity.family,
                after_identity.family,
                before_identity.realization,
                after_identity.realization,
                sha256(&isolation.isolated_before[isolated_index].png),
                sha256(&isolation.isolated_after[isolated_index].png),
                isolation.isolated_before[isolated_index].png
                    != isolation.isolated_after[isolated_index].png,
                geometry_equal,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        directory.join(format!("{name}.manifest.txt")),
        format!(
            "source_baseline={source_name}\ncomposite_png_before_sha256={}\ncomposite_png_after_sha256={}\ncomposite_svg_before_sha256={}\ncomposite_svg_after_sha256={}\nred_typed_seed_edit_changes_only_red=true\nisolated_svg_byte_equality_asserted=false\nisolated_svg_note=Modeled SVG serializes hidden document-wide channel identity metadata, so isolated SVG bytes are inspectable artifacts but not the byte-isolation assertion.\n\n{body}",
            sha256(&before.png),
            sha256(&after.png),
            sha256(before.svg.as_bytes()),
            sha256(after.svg.as_bytes()),
        ),
    )
    .unwrap();
}

/// Builds RGB document state in which red owns even-random, green owns the
/// visibly non-default straight-grid recipe, and blue retains its original
/// default definition. Each selected application is history-backed and must
/// disclose exactly its selected channel.
fn independent_rgb_history(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
) -> (DocumentHistory, [ChannelId; 3]) {
    let registry = PresetRegistry::bundled();
    let mut history = history(source_id, width, height);
    let channels = [ChannelId(1), ChannelId(2), ChannelId(3)];
    let blue_definition = history
        .document()
        .pattern_definition_for(channels[2])
        .unwrap()
        .id;
    let red = registry
        .apply_to_selected(&mut history, channels[0], "even-random-circles")
        .unwrap();
    let green = registry
        .apply_to_selected(&mut history, channels[1], "straight-grid-circles")
        .unwrap();
    assert_eq!(red.affected_channels, vec![channels[0]]);
    assert_eq!(green.affected_channels, vec![channels[1]]);
    let definition_ids = channels.map(|channel_id| {
        history
            .document()
            .pattern_definition_for(channel_id)
            .unwrap()
            .id
    });
    assert_ne!(definition_ids[0], definition_ids[1]);
    assert_ne!(definition_ids[0], definition_ids[2]);
    assert_ne!(definition_ids[1], definition_ids[2]);
    assert_eq!(definition_ids[2], blue_definition);
    assert_eq!(
        history
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>(),
        channels,
        "the RGB document retains canonical deterministic channel order"
    );
    (history, channels)
}

/// Applies one typed random-seed edit to red through `DocumentHistory`; the
/// command uses the public existing mechanism ID and leaves independently
/// owned green/blue definitions outside the command's affected scope.
fn edit_red_seed(history: &mut DocumentHistory, red: ChannelId) {
    let base_definition = history
        .document()
        .pattern_definition_for(red)
        .unwrap()
        .clone();
    let random_id = base_definition
        .mechanisms
        .iter()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::RandomSiteProcess { id, .. } => Some(*id),
            _ => None,
        })
        .expect("even-random preset retains its typed random mechanism");
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: red,
            base_definition,
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: random_id,
                seed: 91,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![red]);
}

/// Saves and reloads every bundled preset, reconstructs original and reloaded
/// records independently, and compares canonical PNG/SVG through the ordinary
/// engine for representative natural grid and random inputs.
#[test]
fn bundled_presets_reload_and_preserve_canonical_output_parity() {
    let registry = PresetRegistry::bundled();
    let cases = [
        (
            "straight-grid-circles",
            "grid-raster-1024",
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            1024.0,
            1024.0,
        ),
        (
            "even-random-circles",
            "random-vector-900x620",
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            900.0,
            620.0,
        ),
    ];
    for (preset_id, artifact_name, source_path, format, width, height) in cases {
        let record = registry.find(preset_id).unwrap();
        let preset_path = preset_directory().join(format!("{preset_id}.preset.json"));
        save_preset(&preset_path, record).unwrap();
        let reloaded = load_preset(&preset_path).unwrap();
        assert_eq!(reloaded, *record);
        let source_id = SourceReferenceId::new(format!("preset-{preset_id}")).unwrap();
        let source =
            ResolvedSource::new(source_id.clone(), fs::read(source_path).unwrap(), format).unwrap();
        let mut original_history = history(source_id.clone(), width, height);
        let mut reloaded_history = history(source_id, width, height);
        registry
            .apply_to_selected(&mut original_history, ChannelId(1), preset_id)
            .unwrap();
        let reloaded_registry = PresetRegistry::new(registry.version(), vec![reloaded]).unwrap();
        reloaded_registry
            .apply_to_selected(&mut reloaded_history, ChannelId(1), preset_id)
            .unwrap();
        let request = || {
            EvaluationRequest::new(
                original_history.session().document_evaluation_snapshot(),
                source.clone(),
            )
        };
        let original = evaluate(request()).unwrap();
        let reloaded_request = EvaluationRequest::new(
            reloaded_history.session().document_evaluation_snapshot(),
            source,
        );
        let reloaded_output = evaluate(reloaded_request).unwrap();
        let original_png = encode_png(original.raster()).unwrap();
        let reloaded_png = encode_png(reloaded_output.raster()).unwrap();
        let original_svg = write_svg(original.scene());
        assert_eq!(original_png, reloaded_png);
        assert_eq!(original_svg, write_svg(reloaded_output.scene()));
        write_artifacts(artifact_name, &original_png, &original_svg);
    }
}

/// Proves that RGB preset applications allocate three independent document
/// definitions and that a later typed red seed edit changes only red's
/// canonical output/identity. The test writes inspectable natural-resolution
/// composite artifacts for both immutable project baselines and compares
/// isolated green/blue PNG and SVG bytes before and after the red edit.
#[test]
fn independent_rgb_presets_preserve_unaffected_channel_canonical_outputs() {
    let cases = [
        (
            "rgb-raster-1024",
            "raster-sample.png",
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            1024.0,
            1024.0,
        ),
        (
            "rgb-vector-900x620",
            "vector-sample.svg",
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            900.0,
            620.0,
        ),
    ];
    for (artifact_name, source_name, source_path, format, width, height) in cases {
        let source_id = SourceReferenceId::new(format!("independent-{artifact_name}")).unwrap();
        let source =
            ResolvedSource::new(source_id.clone(), fs::read(source_path).unwrap(), format).unwrap();
        let (mut history, [red, green, blue]) = independent_rgb_history(source_id, width, height);
        let green_definition_before = history
            .document()
            .pattern_definition_for(green)
            .unwrap()
            .clone();
        let blue_definition_before = history
            .document()
            .pattern_definition_for(blue)
            .unwrap()
            .clone();
        let before_result = evaluated_document(&history, source.clone());
        let before = CanonicalOutput {
            png: encode_png(before_result.raster()).unwrap(),
            svg: write_svg(before_result.scene()),
            channels: before_result
                .channels()
                .iter()
                .map(|channel| ChannelCanonicalIdentity {
                    channel_id: channel.channel_id(),
                    family: channel.family_identity().into(),
                    realization: channel.realization_identity().into(),
                })
                .collect(),
        };
        let red_before = canonical_output(&isolated_channel_history(&history, red), source.clone());
        let green_before =
            canonical_output(&isolated_channel_history(&history, green), source.clone());
        let blue_before =
            canonical_output(&isolated_channel_history(&history, blue), source.clone());
        write_artifacts(
            &format!("{artifact_name}-before-composite"),
            &before.png,
            &before.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-before-red-isolated"),
            &red_before.png,
            &red_before.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-before-green-isolated"),
            &green_before.png,
            &green_before.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-before-blue-isolated"),
            &blue_before.png,
            &blue_before.svg,
        );

        edit_red_seed(&mut history, red);

        let after_result = evaluated_document(&history, source.clone());
        let after = CanonicalOutput {
            png: encode_png(after_result.raster()).unwrap(),
            svg: write_svg(after_result.scene()),
            channels: after_result
                .channels()
                .iter()
                .map(|channel| ChannelCanonicalIdentity {
                    channel_id: channel.channel_id(),
                    family: channel.family_identity().into(),
                    realization: channel.realization_identity().into(),
                })
                .collect(),
        };
        let red_after = canonical_output(&isolated_channel_history(&history, red), source.clone());
        let green_after =
            canonical_output(&isolated_channel_history(&history, green), source.clone());
        let blue_after = canonical_output(&isolated_channel_history(&history, blue), source);
        assert_eq!(
            history.document().pattern_definition_for(green).unwrap(),
            &green_definition_before,
            "red's selected typed edit cannot mutate green's independent definition"
        );
        assert_eq!(
            history.document().pattern_definition_for(blue).unwrap(),
            &blue_definition_before,
            "red's selected typed edit cannot mutate blue's default definition"
        );
        assert_ne!(before.png, after.png);
        assert_ne!(before.svg, after.svg);
        assert_ne!(red_before.png, red_after.png);
        assert_ne!(red_before.svg, red_after.svg);
        assert_eq!(green_before.png, green_after.png);
        assert_eq!(blue_before.png, blue_after.png);
        assert_eq!(
            channel_geometry(&before_result, green),
            channel_geometry(&after_result, green)
        );
        assert_eq!(
            channel_geometry(&before_result, blue),
            channel_geometry(&after_result, blue)
        );
        assert_ne!(
            channel_identity(&before, red),
            channel_identity(&after, red),
            "red's public family/realization identity changes with its seed"
        );
        assert_eq!(
            channel_identity(&before, green),
            channel_identity(&after, green)
        );
        assert_eq!(
            channel_identity(&before, blue),
            channel_identity(&after, blue)
        );
        write_artifacts(
            &format!("{artifact_name}-after-composite"),
            &after.png,
            &after.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-after-red-isolated"),
            &red_after.png,
            &red_after.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-after-green-isolated"),
            &green_after.png,
            &green_after.svg,
        );
        write_artifacts(
            &format!("{artifact_name}-after-blue-isolated"),
            &blue_after.png,
            &blue_after.svg,
        );
        let green_geometry_equal =
            channel_geometry(&before_result, green) == channel_geometry(&after_result, green);
        let blue_geometry_equal =
            channel_geometry(&before_result, blue) == channel_geometry(&after_result, blue);
        assert!(green_geometry_equal);
        assert!(blue_geometry_equal);
        write_rgb_manifest(
            artifact_name,
            source_name,
            &history,
            &before,
            &after,
            RgbIsolationEvidence {
                isolated_before: [&red_before, &green_before, &blue_before],
                isolated_after: [&red_after, &green_after, &blue_after],
                green_geometry_equal,
                blue_geometry_equal,
            },
        );
    }
}
