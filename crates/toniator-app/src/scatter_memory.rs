//! Editing-session memory for inactive Scatter payloads; active recipes remain domain-owned.

use super::*;
use toniator_domain::{RandomSiteCharacter, ValidationError};

/// Retains inactive choices by scope and semantic process slot, never by copy-on-edit IDs.
/// Wizards clone this workspace memory and publish it only with a successful final Apply.
#[derive(Clone, Default)]
pub(super) struct Memory {
    entries: Vec<Entry>,
}

#[derive(Clone)]
struct Entry {
    target: InspectorTarget,
    slot: usize,
    character: RandomSiteCharacter,
}

/// Projects a stored variant's typed choice without retaining descriptor allocation identities.
fn kind(character: &RandomSiteCharacter) -> RandomCharacterKind {
    match character {
        RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
        RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
        RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
    }
}

/// Returns ordered random processes for the exact ALL or named-channel effective definition.
fn processes(
    document: &Document,
    target: InspectorTarget,
) -> Vec<(PatternMechanismId, RandomSiteCharacter)> {
    let id = match target {
        InspectorTarget::DocumentAll => document.pattern_settings().definition_id,
        InspectorTarget::Channel(channel) => match document.effective_channel_pattern(channel) {
            Ok(pattern) => pattern.definition_id,
            Err(_) => return Vec::new(),
        },
    };
    document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == id)
        .map(|bundle| {
            bundle
                .definition
                .mechanisms
                .iter()
                .filter_map(|mechanism| match mechanism {
                    PatternMechanism::RandomSiteProcess { id, character, .. } => {
                        Some((*id, character.clone()))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Memory {
    /// Records active payloads after edits/Undo while retaining the last inactive choices.
    pub(super) fn observe(&mut self, document: &Document, target: InspectorTarget) {
        let processes = processes(document, target);
        self.entries
            .retain(|entry| entry.target != target || entry.slot < processes.len());
        for (slot, (_, character)) in processes.into_iter().enumerate() {
            self.entries.retain(|entry| {
                !(entry.target == target
                    && entry.slot == slot
                    && kind(&entry.character) == kind(&character))
            });
            if !matches!(character, RandomSiteCharacter::RawUniform) {
                self.entries.push(Entry {
                    target,
                    slot,
                    character,
                });
            }
        }
    }

    /// Discards prior family/recipe intent only for the replaced scope; ALL replacement resets all uses.
    pub(super) fn clear_target(&mut self, target: InspectorTarget) {
        self.entries
            .retain(|entry| target != InspectorTarget::DocumentAll && entry.target != target);
    }

    /// Restores inactive scalar payloads onto the newly allocated transition's current field targets.
    ///
    /// # Errors
    /// Returns normal domain transition/bounds errors; no document or history is modified.
    pub(super) fn transition(
        &mut self,
        document: &Document,
        target: InspectorTarget,
        descriptor: &PropertyDescriptor,
        choice: PropertyEnumChoice,
    ) -> Result<VariantTransitionDraft, ValidationError> {
        self.observe(document, target);
        let transition = document.variant_transition_draft(descriptor, choice)?;
        let (PropertyTarget::Mechanism(_, id), PropertyEnumChoice::RandomCharacter(choice)) =
            (descriptor.target, choice)
        else {
            return Ok(transition);
        };
        let Some(slot) = processes(document, target)
            .iter()
            .position(|(current, _)| *current == id)
        else {
            return Ok(transition);
        };
        let Some(entry) = self.entries.iter().find(|entry| {
            entry.target == target && entry.slot == slot && kind(&entry.character) == choice
        }) else {
            return Ok(transition);
        };
        let updates = transition
            .fields()
            .iter()
            .filter_map(|field| {
                let value = match (&entry.character, field.field) {
                    (
                        RandomSiteCharacter::Even {
                            minimum_center_distance,
                        },
                        PropertyFieldId::RandomEvenMinimumCenterDistance,
                    ) => *minimum_center_distance,
                    (
                        RandomSiteCharacter::Clustered {
                            cluster_density, ..
                        },
                        PropertyFieldId::RandomClusterDensity,
                    ) => *cluster_density,
                    (
                        RandomSiteCharacter::Clustered { cluster_spread, .. },
                        PropertyFieldId::RandomClusterSpread,
                    ) => *cluster_spread,
                    (
                        RandomSiteCharacter::Clustered {
                            cluster_strength, ..
                        },
                        PropertyFieldId::RandomClusterStrength,
                    ) => *cluster_strength,
                    _ => return None,
                };
                Some(VariantTransitionFieldUpdate {
                    field: field.field,
                    target: field.target,
                    value: VariantTransitionValue::FiniteF64(value),
                })
            })
            .collect::<Vec<_>>();
        transition.with_updates(&updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replaces a response-edited Spiral with Scatter without projecting obsolete output IDs.
    ///
    /// # Panics
    /// Panics if temporary recipe construction crashes, or replacement/Undo loses valid authority.
    #[test]
    fn scatter_replacement_after_channel_response_edit_remains_valid() {
        for target in [
            InspectorTarget::DocumentAll,
            InspectorTarget::Channel(ChannelId(1)),
        ] {
            let mut history = DocumentHistory::new(
                DocumentSession::new(
                    Document::new_default_document(
                        CanvasSpec {
                            width: 100.0,
                            height: 100.0,
                        },
                        SourceReference::Unassigned,
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            let catalog = PresetRegistry::bundled();
            catalog
                .apply_to_document_base(&mut history, "round-spiral-line")
                .unwrap();
            let descriptor = history
                .document()
                .property_descriptors()
                .into_iter()
                .find(|value| {
                    matches!(value.target, PropertyTarget::ChannelOutput(ChannelId(1), _))
                        && value.field == PropertyFieldId::CurveResponseBias
                })
                .unwrap();
            let command = command_for_inspector_input(
                history.document(),
                Some(ChannelId(1)),
                DefinitionEditScope::SelectedCopy,
                &descriptor,
                InspectorInput::FiniteF64(0.3),
            )
            .unwrap();
            history.apply(&command).unwrap();
            let before = history.document().clone();
            match target {
                InspectorTarget::DocumentAll => catalog
                    .apply_to_document_base(&mut history, "even-random-circles")
                    .unwrap(),
                InspectorTarget::Channel(channel) => catalog
                    .apply_to_selected(&mut history, channel, "even-random-circles")
                    .unwrap(),
            };
            history.document().validate().unwrap();
            assert!(!history.document().property_values().is_empty());
            history.undo().unwrap();
            assert_eq!(history.document(), &before);
        }
    }

    /// Publishes a remembered Scatter transition through the ordinary scope-aware domain command.
    ///
    /// # Panics
    /// Panics if current descriptors, transition payloads or history fail validation.
    fn switch(
        history: &mut DocumentHistory,
        memory: &mut Memory,
        target: InspectorTarget,
        choice: RandomCharacterKind,
    ) {
        let document = history.document();
        let id = processes(document, target)[0].0;
        let descriptor = document.property_descriptors().into_iter().find(|descriptor| {
            descriptor.field == PropertyFieldId::RandomCharacter
                && matches!(descriptor.target, PropertyTarget::Mechanism(_, current) if current == id)
        }).unwrap();
        let transition = memory
            .transition(
                document,
                target,
                &descriptor,
                PropertyEnumChoice::RandomCharacter(choice),
            )
            .unwrap();
        apply_wizard_transition(history, target, &transition).unwrap();
    }

    /// Changes ALL Scatter after independent channel spacing edits without replacing their recipes.
    ///
    /// # Panics
    /// Panics if a copied channel is skipped, unrelated definition state changes, or Undo is partial.
    #[test]
    fn all_scatter_reaches_independent_cmyk_spacing_definitions() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let topology = toniator_domain::ChannelTopology::canonical(
            HalftoneChannelModel::Cmyk,
            toniator_domain::ChannelTopologyTemplate {
                pattern_instance: document
                    .channel_pattern_instance(ChannelId(1))
                    .unwrap()
                    .clone(),
            },
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        history
            .apply(&DocumentCommand::ReplaceChannelTopology {
                model: HalftoneChannelModel::Cmyk,
                topology,
            })
            .unwrap();
        PresetRegistry::bundled()
            .apply_to_document_base(&mut history, "even-random-circles")
            .unwrap();
        let descriptor = history
            .document()
            .property_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.field == PropertyFieldId::RandomDensityModulation)
            .unwrap();
        let transition = history
            .document()
            .variant_transition_draft(
                &descriptor,
                PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
            )
            .unwrap();
        apply_wizard_transition(&mut history, InspectorTarget::DocumentAll, &transition).unwrap();
        let channels = authoritative_channel_ids(history.document());
        for (channel, component) in channels.iter().copied().zip([
            SourceMappingComponent::Red,
            SourceMappingComponent::Green,
            SourceMappingComponent::Blue,
            SourceMappingComponent::Luminance,
        ]) {
            let descriptor = history
                .document()
                .property_descriptors()
                .into_iter()
                .find(|descriptor| {
                    descriptor.field == PropertyFieldId::ArtworkWeightMappingComponent
                        && descriptor.target == PropertyTarget::Channel(channel)
                })
                .unwrap();
            let command = command_for_inspector_input(
                history.document(),
                Some(channel),
                DefinitionEditScope::SelectedCopy,
                &descriptor,
                InspectorInput::EnumChoice(PropertyEnumChoice::SourceMappingComponent(component)),
            )
            .unwrap();
            history.apply(&command).unwrap();
        }
        let before = history.document().clone();
        let mut draft = DocumentHistory::new_draft(&history);
        switch(
            &mut draft,
            &mut Memory::default(),
            InspectorTarget::DocumentAll,
            RandomCharacterKind::RawUniform,
        );
        let result = history.squash_draft(&draft).unwrap();
        assert_eq!(result.affected_channels, channels);
        let mut expected = before.pattern_definition_bundles().to_vec();
        for bundle in &mut expected {
            for mechanism in &mut bundle.definition.mechanisms {
                if let PatternMechanism::RandomSiteProcess { character, .. } = mechanism {
                    *character = RandomSiteCharacter::RawUniform;
                }
            }
        }
        assert_eq!(history.document().pattern_definition_bundles(), expected);
        for channel in &channels {
            assert_eq!(
                history.document().channel_weighting(*channel),
                before.channel_weighting(*channel)
            );
        }
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
        history.redo().unwrap();
        assert_eq!(history.document(), draft.document());
    }

    /// Restores exact Even spacing across final Apply and a newly opened wizard, including renewed IDs.
    /// Cancel discards private payload memory; explicit replacement starts fresh.
    ///
    /// # Panics
    /// Panics if mode switching loses its prior recipe or changes shared seed/output intent.
    #[test]
    fn scatter_round_trip_survives_apply_reopen_and_copy_on_edit() {
        for target in [
            InspectorTarget::DocumentAll,
            InspectorTarget::Channel(ChannelId(1)),
        ] {
            let mut document = DocumentHistory::new(
                DocumentSession::new(
                    Document::new_default_document(
                        CanvasSpec {
                            width: 100.0,
                            height: 100.0,
                        },
                        SourceReference::Unassigned,
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            let catalog = PresetRegistry::bundled();
            match target {
                InspectorTarget::DocumentAll => catalog
                    .apply_to_document_base(&mut document, "even-random-circles")
                    .unwrap(),
                InspectorTarget::Channel(channel) => catalog
                    .apply_to_selected(&mut document, channel, "even-random-circles")
                    .unwrap(),
            };
            let original = wizard_route_for_document(document.document(), target)
                .unwrap()
                .1;
            let mut committed_memory = Memory::default();
            let mut first_memory = committed_memory.clone();
            let mut first = DocumentHistory::new_draft(&document);
            switch(
                &mut first,
                &mut first_memory,
                target,
                RandomCharacterKind::RawUniform,
            );
            document.squash_draft(&first).unwrap();
            committed_memory = first_memory;
            let mut reopened_memory = committed_memory.clone();
            let mut reopened = DocumentHistory::new_draft(&document);
            switch(
                &mut reopened,
                &mut reopened_memory,
                target,
                RandomCharacterKind::Even,
            );
            assert_eq!(
                wizard_route_for_document(reopened.document(), target)
                    .unwrap()
                    .1,
                original
            );
            document.squash_draft(&reopened).unwrap();
            let mut cancelled_memory = committed_memory.clone();
            cancelled_memory.clear_target(target);
            assert!(!committed_memory.entries.is_empty());
            assert!(cancelled_memory.entries.is_empty());
        }
    }
}
