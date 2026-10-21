#[allow(dead_code)]
#[path = "../examples/artistic_tone_probe.rs"]
mod fixtures;
use toniator_domain::*;
use toniator_engine::*;

/// Rebinds only a deliberately changed fixture recipient to the same explicit synthetic transfer.
/// # Panics
/// Panics if current saved-calibration authority rejects the new exact recipe binding.
fn rebind(document: Document, channel: ChannelId) -> Document {
    let artifact = SavedCalibration::new(
        CalibrationBinding::for_channel(&document, channel).unwrap(),
        vec![
            CalibrationKnot {
                input: 0.0,
                coverage: 0.0,
            },
            CalibrationKnot {
                input: 2.0,
                coverage: 1.0,
            },
        ],
        None,
    )
    .unwrap();
    document.with_saved_calibration(channel, artifact).unwrap()
}

/// Edits one selected output using current copy-on-edit/history authority and immutable base evidence.
/// # Panics
/// Panics if a valid selected output cannot form or apply its current tone command.
fn edit(document: Document, channel: ChannelId, multiplier: f64) -> (Document, CommandResult) {
    let effective = document.effective_channel_pattern(channel).unwrap();
    let definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == effective.definition_id)
        .unwrap()
        .definition
        .clone();
    let output_layer_id = definition.output_layers[0].id;
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: channel,
            base_definition: definition,
            edit: PatternDefinitionEdit::SetToneSampleSpacing {
                output_layer_id,
                value: ToneSampleSpacing { multiplier },
            },
        })
        .unwrap();
    (history.document().clone(), result)
}

/// Proves tone rekeys only its actual output, shares family products and preserves unaffected fit donors.
/// # Panics
/// Panics if cache identity, invalidation, binding scope, targeted output or donor preservation regresses.
#[test]
fn current_tone_edit_preserves_sibling_output_cache_and_calibration() {
    let id = SourceReferenceId::new("tone-cache").unwrap();
    let original = fixtures::fixture("round", 1.0, id.clone()).unwrap();
    let channels = original
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    let target = channels[0];
    let sibling = channels[1];
    let (copied, copy_result) = edit(original.clone(), target, 0.75);
    assert_eq!(copy_result.invalidation, Some(InvalidationLevel::Family));
    assert!(copied.saved_calibration_for(target).unwrap().is_none());
    assert_eq!(
        copied.saved_calibration_for(sibling).unwrap(),
        original.saved_calibration_for(sibling).unwrap()
    );
    assert_eq!(
        copied.baseline_saved_calibration().unwrap(),
        original.baseline_saved_calibration().unwrap()
    );
    let copied = rebind(copied, target);
    let source = ResolvedSource::new(
        id,
        include_bytes!("../../../assets/raster-sample.png").as_slice(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let request = |document: Document| {
        EvaluationRequest::new(
            DocumentSession::new(document)
                .unwrap()
                .document_evaluation_snapshot(),
            source.clone(),
        )
        .for_scene()
        .unwrap()
    };
    let mut cache = EvaluationProfileCache::default();
    evaluate_profiled_cached_with_limits(
        request(copied.clone()),
        EvaluationLimits::default(),
        &mut cache,
    )
    .unwrap();
    let warm = evaluate_profiled_cached_with_limits(
        request(copied.clone()),
        EvaluationLimits::default(),
        &mut cache,
    )
    .unwrap();
    assert!(
        warm.diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit
                && channel.realization == CacheDisposition::Hit)
    );
    let (changed, result) = edit(copied.clone(), target, 2.0);
    assert_eq!(result.invalidation, Some(InvalidationLevel::Realization));
    assert!(changed.saved_calibration_for(target).unwrap().is_none());
    assert_eq!(
        changed.saved_calibration_for(sibling).unwrap(),
        copied.saved_calibration_for(sibling).unwrap()
    );
    assert_eq!(
        changed.baseline_saved_calibration().unwrap(),
        copied.baseline_saved_calibration().unwrap()
    );
    let expected_output = changed
        .effective_channel_pattern(target)
        .unwrap()
        .output_settings[0]
        .output_layer_id;
    let changed = rebind(changed, target);
    let evaluated = evaluate_profiled_cached_with_limits(
        request(changed),
        EvaluationLimits::default(),
        &mut cache,
    )
    .unwrap();
    for channel in &evaluated.diagnostics.channels {
        assert_eq!(channel.family, CacheDisposition::Hit);
        assert_eq!(
            channel.realization,
            if channel.channel_id == target {
                CacheDisposition::Miss
            } else {
                CacheDisposition::Hit
            }
        );
        if channel.channel_id == target {
            assert_eq!(channel.outputs[0].output_layer_id, expected_output);
        }
    }
    assert_ne!(
        warm.result.scene().identity().realization_fingerprint(),
        evaluated
            .result
            .scene()
            .identity()
            .realization_fingerprint()
    );
}
