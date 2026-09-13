//! Atomic property edits across compatible effective channel definitions.

use super::*;

/// Identifies one explicit recipe decision; it is editing intent, never persisted state.
#[derive(Clone, Debug)]
pub enum PatternRecipeEdit {
    GuideCount(u8),
    SiteGeneration(PatternRecipeSiteGenerationKind),
    Construction {
        output: usize,
        kind: PatternRecipeConstructionKind,
    },
    ConnectionMethod {
        output: usize,
        kind: PatternRecipeConnectionMethodKind,
    },
    RegionMethod {
        output: usize,
        kind: PatternRecipeRegionMethodKind,
    },
    OutputKind {
        output: usize,
        kind: PatternRecipeOutputKind,
    },
    RemoveOutput(usize),
    Append(PatternRecipeConstructionKind),
    EditableGuides(CanvasSpec),
}

impl PatternRecipeEdit {
    /// Returns the source painter slot for output-specific decisions.
    fn output_index(&self) -> Option<usize> {
        match self {
            Self::Construction { output, .. }
            | Self::ConnectionMethod { output, .. }
            | Self::RegionMethod { output, .. }
            | Self::OutputKind { output, .. }
            | Self::RemoveOutput(output) => Some(*output),
            _ => None,
        }
    }

    /// Applies this decision to its captured recipe using existing recipe validation authority.
    ///
    /// # Errors
    /// Returns incompatible family, output, cardinality or topology diagnostics without mutation.
    pub fn apply(
        &self,
        recipe: &PatternDefinitionRecipe,
    ) -> Result<PatternDefinitionRecipe, ValidationError> {
        self.apply_at(recipe, self.output_index().unwrap_or(0))
    }

    /// Applies an output-specific decision at its compatible target slot.
    ///
    /// # Errors
    /// Returns the existing recipe transformation diagnostics without allocation or publication.
    fn apply_at(
        &self,
        recipe: &PatternDefinitionRecipe,
        output: usize,
    ) -> Result<PatternDefinitionRecipe, ValidationError> {
        match self {
            Self::GuideCount(count) => recipe.with_guide_family_dimension_count(*count),
            Self::SiteGeneration(kind) => recipe.with_site_generation_kind(*kind),
            Self::Construction { kind, .. } => recipe.with_construction_kind(output, *kind),
            Self::ConnectionMethod { kind, .. } => {
                recipe.with_connection_method_kind(output, *kind)
            }
            Self::RegionMethod { kind, .. } => recipe.with_region_method_kind(output, *kind),
            Self::OutputKind { kind, .. } => recipe.with_output_kind(output, *kind),
            Self::RemoveOutput(_) => recipe.without_output(output),
            Self::Append(kind) => recipe.with_appended_construction_kind(*kind),
            Self::EditableGuides(canvas) => recipe.with_editable_guide_paths(canvas.clone()),
        }
    }
}

impl Document {
    /// Assigns a nested editor's curve to the compatible invoking slot across ALL definitions.
    /// A fresh resource protects unrelated aliases; the complete result remains one history edit.
    ///
    /// # Errors
    /// Returns unsupported slot, authored geometry, or complete candidate validation diagnostics.
    pub fn edit_all_pattern_authored_configuration(
        &self,
        edit: &PatternDefinitionEdit,
        resource: &AuthoredStructureDraft,
    ) -> Result<DocumentConfiguration, ValidationError> {
        let base = self
            .definition(self.pattern_settings.definition_id)
            .ok_or_else(|| {
                ValidationError::new("document.pattern_batch", "missing base definition")
            })?;
        validate_definition_edit(base, edit)?;
        let mut differs = false;
        for id in std::iter::once(base.id).chain(
            self.channel_ids()
                .into_iter()
                .filter_map(|channel| self.pattern_definition_id_for(channel)),
        ) {
            let Some(target) = self.definition(id) else {
                continue;
            };
            let Some(mapped) = corresponding_edit(self, base, target, edit) else {
                continue;
            };
            let Some(current) =
                authored_edit_resource(target, &mapped).and_then(|id| self.authored_structure(id))
            else {
                continue;
            };
            differs |=
                current.kind() != resource.kind() || current.segments() != resource.segments();
        }
        if !differs {
            return Ok(DocumentConfiguration::capture(self));
        }
        let mut candidate = self.clone();
        let added = materialize_recipe_resource_table(
            std::slice::from_ref(resource),
            &self.authored_structures,
        )?;
        let structure_id = added[0].id();
        let edit = match edit {
            PatternDefinitionEdit::SetGuideAuthoredStructure {
                mechanism_id,
                dimension_id,
                ..
            } => PatternDefinitionEdit::SetGuideAuthoredStructure {
                mechanism_id: *mechanism_id,
                dimension_id: *dimension_id,
                structure_id,
            },
            PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                output_layer_id, ..
            } => PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                output_layer_id: *output_layer_id,
                structure_id,
            },
            PatternDefinitionEdit::SetCurveMotifAuthoredStructure {
                output_layer_id, ..
            } => PatternDefinitionEdit::SetCurveMotifAuthoredStructure {
                output_layer_id: *output_layer_id,
                structure_id,
            },
            _ => {
                return Err(ValidationError::new(
                    "document.pattern_batch",
                    "nested edit requires an authored curve slot",
                ));
            }
        };
        candidate.authored_structures.extend(added);
        let base = candidate
            .definition(candidate.pattern_settings.definition_id)
            .ok_or_else(|| {
                ValidationError::new("document.pattern_batch", "missing base definition")
            })?;
        candidate.edit_all_pattern_definition_configuration(base, &edit)
    }

    /// Applies one recipe decision to every compatible effective definition as a single configuration.
    /// Each target starts from its own recipe. Surviving output IDs, resource aliases, channel deltas
    /// and compatible End values remain authoritative; only unavailable intent is pruned.
    ///
    /// # Errors
    /// Returns source-edit, allocation or complete-document validation diagnostics without mutation.
    pub fn edit_all_pattern_recipe_configuration(
        &self,
        edit: &PatternRecipeEdit,
    ) -> Result<DocumentConfiguration, ValidationError> {
        let base_id = self.pattern_settings.definition_id;
        let source = self.reconstruct_pattern_definition_recipe(base_id)?;
        edit.apply(&source)?;
        let source_kinds = source.construction_kinds()?;
        let mut ids = BTreeSet::from([base_id]);
        for channel in self.channel_ids() {
            ids.insert(self.effective_channel_pattern(channel)?.definition_id);
        }
        let mut candidate = self.clone();
        for id in ids {
            let (recipe, resources) = candidate.reconstruct_pattern_recipe_with_resources(id)?;
            let target_index = if let Some(index) = edit.output_index() {
                let Some(kind) = source_kinds.get(index) else {
                    continue;
                };
                let ordinal = source_kinds[..index]
                    .iter()
                    .filter(|value| *value == kind)
                    .count();
                let Some(index) = recipe
                    .construction_kinds()?
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| *value == kind)
                    .nth(ordinal)
                    .map(|(index, _)| index)
                else {
                    continue;
                };
                index
            } else {
                0
            };
            let replacement = match edit.apply_at(&recipe, target_index) {
                Ok(value) => value,
                Err(_) if id != base_id => continue,
                Err(error) => return Err(error),
            };
            if replacement == recipe {
                continue;
            }
            let old = candidate
                .bundle(id)
                .ok_or_else(|| {
                    ValidationError::new("document.pattern_batch", "missing effective recipe")
                })?
                .clone();
            let mut materialized =
                candidate.materialize_partial_pattern_recipe(&replacement, &resources)?;
            materialized.bundle.definition.id = id;
            let retained = old
                .definition
                .output_layers
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    !matches!(edit, PatternRecipeEdit::RemoveOutput(_)) || *index != target_index
                })
                .map(|(_, output)| output.id)
                .collect::<Vec<_>>();
            let output_ids = materialized
                .bundle
                .definition
                .output_layers
                .iter()
                .enumerate()
                .map(|(index, output)| {
                    (output.id, retained.get(index).copied().unwrap_or(output.id))
                })
                .collect::<BTreeMap<_, _>>();
            for output in &mut materialized.bundle.definition.output_layers {
                output.id = output_ids[&output.id];
                output.source_filter = match output.source_filter {
                    SiteUseFilter::All => SiteUseFilter::All,
                    SiteUseFilter::SitesUsedBy { output_layer_id } => SiteUseFilter::SitesUsedBy {
                        output_layer_id: output_ids[&output_layer_id],
                    },
                    SiteUseFilter::SitesUnusedBy { output_layer_id } => {
                        SiteUseFilter::SitesUnusedBy {
                            output_layer_id: output_ids[&output_layer_id],
                        }
                    }
                };
            }
            for setting in &mut materialized.bundle.output_settings {
                setting.output_layer_id = output_ids[&setting.output_layer_id];
            }
            candidate
                .authored_structures
                .extend(materialized.authored_structures);
            let index = candidate
                .pattern_definition_bundles
                .iter()
                .position(|bundle| bundle.definition.id == id)
                .ok_or_else(|| {
                    ValidationError::new("document.pattern_batch", "missing replacement slot")
                })?;
            candidate.pattern_definition_bundles[index] = materialized.bundle;
            for channel in candidate.linked_channels(id) {
                candidate.prune_incompatible_channel_pattern_deltas(channel);
            }
        }
        temporal::reconcile_end_after_pattern_structure(self, &mut candidate)?;
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }

    /// Materializes a partial recipe while reusing unchanged ordered authored resources by identity.
    /// Newly appended or changed resources receive fresh IDs and never overwrite other aliases.
    ///
    /// # Errors
    /// Returns resource allocation or recipe validation diagnostics without publication.
    fn materialize_partial_pattern_recipe(
        &self,
        recipe: &PatternDefinitionRecipe,
        old_ids: &[AuthoredStructureId],
    ) -> Result<MaterializedPatternDefinitionRecipe, ValidationError> {
        let PatternStructureRecipe::AuthoredResources {
            resources,
            definition,
        } = &recipe.structure
        else {
            return self.allocate_definition_from_recipe(None, recipe);
        };
        let mut candidate = self.clone();
        let mut ids = Vec::new();
        let mut added = Vec::new();
        for (index, resource) in resources.iter().enumerate() {
            if let Some(id) = old_ids.get(index)
                && let Some(existing) = candidate.authored_structure(*id)
                && existing.kind() == resource.kind()
                && existing.segments() == resource.segments()
            {
                ids.push(*id);
                continue;
            }
            let entries = materialize_recipe_resource_table(
                std::slice::from_ref(resource),
                &candidate.authored_structures,
            )?;
            ids.extend(entries.iter().map(AuthoredStructure::id));
            candidate.authored_structures.extend(entries.clone());
            added.extend(entries);
        }
        let unwrapped = PatternDefinitionRecipe {
            structure: *definition.clone(),
            output_settings: recipe.output_settings.clone(),
        };
        let mut materialized =
            candidate.allocate_definition_from_recipe_with_resource_ids(None, &unwrapped, &ids)?;
        materialized.authored_structures.splice(0..0, added);
        Ok(materialized)
    }

    /// Assigns one structural setting to its compatible slot in every active channel definition.
    ///
    /// Descriptor fields locate compatible slots; typed kind/ordinal maps resolve their references.
    /// Shared definitions are edited once; independent channel copies retain their source mappings,
    /// seeds, output settings and unrelated mechanisms. Definitions without that slot are omitted.
    /// Existing typed commands reconcile dependent End values before the configuration is captured.
    /// No document or history is mutated; callers publish the result as one configuration transition.
    ///
    /// # Errors
    /// Returns a stale base/slot diagnostic or a typed edit/complete-document validation error.
    pub fn edit_all_pattern_definition_configuration(
        &self,
        base_definition: &PatternDefinition,
        edit: &PatternDefinitionEdit,
    ) -> Result<DocumentConfiguration, ValidationError> {
        if self.definition(base_definition.id) != Some(base_definition) {
            return Err(ValidationError::new(
                "document.pattern_batch",
                "the ALL Pattern definition is stale",
            ));
        }
        validate_definition_edit(base_definition, edit)?;
        let mut definitions =
            BTreeSet::from([base_definition.id, self.pattern_settings.definition_id]);
        for channel in self.channel_ids() {
            definitions.insert(self.effective_channel_pattern(channel)?.definition_id);
        }
        let mut candidate = self.clone();
        for definition_id in definitions {
            let definition = candidate.definition(definition_id).ok_or_else(|| {
                ValidationError::new("document.pattern_batch", "an effective Pattern is absent")
            })?;
            let Some(edit) = corresponding_edit(&candidate, base_definition, definition, edit)
            else {
                continue;
            };
            if validate_definition_edit(definition, &edit).is_err() {
                continue;
            }
            let mut changed = definition.clone();
            apply_definition_edit(&mut changed, &edit);
            if &changed == definition {
                continue;
            }
            let command = DocumentCommand::EditSharedPatternDefinition {
                definition_id,
                base_definition: definition.clone(),
                edit,
            };
            if candidate.linked_channels(definition_id).is_empty() {
                // The base remains editable after every channel selects its own definition.
                // Slot validation above and complete validation below own this unpublished edit.
                command.apply_to_valid_document(&mut candidate);
            } else {
                candidate = candidate.apply_command(&command)?.0;
            }
        }
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }

    /// Propagates every changed nested curve from this child snapshot across compatible channel slots.
    /// Resource selection is irrelevant: edited and newly attached curves are derived from authored
    /// state, so switching the resource list cannot drop earlier edits. Existing child changes remain.
    ///
    /// # Errors
    /// Returns stale resource, slot, or complete-document validation diagnostics without mutation.
    pub fn propagate_all_pattern_authored_changes(
        &self,
        before: &Document,
        edited: &Document,
        edit_order: &[AuthoredStructureId],
    ) -> Result<DocumentConfiguration, ValidationError> {
        let changed = edited
            .authored_structures
            .iter()
            .filter(|resource| before.authored_structure(resource.id()) != Some(resource))
            .map(AuthoredStructure::id)
            .collect::<BTreeSet<_>>();
        let mut ids = BTreeSet::from([self.pattern_settings.definition_id]);
        for channel in self.channel_ids() {
            ids.insert(self.effective_channel_pattern(channel)?.definition_id);
        }
        let mut edits = Vec::new();
        for id in ids {
            let Some(definition) = self.definition(id) else {
                continue;
            };
            for mechanism in &definition.mechanisms {
                if let PatternMechanism::GuideDimensions {
                    id: mechanism_id,
                    dimensions,
                } = mechanism
                {
                    for dimension in dimensions {
                        if let GuidePrototype::AuthoredOpenPath { structure_id } =
                            dimension.prototype
                            && changed.contains(&structure_id)
                        {
                            edits.push((
                                id,
                                PatternDefinitionEdit::SetGuidePrototype {
                                    mechanism_id: *mechanism_id,
                                    dimension_id: dimension.id,
                                    prototype: dimension.prototype.clone(),
                                },
                            ));
                        }
                    }
                }
            }
            for output in &definition.output_layers {
                match &output.realization {
                    PatternOutputRealization::MarkPrototype {
                        prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                        ..
                    } if changed.contains(structure_id) => edits.push((
                        id,
                        PatternDefinitionEdit::SetOutputMarkPrototype {
                            output_layer_id: output.id,
                            prototype: MarkPrototype::AuthoredClosedShape {
                                structure_id: *structure_id,
                            },
                        },
                    )),
                    PatternOutputRealization::CurveMotifPaths { structure_id, .. }
                        if changed.contains(structure_id) =>
                    {
                        edits.push((
                            id,
                            PatternDefinitionEdit::SetCurveMotifAuthoredStructure {
                                output_layer_id: output.id,
                                structure_id: *structure_id,
                            },
                        ))
                    }
                    _ => {}
                }
            }
        }
        edits.sort_by_key(|(_, edit)| {
            let id = match edit {
                PatternDefinitionEdit::SetGuidePrototype {
                    prototype: GuidePrototype::AuthoredOpenPath { structure_id },
                    ..
                }
                | PatternDefinitionEdit::SetOutputMarkPrototype {
                    prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                    ..
                }
                | PatternDefinitionEdit::SetCurveMotifAuthoredStructure { structure_id, .. } => {
                    *structure_id
                }
                _ => return 0,
            };
            edit_order
                .iter()
                .rposition(|value| *value == id)
                .map_or(0, |index| index + 1)
        });
        let mut candidate = self.clone();
        for (id, edit) in edits {
            let source = candidate
                .definition(id)
                .ok_or_else(|| {
                    ValidationError::new(
                        "document.pattern_batch",
                        "missing nested source definition",
                    )
                })?
                .clone();
            candidate = candidate
                .edit_all_pattern_definition_configuration(&source, &edit)?
                .bind(&candidate)?;
        }
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }
}

/// Rebinds edited slots by descriptor authority and dependencies by kind/ordinal.
/// Unrelated mechanisms and outputs need not match, and external authored resources keep their IDs.
fn corresponding_edit(
    document: &Document,
    source: &PatternDefinition,
    target: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> Option<PatternDefinitionEdit> {
    let mut mechanisms = BTreeMap::new();
    let mut dimensions = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    for (index, source_mechanism) in source.mechanisms.iter().enumerate() {
        let kind = mechanism_kind(source_mechanism);
        let ordinal = source.mechanisms[..index]
            .iter()
            .filter(|mechanism| mechanism_kind(mechanism) == kind)
            .count();
        let Some(target_mechanism) = target
            .mechanisms
            .iter()
            .filter(|mechanism| mechanism_kind(mechanism) == kind)
            .nth(ordinal)
        else {
            continue;
        };
        mechanisms.insert(source_mechanism.id(), target_mechanism.id());
        let source_dimensions = dimension_ids(source_mechanism);
        let target_dimensions = dimension_ids(target_mechanism);
        dimensions.extend(source_dimensions.into_iter().zip(target_dimensions));
    }
    for (index, source_output) in source.output_layers.iter().enumerate() {
        let kind = output_kind(source_output);
        let ordinal = source.output_layers[..index]
            .iter()
            .filter(|output| output_kind(output) == kind)
            .count();
        if let Some(target_output) = target
            .output_layers
            .iter()
            .filter(|output| output_kind(output) == kind)
            .nth(ordinal)
        {
            outputs.insert(source_output.id(), target_output.id());
        }
    }
    // Descriptor fields locate the edited slot; broader kind maps above resolve its dependencies.
    // This prevents an earlier unrelated path subtype from hiding a later compatible motif slot.
    if !matches!(
        edit,
        PatternDefinitionEdit::SetCurveMotifAuthoredStructure { .. }
            | PatternDefinitionEdit::SetGuideFaceDimensions { .. }
    ) {
        let referenced_mechanisms = std::cell::RefCell::new(BTreeSet::new());
        let referenced_dimensions = std::cell::RefCell::new(BTreeSet::new());
        let referenced_outputs = std::cell::RefCell::new(BTreeSet::new());
        remap_definition_edit_with(
            edit,
            &|id| {
                referenced_mechanisms.borrow_mut().insert(id);
                id
            },
            &|id| {
                referenced_dimensions.borrow_mut().insert(id);
                id
            },
            &|id| {
                referenced_outputs.borrow_mut().insert(id);
                id
            },
        );
        let field = edit.field_projection().field;
        let descriptors = document.property_descriptors();
        let targets = |id| {
            descriptors
                .iter()
                .filter(move |descriptor| {
                    descriptor.field == field
                        && match descriptor.target {
                            PropertyTarget::Definition(definition)
                            | PropertyTarget::Mechanism(definition, _)
                            | PropertyTarget::GuideDimension(definition, ..)
                            | PropertyTarget::OutputLayer(definition, _) => definition == id,
                            _ => false,
                        }
                })
                .map(|descriptor| descriptor.target)
                .collect::<Vec<_>>()
        };
        let source_targets = targets(source.id);
        let index = source_targets.iter().position(|target| match target {
            PropertyTarget::Definition(_) => true,
            PropertyTarget::Mechanism(_, id) => referenced_mechanisms.borrow().contains(id),
            PropertyTarget::GuideDimension(_, mechanism, dimension) => {
                referenced_mechanisms.borrow().contains(mechanism)
                    && referenced_dimensions.borrow().contains(dimension)
            }
            PropertyTarget::OutputLayer(_, id) => referenced_outputs.borrow().contains(id),
            _ => false,
        })?;
        let source_target = source_targets[index];
        let category = std::mem::discriminant(&source_target);
        let ordinal = source_targets[..index]
            .iter()
            .filter(|target| std::mem::discriminant(*target) == category)
            .count();
        let target = targets(target.id)
            .into_iter()
            .filter(|target| std::mem::discriminant(target) == category)
            .nth(ordinal)?;
        match (source_target, target) {
            (PropertyTarget::Mechanism(_, source), PropertyTarget::Mechanism(_, target)) => {
                mechanisms.insert(source, target);
            }
            (
                PropertyTarget::GuideDimension(_, source_m, source_d),
                PropertyTarget::GuideDimension(_, target_m, target_d),
            ) => {
                mechanisms.insert(source_m, target_m);
                dimensions.insert(source_d, target_d);
            }
            (PropertyTarget::OutputLayer(_, source), PropertyTarget::OutputLayer(_, target)) => {
                outputs.insert(source, target);
            }
            _ => {}
        }
    } else {
        // Non-descriptor resource/face operations require their exact active output subtype.
        for (index, output) in source.output_layers.iter().enumerate() {
            let kind = std::mem::discriminant(&output.realization);
            let ordinal = source.output_layers[..index]
                .iter()
                .filter(|output| std::mem::discriminant(&output.realization) == kind)
                .count();
            outputs.remove(&output.id);
            if let Some(target_output) = target
                .output_layers
                .iter()
                .filter(|output| std::mem::discriminant(&output.realization) == kind)
                .nth(ordinal)
            {
                outputs.insert(output.id, target_output.id);
            }
        }
    }
    let missing = std::cell::Cell::new(false);
    let edit = remap_definition_edit_with(
        edit,
        &|id| mapped_id(&mechanisms, id, &missing),
        &|id| mapped_id(&dimensions, id, &missing),
        &|id| mapped_id(&outputs, id, &missing),
    );
    (!missing.get()).then_some(edit)
}

/// Extracts typed guide dimensions in stored order without conflating guide and non-guide mechanisms.
fn dimension_ids(mechanism: &PatternMechanism) -> Vec<GuideDimensionId> {
    match mechanism {
        PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
            dimensions.iter().map(|d| d.id).collect()
        }
        PatternMechanism::GuideDimensions { dimensions, .. } => {
            dimensions.iter().map(|d| d.id).collect()
        }
        _ => Vec::new(),
    }
}

/// Marks an unavailable correspondence; the caller discards the entire remapped edit before use.
fn mapped_id<T: Copy + Ord>(map: &BTreeMap<T, T>, id: T, missing: &std::cell::Cell<bool>) -> T {
    map.get(&id).copied().unwrap_or_else(|| {
        missing.set(true);
        id
    })
}

/// Reads the resource currently assigned to an active nested-editor slot without following other aliases.
fn authored_edit_resource(
    definition: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> Option<AuthoredStructureId> {
    match edit {
        PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id,
            dimension_id,
            ..
        } => definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::GuideDimensions { id, dimensions } if id == mechanism_id => {
                    dimensions
                        .iter()
                        .find(|dimension| dimension.id == *dimension_id)
                        .and_then(|dimension| match dimension.prototype {
                            GuidePrototype::AuthoredOpenPath { structure_id } => Some(structure_id),
                            _ => None,
                        })
                }
                _ => None,
            }),
        PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id, ..
        } => definition
            .output_layers
            .iter()
            .find(|output| output.id == *output_layer_id)
            .and_then(|output| match output.realization {
                PatternOutputRealization::MarkPrototype {
                    prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                    ..
                } => Some(structure_id),
                _ => None,
            }),
        PatternDefinitionEdit::SetCurveMotifAuthoredStructure {
            output_layer_id, ..
        } => definition
            .output_layers
            .iter()
            .find(|output| output.id == *output_layer_id)
            .and_then(|output| match output.realization {
                PatternOutputRealization::CurveMotifPaths { structure_id, .. } => {
                    Some(structure_id)
                }
                _ => None,
            }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps an unlinked ALL base editable while assigning each active independent copy.
    ///
    /// # Panics
    /// Panics if shared-command audience admission leaks into the ALL assignment boundary.
    #[test]
    fn unlinked_base_still_assigns_every_compatible_channel() {
        let mut history = independent_guides();
        for channel in history.document().channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let recipe = history
                .document()
                .reconstruct_pattern_definition_recipe(id)
                .unwrap();
            history
                .apply(
                    &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                        base: history.document().pattern_settings().clone(),
                        channel_id: channel,
                        base_definition: history.document().definition(id).unwrap().clone(),
                        recipe,
                    },
                )
                .unwrap();
        }
        let base = history
            .document()
            .definition(history.document().pattern_settings.definition_id)
            .unwrap()
            .clone();
        assert!(history.document().linked_channels(base.id).is_empty());
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::GuideCount(2))
            .unwrap();
        publish(&mut history, configuration);
        let base = history.document().definition(base.id).unwrap().clone();
        let (mechanism_id, dimension_id) = base
            .mechanisms
            .iter()
            .find_map(|mechanism| {
                dimension_ids(mechanism)
                    .first()
                    .map(|id| (mechanism.id(), *id))
            })
            .unwrap();
        let configuration = history
            .document()
            .edit_all_pattern_definition_configuration(
                &base,
                &PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id,
                    dimension_id,
                    phase: 0.77,
                },
            )
            .unwrap();
        publish(&mut history, configuration);
        for channel in history.document().channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let value = history.document().property_values().into_iter().find(|value| value.descriptor.field == PropertyFieldId::GuidePhase && matches!(value.descriptor.target, PropertyTarget::GuideDimension(definition, ..) if definition == id)).unwrap();
            assert_eq!(value.value, PropertyCurrentValueKind::FiniteF64(0.77));
        }
    }

    /// Propagates two edited guide resources without relying on the resource selected at Apply.
    ///
    /// # Panics
    /// Panics if either slot is dropped, the resource list controls publication, or Undo is partial.
    #[test]
    fn all_nested_resource_changes_survive_one_parent_apply() {
        let mut history = independent_guides();
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::GuideCount(2))
            .unwrap();
        publish(&mut history, configuration);
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::EditableGuides(
                history.document().canvas().clone(),
            ))
            .unwrap();
        publish(&mut history, configuration);
        let before = history.document().clone();
        let mut child = DocumentHistory::new_draft(&history);
        let definition = before
            .definition(before.pattern_settings.definition_id)
            .unwrap();
        let resources = definition
            .mechanisms
            .iter()
            .flat_map(|mechanism| match mechanism {
                PatternMechanism::GuideDimensions { dimensions, .. } => dimensions
                    .iter()
                    .filter_map(|dimension| match dimension.prototype {
                        GuidePrototype::AuthoredOpenPath { structure_id } => Some(structure_id),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                _ => vec![],
            })
            .collect::<Vec<_>>();
        for (index, id) in resources.iter().enumerate() {
            let replacement = AuthoredStructureDraft::new(
                AuthoredStructureKind::OpenPath,
                vec![AuthoredCurveSegment::Line {
                    start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                    end: AuthoredPoint2 {
                        x: 60.0,
                        y: 10.0 + index as f64 * 20.0,
                    },
                }],
            )
            .unwrap();
            child
                .apply(&DocumentCommand::ReplaceAuthoredStructure {
                    base_structure: child.document().authored_structure(*id).unwrap().clone(),
                    replacement,
                })
                .unwrap();
        }
        let configuration = child
            .document()
            .propagate_all_pattern_authored_changes(&before, child.document(), &resources)
            .unwrap();
        publish(&mut child, configuration);
        history.squash_draft(&child).unwrap();
        for channel in history.document().channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let target = history.document().definition(id).unwrap();
            for (index, source_resource) in resources.iter().enumerate() {
                let target_resource = target
                    .mechanisms
                    .iter()
                    .find_map(|mechanism| match mechanism {
                        PatternMechanism::GuideDimensions { dimensions, .. } => match dimensions
                            [index]
                            .prototype
                        {
                            GuidePrototype::AuthoredOpenPath { structure_id } => Some(structure_id),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(target_resource, *source_resource);
            }
        }
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
    }

    /// Creates three independently edited guide recipes with distinct phases and initialized End values.
    ///
    /// # Panics
    /// Panics if canonical recipe, copy-on-edit or timing setup violates domain authority.
    fn independent_guides() -> DocumentHistory {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let recipe = PatternDefinitionRecipe::starter_for_family(PatternRecipeFamilyKind::Guides)
            .with_appended_construction_kind(PatternRecipeConstructionKind::Connections)
            .unwrap();
        history
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base: history.document().pattern_settings().clone(),
                base_definition: history
                    .document()
                    .definition(history.document().pattern_settings.definition_id)
                    .unwrap()
                    .clone(),
                recipe,
            })
            .unwrap();
        for (index, channel) in history.document().channel_ids().into_iter().enumerate() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let definition = history.document().definition(id).unwrap().clone();
            let (mechanism_id, dimension_id) = definition
                .mechanisms
                .iter()
                .find_map(|mechanism| {
                    dimension_ids(mechanism)
                        .first()
                        .map(|dimension| (mechanism.id(), *dimension))
                })
                .unwrap();
            history
                .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                    channel_id: channel,
                    base_definition: definition,
                    edit: PatternDefinitionEdit::SetGuidePhase {
                        mechanism_id,
                        dimension_id,
                        phase: (index + 1) as f64 * 0.1,
                    },
                })
                .unwrap();
        }
        let timing = ProjectTiming::new(
            FrameRate::new(24, 1).unwrap(),
            FrameRange::new(0, 3).unwrap(),
        );
        let command = history
            .document()
            .replace_temporal_authority_command(timing, vec![]);
        history.apply_temporal(&command).unwrap();
        let command = history.document().initialize_end_command().unwrap();
        history.apply_temporal(&command).unwrap();
        history
    }

    /// Publishes a configuration against its exact source as one undoable assignment.
    ///
    /// # Panics
    /// Panics if candidate binding or history publication fails.
    fn publish(history: &mut DocumentHistory, configuration: DocumentConfiguration) {
        let before = history.document().clone();
        history
            .apply_document_configuration(&before, history.revision(), &configuration)
            .unwrap();
    }

    /// Preserves each channel's untouched phase, output identities and End state when ALL adds a guide.
    ///
    /// # Panics
    /// Panics on lost channel intent, incomplete fanout or non-atomic Undo/Redo.
    #[test]
    fn recipe_fanout_preserves_untouched_channels_outputs_and_end() {
        let mut history = independent_guides();
        let before = history.document().clone();
        let configuration = before
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::GuideCount(2))
            .unwrap();
        publish(&mut history, configuration);
        for channel in before.channel_ids() {
            let id = before
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let old = before.definition(id).unwrap();
            let new = history.document().definition(id).unwrap();
            assert_eq!(
                old.output_layers
                    .iter()
                    .map(|output| output.id)
                    .collect::<Vec<_>>(),
                new.output_layers
                    .iter()
                    .map(|output| output.id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                new.mechanisms
                    .iter()
                    .map(|mechanism| dimension_ids(mechanism).len())
                    .sum::<usize>(),
                2
            );
            let phases = |document: &Document| {
                document.property_values().into_iter().filter(|value|
                value.descriptor.field == PropertyFieldId::GuidePhase && matches!(value.descriptor.target, PropertyTarget::GuideDimension(definition, ..) if definition == id)
            ).map(|value| value.value).collect::<Vec<_>>()
            };
            assert_eq!(phases(&before)[0], phases(history.document())[0]);
        }
        assert_eq!(
            history.document().temporal_end_overrides(),
            before.temporal_end_overrides()
        );
        let after = history.document().clone();
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
        history.redo().unwrap();
        assert_eq!(history.document(), &after);
    }

    /// Propagates compatible guide fields across straight and authored producers and retains resources.
    ///
    /// # Panics
    /// Panics if semantic matching skips a producer, aliases duplicate or unrelated values change.
    #[test]
    fn heterogeneous_guide_assignment_and_recipe_resource_identity() {
        let mut history = independent_guides();
        // Only one copy becomes generic first, so the ALL phase edit crosses producer types.
        let channel = history.document().channel_ids()[0];
        let id = history
            .document()
            .effective_channel_pattern(channel)
            .unwrap()
            .definition_id;
        let recipe = history
            .document()
            .reconstruct_pattern_definition_recipe(id)
            .unwrap()
            .with_editable_guide_paths(history.document().canvas().clone())
            .unwrap();
        history
            .apply(
                &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                    base: history.document().pattern_settings().clone(),
                    channel_id: channel,
                    base_definition: history.document().definition(id).unwrap().clone(),
                    recipe,
                },
            )
            .unwrap();
        let base = history
            .document()
            .definition(history.document().pattern_settings.definition_id)
            .unwrap()
            .clone();
        let (mechanism_id, dimension_id) = base
            .mechanisms
            .iter()
            .find_map(|mechanism| {
                dimension_ids(mechanism)
                    .first()
                    .map(|id| (mechanism.id(), *id))
            })
            .unwrap();
        let configuration = history
            .document()
            .edit_all_pattern_definition_configuration(
                &base,
                &PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id,
                    dimension_id,
                    phase: 0.65,
                },
            )
            .unwrap();
        publish(&mut history, configuration);
        let active_ids = history
            .document()
            .channel_ids()
            .into_iter()
            .map(|channel| {
                history
                    .document()
                    .effective_channel_pattern(channel)
                    .unwrap()
                    .definition_id
            })
            .collect::<BTreeSet<_>>();
        assert!(history.document().property_values().iter().filter(|value| value.descriptor.field == PropertyFieldId::GuidePhase && matches!(value.descriptor.target, PropertyTarget::GuideDimension(id, ..) if active_ids.contains(&id))).all(|value| value.value == PropertyCurrentValueKind::FiniteF64(0.65)));
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::EditableGuides(
                history.document().canvas().clone(),
            ))
            .unwrap();
        publish(&mut history, configuration);
        let resources = history.document().authored_structures().to_vec();
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::GuideCount(2))
            .unwrap();
        publish(&mut history, configuration);
        assert!(resources.iter().all(
            |resource| history.document().authored_structure(resource.id()) == Some(resource)
        ));
        let base = history
            .document()
            .definition(history.document().pattern_settings.definition_id)
            .unwrap()
            .clone();
        let (mechanism_id, dimension_id) = base
            .mechanisms
            .iter()
            .find_map(|mechanism| {
                dimension_ids(mechanism)
                    .first()
                    .map(|id| (mechanism.id(), *id))
            })
            .unwrap();
        let configuration = history
            .document()
            .edit_all_pattern_definition_configuration(
                &base,
                &PatternDefinitionEdit::SetGuidePhase {
                    mechanism_id,
                    dimension_id,
                    phase: 0.75,
                },
            )
            .unwrap();
        publish(&mut history, configuration);
        for channel in history.document().channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let phase = history.document().property_values().into_iter().find(|value|
                value.descriptor.field == PropertyFieldId::GuidePhase && matches!(value.descriptor.target, PropertyTarget::GuideDimension(definition, ..) if definition == id)
            ).unwrap();
            assert_eq!(phase.value, PropertyCurrentValueKind::FiniteF64(0.75));
        }
    }

    /// Moves a compatible output relative to its neighbor across copies and removes only that slot.
    ///
    /// # Panics
    /// Panics if output order, unrelated End fields or surviving identities are lost.
    #[test]
    fn output_order_and_removal_fanout_keep_surviving_intent() {
        let mut history = independent_guides();
        let base = history
            .document()
            .bundle(history.document().pattern_settings.definition_id)
            .unwrap()
            .clone();
        let configuration = history
            .document()
            .edit_all_pattern_bundle_configuration(
                &base,
                &PatternDefinitionBundleEdit::MoveOutputLayer {
                    output_layer_id: base.definition.output_layers[0].id,
                    painter_index: 1,
                },
            )
            .unwrap();
        publish(&mut history, configuration);
        for channel in history.document().channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            assert_eq!(
                output_kind(&history.document().definition(id).unwrap().output_layers[0]),
                1
            );
        }
        let before = history.document().clone();
        let configuration = before
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::RemoveOutput(0))
            .unwrap();
        publish(&mut history, configuration);
        for channel in before.channel_ids() {
            let id = before
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            assert_eq!(
                history.document().definition(id).unwrap().output_layers[0].id,
                before.definition(id).unwrap().output_layers[1].id
            );
        }
        history
            .document()
            .materialize_frame(2)
            .unwrap()
            .validate()
            .unwrap();
    }

    /// Assigns a changed nested curve across independent copies without touching other guide slots.
    ///
    /// # Panics
    /// Panics on extra no-op resources, missed copies, changed aliases or partial history publication.
    #[test]
    fn nested_curve_fanout_preserves_other_resources_and_ignores_noop() {
        let mut history = independent_guides();
        let configuration = history
            .document()
            .edit_all_pattern_recipe_configuration(&PatternRecipeEdit::EditableGuides(
                history.document().canvas().clone(),
            ))
            .unwrap();
        publish(&mut history, configuration);
        let before = history.document().clone();
        let base = before
            .definition(before.pattern_settings.definition_id)
            .unwrap();
        let (mechanism_id, dimension_id) = base
            .mechanisms
            .iter()
            .find_map(|mechanism| {
                dimension_ids(mechanism)
                    .first()
                    .map(|id| (mechanism.id(), *id))
            })
            .unwrap();
        let edit = PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id,
            dimension_id,
            structure_id: AuthoredStructureId(0),
        };
        let current = before
            .authored_structure(authored_edit_resource(base, &edit).unwrap())
            .unwrap();
        let resource =
            AuthoredStructureDraft::new(current.kind(), current.segments().to_vec()).unwrap();
        assert_eq!(
            before
                .edit_all_pattern_authored_configuration(&edit, &resource)
                .unwrap(),
            DocumentConfiguration::capture(&before)
        );
        let resource = AuthoredStructureDraft::new(
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 80.0, y: 35.0 },
            }],
        )
        .unwrap();
        publish(
            &mut history,
            before
                .edit_all_pattern_authored_configuration(&edit, &resource)
                .unwrap(),
        );
        for channel in before.channel_ids() {
            let id = history
                .document()
                .effective_channel_pattern(channel)
                .unwrap()
                .definition_id;
            let target = history.document().definition(id).unwrap();
            let mapped = corresponding_edit(history.document(), base, target, &edit).unwrap();
            let current = history
                .document()
                .authored_structure(authored_edit_resource(target, &mapped).unwrap())
                .unwrap();
            assert_eq!(current.segments(), resource.segments());
        }
        assert!(before.authored_structures().iter().all(|resource| {
            history.document().authored_structure(resource.id()) == Some(resource)
        }));
        history.undo().unwrap();
        assert_eq!(history.document(), &before);
    }
}

/// Groups guide producers by their shared editable dimension contract.
fn mechanism_kind(value: &PatternMechanism) -> u8 {
    match value {
        PatternMechanism::StraightGuides { .. } => 0,
        PatternMechanism::GuideIntersections { .. } => 1,
        PatternMechanism::StraightGuideDimensions { .. }
        | PatternMechanism::GuideDimensions { .. } => 2,
        PatternMechanism::SelectedGuideIntersections { .. } => 3,
        PatternMechanism::AlongGuideSites { .. } => 4,
        PatternMechanism::ParametricCurveSource { .. } => 5,
        PatternMechanism::AlongParametricCurveSites { .. } => 6,
        PatternMechanism::RandomSiteProcess { .. } => 7,
        PatternMechanism::SiteDensityModulation { .. } => 8,
        PatternMechanism::SiteExclusion { .. } => 9,
        PatternMechanism::RandomSiteProduct { .. } => 10,
    }
}

/// Groups outputs by shared response capability; typed edit validation checks narrower fields.
fn output_kind(value: &PatternOutputLayer) -> u8 {
    match value.realization {
        PatternOutputRealization::CircularMarks { .. }
        | PatternOutputRealization::MarkPrototype { .. } => 0,
        PatternOutputRealization::GuidePaths { .. }
        | PatternOutputRealization::ParametricPaths { .. }
        | PatternOutputRealization::CurveMotifPaths { .. }
        | PatternOutputRealization::ConnectionPaths { .. }
        | PatternOutputRealization::MazeWalls { .. } => 1,
        PatternOutputRealization::Regions { .. } => 2,
    }
}

/// Resolves one output by response capability and occurrence in authoritative painter order.
fn corresponding_output(
    source: &PatternDefinition,
    target: &PatternDefinition,
    id: PatternOutputLayerId,
) -> Option<PatternOutputLayerId> {
    let index = source
        .output_layers
        .iter()
        .position(|output| output.id == id)?;
    let kind = output_kind(&source.output_layers[index]);
    let ordinal = source.output_layers[..index]
        .iter()
        .filter(|output| output_kind(output) == kind)
        .count();
    target
        .output_layers
        .iter()
        .filter(|output| output_kind(output) == kind)
        .nth(ordinal)
        .map(|output| output.id)
}

impl Document {
    /// Assigns a compatible dependency or relative painter-order change to every active definition.
    /// Shared definitions are edited once and unrelated outputs retain their order and settings.
    /// The returned configuration publishes atomically through ordinary history authority.
    ///
    /// # Errors
    /// Returns a stale source or complete-document validation diagnostic without mutation.
    pub fn edit_all_pattern_bundle_configuration(
        &self,
        base: &PatternDefinitionBundle,
        edit: &PatternDefinitionBundleEdit,
    ) -> Result<DocumentConfiguration, ValidationError> {
        if base.definition.id != self.pattern_settings.definition_id
            || self.bundle(base.definition.id) != Some(base)
        {
            return Err(ValidationError::new(
                "document.pattern_batch",
                "the ALL Pattern bundle is stale",
            ));
        }
        let source_noop = match edit {
            PatternDefinitionBundleEdit::SetSiteUseFilter {
                output_layer_id,
                source_filter,
            } => base.definition.output_layers.iter().any(|output| {
                output.id == *output_layer_id && output.source_filter == *source_filter
            }),
            PatternDefinitionBundleEdit::MoveOutputLayer {
                output_layer_id,
                painter_index,
            } => base
                .definition
                .output_layers
                .get(*painter_index)
                .is_some_and(|output| output.id == *output_layer_id),
            PatternDefinitionBundleEdit::OutputSettings(_) => false,
        };
        if !source_noop {
            edit.apply_to_bundle(&mut base.clone())?;
        }
        let mut ids = BTreeSet::from([base.definition.id]);
        for channel in self.channel_ids() {
            ids.insert(self.effective_channel_pattern(channel)?.definition_id);
        }
        let mut candidate = self.clone();
        for id in ids {
            let bundle = candidate
                .bundle(id)
                .ok_or_else(|| {
                    ValidationError::new("document.pattern_batch", "missing effective bundle")
                })?
                .clone();
            let map = |output| corresponding_output(&base.definition, &bundle.definition, output);
            let mapped = match edit {
                PatternDefinitionBundleEdit::SetSiteUseFilter {
                    output_layer_id,
                    source_filter,
                } => {
                    let Some(output_layer_id) = map(*output_layer_id) else {
                        continue;
                    };
                    let source_filter = match source_filter {
                        SiteUseFilter::All => SiteUseFilter::All,
                        SiteUseFilter::SitesUsedBy { output_layer_id } => {
                            let Some(output_layer_id) = map(*output_layer_id) else {
                                continue;
                            };
                            SiteUseFilter::SitesUsedBy { output_layer_id }
                        }
                        SiteUseFilter::SitesUnusedBy { output_layer_id } => {
                            let Some(output_layer_id) = map(*output_layer_id) else {
                                continue;
                            };
                            SiteUseFilter::SitesUnusedBy { output_layer_id }
                        }
                    };
                    PatternDefinitionBundleEdit::SetSiteUseFilter {
                        output_layer_id,
                        source_filter,
                    }
                }
                PatternDefinitionBundleEdit::MoveOutputLayer {
                    output_layer_id,
                    painter_index,
                } => {
                    let Some(from) = base
                        .definition
                        .output_layers
                        .iter()
                        .position(|output| output.id == *output_layer_id)
                    else {
                        continue;
                    };
                    let Some(anchor) = base
                        .definition
                        .output_layers
                        .get(*painter_index)
                        .and_then(|output| map(output.id))
                    else {
                        continue;
                    };
                    let Some(output_layer_id) = map(*output_layer_id) else {
                        continue;
                    };
                    let mut order: Vec<_> = bundle
                        .definition
                        .output_layers
                        .iter()
                        .map(|output| output.id)
                        .collect();
                    order.retain(|id| *id != output_layer_id);
                    let Some(index) = order.iter().position(|id| *id == anchor) else {
                        continue;
                    };
                    let painter_index = index + usize::from(*painter_index > from);
                    PatternDefinitionBundleEdit::MoveOutputLayer {
                        output_layer_id,
                        painter_index,
                    }
                }
                PatternDefinitionBundleEdit::OutputSettings(_) => {
                    return Err(ValidationError::new(
                        "document.pattern_batch",
                        "output response assignments require a property field",
                    ));
                }
            };
            if mapped.apply_to_bundle(&mut bundle.clone()).is_err() {
                continue;
            }
            let command = DocumentCommand::EditSharedPatternDefinitionBundle {
                definition_id: id,
                base_bundle: bundle,
                edit: mapped,
            };
            if candidate.linked_channels(id).is_empty() {
                command.apply_to_valid_document(&mut candidate);
            } else {
                candidate = candidate.apply_command(&command)?.0;
            }
        }
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }
}
