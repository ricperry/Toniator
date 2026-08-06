//! Metadata-driven local Guided editor support.
//!
//! This is intentionally a definition/instance draft boundary, not a document
//! editor. It turns strict creator metadata into deterministic controls, keeps
//! channel-instance values out of the Pattern Editor, and previews an
//! immutable bundled definition without mutating its registry content. Gate 5
//! owns a shared Guided/Graph draft and document Apply/Cancel lifecycle.

use crate::artwork_pipeline::{ArtworkPipelineSettings, OutputChannelId};
use crate::cancel::CancellationToken;
use crate::definition_runtime::execute_resolved_definition_cancellable;
use crate::pattern::{ArtboardSpace, CanonicalPatternOutput, PATTERN_REGISTRY, PatternId};
use crate::pattern_definition::{
    CreatorParameterCategory, CreatorParameterIncrement, CreatorParameterMetadata,
    CreatorParameterUnit, DefinitionParameterScope, GraphPosition, LiteralValue,
    ParameterApplicability, ParameterAuthoring, ParameterOwnership, PatternDefinition,
    PatternInstanceParameters, PatternParameterConstraints, PatternParameterDefinition,
    TwoDimensionalAxis,
};
use crate::pattern_definition_registry::PatternDefinitionRegistry;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidedWidgetKind {
    BoundedNumeric,
    Integer,
    Angle,
    Percentage,
    NormalizedInfluence,
    DocumentRelativeDistance,
    Toggle,
    Enumeration,
    TwoDimensionalOffset,
    QualityTolerance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedNumericPresentation {
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub digits: u8,
    pub display_multiplier: f64,
    pub unit_suffix: &'static str,
}

impl GuidedNumericPresentation {
    pub fn to_display(&self, stored: f64) -> f64 {
        stored * self.display_multiplier
    }

    pub fn from_display(&self, displayed: f64) -> f64 {
        displayed / self.display_multiplier
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedControlDescriptor {
    pub parameter_id: String,
    pub label: String,
    pub help: String,
    pub group: String,
    pub display_order: u32,
    pub widget: GuidedWidgetKind,
    pub numeric: Option<GuidedNumericPresentation>,
    pub choices: Vec<String>,
    pub applicability: ParameterApplicability,
    pub two_dimensional_pair: Option<(String, TwoDimensionalAxis)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedSection {
    pub id: String,
    pub label: String,
    pub controls: Vec<GuidedControlDescriptor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedDefinitionCatalogEntry {
    pub id: PatternId,
    pub name: String,
    pub summary: String,
    pub editable: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedDefinitionCatalog {
    pub entries: Vec<GuidedDefinitionCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidedDefinitionDraft {
    definition: PatternDefinition,
    instance: PatternInstanceParameters,
    sections: Vec<GuidedSection>,
}

/// One local recipe draft shared by the Guided and Graph editor views.
///
/// It deliberately owns the complete definition and instance payload rather
/// than synthesising a reduced editor model.  This lets a document-local copy
/// retain graph topology, layout, assets, schema, and extension-valid content
/// the current UI does not expose.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedRecipeEditorDraft {
    guided: GuidedDefinitionDraft,
    source: SharedRecipeDraftSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SharedRecipeDraftSource {
    ImmutableBundled(PatternId),
    DocumentLocal(PatternId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuidedEditorError {
    UnsupportedCategory {
        parameter_id: String,
        category: CreatorParameterCategory,
    },
    InvalidMetadata {
        parameter_id: String,
        message: String,
    },
    MissingParameter(String),
    ChannelOwnedParameter(String),
    InvalidValue {
        parameter_id: String,
        message: String,
    },
    UnavailableDefinition {
        id: PatternId,
        reason: String,
    },
    ImmutableBundledDefinition(PatternId),
    MissingGraphNode(String),
}

impl fmt::Display for GuidedEditorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCategory {
                parameter_id,
                category,
            } => write!(
                formatter,
                "Guided editor does not support category {category:?} for `{parameter_id}`"
            ),
            Self::InvalidMetadata {
                parameter_id,
                message,
            } => write!(
                formatter,
                "Guided editor metadata for `{parameter_id}` is invalid: {message}"
            ),
            Self::MissingParameter(parameter_id) => {
                write!(
                    formatter,
                    "Guided editor has no definition parameter `{parameter_id}`"
                )
            }
            Self::ChannelOwnedParameter(parameter_id) => write!(
                formatter,
                "`{parameter_id}` is channel-instance owned and belongs in Channel Settings"
            ),
            Self::InvalidValue {
                parameter_id,
                message,
            } => write!(formatter, "Invalid value for `{parameter_id}`: {message}"),
            Self::UnavailableDefinition { id, reason } => {
                write!(formatter, "Guided editor cannot open `{id}`: {reason}")
            }
            Self::ImmutableBundledDefinition(id) => write!(
                formatter,
                "Bundled definition `{id}` is immutable; duplicate it to a document-local pattern before Apply"
            ),
            Self::MissingGraphNode(node_id) => {
                write!(formatter, "Recipe graph has no positioned node `{node_id}`")
            }
        }
    }
}

impl std::error::Error for GuidedEditorError {}

impl GuidedDefinitionCatalog {
    /// Enumerates registry-resolved definitions by their stable IDs. Entries
    /// whose metadata cannot yet be represented remain visible with an exact
    /// explanation; the catalog never guesses a widget from a display name.
    pub fn from_registry(registry: &PatternDefinitionRegistry) -> Self {
        let entries = registry
            .definitions()
            .map(|resolved| {
                match GuidedDefinitionDraft::new(
                    resolved.definition.clone(),
                    resolved
                        .definition
                        .default_instance_parameters(OutputChannelId::CMYK)
                        .expect("validated registry definition must create a current instance"),
                ) {
                    Ok(_) => GuidedDefinitionCatalogEntry {
                        id: resolved.definition.id.clone(),
                        name: resolved.definition.display.name.clone(),
                        summary: resolved.definition.display.summary.clone(),
                        editable: true,
                        unavailable_reason: None,
                    },
                    Err(error) => GuidedDefinitionCatalogEntry {
                        id: resolved.definition.id.clone(),
                        name: resolved.definition.display.name.clone(),
                        summary: resolved.definition.display.summary.clone(),
                        editable: false,
                        unavailable_reason: Some(error.to_string()),
                    },
                }
            })
            .collect();
        Self { entries }
    }

    pub fn entry(&self, id: &PatternId) -> Option<&GuidedDefinitionCatalogEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn open(
        &self,
        registry: &PatternDefinitionRegistry,
        id: &PatternId,
        channels: impl IntoIterator<Item = OutputChannelId>,
    ) -> Result<GuidedDefinitionDraft, GuidedEditorError> {
        let entry = self
            .entry(id)
            .ok_or_else(|| GuidedEditorError::MissingParameter(id.to_string()))?;
        if !entry.editable {
            return Err(GuidedEditorError::UnavailableDefinition {
                id: id.clone(),
                reason: entry
                    .unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "unsupported creator metadata".into()),
            });
        }
        let definition = registry
            .get(id)
            .map_err(|error| GuidedEditorError::UnavailableDefinition {
                id: id.clone(),
                reason: error.to_string(),
            })?
            .definition
            .clone();
        let instance = definition
            .default_instance_parameters(channels)
            .map_err(|error| GuidedEditorError::UnavailableDefinition {
                id: id.clone(),
                reason: error.to_string(),
            })?;
        GuidedDefinitionDraft::new(definition, instance)
    }
}

impl GuidedDefinitionDraft {
    pub fn new(
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> Result<Self, GuidedEditorError> {
        definition
            .validate_instance_parameters(&instance)
            .map_err(|error| GuidedEditorError::InvalidValue {
                parameter_id: definition.id.to_string(),
                message: error.to_string(),
            })?;
        let definitions: BTreeMap<_, _> = definition
            .parameters
            .iter()
            .map(|parameter| (parameter.key.as_str(), parameter))
            .collect();
        let mut sections = Vec::new();
        for section in &definition.layout.sections {
            let mut controls = Vec::new();
            for parameter_id in &section.parameters {
                let parameter = definitions
                    .get(parameter_id.as_str())
                    .ok_or_else(|| GuidedEditorError::MissingParameter(parameter_id.clone()))?;
                let ParameterAuthoring::Creator(metadata) = &parameter.authoring else {
                    continue;
                };
                if metadata.ownership != ParameterOwnership::PatternDefinition {
                    continue;
                }
                if parameter.scope != DefinitionParameterScope::Pattern {
                    return Err(GuidedEditorError::InvalidMetadata {
                        parameter_id: parameter.key.clone(),
                        message: "definition-owned creator parameter must use pattern scope".into(),
                    });
                }
                controls.push(control_descriptor(parameter, metadata)?);
            }
            controls.sort_by_key(|control| control.display_order);
            if !controls.is_empty() {
                sections.push(GuidedSection {
                    id: section.id.clone(),
                    label: section.label.clone(),
                    controls,
                });
            }
        }
        Ok(Self {
            definition,
            instance,
            sections,
        })
    }

    pub fn definition(&self) -> &PatternDefinition {
        &self.definition
    }

    pub fn instance(&self) -> &PatternInstanceParameters {
        &self.instance
    }

    pub fn sections(&self) -> &[GuidedSection] {
        &self.sections
    }

    pub fn control(&self, parameter_id: &str) -> Option<&GuidedControlDescriptor> {
        self.sections
            .iter()
            .flat_map(|section| &section.controls)
            .find(|control| control.parameter_id == parameter_id)
    }

    pub fn value(&self, parameter_id: &str) -> Option<&LiteralValue> {
        self.instance
            .pattern_values
            .iter()
            .find(|value| value.key == parameter_id)
            .map(|value| &value.value)
    }

    pub fn is_applicable(&self, parameter_id: &str) -> Result<bool, GuidedEditorError> {
        let control = self
            .control(parameter_id)
            .ok_or_else(|| GuidedEditorError::MissingParameter(parameter_id.into()))?;
        match &control.applicability {
            ParameterApplicability::Always => Ok(true),
            ParameterApplicability::WhenParameterEquals { parameter, value } => Ok(self
                .value(parameter)
                .is_some_and(|current| current == value)),
        }
    }

    /// Updates only a definition-owned pattern value by its stable parameter
    /// ID. Channel values remain verbatim in the local instance and are never
    /// displayed or mutated by this surface.
    pub fn set_value(
        &mut self,
        parameter_id: &str,
        value: LiteralValue,
    ) -> Result<(), GuidedEditorError> {
        let parameter = self
            .definition
            .parameters
            .iter()
            .find(|parameter| parameter.key == parameter_id)
            .ok_or_else(|| GuidedEditorError::MissingParameter(parameter_id.into()))?;
        let ParameterAuthoring::Creator(metadata) = &parameter.authoring else {
            return Err(GuidedEditorError::InvalidMetadata {
                parameter_id: parameter_id.into(),
                message: "internal parameters are not Guided controls".into(),
            });
        };
        if metadata.ownership != ParameterOwnership::PatternDefinition
            || parameter.scope != DefinitionParameterScope::Pattern
        {
            return Err(GuidedEditorError::ChannelOwnedParameter(
                parameter_id.into(),
            ));
        }
        if self.control(parameter_id).is_none() {
            return Err(GuidedEditorError::MissingParameter(parameter_id.into()));
        }
        let Some(position) = self
            .instance
            .pattern_values
            .iter_mut()
            .position(|entry| entry.key == parameter_id)
        else {
            return Err(GuidedEditorError::MissingParameter(parameter_id.into()));
        };
        let previous = self.instance.pattern_values[position].value.clone();
        self.instance.pattern_values[position].value = value;
        if let Err(error) = self.definition.validate_instance_parameters(&self.instance) {
            self.instance.pattern_values[position].value = previous;
            return Err(GuidedEditorError::InvalidValue {
                parameter_id: parameter_id.into(),
                message: error.to_string(),
            });
        }
        Ok(())
    }

    /// Executes the exact immutable draft graph through the same generic
    /// canonical emitter used by Gate 3. No document selection or bundle is
    /// mutated while a creator changes a local Guided value.
    pub fn preview_canonical(
        &self,
        pipeline: &ArtworkPipelineSettings,
        artboard: ArtboardSpace,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<CanonicalPatternOutput> {
        execute_resolved_definition_cancellable(
            &self.definition,
            &self.instance,
            pipeline,
            artboard,
            cancellation,
        )
    }
}

impl SharedRecipeEditorDraft {
    /// Opens an immutable registry-resolved definition as a local draft. It
    /// may be inspected and previewed, but cannot be applied directly.
    pub fn bundled(
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> Result<Self, GuidedEditorError> {
        let id = definition.id.clone();
        Ok(Self {
            guided: GuidedDefinitionDraft::new(definition, instance)?,
            source: SharedRecipeDraftSource::ImmutableBundled(id),
        })
    }

    /// Reopens an already document-local definition without dropping content
    /// outside the current Guided or Graph controls.
    pub fn document_local(
        definition: PatternDefinition,
        instance: PatternInstanceParameters,
    ) -> Result<Self, GuidedEditorError> {
        let id = definition.id.clone();
        Ok(Self {
            guided: GuidedDefinitionDraft::new(definition, instance)?,
            source: SharedRecipeDraftSource::DocumentLocal(id),
        })
    }

    /// Makes the required explicit copy of a bundled definition. Only the
    /// durable definition and instance identities change; every other field is
    /// cloned verbatim.
    pub fn duplicate_as_document_local(
        &self,
        document_local_id: PatternId,
    ) -> Result<Self, GuidedEditorError> {
        let mut definition = self.guided.definition.clone();
        let mut instance = self.guided.instance.clone();
        definition.id = document_local_id.clone();
        instance.pattern_id = document_local_id;
        Self::document_local(definition, instance)
    }

    pub fn definition(&self) -> &PatternDefinition {
        self.guided.definition()
    }

    pub fn instance(&self) -> &PatternInstanceParameters {
        self.guided.instance()
    }

    pub fn guided(&self) -> &GuidedDefinitionDraft {
        &self.guided
    }

    pub fn sections(&self) -> &[GuidedSection] {
        self.guided.sections()
    }

    pub fn control(&self, parameter_id: &str) -> Option<&GuidedControlDescriptor> {
        self.guided.control(parameter_id)
    }

    pub fn value(&self, parameter_id: &str) -> Option<&LiteralValue> {
        self.guided.value(parameter_id)
    }

    pub fn is_applicable(&self, parameter_id: &str) -> Result<bool, GuidedEditorError> {
        self.guided.is_applicable(parameter_id)
    }

    pub fn set_value(
        &mut self,
        parameter_id: &str,
        value: LiteralValue,
    ) -> Result<(), GuidedEditorError> {
        self.guided.set_value(parameter_id, value)
    }

    /// Updates an existing graph-layout position by stable node ID. It does
    /// not infer, add, remove, or rewrite graph nodes and edges.
    pub fn set_node_position(
        &mut self,
        node_id: &str,
        position: GraphPosition,
    ) -> Result<(), GuidedEditorError> {
        if !self
            .guided
            .definition
            .recipe
            .nodes
            .iter()
            .any(|node| node.id == node_id)
        {
            return Err(GuidedEditorError::MissingGraphNode(node_id.into()));
        }
        let previous = {
            let Some(current) = self
                .guided
                .definition
                .layout
                .node_positions
                .get_mut(node_id)
            else {
                return Err(GuidedEditorError::MissingGraphNode(node_id.into()));
            };
            let previous = *current;
            *current = position;
            previous
        };
        if let Err(error) = self.guided.definition.validate() {
            self.guided
                .definition
                .layout
                .node_positions
                .insert(node_id.into(), previous);
            return Err(GuidedEditorError::InvalidValue {
                parameter_id: node_id.into(),
                message: error.to_string(),
            });
        }
        Ok(())
    }

    pub fn is_document_local(&self) -> bool {
        matches!(self.source, SharedRecipeDraftSource::DocumentLocal(_))
    }

    /// Validates exactly the payload that Apply will commit, while refusing an
    /// immutable bundled source before a document state can change.
    pub fn validate_for_apply(&self) -> Result<(), GuidedEditorError> {
        if let SharedRecipeDraftSource::ImmutableBundled(id) = &self.source {
            return Err(GuidedEditorError::ImmutableBundledDefinition(id.clone()));
        }
        let id = &self.guided.definition.id;
        if PATTERN_REGISTRY.get(id.clone()).is_some()
            || crate::load_bundled_pattern_definition_registry()
                .map_err(|error| GuidedEditorError::InvalidValue {
                    parameter_id: id.to_string(),
                    message: error.to_string(),
                })?
                .get(id)
                .is_ok()
        {
            return Err(GuidedEditorError::InvalidValue {
                parameter_id: id.to_string(),
                message: "document-local pattern IDs must not collide with built-in or immutable bundled definitions".into(),
            });
        }
        self.guided
            .definition
            .validate()
            .map_err(|error| GuidedEditorError::InvalidValue {
                parameter_id: self.guided.definition.id.to_string(),
                message: error.to_string(),
            })?;
        self.guided
            .definition
            .validate_instance_parameters(&self.guided.instance)
            .map_err(|error| GuidedEditorError::InvalidValue {
                parameter_id: self.guided.definition.id.to_string(),
                message: error.to_string(),
            })?;
        Ok(())
    }

    pub fn preview_canonical(
        &self,
        pipeline: &ArtworkPipelineSettings,
        artboard: ArtboardSpace,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<CanonicalPatternOutput> {
        self.guided
            .preview_canonical(pipeline, artboard, cancellation)
    }
}

fn control_descriptor(
    parameter: &PatternParameterDefinition,
    metadata: &CreatorParameterMetadata,
) -> Result<GuidedControlDescriptor, GuidedEditorError> {
    let numeric = numeric_presentation(parameter, metadata)?;
    let widget = match metadata.category {
        CreatorParameterCategory::BoundedNumber => GuidedWidgetKind::BoundedNumeric,
        CreatorParameterCategory::IntegerCount | CreatorParameterCategory::IntegerValue => {
            GuidedWidgetKind::Integer
        }
        CreatorParameterCategory::Angle => GuidedWidgetKind::Angle,
        CreatorParameterCategory::Percentage => GuidedWidgetKind::Percentage,
        CreatorParameterCategory::NormalizedInfluence => GuidedWidgetKind::NormalizedInfluence,
        CreatorParameterCategory::DocumentRelativeDistance => {
            GuidedWidgetKind::DocumentRelativeDistance
        }
        CreatorParameterCategory::Boolean => GuidedWidgetKind::Toggle,
        CreatorParameterCategory::Enumeration => GuidedWidgetKind::Enumeration,
        CreatorParameterCategory::TwoDimensionalOffset => GuidedWidgetKind::TwoDimensionalOffset,
        CreatorParameterCategory::QualityTolerance => GuidedWidgetKind::QualityTolerance,
        CreatorParameterCategory::ResponseExponent
        | CreatorParameterCategory::Text
        | CreatorParameterCategory::SvgAsset => {
            return Err(GuidedEditorError::UnsupportedCategory {
                parameter_id: parameter.key.clone(),
                category: metadata.category,
            });
        }
    };
    let two_dimensional_pair = metadata
        .two_dimensional
        .as_ref()
        .map(|relation| (relation.pair_id.clone(), relation.axis));
    Ok(GuidedControlDescriptor {
        parameter_id: parameter.key.clone(),
        label: parameter.label.clone(),
        help: parameter.help.clone(),
        group: metadata.group.clone(),
        display_order: metadata.display_order,
        widget,
        numeric,
        choices: parameter.choices.clone(),
        applicability: metadata.applicability.clone(),
        two_dimensional_pair,
    })
}

fn numeric_presentation(
    parameter: &PatternParameterDefinition,
    metadata: &CreatorParameterMetadata,
) -> Result<Option<GuidedNumericPresentation>, GuidedEditorError> {
    let (minimum, maximum, _constraint_step) = match parameter.constraints {
        PatternParameterConstraints::Number {
            minimum,
            maximum,
            step,
        } => (minimum, maximum, step),
        PatternParameterConstraints::Integer {
            minimum,
            maximum,
            step,
        } => (minimum as f64, maximum as f64, step as f64),
        _ => return Ok(None),
    };
    let is_numeric_category = matches!(
        metadata.category,
        CreatorParameterCategory::BoundedNumber
            | CreatorParameterCategory::IntegerCount
            | CreatorParameterCategory::IntegerValue
            | CreatorParameterCategory::Angle
            | CreatorParameterCategory::Percentage
            | CreatorParameterCategory::NormalizedInfluence
            | CreatorParameterCategory::ResponseExponent
            | CreatorParameterCategory::DocumentRelativeDistance
            | CreatorParameterCategory::TwoDimensionalOffset
            | CreatorParameterCategory::QualityTolerance
    );
    if !is_numeric_category {
        return Ok(None);
    }
    let CreatorParameterIncrement::Number(increment) = metadata.increment else {
        if matches!(
            metadata.category,
            CreatorParameterCategory::IntegerCount | CreatorParameterCategory::IntegerValue
        ) {
            let CreatorParameterIncrement::Integer(increment) = metadata.increment else {
                return Err(GuidedEditorError::InvalidMetadata {
                    parameter_id: parameter.key.clone(),
                    message: "integer category requires an integer increment".into(),
                });
            };
            return Ok(Some(GuidedNumericPresentation {
                minimum,
                maximum,
                step: increment as f64,
                digits: 0,
                display_multiplier: 1.0,
                unit_suffix: unit_suffix(metadata.unit),
            }));
        }
        return Err(GuidedEditorError::InvalidMetadata {
            parameter_id: parameter.key.clone(),
            message: "numeric category requires a numeric increment".into(),
        });
    };
    let (display_multiplier, unit_suffix) =
        if metadata.category == CreatorParameterCategory::Percentage {
            (100.0, "%")
        } else {
            (1.0, unit_suffix(metadata.unit))
        };
    Ok(Some(GuidedNumericPresentation {
        minimum: minimum * display_multiplier,
        maximum: maximum * display_multiplier,
        step: increment * display_multiplier,
        digits: metadata.precision,
        display_multiplier,
        unit_suffix,
    }))
}

fn unit_suffix(unit: CreatorParameterUnit) -> &'static str {
    match unit {
        CreatorParameterUnit::None
        | CreatorParameterUnit::Unitless
        | CreatorParameterUnit::Normalized => "",
        CreatorParameterUnit::Count => "count",
        CreatorParameterUnit::Degrees => "°",
        CreatorParameterUnit::Percent => "%",
        CreatorParameterUnit::DocumentRelativeDistance => "document units",
        CreatorParameterUnit::Pixels => "px",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CancellationToken, OutputChannelId, PatternId, load_bundled_pattern_definition_registry,
        load_bundled_quadratic_radial_spiral_definition, load_bundled_wave_line_field_definition,
    };
    use sha2::{Digest, Sha256};

    fn spiral_draft() -> GuidedDefinitionDraft {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        GuidedDefinitionDraft::new(definition, instance).unwrap()
    }

    fn wave_line_field_draft() -> GuidedDefinitionDraft {
        let definition = load_bundled_wave_line_field_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        GuidedDefinitionDraft::new(definition, instance).unwrap()
    }

    #[test]
    fn wave_line_field_guided_controls_are_schema_ordered_and_channel_width_is_not_duplicated() {
        let draft = wave_line_field_draft();
        assert_eq!(draft.definition().id, PatternId::WAVE_LINE_FIELD_V1);
        assert_eq!(
            draft
                .sections()
                .iter()
                .map(|section| section.id.as_str())
                .collect::<Vec<_>>(),
            vec!["field", "wave", "coverage"]
        );
        let controls = draft
            .sections()
            .iter()
            .flat_map(|section| &section.controls)
            .map(|control| control.parameter_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            controls,
            vec![
                "line-spacing",
                "orientation-degrees",
                "amplitude",
                "wavelength",
                "phase-degrees",
                "edge-overscan"
            ]
        );
        assert!(draft.control("seed").is_none());
        assert_eq!(
            draft
                .control("orientation-degrees")
                .unwrap()
                .numeric
                .as_ref()
                .unwrap()
                .unit_suffix,
            "°"
        );
        assert_eq!(
            draft
                .control("line-spacing")
                .unwrap()
                .numeric
                .as_ref()
                .unwrap()
                .unit_suffix,
            "document units"
        );
    }

    #[test]
    fn wave_line_field_channel_draft_rejects_invalid_width_order_before_preview() {
        let definition = load_bundled_wave_line_field_definition().unwrap();
        let mut instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        instance.output_channel_values[0]
            .values
            .iter_mut()
            .find(|value| value.key == "line-width-min")
            .unwrap()
            .value = LiteralValue::Number(1.4);
        assert!(matches!(
            GuidedDefinitionDraft::new(definition, instance),
            Err(GuidedEditorError::InvalidValue { .. })
        ));
    }

    #[test]
    fn wave_line_field_guided_preview_is_generic_and_parameter_sensitive() {
        let baseline = wave_line_field_draft();
        let token = CancellationToken::new();
        let artboard = ArtboardSpace {
            width: 160,
            height: 120,
        };
        let original = baseline
            .preview_canonical(&ArtworkPipelineSettings::default(), artboard, &token)
            .unwrap();
        for (parameter, value) in [
            ("line-spacing", LiteralValue::Number(28.0)),
            ("orientation-degrees", LiteralValue::Number(90.0)),
            ("amplitude", LiteralValue::Number(8.0)),
            ("wavelength", LiteralValue::Number(80.0)),
            ("phase-degrees", LiteralValue::Number(45.0)),
        ] {
            let mut draft = baseline.clone();
            draft.set_value(parameter, value).unwrap();
            assert_ne!(
                draft
                    .preview_canonical(&ArtworkPipelineSettings::default(), artboard, &token)
                    .unwrap(),
                original,
                "{parameter} must use the generic resolved runtime"
            );
        }
        let mut invalid = baseline;
        assert!(
            invalid
                .set_value("amplitude", LiteralValue::Number(12.0))
                .is_err(),
            "schema relation must reject amplitude at half spacing before preview"
        );
    }

    #[test]
    fn spiral_guided_controls_are_schema_ordered_and_exclude_channel_authority() {
        let draft = spiral_draft();
        assert_eq!(
            draft
                .sections()
                .iter()
                .map(|section| section.id.as_str())
                .collect::<Vec<_>>(),
            vec!["geometry", "orientation", "center", "sampling", "coverage"]
        );
        let controls: Vec<_> = draft
            .sections()
            .iter()
            .flat_map(|section| &section.controls)
            .map(|control| (control.parameter_id.as_str(), control.widget))
            .collect();
        assert_eq!(
            controls,
            vec![
                ("turns", GuidedWidgetKind::BoundedNumeric),
                (
                    "starting-radius",
                    GuidedWidgetKind::DocumentRelativeDistance
                ),
                (
                    "radial-growth-per-revolution",
                    GuidedWidgetKind::DocumentRelativeDistance
                ),
                (
                    "spacing-growth-per-revolution",
                    GuidedWidgetKind::DocumentRelativeDistance
                ),
                ("starting-angle-degrees", GuidedWidgetKind::Angle),
                ("direction", GuidedWidgetKind::Enumeration),
                ("center-x", GuidedWidgetKind::TwoDimensionalOffset),
                ("center-y", GuidedWidgetKind::TwoDimensionalOffset),
                (
                    "maximum-sample-distance",
                    GuidedWidgetKind::DocumentRelativeDistance
                ),
                ("edge-extension", GuidedWidgetKind::Toggle),
                ("edge-overscan", GuidedWidgetKind::DocumentRelativeDistance),
            ]
        );
        assert!(draft.control("enabled").is_none());
        assert!(draft.control("color").is_none());
        assert!(draft.control("opacity").is_none());
        assert_eq!(
            draft.control("center-x").unwrap().two_dimensional_pair,
            Some(("center".into(), TwoDimensionalAxis::X))
        );
        let percentage = GuidedNumericPresentation {
            minimum: 0.0,
            maximum: 100.0,
            step: 0.01,
            digits: 2,
            display_multiplier: 100.0,
            unit_suffix: "%",
        };
        assert_eq!(percentage.to_display(0.42), 42.0);
        assert_eq!(percentage.from_display(42.0), 0.42);
        assert_eq!(
            draft
                .control("starting-radius")
                .and_then(|control| control.numeric.as_ref())
                .map(|numeric| numeric.unit_suffix),
            Some("document units")
        );
        assert_eq!(unit_suffix(CreatorParameterUnit::Count), "count");
        assert_eq!(unit_suffix(CreatorParameterUnit::Pixels), "px");
    }

    #[test]
    fn catalog_is_registry_backed_and_selects_spiral_by_stable_id() {
        let registry = load_bundled_pattern_definition_registry().unwrap();
        let catalog = GuidedDefinitionCatalog::from_registry(&registry);
        assert_eq!(catalog.entries.len(), registry.len());
        let entry = catalog
            .entry(&PatternId::QUADRATIC_RADIAL_SPIRAL_V1)
            .unwrap();
        assert_eq!(entry.name, "Quadratic Radial Spiral");
        assert!(entry.editable);
        let draft = catalog
            .open(
                &registry,
                &PatternId::QUADRATIC_RADIAL_SPIRAL_V1,
                OutputChannelId::CMYK,
            )
            .unwrap();
        assert_eq!(draft.definition().id, PatternId::QUADRATIC_RADIAL_SPIRAL_V1);
    }

    #[test]
    fn local_spiral_preview_changes_for_every_exposed_structural_value() {
        let baseline = spiral_draft();
        let pipeline = ArtworkPipelineSettings::default();
        let artboard = ArtboardSpace {
            width: 160,
            height: 120,
        };
        let token = CancellationToken::new();
        let baseline_output = baseline
            .preview_canonical(&pipeline, artboard, &token)
            .unwrap();
        let cases = [
            ("turns", LiteralValue::Number(20.25)),
            ("starting-radius", LiteralValue::Number(1.0)),
            ("radial-growth-per-revolution", LiteralValue::Number(21.0)),
            ("spacing-growth-per-revolution", LiteralValue::Number(0.25)),
            ("starting-angle-degrees", LiteralValue::Number(1.0)),
            ("direction", LiteralValue::Choice("counterclockwise".into())),
            ("center-x", LiteralValue::Number(1.0)),
            ("center-y", LiteralValue::Number(1.0)),
            ("maximum-sample-distance", LiteralValue::Number(3.5)),
        ];
        for (parameter, value) in cases {
            let mut draft = baseline.clone();
            draft.set_value(parameter, value).unwrap();
            let output = draft
                .preview_canonical(&pipeline, artboard, &token)
                .unwrap();
            assert_ne!(
                output, baseline_output,
                "{parameter} must change local canonical preview"
            );
        }
        // The default 20 authored turns already exceed this small artboard's
        // corner radius, so coverage sensitivity needs a short authored path
        // rather than falsely treating an intentional no-op as a UI failure.
        let mut extension_on = baseline.clone();
        extension_on
            .set_value("turns", LiteralValue::Number(1.0))
            .unwrap();
        let extension_on_output = extension_on
            .preview_canonical(&pipeline, artboard, &token)
            .unwrap();
        let mut extension_off = extension_on.clone();
        extension_off
            .set_value("edge-extension", LiteralValue::Boolean(false))
            .unwrap();
        assert_ne!(
            extension_off
                .preview_canonical(&pipeline, artboard, &token)
                .unwrap(),
            extension_on_output,
            "edge-extension must change local canonical preview when coverage needs extension"
        );
        let mut extra_overscan = extension_on;
        extra_overscan
            .set_value("edge-overscan", LiteralValue::Number(40.0))
            .unwrap();
        assert_ne!(
            extra_overscan
                .preview_canonical(&pipeline, artboard, &token)
                .unwrap(),
            extension_on_output,
            "edge-overscan must change local canonical preview when coverage needs extension"
        );
    }

    #[test]
    fn conditional_controls_and_immutable_channel_values_remain_local() {
        let mut draft = spiral_draft();
        let original_definition = draft.definition().clone();
        let original_channels = draft.instance().output_channel_values.clone();
        assert!(draft.is_applicable("edge-overscan").unwrap());
        draft
            .set_value("edge-extension", LiteralValue::Boolean(false))
            .unwrap();
        assert!(!draft.is_applicable("edge-overscan").unwrap());
        assert!(matches!(
            draft.set_value("opacity", LiteralValue::Number(0.5)),
            Err(GuidedEditorError::ChannelOwnedParameter(_))
        ));
        assert_eq!(draft.definition(), &original_definition);
        assert_eq!(draft.instance().output_channel_values, original_channels);
    }

    #[test]
    fn shared_draft_duplicate_preserves_complete_recipe_and_instance_payload() {
        let definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let bundled =
            SharedRecipeEditorDraft::bundled(definition.clone(), instance.clone()).unwrap();
        assert!(matches!(
            bundled.validate_for_apply(),
            Err(GuidedEditorError::ImmutableBundledDefinition(_))
        ));
        let custom_id = PatternId::new("custom.quadratic-radial-spiral.1").unwrap();
        let mut duplicated = bundled
            .duplicate_as_document_local(custom_id.clone())
            .unwrap();
        assert!(duplicated.is_document_local());
        let mut expected_definition = definition;
        let mut expected_instance = instance;
        expected_definition.id = custom_id.clone();
        expected_instance.pattern_id = custom_id;
        assert_eq!(duplicated.definition(), &expected_definition);
        assert_eq!(duplicated.instance(), &expected_instance);

        let node_id = duplicated.definition().recipe.nodes[0].id.clone();
        let original_assets = duplicated.definition().assets.clone();
        let original_edges = duplicated.definition().recipe.edges.clone();
        duplicated
            .set_value("turns", LiteralValue::Number(7.5))
            .unwrap();
        duplicated
            .set_node_position(&node_id, GraphPosition { x: 42.0, y: 24.0 })
            .unwrap();
        assert_eq!(duplicated.value("turns"), Some(&LiteralValue::Number(7.5)));
        assert_eq!(
            duplicated.definition().layout.node_positions[&node_id],
            GraphPosition { x: 42.0, y: 24.0 }
        );
        assert_eq!(duplicated.definition().assets, original_assets);
        assert_eq!(duplicated.definition().recipe.edges, original_edges);
        duplicated.validate_for_apply().unwrap();
    }

    #[test]
    fn shared_draft_preserves_valid_content_outside_guided_and_bounded_graph_views() {
        const UNEXPOSED_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><path d=\"M0 0h1v1H0z\"/></svg>";
        let mut definition = load_bundled_quadratic_radial_spiral_definition().unwrap();
        let instance = definition
            .default_instance_parameters(OutputChannelId::CMYK)
            .unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(UNEXPOSED_SVG.as_bytes()));
        definition.assets.push(crate::EmbeddedSvgAsset {
            digest,
            svg: UNEXPOSED_SVG.into(),
        });
        definition
            .quick_controls
            .push(crate::QuickControlDefinition {
                id: "unexposed-turns-quick-control".into(),
                parameter: "turns".into(),
                scope: DefinitionParameterScope::Pattern,
                kind: crate::QuickControlKind::Slider,
                label: "Unexposed Turns Shortcut".into(),
            });
        definition.layout.node_positions.insert(
            "unexposed-layout-anchor".into(),
            GraphPosition { x: -77.5, y: 19.25 },
        );
        definition.validate().unwrap();

        let unexposed_assets = definition.assets.clone();
        let unexposed_quick_controls = definition.quick_controls.clone();
        let unexposed_layout_anchor = definition.layout.node_positions["unexposed-layout-anchor"];
        let unexposed_operation_arguments = definition.recipe.nodes[0].parameters.clone();
        let bundled =
            SharedRecipeEditorDraft::bundled(definition.clone(), instance.clone()).unwrap();
        assert!(
            bundled
                .sections()
                .iter()
                .flat_map(|section| &section.controls)
                .all(|control| control.parameter_id != "unexposed-turns-quick-control")
        );
        assert!(
            bundled
                .definition()
                .recipe
                .nodes
                .iter()
                .all(|node| node.id != "unexposed-layout-anchor")
        );

        let custom_id = PatternId::new("custom.unexposed-content.1").unwrap();
        let mut draft = bundled
            .duplicate_as_document_local(custom_id.clone())
            .unwrap();
        let node_id = draft.definition().recipe.nodes[0].id.clone();
        let pipeline = ArtworkPipelineSettings::default();
        let artboard = ArtboardSpace {
            width: 160,
            height: 120,
        };
        let token = CancellationToken::new();
        let before_layout_edit = draft
            .preview_canonical(&pipeline, artboard, &token)
            .unwrap();
        draft
            .set_value("turns", LiteralValue::Number(9.25))
            .unwrap();
        let after_guided_edit = draft
            .preview_canonical(&pipeline, artboard, &token)
            .unwrap();
        draft
            .set_node_position(&node_id, GraphPosition { x: 31.0, y: -12.0 })
            .unwrap();
        assert_eq!(
            draft
                .preview_canonical(&pipeline, artboard, &token)
                .unwrap(),
            after_guided_edit,
            "graph layout positions are authoring-only and must not alter canonical output"
        );
        assert_ne!(before_layout_edit, after_guided_edit);
        draft.validate_for_apply().unwrap();

        assert_eq!(draft.definition().assets, unexposed_assets);
        assert_eq!(draft.definition().quick_controls, unexposed_quick_controls);
        assert_eq!(
            draft.definition().layout.node_positions["unexposed-layout-anchor"],
            unexposed_layout_anchor
        );
        assert_eq!(
            draft.definition().recipe.nodes[0].parameters,
            unexposed_operation_arguments
        );
        let serialized = serde_json::to_vec(draft.definition()).unwrap();
        let reparsed: PatternDefinition = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(reparsed, *draft.definition());
        assert_eq!(reparsed.id, custom_id);
    }
}
