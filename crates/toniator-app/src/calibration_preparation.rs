//! Data-only saved-calibration planning and execution for guarded app candidates.

use std::sync::atomic::{AtomicBool, Ordering};

use toniator_domain::{
    AttachedCalibration, CalibrationBinding, ChannelId, Document, SavedCalibration, ValidationError,
};
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{calibrate_saved_binding, calibrate_saved_pattern},
};

/// Identifies one GTK-owned operation whose worker only returns immutable calibration data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TaskId(pub(crate) u64);

/// Carries all exact artifacts measured for one guarded private candidate.
#[derive(Debug)]
pub(crate) struct PreparedCalibrations {
    pub(crate) active: Vec<AttachedCalibration>,
    pub(crate) baseline: Option<SavedCalibration>,
}

/// Separates fresh exact-binding measurements from explicit publication recipients.
#[derive(Debug)]
pub(crate) struct PreparationPlan {
    pub(crate) active: Vec<ChannelId>,
    pub(crate) baseline: bool,
    recipients: Vec<(ChannelId, ChannelId)>,
    reused: Vec<AttachedCalibration>,
    baseline_from_active: Option<ChannelId>,
    baseline_reused: Option<SavedCalibration>,
}

impl PreparationPlan {
    /// Reports whether neither fresh measurement nor missing-assignment publication is required.
    pub(crate) fn is_empty(&self) -> bool {
        self.active.is_empty()
            && !self.baseline
            && self.reused.is_empty()
            && self.baseline_from_active.is_none()
            && self.baseline_reused.is_none()
    }
}

/// Finds channels whose complete structural binding differs between two validated snapshots.
///
/// Channel identity is used only to compare before and after; the binding itself remains the
/// domain authority. Scalar Feature size, artwork, whole-Pattern rotation, response ranges, and
/// model changes therefore stay outside this trigger when the binding is unchanged.
///
/// # Errors
/// Returns a reconstruction error if either snapshot cannot form a current typed channel binding.
pub(crate) fn changed_channels(
    before: &Document,
    after: &Document,
) -> Result<Vec<ChannelId>, ValidationError> {
    let mut changed = Vec::new();
    let before_channels = crate::authoritative_channel_ids(before);
    for channel_id in crate::authoritative_channel_ids(after) {
        let next = CalibrationBinding::for_channel(after, channel_id)?;
        let Some(previous) = before_channels
            .contains(&channel_id)
            .then(|| CalibrationBinding::for_channel(before, channel_id))
            .transpose()?
        else {
            changed.push(channel_id);
            continue;
        };
        if !previous.is_compatible_with(&next) {
            changed.push(channel_id);
        }
    }
    Ok(changed)
}

/// Selects one representative per missing or explicitly forced complete binding.
///
/// Existing project or Preset companions are reused through `saved_calibration_for`; equal
/// bindings are deduplicated by exact domain equality, not a UI approximation or partial key.
///
/// # Errors
/// Returns a domain reconstruction error before any measurement starts.
#[cfg(test)]
pub(crate) fn representatives_to_prepare(
    document: &Document,
    channel_ids: &[ChannelId],
    force: bool,
) -> Result<Vec<ChannelId>, ValidationError> {
    Ok(plan_to_prepare(document, channel_ids, force, false)?.active)
}

/// Plans named-only or ALL publication independently of exact-binding measurement deduplication.
///
/// Existing assignments always win. Only missing assignments borrow a compatible donor; forced
/// recipients are exactly the supplied channel IDs. An existing last-ALL baseline survives named
/// recalibration. A forced ALL baseline never reuses an old measurement.
///
/// # Errors
/// Returns invalid channel or typed binding diagnostics before any measurement starts.
pub(crate) fn plan_to_prepare(
    document: &Document,
    channel_ids: &[ChannelId],
    force: bool,
    force_baseline: bool,
) -> Result<PreparationPlan, ValidationError> {
    let mut plan = PreparationPlan {
        active: Vec::new(),
        baseline: false,
        recipients: Vec::new(),
        reused: Vec::new(),
        baseline_from_active: None,
        baseline_reused: None,
    };
    let mut measured = Vec::<(CalibrationBinding, ChannelId)>::new();
    for channel in channel_ids.iter().copied() {
        let binding = CalibrationBinding::for_channel(document, channel)?;
        if plan.recipients.iter().any(|(id, _)| *id == channel)
            || plan.reused.iter().any(|entry| entry.channel_id == channel)
        {
            continue;
        }
        if !force {
            if document.saved_calibration_for(channel)?.is_some() {
                continue;
            }
            if let Some(artifact) = reusable_companion(document, &binding)? {
                plan.reused.push(AttachedCalibration {
                    channel_id: channel,
                    artifact,
                });
                continue;
            }
        }
        let representative = if let Some((_, representative)) =
            measured.iter().find(|(current, _)| current == &binding)
        {
            *representative
        } else {
            measured.push((binding, channel));
            plan.active.push(channel);
            channel
        };
        plan.recipients.push((channel, representative));
    }
    if force_baseline || document.baseline_saved_calibration()?.is_none() {
        let binding = document.baseline_calibration_binding()?;
        if let Some((_, representative)) = measured.iter().find(|(current, _)| current == &binding)
        {
            plan.baseline_from_active = Some(*representative);
        } else if !force_baseline {
            if let Some(artifact) = reusable_companion(document, &binding)? {
                plan.baseline_reused = Some(artifact);
            } else {
                plan.baseline = true;
            }
        } else {
            plan.baseline = true;
        }
    }
    Ok(plan)
}

/// Finds a compatible donor solely to fill a missing assignment during guarded publication.
///
/// # Errors
/// Returns invalid baseline reconstruction diagnostics; no assignment is replaced here.
fn reusable_companion(
    document: &Document,
    binding: &CalibrationBinding,
) -> Result<Option<SavedCalibration>, ValidationError> {
    let baseline = document.baseline_saved_calibration()?;
    Ok(document
        .saved_calibrations()
        .iter()
        .find(|entry| entry.artifact.binding().is_compatible_with(binding))
        .map(|entry| entry.artifact.clone())
        .or_else(|| {
            baseline
                .filter(|artifact| artifact.binding().is_compatible_with(binding))
                .cloned()
        }))
}

/// Measures exact current bindings once, then expands only the plan's explicit recipients.
///
/// The caller retains revision/operation guards and publishes all assignments atomically. Missing
/// donors and fresh results remain data-only; this worker never mutates document or GTK authority.
///
/// # Errors
/// Returns reconstruction, cancellation, validation, or bounded fitter diagnostics with no publication.
pub(crate) fn prepare(
    document: &Document,
    channel_ids: &[ChannelId],
    force: bool,
    force_baseline: bool,
    cancelled: &AtomicBool,
) -> Result<PreparedCalibrations, String> {
    let plan = plan_to_prepare(document, channel_ids, force, force_baseline)
        .map_err(|error| error.to_string())?;
    let mut measured = Vec::with_capacity(plan.active.len());
    for channel in plan.active {
        if cancelled.load(Ordering::Acquire) {
            return Err("Calibration cancelled.".to_owned());
        }
        let artifact =
            calibrate_saved_pattern(document, channel, EvaluationLimits::default(), &|| {
                cancelled.load(Ordering::Acquire)
            })
            .map_err(|error| error.to_string())?;
        measured.push((channel, artifact));
    }
    let baseline = if let Some(channel) = plan.baseline_from_active {
        Some(
            measured
                .iter()
                .find(|(id, _)| *id == channel)
                .expect("planned active representative")
                .1
                .clone(),
        )
    } else if plan.baseline {
        let binding = document
            .baseline_calibration_binding()
            .map_err(|error| error.to_string())?;
        Some(
            calibrate_saved_binding(&binding, EvaluationLimits::default(), &|| {
                cancelled.load(Ordering::Acquire)
            })
            .map_err(|error| error.to_string())?,
        )
    } else {
        plan.baseline_reused
    };
    let mut active = plan.reused;
    for (channel_id, representative) in plan.recipients {
        let artifact = measured
            .iter()
            .find(|(id, _)| *id == representative)
            .expect("planned active representative")
            .1
            .clone();
        active.push(AttachedCalibration {
            channel_id,
            artifact,
        });
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("Calibration cancelled.".to_owned());
    }
    Ok(PreparedCalibrations { active, baseline })
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::{
        CALIBRATION_PROFILE_SEEDED, CalibrationKnot, CanvasSpec, Document, DocumentCommand,
        DocumentHistory, DocumentSession, PatternDefinitionEdit, PatternMechanism, PropertyFieldId,
        PropertyTarget, SavedCalibration, SourceReference,
    };
    use toniator_patterns::PresetRegistry;

    /// Installs a synthetic exact companion for the bundled seed-bearing Clustered recipe.
    ///
    /// # Panics
    /// Panics if current catalog materialization or saved binding authority is invalid.
    fn clustered_with_companion() -> Document {
        let document = default_document();
        let recipe = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|record| record.metadata.id == "clustered-dispersion-random-links")
            .expect("Clustered catalog recipe exists")
            .recipe
            .clone();
        let base = document.pattern_settings().clone();
        let base_definition = document.pattern_definition_bundles()[0].definition.clone();
        let candidate = document
            .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe,
            })
            .expect("Clustered recipe materializes")
            .0;
        let binding = candidate.baseline_calibration_binding().unwrap();
        let artifact = SavedCalibration::new(
            binding,
            vec![
                CalibrationKnot {
                    input: 0.0,
                    coverage: 0.0,
                },
                CalibrationKnot {
                    input: 2.0,
                    coverage: 0.9,
                },
            ],
            None,
        )
        .unwrap();
        assert_eq!(artifact.profile(), CALIBRATION_PROFILE_SEEDED);
        let entries = crate::authoritative_channel_ids(&candidate)
            .into_iter()
            .map(|channel_id| AttachedCalibration {
                channel_id,
                artifact: artifact.clone(),
            })
            .collect();
        candidate
            .with_saved_calibration_authority(entries, Some(artifact))
            .unwrap()
    }

    /// Reuses active and hidden ALL companions after either variation edit while the authored
    /// definition seed changes and the family still requires regeneration.
    ///
    /// # Panics
    /// Panics if current structural commands, binding normalization, or preparation regress.
    #[test]
    fn scatter_and_line_variation_edits_do_not_schedule_calibration() {
        let before = clustered_with_companion();
        let channel_id = crate::authoritative_channel_ids(&before)[0];
        let definition = before
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == before.pattern_settings().definition_id)
            .unwrap()
            .definition
            .clone();
        let mechanism_id = definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::RandomSiteProcess { id, .. } => Some(*id),
                _ => None,
            })
            .unwrap();
        let output_layer_id = definition.output_layers[0].id;
        for edit in [
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id,
                seed: 0,
            },
            PatternDefinitionEdit::SetConnectionSeed {
                output_layer_id,
                seed: u32::MAX,
            },
        ] {
            let (after, result) = before
                .apply_command(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: definition.id,
                    base_definition: definition.clone(),
                    edit,
                })
                .expect("variation edit applies");
            assert_eq!(
                result.invalidation,
                Some(toniator_domain::InvalidationLevel::Family)
            );
            assert!(changed_channels(&before, &after).unwrap().is_empty());
            assert_eq!(
                after.baseline_saved_calibration().unwrap(),
                before.baseline_saved_calibration().unwrap()
            );
            assert_eq!(
                after.saved_calibration_for(channel_id).unwrap(),
                before.saved_calibration_for(channel_id).unwrap()
            );
            let channels = crate::authoritative_channel_ids(&after);
            assert!(
                plan_to_prepare(&after, &channels, false, false)
                    .unwrap()
                    .is_empty()
            );
            assert_ne!(
                after.pattern_definition_bundles(),
                before.pattern_definition_bundles()
            );
        }
    }

    /// Keeps a named-channel variation edit from fitting or erasing the hidden shared companion.
    ///
    /// # Panics
    /// Panics if copy-on-edit definition identity is confused with saved geometry identity.
    #[test]
    fn named_line_variation_reuses_shared_companion() {
        let before = clustered_with_companion();
        let channel_id = crate::authoritative_channel_ids(&before)[0];
        let definition = before
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == before.pattern_settings().definition_id)
            .unwrap()
            .definition
            .clone();
        let (after, result) = before
            .apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition: definition.clone(),
                edit: PatternDefinitionEdit::SetConnectionSeed {
                    output_layer_id: definition.output_layers[0].id,
                    seed: u32::MAX,
                },
            })
            .unwrap();
        assert_eq!(
            result.invalidation,
            Some(toniator_domain::InvalidationLevel::Family)
        );
        assert!(changed_channels(&before, &after).unwrap().is_empty());
        assert_eq!(
            after.baseline_saved_calibration().unwrap(),
            before.baseline_saved_calibration().unwrap()
        );
        assert_eq!(
            after.saved_calibration_for(channel_id).unwrap(),
            before.saved_calibration_for(channel_id).unwrap()
        );
        let channels = crate::authoritative_channel_ids(&after);
        assert!(
            plan_to_prepare(&after, &channels, false, false)
                .unwrap()
                .is_empty()
        );
        assert_ne!(
            after
                .effective_channel_pattern(channel_id)
                .unwrap()
                .definition_id,
            before
                .effective_channel_pattern(channel_id)
                .unwrap()
                .definition_id
        );
    }

    /// Builds one current RGB document with its checked-in exact default companions.
    ///
    /// # Panics
    /// Panics if the checked-in default document cannot validate.
    fn default_document() -> Document {
        Document::new_default_document(
            CanvasSpec {
                width: 1024.0,
                height: 1024.0,
            },
            SourceReference::Unassigned,
        )
        .expect("default calibration fixture validates")
    }

    /// Reuses exact bundled companions and asks for only one forced fit per equivalent binding.
    ///
    /// # Panics
    /// Panics if duplicate channels are not grouped by their complete typed binding.
    #[test]
    fn exact_companions_are_reused_and_forced_bindings_are_deduplicated() {
        let document = default_document();
        let channels = crate::authoritative_channel_ids(&document);
        assert!(
            representatives_to_prepare(&document, &channels, false)
                .expect("bindings reconstruct")
                .is_empty()
        );
        assert!(
            plan_to_prepare(&document, &channels, false, false)
                .expect("default baseline is current")
                .is_empty()
        );
        assert_eq!(
            reusable_companion(&document, &document.baseline_calibration_binding().unwrap())
                .unwrap(),
            document.baseline_saved_calibration().unwrap().cloned()
        );

        let forced =
            representatives_to_prepare(&document, &channels, true).expect("bindings reconstruct");
        let unique = channels
            .iter()
            .map(|channel| CalibrationBinding::for_channel(&document, *channel).unwrap())
            .fold(Vec::<CalibrationBinding>::new(), |mut unique, binding| {
                if !unique.contains(&binding) {
                    unique.push(binding);
                }
                unique
            });
        assert_eq!(forced.len(), unique.len());
    }

    /// Keeps every channel from a changed-binding comparison within domain-owned structural scope.
    ///
    /// # Panics
    /// Panics if an unchanged document spuriously schedules any reference calibration.
    #[test]
    fn unchanged_candidate_has_no_structural_calibration_trigger() {
        let document = default_document();
        assert!(
            changed_channels(&document, &document)
                .expect("bindings reconstruct")
                .is_empty()
        );
    }

    /// Reuses saved calibration for both whole-Pattern and artistic individual Shape rotation.
    ///
    /// # Panics
    /// Panics if a current history command cannot be constructed or the binding trigger changes.
    #[test]
    fn whole_and_shape_rotation_reuse_calibration() {
        let document = default_document();
        let channel_id = crate::authoritative_channel_ids(&document)[0];

        let mut placement = DocumentHistory::new(
            DocumentSession::new(document.clone()).expect("valid default session"),
        );
        let command = document
            .set_channel_pattern_rotation_for_effective(channel_id, 37.0)
            .expect("whole rotation command validates");
        placement.apply(&command).expect("whole rotation applies");
        assert!(
            changed_channels(&document, placement.document())
                .expect("rotated binding reconstructs")
                .is_empty()
        );

        let mut shape = DocumentHistory::new(
            DocumentSession::new(document.clone()).expect("valid default session"),
        );
        let command = document
            .set_channel_shape_rotation_for_effective(channel_id, 37.0)
            .expect("shape rotation command validates");
        shape.apply(&command).expect("shape rotation applies");
        assert!(
            changed_channels(&document, shape.document())
                .expect("shape binding reconstructs")
                .is_empty()
        );
        assert!(
            plan_to_prepare(shape.document(), &[channel_id], false, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            shape.document().saved_calibration_for(channel_id).unwrap(),
            document.saved_calibration_for(channel_id).unwrap()
        );
        let animated = document
            .clone()
            .with_temporal_authority(
                toniator_domain::ProjectTiming::new(
                    toniator_domain::FrameRate::new(24, 1).unwrap(),
                    toniator_domain::FrameRange::new(0, 3).unwrap(),
                ),
                vec![
                    toniator_domain::TemporalEndOverride::Scalar(
                        toniator_domain::ScalarEndOverride {
                            target: PropertyTarget::Document,
                            field: PropertyFieldId::DensityAspect,
                            end: 1.4,
                            easing: toniator_domain::Easing::Linear,
                        },
                    ),
                    toniator_domain::TemporalEndOverride::Scalar(
                        toniator_domain::ScalarEndOverride {
                            target: PropertyTarget::Channel(channel_id),
                            field: PropertyFieldId::ShapeRotationDegrees,
                            end: 360.0,
                            easing: toniator_domain::Easing::Linear,
                        },
                    ),
                ],
            )
            .unwrap();
        assert!(
            plan_to_prepare(
                &animated,
                &crate::authoritative_channel_ids(&animated),
                false,
                false
            )
            .unwrap()
            .is_empty()
        );
        for frame in 0..3 {
            animated
                .materialize_frame(frame)
                .unwrap()
                .validate()
                .unwrap();
        }
    }

    /// Keeps a failed multi-artifact attachment from publishing its earlier valid entries.
    ///
    /// The second channel has a distinct static aspect binding, so reusing the first channel's companion
    /// fails after the first entry would have attached to a directly-mutated history.
    ///
    /// # Panics
    /// Panics if the default fixture, structural edit, or exact current companion is unavailable.
    #[test]
    fn attachment_failure_leaves_the_private_history_unchanged() {
        let source = default_document();
        let channels = crate::authoritative_channel_ids(&source);
        let first = channels[0];
        let second = channels[1];
        let rotated = source
            .edit_channel_start_configuration(&[(
                PropertyTarget::Channel(second),
                PropertyFieldId::DensityAspect,
                1.3,
            )])
            .unwrap()
            .bind(&source)
            .unwrap();
        let mut history = DocumentHistory::new(
            DocumentSession::new(rotated).expect("rotated document remains valid"),
        );
        let before = history.document().clone();
        let revision = history.revision();
        let artifact = source
            .saved_calibration_for(first)
            .expect("first binding reconstructs")
            .expect("default companion exists")
            .clone();

        let result = crate::attach_calibrations_to_history(
            &mut history,
            PreparedCalibrations {
                active: vec![
                    AttachedCalibration {
                        channel_id: first,
                        artifact: artifact.clone(),
                    },
                    AttachedCalibration {
                        channel_id: second,
                        artifact,
                    },
                ],
                baseline: None,
            },
        );

        assert!(result.is_err());
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
        assert!(!history.can_undo());
    }

    /// Treats a canceled request as a terminal worker result before starting any fit.
    ///
    /// # Panics
    /// Panics if a canceled candidate starts measurement or returns a publishable result.
    #[test]
    fn canceled_request_returns_no_artifacts() {
        let document = default_document();
        let cancelled = AtomicBool::new(true);
        let channels = crate::authoritative_channel_ids(&document);
        assert_eq!(
            prepare(&document, &channels, true, false, &cancelled)
                .expect_err("canceled work cannot publish"),
            "Calibration cancelled."
        );
    }

    /// Plans a hidden ALL fit after every named channel diverges and respects Recalibrate scope.
    ///
    /// # Panics
    /// Panics if structural aspect edits or synthetic exact companions cannot validate.
    #[test]
    fn hidden_all_plan_is_independent_of_named_recalibrate() {
        let mut history = DocumentHistory::new(DocumentSession::new(default_document()).unwrap());
        let channels = crate::authoritative_channel_ids(history.document());
        for (channel, aspect) in channels.iter().copied().zip([1.1, 1.3, 1.6]) {
            let base = history.document().clone();
            let config = base
                .edit_channel_start_configuration(&[(
                    PropertyTarget::Channel(channel),
                    PropertyFieldId::DensityAspect,
                    aspect,
                )])
                .unwrap();
            history
                .apply_document_configuration(&base, history.revision(), &config)
                .unwrap();
            let binding = CalibrationBinding::for_channel(history.document(), channel).unwrap();
            let artifact = SavedCalibration::new(
                binding,
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
            let base = history.document().clone();
            history
                .attach_saved_calibration(&base, history.revision(), channel, artifact)
                .unwrap();
        }
        let document = history.document();
        let named = plan_to_prepare(document, &[channels[0]], true, false).unwrap();
        assert_eq!(named.active, vec![channels[0]]);
        assert!(!named.baseline);
        let all = plan_to_prepare(document, &channels, true, true).unwrap();
        assert_eq!(all.active.len(), channels.len());
        assert!(all.baseline);
        assert_eq!(
            reusable_companion(document, &document.baseline_calibration_binding().unwrap())
                .unwrap()
                .as_ref(),
            document.baseline_saved_calibration().unwrap()
        );

        let before = document.clone();
        let base = document.pattern_settings().clone();
        let mut settings = base.clone();
        settings.density.aspect = 1.4;
        history
            .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
            .unwrap();
        assert_eq!(
            changed_channels(&before, history.document()).unwrap(),
            channels
        );
        for channel in channels.iter().copied() {
            let binding = CalibrationBinding::for_channel(history.document(), channel).unwrap();
            let artifact = SavedCalibration::new(
                binding,
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
            let base = history.document().clone();
            history
                .attach_saved_calibration(&base, history.revision(), channel, artifact)
                .unwrap();
        }
        let hidden_only = plan_to_prepare(history.document(), &channels, false, false).unwrap();
        assert!(hidden_only.active.is_empty());
        assert!(hidden_only.baseline);
    }

    /// Keeps a named Recalibrate from claiming success when another channel remains unmeasured.
    ///
    /// # Panics
    /// Panics if a missing companion can enter a completed private history or the error is vague.
    #[test]
    fn named_recalibrate_on_incomplete_candidate_requires_all_recovery() {
        let source = default_document();
        let channels = crate::authoritative_channel_ids(&source);
        let mut history = DocumentHistory::new(DocumentSession::new(source.clone()).unwrap());
        let base = history.document().clone();
        let config = base
            .edit_channel_start_configuration(&[(
                PropertyTarget::Channel(channels[1]),
                PropertyFieldId::DensityAspect,
                1.2,
            )])
            .unwrap();
        history
            .apply_document_configuration(&base, history.revision(), &config)
            .unwrap();
        assert!(
            history
                .document()
                .saved_calibration_for(channels[1])
                .unwrap()
                .is_none()
        );
        let before = history.document().clone();
        let revision = history.revision();
        let artifact = source
            .saved_calibration_for(channels[0])
            .unwrap()
            .unwrap()
            .clone();
        let result = crate::attach_calibrations_to_history(
            &mut history,
            PreparedCalibrations {
                active: vec![AttachedCalibration {
                    channel_id: channels[0],
                    artifact,
                }],
                baseline: None,
            },
        );
        assert!(result.unwrap_err().contains("Recalibrate All"));
        assert_eq!(history.document(), &before);
        assert_eq!(history.revision(), revision);
    }
    /// Separates forced named/ALL recipients from deduplicated fits and fills only missing assignments.
    ///
    /// # Panics
    /// Panics on fanout, lost existing payloads, baseline overwrite, or a spurious fit for a donor.
    #[test]
    fn independent_assignments_have_explicit_recalibration_recipients() {
        let source = default_document();
        let ids = crate::authoritative_channel_ids(&source);
        let old = source.saved_calibration_for(ids[0]).unwrap().unwrap();
        let mut raw = old.raw().to_vec();
        raw[1].coverage += 1e-13;
        let different = SavedCalibration::with_profile(
            old.profile().into(),
            old.binding().clone(),
            raw,
            old.uniform().cloned(),
        )
        .unwrap();
        let source = source
            .with_saved_calibration(ids[1], different.clone())
            .unwrap();
        let named = plan_to_prepare(&source, &[ids[0]], true, false).unwrap();
        assert_eq!(named.active, vec![ids[0]]);
        assert_eq!(named.recipients, vec![(ids[0], ids[0])]);
        assert!(named.baseline_from_active.is_none());
        assert!(named.baseline_reused.is_none());
        let all = plan_to_prepare(&source, &ids, true, true).unwrap();
        assert_eq!(all.active, vec![ids[0]]);
        assert_eq!(
            all.recipients,
            ids.iter().map(|id| (*id, ids[0])).collect::<Vec<_>>()
        );
        assert_eq!(all.baseline_from_active, Some(ids[0]));
        let missing = source
            .with_saved_calibrations(
                source
                    .saved_calibrations()
                    .iter()
                    .filter(|entry| entry.channel_id != ids[2])
                    .cloned()
                    .collect(),
            )
            .unwrap();
        let plan = plan_to_prepare(&missing, &ids, false, false).unwrap();
        assert!(plan.active.is_empty());
        assert_eq!(plan.reused.len(), 1);
        let result = prepare(&missing, &ids, false, false, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.active.len(), 1);
        assert_eq!(result.active[0].channel_id, ids[2]);
        let mut history = DocumentHistory::new(DocumentSession::new(missing).unwrap());
        crate::attach_calibrations_to_history(&mut history, result).unwrap();
        assert_eq!(
            history.document().saved_calibration_for(ids[1]).unwrap(),
            Some(&different)
        );
        assert_eq!(
            history.document().baseline_saved_calibration().unwrap(),
            source.baseline_saved_calibration().unwrap()
        );
    }
}
