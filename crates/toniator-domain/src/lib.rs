#![forbid(unsafe_code)]

//! Authoritative, headless document concepts for Toniator.

use std::{
    collections::HashSet,
    error::Error,
    fmt,
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

/// A stable identifier for an authoritative document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(pub u64);

/// A stable identifier for a structural pattern definition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternDefinitionId(pub u64);

/// A stable address for a typed structural mechanism.  These IDs are owned by
/// the document rather than by an evaluator or a UI draft.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternMechanismId(pub u64);

/// A stable address for one ordered typed output layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternOutputLayerId(pub u64);

/// A stable identifier for one reusable authored construction structure owned
/// by exactly one document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthoredStructureId(pub u64);

/// One finite document-space point in an authored construction structure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredPoint2 {
    pub x: f64,
    pub y: f64,
}

/// One explicit construction segment in an authored open path or closed shape.
#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredCurveSegment {
    Line {
        start: AuthoredPoint2,
        end: AuthoredPoint2,
    },
    CubicBezier {
        start: AuthoredPoint2,
        control_1: AuthoredPoint2,
        control_2: AuthoredPoint2,
        end: AuthoredPoint2,
    },
}

impl AuthoredCurveSegment {
    /// Returns the explicit segment start without inferring a connection from adjacent storage.
    pub const fn start(&self) -> AuthoredPoint2 {
        match self {
            Self::Line { start, .. } | Self::CubicBezier { start, .. } => *start,
        }
    }

    /// Returns the explicit segment end without inferring a connection from adjacent storage.
    pub const fn end(&self) -> AuthoredPoint2 {
        match self {
            Self::Line { end, .. } | Self::CubicBezier { end, .. } => *end,
        }
    }
}

/// The declared topology of one authored construction structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredStructureKind {
    OpenPath,
    ClosedShape,
}

/// One deterministic typed consumer of a document-owned authored structure.
///
/// This projection names only persisted document identifiers. Presentation labels, ordinals, and
/// editing prompts remain frontend policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredStructureUse {
    /// One guide-dimension prototype reference owned by a channel's current definition.
    Guide {
        channel_id: ChannelId,
        definition_id: PatternDefinitionId,
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        structure_id: AuthoredStructureId,
    },
    /// One mark-output prototype reference owned by a channel's current definition.
    Mark {
        channel_id: ChannelId,
        definition_id: PatternDefinitionId,
        output_layer_id: PatternOutputLayerId,
        structure_id: AuthoredStructureId,
    },
}

impl AuthoredStructureUse {
    /// Returns the selected reusable structure identity without inventing presentation policy.
    pub const fn structure_id(&self) -> AuthoredStructureId {
        match self {
            Self::Guide { structure_id, .. } | Self::Mark { structure_id, .. } => *structure_id,
        }
    }
}

/// One unambiguous selected-channel descriptor target for attaching a newly created structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredStructureAttachment {
    /// Attaches a new open path by activating one exact generic guide dimension.
    Guide {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
    },
    /// Replaces one ordinary selected-channel straight grid with one fresh generic
    /// authored-guide/along-guide definition as the new open path is attached.
    ///
    /// This intent is deliberately available only through `DocumentHistory`'s
    /// grouped add-and-attach transition. It preserves the old shared definition
    /// for linked channels and never exposes an intermediate orphan resource.
    GuideCustomAlongLayout,
    /// Attaches a new closed shape by activating one exact mark output layer.
    Mark {
        output_layer_id: PatternOutputLayerId,
    },
}

/// A validated ID-free authored-structure payload for an authoritative add or replacement command.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredStructureDraft {
    kind: AuthoredStructureKind,
    segments: Vec<AuthoredCurveSegment>,
}

impl AuthoredStructureDraft {
    /// Validates one ID-free authored payload before it can become document-owned state.
    ///
    /// # Errors
    ///
    /// Returns the stable authored-structure topology, coordinate, or segment-limit diagnostic.
    pub fn new(
        kind: AuthoredStructureKind,
        segments: Vec<AuthoredCurveSegment>,
    ) -> Result<Self, ValidationError> {
        validate_authored_structure_segments(kind, &segments)?;
        Ok(Self { kind, segments })
    }

    /// Returns the declared topology without deriving closure from endpoint coincidence.
    pub const fn kind(&self) -> AuthoredStructureKind {
        self.kind
    }

    /// Returns immutable explicit construction segments in their authored order.
    pub fn segments(&self) -> &[AuthoredCurveSegment] {
        &self.segments
    }
}

/// A validated reusable authored structure with a stable document-scoped identity.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredStructure {
    id: AuthoredStructureId,
    kind: AuthoredStructureKind,
    segments: Vec<AuthoredCurveSegment>,
}

impl AuthoredStructure {
    /// Validates one stable document-owned authored structure without assigning or remapping its ID.
    ///
    /// # Errors
    ///
    /// Returns an authored-structure ID, topology, coordinate, or segment-limit diagnostic.
    pub fn new(
        id: AuthoredStructureId,
        kind: AuthoredStructureKind,
        segments: Vec<AuthoredCurveSegment>,
    ) -> Result<Self, ValidationError> {
        validate_authored_structure_id(id)?;
        validate_authored_structure_segments(kind, &segments)?;
        Ok(Self { id, kind, segments })
    }

    /// Returns the immutable document-scoped identity preserved by replacement, persistence, and history.
    pub const fn id(&self) -> AuthoredStructureId {
        self.id
    }

    /// Returns the declared topology without deriving closure from endpoint coincidence.
    pub const fn kind(&self) -> AuthoredStructureKind {
        self.kind
    }

    /// Returns immutable explicit construction segments in their authored order.
    pub fn segments(&self) -> &[AuthoredCurveSegment] {
        &self.segments
    }

    /// Rebuilds this structure with a replacement payload while preserving its stable identity.
    ///
    /// # Errors
    ///
    /// Returns the stable authored-structure topology, coordinate, or segment-limit diagnostic;
    /// the target ID is retained exactly and never reallocated.
    pub(crate) fn replace_with(
        &self,
        replacement: &AuthoredStructureDraft,
    ) -> Result<Self, ValidationError> {
        Self::new(self.id, replacement.kind, replacement.segments.clone())
    }
}

/// The value category exposed by one authored-structure descriptor field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredStructureValueKind {
    EnumChoice,
    SegmentSequence,
}

/// The stable identity of one editable authored-structure field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredStructureFieldId {
    Kind,
    Segments,
}

/// The construction variant of one authored curve segment for descriptor choices and UI-neutral tooling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredCurveSegmentKind {
    Line,
    CubicBezier,
}

/// Value-free editing contract for one authored-structure field.
///
/// `invalidation` is a target-independent conservative upper bound for any
/// command affecting this field. A descriptor refines it against its concrete
/// structure kind: closed-shape segment edits are `Realization`, while any
/// open-path segment edit remains `Family`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthoredStructureFieldContract {
    pub field: AuthoredStructureFieldId,
    pub value_kind: AuthoredStructureValueKind,
    pub allowed_structure_kinds: &'static [AuthoredStructureKind],
    pub allowed_segment_kinds: &'static [AuthoredCurveSegmentKind],
    pub maximum_segments: Option<usize>,
    pub shared_edit: bool,
    pub invalidation: InvalidationLevel,
}

/// Value-free descriptor for one document-owned authored structure field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthoredStructureFieldDescriptor {
    pub target: AuthoredStructureId,
    pub field: AuthoredStructureFieldId,
    pub value_kind: AuthoredStructureValueKind,
    pub allowed_structure_kinds: &'static [AuthoredStructureKind],
    pub allowed_segment_kinds: &'static [AuthoredCurveSegmentKind],
    pub maximum_segments: Option<usize>,
    pub shared_edit: bool,
    pub invalidation: InvalidationLevel,
}

const AUTHORED_STRUCTURE_KINDS: &[AuthoredStructureKind] = &[
    AuthoredStructureKind::OpenPath,
    AuthoredStructureKind::ClosedShape,
];
const AUTHORED_CURVE_SEGMENT_KINDS: &[AuthoredCurveSegmentKind] = &[
    AuthoredCurveSegmentKind::Line,
    AuthoredCurveSegmentKind::CubicBezier,
];
const AUTHORED_STRUCTURE_FIELD_CONTRACTS: &[AuthoredStructureFieldContract] = &[
    AuthoredStructureFieldContract {
        field: AuthoredStructureFieldId::Kind,
        value_kind: AuthoredStructureValueKind::EnumChoice,
        allowed_structure_kinds: AUTHORED_STRUCTURE_KINDS,
        allowed_segment_kinds: &[],
        maximum_segments: None,
        shared_edit: true,
        invalidation: InvalidationLevel::Family,
    },
    AuthoredStructureFieldContract {
        field: AuthoredStructureFieldId::Segments,
        value_kind: AuthoredStructureValueKind::SegmentSequence,
        allowed_structure_kinds: &[],
        allowed_segment_kinds: AUTHORED_CURVE_SEGMENT_KINDS,
        maximum_segments: Some(4_096),
        shared_edit: true,
        invalidation: InvalidationLevel::Family,
    },
];

/// Returns the complete value-free authored-structure editing contract.
pub const fn authored_structure_field_contracts() -> &'static [AuthoredStructureFieldContract] {
    AUTHORED_STRUCTURE_FIELD_CONTRACTS
}

/// A stable identifier for one structural guide dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuideDimensionId(pub u64);

/// A finite, independently-addressable repeated straight-guide dimension.
/// `baseline_angle_degrees` describes the local-space normal; the channel
/// layout transform is applied by family planning, never after a finite set of
/// guides has been generated.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightGuideDimension {
    pub id: GuideDimensionId,
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub repetition: StraightGuideRepetition,
}

/// One persisted prototype for a generic guide dimension.  Authored-path
/// references remain document-local intent; the procedural arc is the only
/// Stage 20D constructed prototype.
#[derive(Clone, Debug, PartialEq)]
pub enum GuidePrototype {
    AuthoredOpenPath {
        structure_id: AuthoredStructureId,
    },
    CircularArc {
        center: AuthoredPoint2,
        radius: f64,
        start_angle_degrees: f64,
        sweep_angle_degrees: f64,
    },
}

/// The bounded Stage 20D repetition vocabulary.  A transform-stack direction
/// is relative to the owning dimension baseline and never carries scale/shear.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveRepetition {
    Single,
    TransformStack {
        direction_degrees: f64,
        spacing_multiplier: f64,
    },
    /// Repeats one guide along its local normals at an authored absolute document-space gap.
    NormalOffset {
        spacing: f64,
        sides: OffsetSides,
        cleanup: OffsetCleanup,
    },
}

/// Compatibility spelling for persisted/current guide mechanisms. New common
/// curve sources are authored against `CurveRepetition`; the v4 guide DTO name
/// intentionally remains unchanged.
pub type GuideRepetition = CurveRepetition;

/// One bounded analytic curve source owned by the parametric family.
#[derive(Clone, Debug, PartialEq)]
pub enum ParametricCurve {
    /// An open spiral with a round or exact square centerline construction.
    Spiral(SpiralCurve),
}

/// Structural intent for one finite spiral centered at the local origin.
#[derive(Clone, Debug, PartialEq)]
pub struct SpiralCurve {
    pub shape: SpiralShape,
    pub turns: f64,
    pub radial_spacing: f64,
    pub phase_degrees: f64,
    pub winding: CurveWinding,
}

/// Selects the exact local construction family for a spiral source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpiralShape {
    Round,
    Square,
}

/// Selects the signed angular orientation of a spiral source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveWinding {
    Clockwise,
    CounterClockwise,
}

/// Selects the signed normal-offset directions derived from one source guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetSides {
    /// Emits the source followed by positive left-normal offsets.
    Left,
    /// Emits negative right-normal offsets followed by the source.
    Right,
    /// Emits negative offsets, the source, and positive offsets in signed order.
    Both,
}

/// Selects the accepted deterministic cleanup policy for derived offset paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetCleanup {
    /// Splits and removes reversal loops without manufacturing region topology.
    DissolveCrossings,
}

/// One finite, stable generic guide dimension authored in definition-local
/// coordinates.  Its raw angle and phase bits are authoritative identity and
/// are not normalized during validation or persistence.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideDimension {
    pub id: GuideDimensionId,
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub prototype: GuidePrototype,
    pub repetition: GuideRepetition,
}

/// The deliberately small Stage 16A repetition vocabulary.  The spacing is a
/// multiplier of the resolved channel density, so authored density remains a
/// channel concern rather than becoming a second structural density system.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightGuideRepetition {
    pub spacing_multiplier: f64,
}

/// Stable typed orientation for a mark prototype.  Circle rendering remains
/// visually invariant, but orientation is still part of the realization
/// contract so later compatible prototypes do not require renderer dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkOrientation {
    Fixed,
    GuideTangent { dimension_id: GuideDimensionId },
    GuideNormal { dimension_id: GuideDimensionId },
}

/// The persisted filled-mark prototype selected by one output layer.
///
/// A shape variant carries its document-scoped resource identity explicitly;
/// callers never infer a structure from a family, preset, or renderer mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkPrototype {
    Circle,
    AuthoredClosedShape { structure_id: AuthoredStructureId },
}

/// A stable identifier for a channel owned by a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

/// The discrete revision of an authoritative document session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// A stable logical identifier for source artwork. It deliberately does not
/// encode a filesystem location, source bytes, or decoded pixels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SourceReferenceId(String);

impl SourceReferenceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be empty",
            ));
        }
        if value.contains('/') || value.contains('\\') {
            return Err(ValidationError::new(
                "source.reference_id",
                "source reference ID must not be a filesystem path",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Document-space canvas dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSpec {
    pub width: f64,
    pub height: f64,
}

/// The continuous density metric supplied to a channel's pattern layout.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityMetric2D {
    pub across_x: f64,
    pub across_y: f64,
    pub aspect_locked: bool,
}

/// The document-owned shared pattern settings from which every channel derives
/// its effective family and realization inputs.  `aspect_locked` belongs to
/// this base density and channels can only contribute additive values.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPatternSettings {
    pub definition_id: PatternDefinitionId,
    pub density: DensityMetric2D,
    pub pattern_rotation_degrees: f64,
    pub shape_rotation_degrees: f64,
}

/// The presently supported normalized output-response branches.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternGeometryResponse {
    Marks(MarkGeometryResponse),
    /// Defines normalized round-brush widths for a guide-path output.
    Connected(ConnectedGeometryResponse),
    /// Stage 20O ordinary cells have one fixed treatment.
    Regions(RegionGeometryResponse),
}

/// The fixed Stage 20O response for ordinary canonical region output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegionGeometryResponse {
    /// Retains the complete untreated ordinary Voronoi cell.
    #[default]
    Full,
}

/// One structural-output response bound atomically to an output-layer identity.
///
/// Stage 20N keeps this value separate from document-wide layout settings so every future
/// heterogeneous output has exactly one typed base response owned by its definition bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternOutputSettings {
    pub output_layer_id: PatternOutputLayerId,
    pub response: PatternGeometryResponse,
}

/// Atomic structural definition and ordered typed output-response settings.
///
/// This is the Stage 20N authority boundary. Existing current-format adapters may construct
/// it from their singular response until schema-v5 replacement is completed, but consumers
/// must validate this record before treating it as effective state.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinitionBundle {
    pub definition: PatternDefinition,
    pub output_settings: Vec<PatternOutputSettings>,
}

/// Optional additive delta for exactly one typed structural output response.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternOutputResponseDelta {
    pub output_layer_id: PatternOutputLayerId,
    pub delta: ChannelGeometryResponseDelta,
}

/// One resolved output response projected in structural output order.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectivePatternOutputSettings {
    pub output_layer_id: PatternOutputLayerId,
    pub response: PatternGeometryResponse,
}

impl PatternDefinitionBundle {
    /// Validates atomic output settings against the exact definition output order and kinds.
    ///
    /// # Errors
    ///
    /// Returns stable bundle alignment, duplicate, foreign, or response-kind diagnostics without
    /// mutating the supplied definition or exposing a partially accepted bundle.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.definition.output_layers.len() != self.output_settings.len() {
            return Err(ValidationError::new(
                "pattern.bundle.output_settings.cardinality",
                "pattern definition bundles require one output setting per structural output",
            ));
        }
        for (output, settings) in self
            .definition
            .output_layers
            .iter()
            .zip(&self.output_settings)
        {
            if output.id() != settings.output_layer_id {
                return Err(ValidationError::new(
                    "pattern.bundle.output_settings.order",
                    "pattern definition bundle settings must match structural output IDs in order",
                ));
            }
            validate_response_for_output(output, &settings.response)?;
        }
        Ok(())
    }

    /// Returns one validated ordered base response by output identity.
    pub fn output_settings(&self) -> &[PatternOutputSettings] {
        &self.output_settings
    }
}

impl Deref for PatternDefinitionBundle {
    type Target = PatternDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl DerefMut for PatternDefinitionBundle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.definition
    }
}

/// Binds every internally allocated structural output to its sole current typed base response.
///
/// The helper is only used while materializing a new authoritative bundle; validation still
/// checks the exact output IDs, order, and kinds before the candidate can be published.
fn bundle_from_definition(definition: PatternDefinition) -> PatternDefinitionBundle {
    let output_settings = definition
        .output_layers
        .iter()
        .map(|output| PatternOutputSettings {
            output_layer_id: output.id(),
            response: match output {
                PatternOutputLayer::CircularMarks { .. }
                | PatternOutputLayer::MarkPrototype { .. } => {
                    PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    })
                }
                PatternOutputLayer::GuidePaths { .. }
                | PatternOutputLayer::ParametricPaths { .. }
                | PatternOutputLayer::ConnectionPaths { .. }
                | PatternOutputLayer::MazeWalls { .. } => {
                    PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                        minimum_thickness: 0.0,
                        maximum_thickness: 1.0,
                    })
                }
                PatternOutputLayer::Regions { .. } => {
                    PatternGeometryResponse::Regions(RegionGeometryResponse::Full)
                }
            },
        })
        .collect();
    PatternDefinitionBundle {
        definition,
        output_settings,
    }
}

/// Binds ID-free ordered recipe responses to freshly allocated structural outputs.
///
/// # Errors
///
/// Returns a stable cardinality or response-kind diagnostic when a complete
/// recipe cannot bind one response to every output. Stage 20N retains the
/// current single-output authoring gate but does not collapse the ordered
/// representation.
fn bind_recipe_output_settings(
    definition: PatternDefinition,
    recipes: &[PatternOutputSettingsRecipe],
) -> Result<PatternDefinitionBundle, ValidationError> {
    if recipes.len() != definition.output_layers.len() {
        return Err(ValidationError::new(
            "preset.recipe.output_settings.cardinality",
            "recipe output settings must cover every structural output",
        ));
    }
    let bundle = PatternDefinitionBundle {
        output_settings: definition
            .output_layers
            .iter()
            .zip(recipes)
            .map(|(output, recipe)| PatternOutputSettings {
                output_layer_id: output.id(),
                response: recipe.response.clone(),
            })
            .collect(),
        definition,
    };
    bundle.validate()?;
    Ok(bundle)
}

/// Validates ordered keyed deltas against a definition bundle without applying arithmetic.
///
/// # Errors
///
/// Returns stable cardinality/order/foreign/type diagnostics; absent entries inherit and are valid.
pub fn validate_pattern_output_deltas(
    bundle: &PatternDefinitionBundle,
    deltas: &[PatternOutputResponseDelta],
) -> Result<(), ValidationError> {
    bundle.validate()?;
    let mut previous = None;
    for entry in deltas {
        let Some(index) = bundle
            .output_settings
            .iter()
            .position(|base| base.output_layer_id == entry.output_layer_id)
        else {
            return Err(ValidationError::new(
                "channel.pattern.output_deltas.foreign",
                "channel output response delta targets a foreign output layer",
            ));
        };
        if previous.is_some_and(|value| index <= value) {
            return Err(ValidationError::new(
                "channel.pattern.output_deltas.order",
                "channel output response deltas must be unique and structural-order sorted",
            ));
        }
        previous = Some(index);
        validate_response_delta_kind(&bundle.output_settings[index].response, &entry.delta)?;
    }
    Ok(())
}

/// Inserts or replaces one keyed response delta in the bundle's structural output order.
///
/// This preserves the persisted ordered-intent invariant without deriving an effective response.
/// The caller supplies a validated bundle selected by its channel; foreign output IDs and
/// response-kind mismatches are rejected before any delta vector mutation becomes authoritative.
///
/// # Errors
///
/// Returns stable foreign, response-kind, or ordered-delta diagnostics and leaves `deltas`
/// unchanged when the supplied entry cannot belong to `bundle`.
fn upsert_pattern_output_delta_in_order(
    bundle: &PatternDefinitionBundle,
    deltas: &mut Vec<PatternOutputResponseDelta>,
    entry: PatternOutputResponseDelta,
) -> Result<(), ValidationError> {
    validate_pattern_output_deltas(bundle, deltas)?;
    let target_index = bundle
        .output_settings
        .iter()
        .position(|setting| setting.output_layer_id == entry.output_layer_id)
        .ok_or(ValidationError::new(
            "channel.pattern.output_layer_id",
            "response delta targets a missing output",
        ))?;
    validate_response_delta_kind(&bundle.output_settings[target_index].response, &entry.delta)?;
    if let Some(existing) = deltas
        .iter_mut()
        .find(|existing| existing.output_layer_id == entry.output_layer_id)
    {
        existing.delta = entry.delta;
        return Ok(());
    }
    let insertion_index = deltas
        .iter()
        .position(|existing| {
            bundle
                .output_settings
                .iter()
                .position(|setting| setting.output_layer_id == existing.output_layer_id)
                .expect("validated delta belongs to bundle")
                > target_index
        })
        .unwrap_or(deltas.len());
    deltas.insert(insertion_index, entry);
    Ok(())
}

/// Retains only deltas that bind to the supplied replacement bundle, in structural output order.
///
/// This is used after definition selection or shared-recipe replacement. It preserves an exact
/// output-ID and response-kind match verbatim while deterministically dropping foreign or
/// incompatible intent without deriving effective values.
fn prune_output_response_deltas_for_bundle(
    bundle: &PatternDefinitionBundle,
    deltas: &[PatternOutputResponseDelta],
) -> Vec<PatternOutputResponseDelta> {
    bundle
        .output_settings
        .iter()
        .filter_map(|setting| {
            deltas
                .iter()
                .find(|entry| {
                    entry.output_layer_id == setting.output_layer_id
                        && validate_response_delta_kind(&setting.response, &entry.delta).is_ok()
                })
                .cloned()
        })
        .collect()
}

/// Resolves finite typed output responses in structural order while retaining unclamped arithmetic.
///
/// # Errors
///
/// Returns the base/delta alignment or finite response diagnostic atomically; no frontend computes
/// this inheritance or arithmetic itself.
pub fn effective_pattern_output_settings(
    bundle: &PatternDefinitionBundle,
    deltas: &[PatternOutputResponseDelta],
) -> Result<Vec<EffectivePatternOutputSettings>, ValidationError> {
    validate_pattern_output_deltas(bundle, deltas)?;
    bundle
        .output_settings
        .iter()
        .map(|base| {
            let response = match deltas
                .iter()
                .find(|delta| delta.output_layer_id == base.output_layer_id)
            {
                None => base.response.clone(),
                Some(delta) => apply_response_delta(&base.response, &delta.delta)?,
            };
            validate_pattern_geometry_response(&response)?;
            Ok(EffectivePatternOutputSettings {
                output_layer_id: base.output_layer_id,
                response,
            })
        })
        .collect()
}

/// Validates that one response branch belongs to the exact structural output kind.
fn validate_response_for_output(
    output: &PatternOutputLayer,
    response: &PatternGeometryResponse,
) -> Result<(), ValidationError> {
    let marks = matches!(
        output,
        PatternOutputLayer::CircularMarks { .. } | PatternOutputLayer::MarkPrototype { .. }
    );
    let connected = matches!(
        output,
        PatternOutputLayer::GuidePaths { .. }
            | PatternOutputLayer::ParametricPaths { .. }
            | PatternOutputLayer::ConnectionPaths { .. }
            | PatternOutputLayer::MazeWalls { .. }
    );
    let regions = matches!(output, PatternOutputLayer::Regions { .. });
    match (marks, connected, regions, response) {
        (true, false, false, PatternGeometryResponse::Marks(_))
        | (false, true, false, PatternGeometryResponse::Connected(_))
        | (false, false, true, PatternGeometryResponse::Regions(_)) => {
            validate_pattern_geometry_response(response)
        }
        _ => Err(ValidationError::new(
            "pattern.bundle.output_settings.kind",
            "pattern output setting response must match its structural output kind",
        )),
    }
}

/// Validates that one optional additive response delta has the exact base response branch.
fn validate_response_delta_kind(
    base: &PatternGeometryResponse,
    delta: &ChannelGeometryResponseDelta,
) -> Result<(), ValidationError> {
    match (base, delta) {
        (PatternGeometryResponse::Marks(_), ChannelGeometryResponseDelta::Marks(_))
        | (PatternGeometryResponse::Connected(_), ChannelGeometryResponseDelta::Connected(_)) => {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "channel.pattern.output_deltas.kind",
            "channel output response delta must match its base response kind",
        )),
    }
}

/// Applies one typed response delta without clamping and retains finite domain validation.
fn apply_response_delta(
    base: &PatternGeometryResponse,
    delta: &ChannelGeometryResponseDelta,
) -> Result<PatternGeometryResponse, ValidationError> {
    let response = match (base, delta) {
        (PatternGeometryResponse::Marks(base), ChannelGeometryResponseDelta::Marks(delta)) => {
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: base.minimum_fill + delta.minimum_fill_delta.unwrap_or(0.0),
                maximum_fill: base.maximum_fill + delta.maximum_fill_delta.unwrap_or(0.0),
            })
        }
        (
            PatternGeometryResponse::Connected(base),
            ChannelGeometryResponseDelta::Connected(delta),
        ) => PatternGeometryResponse::Connected(ConnectedGeometryResponse {
            minimum_thickness: base.minimum_thickness
                + delta.minimum_thickness_delta.unwrap_or(0.0),
            maximum_thickness: base.maximum_thickness
                + delta.maximum_thickness_delta.unwrap_or(0.0),
        }),
        _ => {
            return Err(ValidationError::new(
                "channel.pattern.output_deltas.kind",
                "channel output response delta must match its base response kind",
            ));
        }
    };
    Ok(response)
}

/// Validates one typed geometry response through the existing domain-owned bounds authority.
fn validate_pattern_geometry_response(
    response: &PatternGeometryResponse,
) -> Result<(), ValidationError> {
    match response {
        PatternGeometryResponse::Marks(value) => validate_mark_response(value),
        PatternGeometryResponse::Connected(value) => validate_connected_response(value),
        PatternGeometryResponse::Regions(RegionGeometryResponse::Full) => Ok(()),
    }
}

/// An optional additive density adjustment for one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityMetricDelta2D {
    pub across_x_delta: f64,
    pub across_y_delta: f64,
}

/// The optional layout intent stored by a channel.  Translation remains
/// absolute channel placement while density and rotation are relative to the
/// document base.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPatternLayoutDelta {
    pub density: Option<DensityMetricDelta2D>,
    pub rotation_degrees: Option<f64>,
    pub translation_x: f64,
    pub translation_y: f64,
}

/// A mark-only response delta.  An absent member explicitly inherits the
/// corresponding document response value.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkGeometryResponseDelta {
    pub minimum_fill_delta: Option<f64>,
    pub maximum_fill_delta: Option<f64>,
}

/// The normalized width response for one guide-path output.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedGeometryResponse {
    pub minimum_thickness: f64,
    pub maximum_thickness: f64,
}

/// Optional additive channel intent for the document-owned stroke response.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedGeometryResponseDelta {
    pub minimum_thickness_delta: Option<f64>,
    pub maximum_thickness_delta: Option<f64>,
}

/// The optional response-delta branch stored by a channel.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelGeometryResponseDelta {
    Marks(MarkGeometryResponseDelta),
    Connected(ConnectedGeometryResponseDelta),
}

/// The persisted pattern intent owned by one channel.  It never contains a
/// resolved effective instance and therefore remains valid when its base
/// changes only if the domain can resolve it.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPatternInstance {
    pub definition_override: Option<PatternDefinitionId>,
    pub layout_delta: ChannelPatternLayoutDelta,
    pub shape_rotation_delta_degrees: Option<f64>,
    pub output_response_deltas: Vec<PatternOutputResponseDelta>,
}

/// The sole resolved pattern input for one channel.  Engine and frontends read
/// this projection and never recompute inheritance or arithmetic themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveChannelPatternInstance {
    pub definition_id: PatternDefinitionId,
    pub density: DensityMetric2D,
    pub pattern_rotation_degrees: f64,
    pub translation_x: f64,
    pub translation_y: f64,
    pub shape_rotation_degrees: f64,
    pub output_settings: Vec<EffectivePatternOutputSettings>,
}

/// Selects the authoritative definition whose active structural capabilities
/// are projected. Channel scope resolves Stage 20G inheritance exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternCapabilityScope {
    DocumentBase,
    Channel(ChannelId),
}

/// Describes the active structural capabilities of one validated definition.
/// It is derived-only workflow information and is never persisted or cached.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternCapabilityProjection {
    pub definition_id: PatternDefinitionId,
    pub family: PatternFamilyCapabilityProjection,
    /// Ordered structural output facts paired with their authoritative base or effective response.
    pub outputs: Vec<PatternOutputCapabilityRecord>,
}

/// One definition-order output projection with no frontend-owned response interpretation.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternOutputCapabilityRecord {
    pub output_layer_id: PatternOutputLayerId,
    pub structural: PatternOutputCapabilityProjection,
    pub response: PatternGeometryResponse,
}

/// Identifies the active family branch without inferring a named preset or UI page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternFamilyCapabilityProjection {
    Grid(GridCapabilityProjection),
    Dispersion(DispersionCapabilityProjection),
    Parametric(ParametricCapabilityProjection),
}

/// Read-only workflow facts for a finite parametric curve source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricCapabilityProjection {
    pub generator: GeneratorCapabilities,
    pub supports_round_spiral: bool,
    pub supports_square_spiral: bool,
    pub raw_paths: bool,
    pub sites_along_curve: bool,
    pub normal_offset: bool,
}

/// States which current generator inputs participate in the active family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorCapabilities {
    pub density: bool,
    pub seed: bool,
}

/// Describes the active guide family structure and its published site product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridCapabilityProjection {
    pub generator: GeneratorCapabilities,
    pub guides: GuideCapabilities,
    pub site_product: GuideSiteProductCapability,
}

/// Describes currently active guide controls without granting edit authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideCapabilities {
    pub count: u8,
    pub spacing: bool,
    pub phase: bool,
    pub editable_curve: bool,
    pub prototype_kinds: Vec<GuidePrototypeKind>,
}

/// Identifies the active site product emitted by a guide family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideSiteProductCapability {
    Intersections,
    AlongGuides,
}

/// Describes the active dispersion mechanisms without exposing their payload values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispersionCapabilityProjection {
    pub generator: GeneratorCapabilities,
    pub character: RandomCharacterKind,
    pub density_modulation: DensityModulationKind,
    pub exclusion: ExclusionKind,
}

/// Describes one active output layer in stored definition order.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternOutputCapabilityProjection {
    Marks(MarkOutputCapabilityProjection),
    /// Reports the structural guide-path output without evaluator or renderer authority.
    GuidePaths(GuidePathOutputCapabilityProjection),
    /// Reports authored connection controls without exposing a derived graph or path collection.
    ConnectionPaths(ConnectionPathOutputCapabilityProjection),
    /// Reports authored conventional wall-maze controls without exposing derived arrangement state.
    MazeWalls(MazeWallOutputCapabilityProjection),
    /// Reports ordinary full-cell region capability without exposing topology or treatments.
    Regions(RegionOutputCapabilityProjection),
}

/// Describes the active current mark output without creating renderer authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkOutputCapabilityProjection {
    pub prototype: MarkPrototypeKind,
    pub orientation: MarkOrientationKind,
    pub fill_range: bool,
}

/// Describes the only accepted Stage 20I stroke style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuidePathOutputCapabilityProjection {
    pub round_join: bool,
    pub round_cap: bool,
    pub thickness_range: bool,
}

/// Read-only connection-output controls projected from one validated definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionPathOutputCapabilityProjection {
    pub program_kind: ConnectionProgramKind,
    pub maximum_degree: u32,
    pub maximum_distance: f64,
    pub minimum_degree: Option<u32>,
    pub seed: Option<u32>,
    pub round_join: bool,
    pub round_cap: bool,
    pub thickness_range: bool,
}

/// Payload-free connection program discriminant for capability consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionProgramKind {
    NearestLinks,
    RandomLinks,
    GridSpanningTree,
}

/// Read-only authored controls for one conventional wall-maze output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MazeWallOutputCapabilityProjection {
    pub seeded_recursive_backtracker: bool,
    pub seed: u32,
    pub round_join: bool,
    pub round_cap: bool,
    pub thickness_range: bool,
}

/// Read-only source kind for a fixed full canonical-region output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionSourceCapabilityKind {
    /// Builds ordinary cells from an eligible family-site product.
    OrdinaryVoronoi {
        site_mechanism_id: PatternMechanismId,
    },
    /// Builds complete bounded faces from selected ordered guide dimensions.
    GuideFaces {
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
}

/// Read-only fixed-region capability facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionOutputCapabilityProjection {
    pub source: RegionSourceCapabilityKind,
    pub full_treatment_only: bool,
    pub sampled_paint: bool,
}

/// The only supported guide-path join policy in Stage 20I.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeJoin {
    Round,
}

/// The only supported guide-path cap policy in Stage 20I.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeCap {
    Round,
}

/// Persisted authored stroke style; response width remains document/channel authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathStrokeStyle {
    pub join: StrokeJoin,
    pub cap: StrokeCap,
}

/// Authored mutual-nearest bounds consumed by one connection program.
#[derive(Clone, Copy, Debug)]
pub struct ConnectionAdjacencyIntent {
    pub maximum_degree: u32,
    pub maximum_distance: f64,
}

impl Eq for ConnectionAdjacencyIntent {}

impl PartialEq for ConnectionAdjacencyIntent {
    /// Compares exactly persisted control bits so invalid constructible NaNs remain reflexive under `Eq`.
    fn eq(&self, other: &Self) -> bool {
        self.maximum_degree == other.maximum_degree
            && self.maximum_distance.to_bits() == other.maximum_distance.to_bits()
    }
}

impl ConnectionAdjacencyIntent {
    /// Validates the bounded finite adjacency intent without deriving a graph.
    ///
    /// # Errors
    ///
    /// Returns a stable document diagnostic when the degree or distance is outside the Stage 20M contract.
    pub fn validate(self) -> Result<(), ValidationError> {
        if !(1..=32).contains(&self.maximum_degree) {
            return Err(ValidationError::new(
                "connection.adjacency.maximum_degree",
                "connection maximum degree must be in 1..=32",
            ));
        }
        if !self.maximum_distance.is_finite() || self.maximum_distance <= 0.0 {
            return Err(ValidationError::new(
                "connection.adjacency.maximum_distance",
                "connection maximum distance must be finite and positive",
            ));
        }
        Ok(())
    }
}

/// The deterministic Stage 20M maze selection contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridMazeAlgorithm {
    RecursiveBacktracker,
}

/// Persisted conventional wall-maze intent; all walls, faces, passages, and solutions remain derived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MazeProgram {
    pub algorithm: GridMazeAlgorithm,
    pub seed: u32,
}

impl MazeProgram {
    /// Validates the fixed recursive-backtracker selector before arrangement allocation.
    ///
    /// # Errors
    ///
    /// This present finite program has no invalid payload values and therefore cannot fail.
    pub const fn validate(&self) -> Result<(), ValidationError> {
        match self.algorithm {
            GridMazeAlgorithm::RecursiveBacktracker => Ok(()),
        }
    }
}

/// The deterministic Stage 20M spanning-tree selection contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridSpanningTreeAlgorithm {
    RandomizedPrim,
}

/// Persisted connection intent; selected edges and paths remain derived-only geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionProgram {
    NearestLinks {
        adjacency: ConnectionAdjacencyIntent,
    },
    RandomLinks {
        adjacency: ConnectionAdjacencyIntent,
        minimum_degree: u32,
        seed: u32,
    },
    GridSpanningTree {
        adjacency: ConnectionAdjacencyIntent,
        algorithm: GridSpanningTreeAlgorithm,
        seed: u32,
    },
}

impl Eq for ConnectionProgram {}

impl ConnectionProgram {
    /// Returns the payload-free authored program choice without deriving topology.
    pub const fn kind(&self) -> ConnectionProgramKind {
        match self {
            Self::NearestLinks { .. } => ConnectionProgramKind::NearestLinks,
            Self::RandomLinks { .. } => ConnectionProgramKind::RandomLinks,
            Self::GridSpanningTree { .. } => ConnectionProgramKind::GridSpanningTree,
        }
    }

    /// Returns the authored adjacency intent without constructing derived topology.
    pub const fn adjacency(&self) -> ConnectionAdjacencyIntent {
        match self {
            Self::NearestLinks { adjacency }
            | Self::RandomLinks { adjacency, .. }
            | Self::GridSpanningTree { adjacency, .. } => *adjacency,
        }
    }

    /// Returns the program seed when selection is randomized.
    pub const fn seed(&self) -> Option<u32> {
        match self {
            Self::NearestLinks { .. } => None,
            Self::RandomLinks { seed, .. } | Self::GridSpanningTree { seed, .. } => Some(*seed),
        }
    }

    /// Validates all authored program controls before any family, graph, or path allocation.
    ///
    /// # Errors
    ///
    /// Returns stable connection diagnostics for invalid adjacency or random-degree intent.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.adjacency().validate()?;
        if let Self::RandomLinks {
            adjacency,
            minimum_degree,
            ..
        } = self
            && *minimum_degree > adjacency.maximum_degree
        {
            return Err(ValidationError::new(
                "connection.minimum_degree",
                "connection minimum degree must not exceed maximum degree",
            ));
        }
        Ok(())
    }
}

impl Default for PathStrokeStyle {
    fn default() -> Self {
        Self {
            join: StrokeJoin::Round,
            cap: StrokeCap::Round,
        }
    }
}

/// Per-channel placement controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelPatternLayout {
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    pub translation_x: f64,
    pub translation_y: f64,
}

/// Canonical linear RGBA color components.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorValue {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorComponent {
    Red,
    Green,
    Blue,
    Alpha,
}

/// Presentation settings that do not alter canonical geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelAppearance {
    pub visible: bool,
    pub color: ColorValue,
    pub opacity: f64,
}

/// The normalized mark-fill response contract shared by every ordinary family.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkGeometryResponse {
    pub minimum_fill: f64,
    pub maximum_fill: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkGeometryFieldEdit {
    MinimumFill(f64),
    MaximumFill(f64),
}

/// The source component selected by a channel's authoritative source mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceComponent {
    Luminance,
    Alpha,
}

/// The sole supported document-to-source placement contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePlacement {
    #[default]
    StretchToCanvas,
}

/// Authoritative source interpretation for one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelSourceMapping {
    pub component: SourceComponent,
    pub placement: SourcePlacement,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyMappingFieldEdit {
    Component(SourceComponent),
    Placement(SourcePlacement),
}

/// The fixed set of authoritative complete-document channel arrangements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalftoneChannelModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}

/// One semantic role in an ordered halftone channel topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalftoneChannelRole {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    SourceColor,
}

impl HalftoneChannelModel {
    fn canonical_roles(self) -> &'static [HalftoneChannelRole] {
        match self {
            Self::Rgb => &[
                HalftoneChannelRole::Red,
                HalftoneChannelRole::Green,
                HalftoneChannelRole::Blue,
            ],
            Self::Cmyk => &[
                HalftoneChannelRole::Cyan,
                HalftoneChannelRole::Magenta,
                HalftoneChannelRole::Yellow,
                HalftoneChannelRole::Black,
            ],
            Self::SourceColorAlpha => &[HalftoneChannelRole::SourceColor],
        }
    }
}

/// A scalar component supported by the Stage 9 channel-mapping authority.
///
/// Evaluation of these source fields is deliberately deferred to Stage 9B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMappingComponent {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    Alpha,
    Luminance,
}

/// Complete source-mapping state used by a modeled topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceMapping {
    pub component: SourceMappingComponent,
    pub placement: SourcePlacement,
    pub inverted: bool,
    pub gain: f64,
    pub bias: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModeledMappingFieldEdit {
    Component(SourceMappingComponent),
    Placement(SourcePlacement),
    Inverted(bool),
    Gain(f64),
    Bias(f64),
}

impl SourceMapping {
    pub const fn canonical(component: SourceMappingComponent) -> Self {
        Self {
            component,
            placement: SourcePlacement::StretchToCanvas,
            inverted: false,
            gain: 1.0,
            bias: 0.0,
        }
    }

    /// Applies the authoritative Stage 9 transform. Source-field evaluation is
    /// intentionally not part of this domain-only slice.
    pub fn transform(self, value: f64) -> f64 {
        let value = if self.inverted { 1.0 - value } else { value };
        (self.gain * value + self.bias).clamp(0.0, 1.0)
    }
}

/// Presentation paint for a modeled channel. Sampled source paint is a
/// validated representation only until Stage 9B realizes it.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelPaint {
    Solid(ColorValue),
    SampledSource,
}

/// The pattern/layout/mark-response input cloned by a canonical topology
/// factory. It deliberately has no role-specific geometry defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelTopologyTemplate {
    pub pattern_instance: ChannelPatternInstance,
}

/// One complete modeled channel in its authoritative ordered topology.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeledChannelState {
    pub role: HalftoneChannelRole,
    pub id: ChannelId,
    pub pattern_instance: ChannelPatternInstance,
    pub mapping: SourceMapping,
    pub paint: ChannelPaint,
    pub visible: bool,
    pub opacity: f64,
}

/// A complete ordered channel topology supplied atomically with its model.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelTopology {
    channels: Vec<ModeledChannelState>,
}

impl ChannelTopology {
    pub fn new(channels: Vec<ModeledChannelState>) -> Self {
        Self { channels }
    }

    pub fn channels(&self) -> &[ModeledChannelState] {
        &self.channels
    }

    /// Builds the exact canonical topology for `model` by cloning one caller
    /// supplied template. Pattern-definition compatibility is checked when the
    /// topology is validated against a document.
    pub fn canonical(
        model: HalftoneChannelModel,
        template: ChannelTopologyTemplate,
    ) -> Result<Self, ValidationError> {
        validate_channel_pattern_instance(&template.pattern_instance)?;

        Ok(Self::new(
            model
                .canonical_roles()
                .iter()
                .copied()
                .map(|role| ModeledChannelState::canonical(role, &template))
                .collect(),
        ))
    }
}

impl ModeledChannelState {
    fn canonical(role: HalftoneChannelRole, template: &ChannelTopologyTemplate) -> Self {
        let (id, component, paint) = match role {
            HalftoneChannelRole::Red => (
                ChannelId(1),
                SourceMappingComponent::Red,
                ChannelPaint::Solid(ColorValue::red()),
            ),
            HalftoneChannelRole::Green => (
                ChannelId(2),
                SourceMappingComponent::Green,
                ChannelPaint::Solid(ColorValue::green()),
            ),
            HalftoneChannelRole::Blue => (
                ChannelId(3),
                SourceMappingComponent::Blue,
                ChannelPaint::Solid(ColorValue::blue()),
            ),
            HalftoneChannelRole::Cyan => (
                ChannelId(4),
                SourceMappingComponent::Cyan,
                ChannelPaint::Solid(ColorValue::cyan()),
            ),
            HalftoneChannelRole::Magenta => (
                ChannelId(5),
                SourceMappingComponent::Magenta,
                ChannelPaint::Solid(ColorValue::magenta()),
            ),
            HalftoneChannelRole::Yellow => (
                ChannelId(6),
                SourceMappingComponent::Yellow,
                ChannelPaint::Solid(ColorValue::yellow()),
            ),
            HalftoneChannelRole::Black => (
                ChannelId(7),
                SourceMappingComponent::Black,
                ChannelPaint::Solid(ColorValue::black()),
            ),
            HalftoneChannelRole::SourceColor => (
                ChannelId(8),
                SourceMappingComponent::Alpha,
                ChannelPaint::SampledSource,
            ),
        };
        let mapping = SourceMapping::canonical(component);
        Self {
            role,
            id,
            pattern_instance: template.pattern_instance.clone(),
            mapping,
            paint,
            visible: true,
            opacity: 1.0,
        }
    }
}

impl ColorValue {
    fn red() -> Self {
        Self {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn green() -> Self {
        Self {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn blue() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn cyan() -> Self {
        Self {
            red: 0.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn magenta() -> Self {
        Self {
            red: 1.0,
            green: 0.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }
    fn yellow() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
    fn black() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }
}

/// Exactly one structural root for a pattern definition.  The currently
/// supported root deliberately names mechanisms rather than a named artistic
/// result; later roots are data additions, not renderer branches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternFamily {
    GuideIntersections {
        guide_mechanism_id: PatternMechanismId,
        site_mechanism_id: PatternMechanismId,
    },
    /// A deterministic, site-only family.  Its ordered mechanisms keep the
    /// base process, density field, collision policy, and published site
    /// product independently addressable without inventing named patterns.
    RandomSites {
        base_site_process_id: PatternMechanismId,
        density_modulation_id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        site_product_id: PatternMechanismId,
    },
    /// One finite analytic source that publishes either raw paths or sites along that path.
    ParametricCurve {
        curve_mechanism_id: PatternMechanismId,
        site_mechanism_id: Option<PatternMechanismId>,
    },
}

/// The distinct structural character of an unmodulated candidate process.
#[derive(Clone, Debug, PartialEq)]
pub enum RandomSiteCharacter {
    /// Independent uniform candidates; close neighbors are permitted unless
    /// the separately declared exclusion mechanism rejects them.
    RawUniform,
    /// Sequential dart throwing with a declared center-distance construction.
    /// This is deliberately not an alias named "BlueNoise" or "Poisson".
    Even { minimum_center_distance: f64 },
    /// Parent-centered Gaussian islands mixed with a uniform background.
    Clustered {
        cluster_density: f64,
        cluster_spread: f64,
        cluster_strength: f64,
    },
}

/// Density enters site placement before collision acceptance.  Artwork
/// weighting is an explicit structural dependency, not mark modulation.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteDensityModulation {
    Uniform,
    ArtworkWeighted {
        mapping: SourceMapping,
        strength: f64,
        response: ArtworkWeightResponse,
    },
}

/// Typed response applied at the decoder-owned artwork density field boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkWeightResponse {
    Linear,
    /// A fixed IEEE-arithmetic curve: `x * x * (3 - 2 * x)` for clamped x.
    Smoothstep,
}

/// Size policy used by a visible-mark exclusion constraint.  The current
/// maximum-support policy is conservative: every realized circle is bounded
/// by the family support radius, so the stated separation survives realization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleMarkSizingPolicy {
    MaximumSupportRadius,
}

/// Collision behavior remains independent from candidate character and
/// density modulation.
#[derive(Clone, Debug, PartialEq)]
pub enum SiteExclusionPolicy {
    None,
    MinimumCenterDistance {
        minimum: f64,
    },
    VisibleMarkMargin {
        margin: f64,
        sizing: VisibleMarkSizingPolicy,
    },
}

/// A typed reusable structural mechanism.  Stage 14 retains only the accepted
/// straight-guide/intersection meaning; it is intentionally not a node graph.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternMechanism {
    StraightGuides {
        id: PatternMechanismId,
    },
    GuideIntersections {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
    },
    /// Generalized ordered straight dimensions.  The legacy `StraightGuides`
    /// form remains readable/writable for exact existing-v2 bytes.
    StraightGuideDimensions {
        id: PatternMechanismId,
        dimensions: Vec<StraightGuideDimension>,
    },
    /// Generic Stage 20D guide producer for document-owned open paths and
    /// deterministic circular arcs.  It remains a root consumed by existing
    /// selected-intersection and along-guide product mechanisms.
    GuideDimensions {
        id: PatternMechanismId,
        dimensions: Vec<GuideDimension>,
    },
    /// An explicit selected intersection product; selections are stable IDs,
    /// not positional aliases.
    SelectedGuideIntersections {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
        merge_epsilon: f64,
    },
    /// Regular arc-length sites along explicitly selected guides.
    AlongGuideSites {
        id: PatternMechanismId,
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
        interval_multiplier: f64,
        phase: f64,
    },
    /// One finite parametric source and its reusable Stage 20J repetition policy.
    ParametricCurveSource {
        id: PatternMechanismId,
        curve: ParametricCurve,
        repetition: CurveRepetition,
    },
    /// Equal-arc sites along one parametric source. `interval` is absolute document distance.
    AlongParametricCurveSites {
        id: PatternMechanismId,
        curve_mechanism_id: PatternMechanismId,
        interval: f64,
        phase: f64,
    },
    RandomSiteProcess {
        id: PatternMechanismId,
        character: RandomSiteCharacter,
        seed: u32,
    },
    SiteDensityModulation {
        id: PatternMechanismId,
        base_site_process_id: PatternMechanismId,
        modulation: SiteDensityModulation,
    },
    SiteExclusion {
        id: PatternMechanismId,
        density_modulation_id: PatternMechanismId,
        policy: SiteExclusionPolicy,
    },
    /// The published, reproducible site collection. Work limits are
    /// definition-owned: attempts bound candidate generation and neighbor
    /// checks bound deterministic spatial-index exclusion work.
    RandomSiteProduct {
        id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
    },
}

impl PatternMechanism {
    pub const fn id(&self) -> PatternMechanismId {
        match self {
            Self::StraightGuides { id }
            | Self::GuideIntersections { id, .. }
            | Self::StraightGuideDimensions { id, .. }
            | Self::GuideDimensions { id, .. }
            | Self::SelectedGuideIntersections { id, .. }
            | Self::AlongGuideSites { id, .. }
            | Self::ParametricCurveSource { id, .. }
            | Self::AlongParametricCurveSites { id, .. }
            | Self::RandomSiteProcess { id, .. }
            | Self::SiteDensityModulation { id, .. }
            | Self::SiteExclusion { id, .. }
            | Self::RandomSiteProduct { id, .. } => *id,
        }
    }
}

/// One ordered typed output layer.  Additional layer variants are deferred;
/// their absence is a capability failure, never a legacy fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternOutputLayer {
    CircularMarks {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
    },
    MarkPrototype {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
        prototype: MarkPrototype,
        orientation: MarkOrientation,
    },
    /// Consumes one ordered guide-path mechanism as round-brush strokes.
    GuidePaths {
        id: PatternOutputLayerId,
        guide_mechanism_id: PatternMechanismId,
        style: PathStrokeStyle,
    },
    /// Consumes one ordered parametric source as canonical connected paths.
    ParametricPaths {
        id: PatternOutputLayerId,
        curve_mechanism_id: PatternMechanismId,
        style: PathStrokeStyle,
    },
    /// Consumes one typed site product as deterministic positive connection paths.
    ConnectionPaths {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
        program: ConnectionProgram,
        style: PathStrokeStyle,
    },
    /// Consumes a straight-guide intersection site product as positive retained maze walls.
    MazeWalls {
        id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
        program: MazeProgram,
        style: PathStrokeStyle,
    },
    /// Consumes one eligible reusable site product as ordinary canonical Voronoi regions.
    Regions {
        id: PatternOutputLayerId,
        source: RegionSourceIntent,
    },
}

/// Authored source identity for one fixed full canonical-region output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionSourceIntent {
    /// Builds ordinary cells from the complete family site product with this mechanism identity.
    VoronoiSites {
        site_mechanism_id: PatternMechanismId,
    },
    /// Builds complete bounded arrangement faces from two or three selected guides.
    GuideFaces {
        guide_mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
}

impl RegionSourceIntent {
    /// Returns the consumed site-product mechanism only for ordinary Voronoi output.
    pub const fn site_mechanism_id(&self) -> Option<PatternMechanismId> {
        match self {
            Self::VoronoiSites { site_mechanism_id } => Some(*site_mechanism_id),
            Self::GuideFaces { .. } => None,
        }
    }

    /// Returns the consumed guide mechanism only for arrangement-face output.
    pub fn guide_faces(&self) -> Option<(PatternMechanismId, &[GuideDimensionId])> {
        match self {
            Self::VoronoiSites { .. } => None,
            Self::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => Some((*guide_mechanism_id, dimensions)),
        }
    }
}

impl PatternOutputLayer {
    pub const fn id(&self) -> PatternOutputLayerId {
        match self {
            Self::CircularMarks { id, .. }
            | Self::MarkPrototype { id, .. }
            | Self::GuidePaths { id, .. }
            | Self::ParametricPaths { id, .. }
            | Self::ConnectionPaths { id, .. }
            | Self::MazeWalls { id, .. }
            | Self::Regions { id, .. } => *id,
        }
    }
}

/// Structural modulation is a separate typed top-level slot.  The accepted
/// v1 configuration has no modulation; later additions must remain typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternModulation;

/// Structural coverage planning.  Canvas boundaries remain final-consumer
/// clipping and are never topology input.
#[derive(Clone, Debug, PartialEq)]
pub struct CoveragePolicy {
    pub guard_steps: u32,
    pub additional_margin: f64,
}

/// Structural pattern authority owned by the document.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinition {
    pub id: PatternDefinitionId,
    pub name: String,
    pub family: PatternFamily,
    pub mechanisms: Vec<PatternMechanism>,
    pub output_layers: Vec<PatternOutputLayer>,
    pub modulation: PatternModulation,
    pub coverage: CoveragePolicy,
}

impl PatternDefinition {
    /// Builds the sole Stage 14 adapter configuration.  It is data only: the
    /// engine asks this definition for typed capability, never for a preset or
    /// legacy name.
    pub fn supported_straight_grid(
        id: PatternDefinitionId,
        name: impl Into<String>,
        guide_id: PatternMechanismId,
        intersections_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        coverage: CoveragePolicy,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            family: PatternFamily::GuideIntersections {
                guide_mechanism_id: guide_id,
                site_mechanism_id: intersections_id,
            },
            mechanisms: vec![
                PatternMechanism::StraightGuides { id: guide_id },
                PatternMechanism::GuideIntersections {
                    id: intersections_id,
                    guide_mechanism_id: guide_id,
                },
            ],
            output_layers: vec![PatternOutputLayer::CircularMarks {
                id: output_id,
                site_mechanism_id: intersections_id,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }

    /// Constructs an explicit generalized straight-guide definition without a
    /// named pattern/preset discriminator.  Selection IDs must appear in the
    /// stored dimension order and are validated with the document.
    #[allow(clippy::too_many_arguments)] // Stable IDs are intentionally explicit at the schema boundary.
    pub fn generalized_straight_guides(
        id: PatternDefinitionId,
        name: impl Into<String>,
        guide_id: PatternMechanismId,
        site_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        dimensions: Vec<StraightGuideDimension>,
        product: GeneralizedSiteProduct,
        orientation: MarkOrientation,
        coverage: CoveragePolicy,
    ) -> Self {
        let site = match product {
            GeneralizedSiteProduct::Intersections {
                dimensions,
                merge_epsilon,
            } => PatternMechanism::SelectedGuideIntersections {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                merge_epsilon,
            },
            GeneralizedSiteProduct::AlongGuides {
                dimensions,
                interval_multiplier,
                phase,
            } => PatternMechanism::AlongGuideSites {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                interval_multiplier,
                phase,
            },
        };
        Self {
            id,
            name: name.into(),
            family: PatternFamily::GuideIntersections {
                guide_mechanism_id: guide_id,
                site_mechanism_id: site_id,
            },
            mechanisms: vec![
                PatternMechanism::StraightGuideDimensions {
                    id: guide_id,
                    dimensions,
                },
                site,
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: output_id,
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }

    /// Constructs an explicit Stage 20D generic guide definition with exactly
    /// one existing site product and one existing circle output layer.
    #[allow(clippy::too_many_arguments)]
    pub fn generalized_guides(
        id: PatternDefinitionId,
        name: impl Into<String>,
        guide_id: PatternMechanismId,
        site_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        dimensions: Vec<GuideDimension>,
        product: GeneralizedSiteProduct,
        orientation: MarkOrientation,
        coverage: CoveragePolicy,
    ) -> Self {
        let site = match product {
            GeneralizedSiteProduct::Intersections {
                dimensions,
                merge_epsilon,
            } => PatternMechanism::SelectedGuideIntersections {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                merge_epsilon,
            },
            GeneralizedSiteProduct::AlongGuides {
                dimensions,
                interval_multiplier,
                phase,
            } => PatternMechanism::AlongGuideSites {
                id: site_id,
                guide_mechanism_id: guide_id,
                dimensions,
                interval_multiplier,
                phase,
            },
        };
        Self {
            id,
            name: name.into(),
            family: PatternFamily::GuideIntersections {
                guide_mechanism_id: guide_id,
                site_mechanism_id: site_id,
            },
            mechanisms: vec![
                PatternMechanism::GuideDimensions {
                    id: guide_id,
                    dimensions,
                },
                site,
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: output_id,
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }

    /// Constructs the bounded Stage 16B random-site mechanism chain.  The
    /// caller supplies document-wide IDs explicitly; document validation owns
    /// collision and order checks.
    #[allow(clippy::too_many_arguments)]
    pub fn random_sites(
        id: PatternDefinitionId,
        name: impl Into<String>,
        base_id: PatternMechanismId,
        modulation_id: PatternMechanismId,
        exclusion_id: PatternMechanismId,
        site_id: PatternMechanismId,
        output_id: PatternOutputLayerId,
        character: RandomSiteCharacter,
        seed: u32,
        density_modulation: SiteDensityModulation,
        exclusion: SiteExclusionPolicy,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
        coverage: CoveragePolicy,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            family: PatternFamily::RandomSites {
                base_site_process_id: base_id,
                density_modulation_id: modulation_id,
                exclusion_id,
                site_product_id: site_id,
            },
            mechanisms: vec![
                PatternMechanism::RandomSiteProcess {
                    id: base_id,
                    character,
                    seed,
                },
                PatternMechanism::SiteDensityModulation {
                    id: modulation_id,
                    base_site_process_id: base_id,
                    modulation: density_modulation,
                },
                PatternMechanism::SiteExclusion {
                    id: exclusion_id,
                    density_modulation_id: modulation_id,
                    policy: exclusion,
                },
                PatternMechanism::RandomSiteProduct {
                    id: site_id,
                    exclusion_id,
                    maximum_attempts,
                    maximum_neighbor_checks,
                },
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: output_id,
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            }],
            modulation: PatternModulation,
            coverage,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeneralizedSiteProduct {
    Intersections {
        dimensions: Vec<GuideDimensionId>,
        merge_epsilon: f64,
    },
    AlongGuides {
        dimensions: Vec<GuideDimensionId>,
        interval_multiplier: f64,
        phase: f64,
    },
}

/// Source state owned by the document, never a filesystem path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceReference {
    #[default]
    Unassigned,
    Assigned(SourceReferenceId),
}

/// Per-channel authoritative state.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelState {
    pub id: ChannelId,
    pub pattern_instance: ChannelPatternInstance,
    pub appearance: ChannelAppearance,
    pub source_mapping: ChannelSourceMapping,
}

/// The sole document model. Its collections are read-only outside this crate.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    id: DocumentId,
    canvas: CanvasSpec,
    source: SourceReference,
    pattern_definition_bundles: Vec<PatternDefinitionBundle>,
    channel_configuration: ChannelConfiguration,
    authored_structures: Vec<AuthoredStructure>,
    pattern_settings: DocumentPatternSettings,
}

#[derive(Clone, Debug, PartialEq)]
enum ChannelConfiguration {
    Legacy(Vec<ChannelState>),
    Topology {
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    },
}

impl Document {
    /// Builds the accepted default document directly, without a
    /// command transition.  Frontends use this for new and direct-source
    /// workspaces so their `DocumentSession`/`DocumentHistory` begins at
    /// revision zero. The fixed template stays encapsulated in the headless
    /// document layer rather than requiring caller-side construction details.
    /// It initializes the Stage 20C authored-structure store empty.
    ///
    /// # Errors
    ///
    /// Returns the existing topology or document validation diagnostics; the
    /// default never manufactures authored structures.
    pub fn new_default_document(
        canvas: CanvasSpec,
        source: SourceReference,
    ) -> Result<Self, ValidationError> {
        let settings = DocumentPatternSettings {
            definition_id: PatternDefinitionId(1),
            density: DensityMetric2D {
                across_x: canvas.width / 10.0,
                across_y: canvas.height / 10.0,
                aspect_locked: true,
            },
            pattern_rotation_degrees: 0.0,
            shape_rotation_degrees: 0.0,
        };
        let definition = PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "Straight circular marks",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
        );
        let bundle = PatternDefinitionBundle {
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(1),
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            }],
            definition,
        };
        let template = ChannelTopologyTemplate {
            pattern_instance: ChannelPatternInstance {
                definition_override: None,
                layout_delta: ChannelPatternLayoutDelta {
                    density: None,
                    rotation_degrees: None,
                    translation_x: 0.0,
                    translation_y: 0.0,
                },
                shape_rotation_delta_degrees: None,
                output_response_deltas: Vec::new(),
            },
        };
        let topology = ChannelTopology::canonical(HalftoneChannelModel::Rgb, template)?;
        Self::with_source_and_topology(
            DocumentId(1),
            canvas,
            source,
            vec![bundle],
            settings,
            HalftoneChannelModel::Rgb,
            topology,
        )
    }

    /// Constructs and validates a document with an unassigned source reference.
    /// It delegates to the empty-authored-store legacy constructor.
    ///
    /// # Errors
    ///
    /// Returns any legacy document validation diagnostic after validating the
    /// complete candidate and its empty authored-structure store.
    pub fn new(
        id: DocumentId,
        canvas: CanvasSpec,
        pattern_definition_bundles: Vec<PatternDefinitionBundle>,
        pattern_settings: DocumentPatternSettings,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        Self::with_source(
            id,
            canvas,
            SourceReference::Unassigned,
            pattern_definition_bundles,
            pattern_settings,
            channels,
        )
    }

    /// Constructs and validates a document with explicitly supplied source state.
    /// It initializes the Stage 20C authored-structure store empty.
    ///
    /// # Errors
    ///
    /// Returns any legacy document validation diagnostic after validating the
    /// empty authored-structure store with the complete candidate.
    pub fn with_source(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definition_bundles: Vec<PatternDefinitionBundle>,
        pattern_settings: DocumentPatternSettings,
        channels: Vec<ChannelState>,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definition_bundles,
            channel_configuration: ChannelConfiguration::Legacy(channels),
            authored_structures: Vec::new(),
            pattern_settings,
        };
        document.validate()?;
        Ok(document)
    }

    /// Constructs a complete modeled document from an explicit, already
    /// ordered topology. This is intentionally the narrow construction seam
    /// used by persistence to rebuild a validated authoritative document; it
    /// does not expose the private channel-configuration representation. It
    /// initializes the Stage 20C authored-structure store empty.
    ///
    /// # Errors
    ///
    /// Returns any topology or complete-document validation diagnostic after
    /// validating the empty authored-structure store.
    pub fn with_source_and_topology(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definition_bundles: Vec<PatternDefinitionBundle>,
        pattern_settings: DocumentPatternSettings,
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definition_bundles,
            channel_configuration: ChannelConfiguration::Topology { model, topology },
            authored_structures: Vec::new(),
            pattern_settings,
        };
        document.validate()?;
        Ok(document)
    }

    /// Constructs and validates a legacy-channel document with explicit authored construction structures.
    ///
    /// # Errors
    ///
    /// Returns any existing document validation failure or an authored-structure ID, topology,
    /// coordinate, continuity, closure, or document-wide limit diagnostic.
    pub fn with_source_and_authored_structures(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definition_bundles: Vec<PatternDefinitionBundle>,
        pattern_settings: DocumentPatternSettings,
        channels: Vec<ChannelState>,
        authored_structures: Vec<AuthoredStructure>,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definition_bundles,
            channel_configuration: ChannelConfiguration::Legacy(channels),
            authored_structures,
            pattern_settings,
        };
        document.validate()?;
        Ok(document)
    }

    /// Constructs and validates a modeled-topology document with explicit authored construction structures.
    ///
    /// # Errors
    ///
    /// Returns any existing document validation failure or an authored-structure ID, topology,
    /// coordinate, continuity, closure, or document-wide limit diagnostic.
    #[allow(clippy::too_many_arguments)] // Explicit persisted authorities stay visible at this schema boundary.
    pub fn with_source_topology_and_authored_structures(
        id: DocumentId,
        canvas: CanvasSpec,
        source: SourceReference,
        pattern_definition_bundles: Vec<PatternDefinitionBundle>,
        pattern_settings: DocumentPatternSettings,
        model: HalftoneChannelModel,
        topology: ChannelTopology,
        authored_structures: Vec<AuthoredStructure>,
    ) -> Result<Self, ValidationError> {
        let document = Self {
            id,
            canvas,
            source,
            pattern_definition_bundles,
            channel_configuration: ChannelConfiguration::Topology { model, topology },
            authored_structures,
            pattern_settings,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    pub fn source(&self) -> &SourceReference {
        &self.source
    }

    pub fn pattern_definition_bundles(&self) -> &[PatternDefinitionBundle] {
        &self.pattern_definition_bundles
    }

    /// Returns the persisted document-wide base settings.  This is the only
    /// shared pattern authority; frontends must construct mutations through
    /// domain commands rather than calculate channel inheritance themselves.
    pub fn pattern_settings(&self) -> &DocumentPatternSettings {
        &self.pattern_settings
    }

    /// Returns one channel's stored replacement and additive intent without
    /// exposing a derived effective value as persistable state.
    pub fn channel_pattern_instance(
        &self,
        channel_id: ChannelId,
    ) -> Option<&ChannelPatternInstance> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.pattern_instance),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &channel.pattern_instance),
        }
    }

    /// Removes only response deltas that cannot apply to this channel's newly
    /// selected definition bundle.
    ///
    /// Definition selection is allowed to replace an output contract.  This
    /// keeps matching output IDs and response kinds verbatim while dropping
    /// foreign or incompatible deltas before the candidate is validated, so a
    /// valid structural replacement never fails solely because of stale
    /// channel-relative response intent.
    fn prune_incompatible_output_response_deltas(&mut self, channel_id: ChannelId) {
        let definition_id = self
            .channel_pattern_instance(channel_id)
            .expect("validated channel exists")
            .definition_override
            .unwrap_or(self.pattern_settings.definition_id);
        let bundle = self
            .bundle(definition_id)
            .expect("validated selected bundle exists")
            .clone();
        let instance = self
            .channel_pattern_instance_mut(channel_id)
            .expect("validated channel exists");
        instance.output_response_deltas =
            prune_output_response_deltas_for_bundle(&bundle, &instance.output_response_deltas);
    }

    /// Removes only deltas made foreign or type-incompatible by a document
    /// base-definition replacement while preserving overridden channels.
    fn prune_document_base_output_response_deltas(&mut self) {
        let channel_ids = self.channel_ids();
        for channel_id in channel_ids {
            self.prune_incompatible_output_response_deltas(channel_id);
        }
    }

    /// Rebinds one cloned definition's base responses to the duplicate output
    /// IDs in exact structural order.
    ///
    /// # Errors
    ///
    /// Returns an allocation/remap diagnostic when the clone does not retain a
    /// one-to-one, response-kind-compatible output correspondence. It never
    /// exposes an unbound duplicate bundle.
    fn bundle_for_duplicate(
        &self,
        source: &PatternDefinitionBundle,
        duplicate: PatternDefinition,
    ) -> Result<PatternDefinitionBundle, ValidationError> {
        if source.definition.output_layers.len() != duplicate.output_layers.len() {
            return Err(ValidationError::new(
                "pattern.bundle.duplicate.outputs",
                "duplicated definitions must retain exact output correspondence",
            ));
        }
        let output_settings = source
            .output_settings
            .iter()
            .zip(&duplicate.output_layers)
            .map(|(settings, output)| {
                validate_response_for_output(output, &settings.response)?;
                Ok(PatternOutputSettings {
                    output_layer_id: output.id(),
                    response: settings.response.clone(),
                })
            })
            .collect::<Result<Vec<_>, ValidationError>>()?;
        let bundle = PatternDefinitionBundle {
            definition: duplicate,
            output_settings,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    /// Remaps one channel's stored deltas through a validated copy-on-edit
    /// output correspondence, retaining their authored order and values.
    fn remap_channel_output_response_deltas(
        &mut self,
        channel_id: ChannelId,
        source: &PatternDefinitionBundle,
        duplicate: &PatternDefinitionBundle,
    ) {
        let correspondences: Vec<_> = source
            .output_settings
            .iter()
            .zip(&duplicate.output_settings)
            .map(|(old, new)| (old.output_layer_id, new.output_layer_id))
            .collect();
        let deltas = &mut self
            .channel_pattern_instance_mut(channel_id)
            .expect("validated channel exists")
            .output_response_deltas;
        for entry in deltas {
            entry.output_layer_id = correspondences
                .iter()
                .find(|(old, _)| *old == entry.output_layer_id)
                .map(|(_, new)| *new)
                .expect("validated source delta has duplicate correspondence");
        }
    }

    /// Resolves and validates the only effective pattern instance for a
    /// channel.  No caller may recompute base inheritance, additive deltas,
    /// aspect locking, or response compatibility.
    ///
    /// # Errors
    ///
    /// Returns a stable validation error for a missing channel/definition,
    /// non-finite addition, invalid density, incompatible response, or mark
    /// response bounds; it never mutates the document.
    pub fn effective_channel_pattern(
        &self,
        channel_id: ChannelId,
    ) -> Result<EffectiveChannelPatternInstance, ValidationError> {
        if !self.has_channel(channel_id) {
            return Err(ValidationError::new(
                "channel.id",
                "command targets a missing channel",
            ));
        }
        let instance = self
            .channel_pattern_instance(channel_id)
            .ok_or(ValidationError::new(
                "channel.pattern_instance",
                "channel is missing persisted pattern intent",
            ))?;
        let definition_id = instance
            .definition_override
            .unwrap_or(self.pattern_settings.definition_id);
        let bundle = self.bundle(definition_id).ok_or(ValidationError::new(
            "channel.pattern.definition_id",
            "channel resolves a missing pattern definition",
        ))?;
        let definition = &bundle.definition;
        let density_delta = instance.layout_delta.density.as_ref();
        let density = DensityMetric2D {
            across_x: self.pattern_settings.density.across_x
                + density_delta.map_or(0.0, |value| value.across_x_delta),
            across_y: self.pattern_settings.density.across_y
                + density_delta.map_or(0.0, |value| value.across_y_delta),
            aspect_locked: self.pattern_settings.density.aspect_locked,
        };
        let pattern_rotation_degrees = self.pattern_settings.pattern_rotation_degrees
            + instance.layout_delta.rotation_degrees.unwrap_or(0.0);
        let shape_rotation_degrees = self.pattern_settings.shape_rotation_degrees
            + instance.shape_rotation_delta_degrees.unwrap_or(0.0);
        let output_settings =
            effective_pattern_output_settings(bundle, &instance.output_response_deltas)?;
        let effective = EffectiveChannelPatternInstance {
            definition_id,
            density,
            pattern_rotation_degrees,
            translation_x: instance.layout_delta.translation_x,
            translation_y: instance.layout_delta.translation_y,
            shape_rotation_degrees,
            output_settings,
        };
        validate_effective_pattern(&effective)?;
        validate_effective_response_compatibility(definition, &effective.output_settings)?;
        if matches!(
            definition.output_layers.as_slice(),
            [PatternOutputLayer::GuidePaths { .. }
                | PatternOutputLayer::ConnectionPaths { .. }
                | PatternOutputLayer::MazeWalls { .. }]
        ) && effective.shape_rotation_degrees != 0.0
        {
            return Err(ValidationError::new(
                "effective_pattern.shape_rotation_degrees",
                "shape rotation is inapplicable to guide-path output",
            ));
        }
        Ok(effective)
    }

    /// Projects active recipe capabilities from the document base or one resolved channel.
    /// Channel scope delegates inheritance and delta resolution to the sole Stage 20G
    /// effective-pattern authority; the returned derived value never mutates state,
    /// participates in commands, or changes cache identity.
    ///
    /// # Errors
    ///
    /// Returns an established validation error when the selected channel or authoritative
    /// definition is missing or when its stored recipe cannot form an accepted current projection.
    pub fn pattern_capabilities(
        &self,
        scope: PatternCapabilityScope,
    ) -> Result<PatternCapabilityProjection, ValidationError> {
        let (definition_id, settings) = match scope {
            PatternCapabilityScope::DocumentBase => (self.pattern_settings.definition_id, None),
            PatternCapabilityScope::Channel(channel_id) => {
                let effective = self.effective_channel_pattern(channel_id)?;
                (effective.definition_id, Some(effective.output_settings))
            }
        };
        let bundle = self
            .pattern_definition_bundles
            .iter()
            .find(|bundle| bundle.definition.id == definition_id)
            .ok_or(ValidationError::new(
                "document.pattern_settings.definition_id",
                "capability scope resolves a missing pattern definition",
            ))?;
        let settings = settings.unwrap_or_else(|| {
            bundle
                .output_settings
                .iter()
                .map(|value| EffectivePatternOutputSettings {
                    output_layer_id: value.output_layer_id,
                    response: value.response.clone(),
                })
                .collect()
        });
        project_validated_pattern_definition(&bundle.definition, &settings)
    }

    /// Builds a stale-aware density-delta command from desired effective
    /// values.  When the base owns an aspect lock both stored axes are derived
    /// here, so frontends never subtract the document base.
    pub fn set_channel_density_for_effective(
        &self,
        channel_id: ChannelId,
        edited_axis: DensityEditedAxis,
        desired: DensityMetric2D,
    ) -> Result<DocumentCommand, ValidationError> {
        validate_positive_finite(desired.across_x, "channel.pattern.desired_density.across_x")?;
        validate_positive_finite(desired.across_y, "channel.pattern.desired_density.across_y")?;
        let mut desired = desired;
        if self.pattern_settings.density.aspect_locked {
            let value = match edited_axis {
                DensityEditedAxis::AcrossX => desired.across_x,
                DensityEditedAxis::AcrossY => desired.across_y,
            };
            let paired = derive_density_axis(&self.canvas, value, edited_axis)?;
            match edited_axis {
                DensityEditedAxis::AcrossX => desired.across_y = paired,
                DensityEditedAxis::AcrossY => desired.across_x = paired,
            }
        }
        Ok(DocumentCommand::SetChannelDensityDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            density: DensityMetricDelta2D {
                across_x_delta: desired.across_x - self.pattern_settings.density.across_x,
                across_y_delta: desired.across_y - self.pattern_settings.density.across_y,
            },
        })
    }

    /// Builds an atomic document-base density edit and derives the locked
    /// companion axis within domain authority.
    pub fn set_document_density_axis(
        &self,
        edited_axis: DensityEditedAxis,
        value: f64,
    ) -> Result<DocumentCommand, ValidationError> {
        validate_positive_finite(value, "document.pattern_settings.density")?;
        let mut settings = self.pattern_settings.clone();
        match edited_axis {
            DensityEditedAxis::AcrossX => settings.density.across_x = value,
            DensityEditedAxis::AcrossY => settings.density.across_y = value,
        }
        if settings.density.aspect_locked {
            let paired = derive_density_axis(&self.canvas, value, edited_axis)?;
            match edited_axis {
                DensityEditedAxis::AcrossX => settings.density.across_y = paired,
                DensityEditedAxis::AcrossY => settings.density.across_x = paired,
            }
        }
        Ok(DocumentCommand::SetDocumentPatternSettings {
            base: self.pattern_settings.clone(),
            settings,
        })
    }

    /// Builds an atomic document-owned aspect-lock transition without allowing
    /// any channel to override the lock.
    pub fn set_document_density_aspect_lock(
        &self,
        aspect_locked: bool,
    ) -> Result<DocumentCommand, ValidationError> {
        let mut settings = self.pattern_settings.clone();
        settings.density.aspect_locked = aspect_locked;
        if aspect_locked {
            settings.density.across_y = derive_density_axis(
                &self.canvas,
                settings.density.across_x,
                DensityEditedAxis::AcrossX,
            )?;
        }
        Ok(DocumentCommand::SetDocumentPatternSettings {
            base: self.pattern_settings.clone(),
            settings,
        })
    }

    /// Builds a stale-aware rotation-delta command from a desired effective
    /// layout rotation without normalizing authored degrees.
    pub fn set_channel_pattern_rotation_for_effective(
        &self,
        channel_id: ChannelId,
        desired_rotation_degrees: f64,
    ) -> Result<DocumentCommand, ValidationError> {
        validate_finite(
            desired_rotation_degrees,
            "channel.pattern.desired_rotation_degrees",
        )?;
        Ok(DocumentCommand::SetChannelPatternRotationDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            rotation_degrees: desired_rotation_degrees
                - self.pattern_settings.pattern_rotation_degrees,
        })
    }

    /// Builds a stale-aware shape-rotation delta from desired effective
    /// degrees without coupling the mark response branch to rotation.
    pub fn set_channel_shape_rotation_for_effective(
        &self,
        channel_id: ChannelId,
        desired_rotation_degrees: f64,
    ) -> Result<DocumentCommand, ValidationError> {
        validate_finite(
            desired_rotation_degrees,
            "channel.pattern.desired_shape_rotation_degrees",
        )?;
        Ok(DocumentCommand::SetChannelShapeRotationDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            rotation_degrees: desired_rotation_degrees
                - self.pattern_settings.shape_rotation_degrees,
        })
    }

    /// Builds a stale-aware output-keyed response delta from desired effective values.
    pub fn set_channel_output_response_for_effective(
        &self,
        channel_id: ChannelId,
        output_layer_id: PatternOutputLayerId,
        desired: PatternGeometryResponse,
    ) -> Result<DocumentCommand, ValidationError> {
        let effective = self.effective_channel_pattern(channel_id)?;
        let base = self
            .bundle(effective.definition_id)
            .and_then(|bundle| {
                bundle
                    .output_settings
                    .iter()
                    .find(|setting| setting.output_layer_id == output_layer_id)
            })
            .ok_or(ValidationError::new(
                "channel.pattern.output_layer_id",
                "channel output response targets a missing structural output",
            ))?;
        let delta = match (&base.response, desired) {
            (PatternGeometryResponse::Marks(base), PatternGeometryResponse::Marks(desired)) => {
                validate_mark_response(&desired)?;
                ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                    minimum_fill_delta: Some(desired.minimum_fill - base.minimum_fill),
                    maximum_fill_delta: Some(desired.maximum_fill - base.maximum_fill),
                })
            }
            (
                PatternGeometryResponse::Connected(base),
                PatternGeometryResponse::Connected(desired),
            ) => {
                validate_connected_response(&desired)?;
                ChannelGeometryResponseDelta::Connected(ConnectedGeometryResponseDelta {
                    minimum_thickness_delta: Some(
                        desired.minimum_thickness - base.minimum_thickness,
                    ),
                    maximum_thickness_delta: Some(
                        desired.maximum_thickness - base.maximum_thickness,
                    ),
                })
            }
            _ => {
                return Err(ValidationError::new(
                    "channel.pattern.geometry_response",
                    "desired response must match the document response branch",
                ));
            }
        };
        Ok(DocumentCommand::SetChannelOutputResponseDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            output_layer_id,
            delta,
        })
    }

    /// Builds the stale-aware geometry-response reset without materializing an inherited value.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the requested channel does not exist; command execution
    /// still validates that remaining overrides are compatible atomically.
    pub fn reset_channel_output_response_delta(
        &self,
        channel_id: ChannelId,
        output_layer_id: PatternOutputLayerId,
    ) -> Result<DocumentCommand, ValidationError> {
        self.channel_pattern_instance(channel_id)
            .ok_or(ValidationError::new(
                "channel.pattern.channel",
                "channel pattern instance is missing",
            ))?;
        Ok(DocumentCommand::ResetChannelOutputResponseDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            output_layer_id,
        })
    }

    /// Builds a stale-aware mark-response field delta while retaining the
    /// other optional member exactly as authored. This prevents a one-field
    /// edit from materializing an inherited companion value.
    ///
    /// # Errors
    ///
    /// Returns a validation error without mutation when the channel is
    /// missing, the desired resolved response is invalid, or the stored base
    /// cannot produce a finite delta.
    pub fn set_channel_mark_response_field_for_effective(
        &self,
        channel_id: ChannelId,
        output_layer_id: PatternOutputLayerId,
        edit: MarkGeometryFieldEdit,
    ) -> Result<DocumentCommand, ValidationError> {
        let effective = self.effective_channel_pattern(channel_id)?;
        let Some(PatternGeometryResponse::Marks(mut desired)) = effective
            .output_settings
            .iter()
            .find(|setting| setting.output_layer_id == output_layer_id)
            .map(|setting| setting.response.clone())
        else {
            return Err(ValidationError::new(
                "channel.pattern.geometry_response",
                "mark response editing requires the marks branch",
            ));
        };
        let Some(PatternGeometryResponse::Marks(base)) = self
            .bundle(effective.definition_id)
            .and_then(|bundle| {
                bundle
                    .output_settings
                    .iter()
                    .find(|setting| setting.output_layer_id == output_layer_id)
            })
            .map(|setting| &setting.response)
        else {
            return Err(ValidationError::new(
                "channel.pattern.geometry_response",
                "mark response editing requires the marks base",
            ));
        };
        let instance = self
            .channel_pattern_instance(channel_id)
            .ok_or(ValidationError::new(
                "channel.id",
                "command targets a missing channel",
            ))?;
        let mut delta = match instance
            .output_response_deltas
            .iter()
            .find(|delta| delta.output_layer_id == output_layer_id)
            .map(|delta| &delta.delta)
        {
            Some(ChannelGeometryResponseDelta::Marks(delta)) => delta.clone(),
            Some(ChannelGeometryResponseDelta::Connected(_)) => {
                return Err(ValidationError::new(
                    "channel.pattern.geometry_response_delta",
                    "mark response editing requires a matching mark delta",
                ));
            }
            None => MarkGeometryResponseDelta {
                minimum_fill_delta: None,
                maximum_fill_delta: None,
            },
        };
        match edit {
            MarkGeometryFieldEdit::MinimumFill(value) => {
                desired.minimum_fill = value;
                delta.minimum_fill_delta = Some(value - base.minimum_fill);
            }
            MarkGeometryFieldEdit::MaximumFill(value) => {
                desired.maximum_fill = value;
                delta.maximum_fill_delta = Some(value - base.maximum_fill);
            }
        }
        validate_mark_response(&desired)?;
        Ok(DocumentCommand::SetChannelOutputResponseDelta {
            base: self.pattern_settings.clone(),
            channel_id,
            output_layer_id,
            delta: ChannelGeometryResponseDelta::Marks(delta),
        })
    }

    /// Returns document-owned authored structures in stable creation order.
    pub fn authored_structures(&self) -> &[AuthoredStructure] {
        &self.authored_structures
    }

    /// Resolves one authored structure only within this document's stable ID namespace.
    pub fn authored_structure(&self, id: AuthoredStructureId) -> Option<&AuthoredStructure> {
        self.authored_structures
            .iter()
            .find(|structure| structure.id == id)
    }

    /// Projects every persisted guide and mark use in deterministic document, mechanism, and layer order.
    ///
    /// The projection is read-only and carries no UI labels, ordinals, prompts, or editor scope state.
    pub fn authored_structure_uses(&self) -> Vec<AuthoredStructureUse> {
        let mut uses = Vec::new();
        for channel_id in self.channel_ids() {
            let Some(definition_id) = self.pattern_definition_id_for(channel_id) else {
                continue;
            };
            let Some(definition) = self.definition(definition_id) else {
                continue;
            };
            for mechanism in &definition.mechanisms {
                let PatternMechanism::GuideDimensions { id, dimensions } = mechanism else {
                    continue;
                };
                for dimension in dimensions {
                    if let GuidePrototype::AuthoredOpenPath { structure_id } = &dimension.prototype
                    {
                        uses.push(AuthoredStructureUse::Guide {
                            channel_id,
                            definition_id,
                            mechanism_id: *id,
                            dimension_id: dimension.id,
                            structure_id: *structure_id,
                        });
                    }
                }
            }
            for layer in &definition.output_layers {
                if let PatternOutputLayer::MarkPrototype {
                    id,
                    prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                    ..
                } = layer
                {
                    uses.push(AuthoredStructureUse::Mark {
                        channel_id,
                        definition_id,
                        output_layer_id: *id,
                        structure_id: *structure_id,
                    });
                }
            }
        }
        uses
    }

    /// Reports whether any guide or mark output shares this authored structure reference.
    ///
    /// The document owns this referential-integrity boundary; evaluators and
    /// renderers must not retain resources that this method permits removing.
    fn authored_structure_is_referenced(&self, id: AuthoredStructureId) -> bool {
        self.pattern_definition_bundles.iter().any(|bundle| {
            let definition = &bundle.definition;
            definition.mechanisms.iter().any(|mechanism| {
                matches!(mechanism,
                    PatternMechanism::GuideDimensions { dimensions, .. }
                        if dimensions.iter().any(|dimension| matches!(
                            dimension.prototype,
                            GuidePrototype::AuthoredOpenPath { structure_id } if structure_id == id
                        ))
                )
            }) || definition.output_layers.iter().any(|layer| {
                matches!(
                    layer,
                    PatternOutputLayer::MarkPrototype {
                        prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                        ..
                    } if *structure_id == id
                )
            })
        })
    }

    /// Returns value-free descriptors for every authored structure without exposing values or UI layout.
    pub fn authored_structure_descriptors(&self) -> Vec<AuthoredStructureFieldDescriptor> {
        self.authored_structures
            .iter()
            .flat_map(|structure| {
                authored_structure_field_contracts()
                    .iter()
                    .map(move |contract| AuthoredStructureFieldDescriptor {
                        target: structure.id,
                        field: contract.field,
                        value_kind: contract.value_kind,
                        allowed_structure_kinds: contract.allowed_structure_kinds,
                        allowed_segment_kinds: contract.allowed_segment_kinds,
                        maximum_segments: contract.maximum_segments,
                        shared_edit: contract.shared_edit,
                        invalidation: match contract.field {
                            AuthoredStructureFieldId::Kind => InvalidationLevel::Family,
                            AuthoredStructureFieldId::Segments => {
                                authored_structure_invalidation(structure.kind)
                            }
                        },
                    })
            })
            .collect()
    }

    /// Returns the authoritative definition currently targeted by one channel.
    /// The returned value is immutable; mutations still require a validated
    /// command through `DocumentHistory`.
    pub fn pattern_definition_for(&self, channel_id: ChannelId) -> Option<&PatternDefinition> {
        self.pattern_definition_id_for(channel_id)
            .and_then(|definition_id| self.definition(definition_id))
    }

    /// Returns legacy single-channel state only. A modeled topology is not a
    /// legacy evaluation input and therefore is intentionally not projected.
    pub fn channels(&self) -> Option<&[ChannelState]> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => Some(channels),
            ChannelConfiguration::Topology { .. } => None,
        }
    }

    /// The installed Stage 9 model, if this document has received an atomic
    /// model/topology replacement. Existing pre-Stage-9 documents retain their
    /// accepted single-channel representation until explicitly replaced.
    pub fn channel_model(&self) -> Option<HalftoneChannelModel> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(_) => None,
            ChannelConfiguration::Topology { model, .. } => Some(*model),
        }
    }

    pub fn channel_topology(&self) -> Option<&ChannelTopology> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(_) => None,
            ChannelConfiguration::Topology { topology, .. } => Some(topology),
        }
    }

    /// Builds and validates a canonical modeled topology using definitions in
    /// this document. The factory never selects geometry settings.
    pub fn canonical_channel_topology(
        &self,
        model: HalftoneChannelModel,
        template: ChannelTopologyTemplate,
    ) -> Result<ChannelTopology, ValidationError> {
        let topology = ChannelTopology::canonical(model, template)?;
        validate_topology(model, &topology, &self.pattern_definition_bundles)?;
        Ok(topology)
    }

    /// Returns a legacy channel only; modeled channels are available through
    /// `channel_topology` and `modeled_channel`.
    pub fn channel(&self, channel_id: ChannelId) -> Option<&ChannelState> {
        self.channels()?
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    pub fn modeled_channel(&self, channel_id: ChannelId) -> Option<&ModeledChannelState> {
        self.channel_topology()?
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    /// Returns deterministic schema-derived descriptors from the exhaustive
    /// field contract. Only the three state-dependent capability predicates
    /// supply runtime context; no static metadata is repeated here.
    pub fn property_descriptors(&self) -> Vec<PropertyDescriptor> {
        let mut descriptors = vec![descriptor_from_contract(
            PropertyFieldId::SourceReference,
            PropertyTarget::Document,
        )];
        for field in [
            PropertyFieldId::DefinitionSelection,
            PropertyFieldId::DensityAcrossX,
            PropertyFieldId::DensityAcrossY,
            PropertyFieldId::DensityAspectLocked,
            PropertyFieldId::RotationDegrees,
            PropertyFieldId::ShapeRotationDegrees,
        ] {
            descriptors.push(descriptor_from_contract(field, PropertyTarget::Document));
        }
        for channel_id in self.channel_ids() {
            let target = PropertyTarget::Channel(channel_id);
            for field in [
                PropertyFieldId::DensityAcrossX,
                PropertyFieldId::DensityAcrossY,
                PropertyFieldId::RotationDegrees,
                PropertyFieldId::TranslationX,
                PropertyFieldId::TranslationY,
                PropertyFieldId::ShapeRotationDegrees,
                PropertyFieldId::Opacity,
                PropertyFieldId::Visibility,
                PropertyFieldId::DefinitionSelection,
            ] {
                descriptors.push(descriptor_from_contract(field, target));
            }
            if let Ok(effective) = self.effective_channel_pattern(channel_id)
                && let Some(definition) = self.definition(effective.definition_id)
            {
                for output in &definition.output_layers {
                    let fields: &[PropertyFieldId] = match output {
                        PatternOutputLayer::CircularMarks { .. }
                        | PatternOutputLayer::MarkPrototype { .. } => &[
                            PropertyFieldId::MarkMinimumFill,
                            PropertyFieldId::MarkMaximumFill,
                        ],
                        PatternOutputLayer::GuidePaths { .. }
                        | PatternOutputLayer::ParametricPaths { .. }
                        | PatternOutputLayer::ConnectionPaths { .. }
                        | PatternOutputLayer::MazeWalls { .. } => &[
                            PropertyFieldId::ConnectedMinimumThickness,
                            PropertyFieldId::ConnectedMaximumThickness,
                        ],
                        PatternOutputLayer::Regions { .. } => &[],
                    };
                    for field in fields {
                        descriptors.push(descriptor_from_contract(
                            *field,
                            PropertyTarget::ChannelOutput(channel_id, output.id()),
                        ));
                    }
                }
            }
            if let Some(model) = self.channel_model() {
                for field in [
                    PropertyFieldId::ModeledMappingComponent,
                    PropertyFieldId::ModeledMappingPlacement,
                    PropertyFieldId::ModeledMappingInverted,
                    PropertyFieldId::ModeledMappingGain,
                    PropertyFieldId::ModeledMappingBias,
                ] {
                    descriptors.push(descriptor_from_contract(field, target));
                }
                let context = if model == HalftoneChannelModel::SourceColorAlpha {
                    DescriptorRuntimeContext::Paint {
                        choices: SAMPLED_PAINT_CHOICES,
                        dependency: PropertyDependency::SampledPaint,
                    }
                } else {
                    DescriptorRuntimeContext::Paint {
                        choices: SOLID_PAINT_CHOICES,
                        dependency: PropertyDependency::SolidPaint,
                    }
                };
                descriptors.push(descriptor_with_runtime_context(
                    PropertyFieldId::Paint,
                    target,
                    context,
                ));
                if model != HalftoneChannelModel::SourceColorAlpha {
                    for field in [
                        PropertyFieldId::ColorRed,
                        PropertyFieldId::ColorGreen,
                        PropertyFieldId::ColorBlue,
                        PropertyFieldId::ColorAlpha,
                    ] {
                        descriptors.push(descriptor_from_contract(field, target));
                    }
                }
            } else {
                for field in [
                    PropertyFieldId::LegacyMappingComponent,
                    PropertyFieldId::LegacyMappingPlacement,
                    PropertyFieldId::ColorRed,
                    PropertyFieldId::ColorGreen,
                    PropertyFieldId::ColorBlue,
                    PropertyFieldId::ColorAlpha,
                ] {
                    descriptors.push(descriptor_from_contract(field, target));
                }
            }
        }
        for bundle in &self.pattern_definition_bundles {
            let definition = &bundle.definition;
            let definition_target = PropertyTarget::Definition(definition.id);
            for field in [
                PropertyFieldId::CoverageGuardSteps,
                PropertyFieldId::CoverageAdditionalMargin,
            ] {
                descriptors.push(descriptor_from_contract(field, definition_target));
            }
            for mechanism in &definition.mechanisms {
                let target = PropertyTarget::Mechanism(definition.id, mechanism.id());
                match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
                        for dimension in dimensions {
                            let target = PropertyTarget::GuideDimension(
                                definition.id,
                                mechanism.id(),
                                dimension.id,
                            );
                            for field in [
                                PropertyFieldId::GuideBaselineAngle,
                                PropertyFieldId::GuidePhase,
                                PropertyFieldId::GuideSpacingMultiplier,
                            ] {
                                descriptors.push(descriptor_from_contract(field, target));
                            }
                        }
                    }
                    PatternMechanism::GuideDimensions { dimensions, .. } => {
                        for dimension in dimensions {
                            let target = PropertyTarget::GuideDimension(
                                definition.id,
                                mechanism.id(),
                                dimension.id,
                            );
                            let mut fields = vec![
                                PropertyFieldId::GuidePrototype,
                                PropertyFieldId::GuideBaselineAngle,
                                PropertyFieldId::GuidePhase,
                                PropertyFieldId::GuideRepetition,
                            ];
                            match dimension.prototype {
                                GuidePrototype::AuthoredOpenPath { .. } => {
                                    fields.push(PropertyFieldId::GuideAuthoredStructure);
                                }
                                GuidePrototype::CircularArc { .. } => fields.extend([
                                    PropertyFieldId::GuideArcCenterX,
                                    PropertyFieldId::GuideArcCenterY,
                                    PropertyFieldId::GuideArcRadius,
                                    PropertyFieldId::GuideArcStartAngle,
                                    PropertyFieldId::GuideArcSweepAngle,
                                ]),
                            }
                            if matches!(
                                dimension.repetition,
                                GuideRepetition::TransformStack { .. }
                            ) {
                                fields.extend([
                                    PropertyFieldId::GuideStackDirection,
                                    PropertyFieldId::GuideStackSpacingMultiplier,
                                ]);
                            }
                            if matches!(dimension.repetition, GuideRepetition::NormalOffset { .. })
                            {
                                fields.extend([
                                    PropertyFieldId::GuideOffsetSpacing,
                                    PropertyFieldId::GuideOffsetSides,
                                    PropertyFieldId::GuideOffsetCleanup,
                                ]);
                            }
                            for field in fields {
                                descriptors.push(descriptor_from_contract(field, target));
                            }
                        }
                    }
                    PatternMechanism::SelectedGuideIntersections { .. } => {
                        for field in [
                            PropertyFieldId::IntersectionDimensions,
                            PropertyFieldId::IntersectionMergeEpsilon,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::AlongGuideSites { .. } => {
                        for field in [
                            PropertyFieldId::AlongGuideDimensions,
                            PropertyFieldId::AlongGuideIntervalMultiplier,
                            PropertyFieldId::AlongGuidePhase,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::ParametricCurveSource { repetition, .. } => {
                        let mut fields = vec![
                            PropertyFieldId::ParametricShape,
                            PropertyFieldId::ParametricTurns,
                            PropertyFieldId::ParametricRadialSpacing,
                            PropertyFieldId::ParametricPhase,
                            PropertyFieldId::ParametricWinding,
                            PropertyFieldId::ParametricRepetition,
                        ];
                        match repetition {
                            CurveRepetition::Single => {}
                            CurveRepetition::TransformStack { .. } => fields.extend([
                                PropertyFieldId::ParametricStackDirection,
                                PropertyFieldId::ParametricStackSpacingMultiplier,
                            ]),
                            CurveRepetition::NormalOffset { .. } => fields.extend([
                                PropertyFieldId::ParametricOffsetSpacing,
                                PropertyFieldId::ParametricOffsetSides,
                                PropertyFieldId::ParametricOffsetCleanup,
                            ]),
                        }
                        for field in fields {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::AlongParametricCurveSites { .. } => {
                        for field in [
                            PropertyFieldId::AlongParametricInterval,
                            PropertyFieldId::AlongParametricPhase,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::RandomSiteProcess { character, .. } => {
                        for field in [
                            PropertyFieldId::RandomCharacter,
                            PropertyFieldId::RandomSeed,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                        match character {
                            RandomSiteCharacter::RawUniform => {}
                            RandomSiteCharacter::Even { .. } => {
                                descriptors.push(descriptor_from_contract(
                                    PropertyFieldId::RandomEvenMinimumCenterDistance,
                                    target,
                                ))
                            }
                            RandomSiteCharacter::Clustered { .. } => {
                                for field in [
                                    PropertyFieldId::RandomClusterDensity,
                                    PropertyFieldId::RandomClusterSpread,
                                    PropertyFieldId::RandomClusterStrength,
                                ] {
                                    descriptors.push(descriptor_from_contract(field, target));
                                }
                            }
                        }
                    }
                    PatternMechanism::SiteDensityModulation { modulation, .. } => {
                        descriptors.push(descriptor_with_runtime_context(
                            PropertyFieldId::RandomDensityModulation,
                            target,
                            DescriptorRuntimeContext::DensityModulation {
                                dependency: if matches!(
                                    modulation,
                                    SiteDensityModulation::ArtworkWeighted { .. }
                                ) {
                                    PropertyDependency::ArtworkWeightedDensity
                                } else {
                                    PropertyDependency::RandomProcess
                                },
                            },
                        ));
                        if matches!(modulation, SiteDensityModulation::ArtworkWeighted { .. }) {
                            for field in [
                                PropertyFieldId::ArtworkWeightMappingComponent,
                                PropertyFieldId::ArtworkWeightMappingPlacement,
                                PropertyFieldId::ArtworkWeightMappingInverted,
                                PropertyFieldId::ArtworkWeightMappingGain,
                                PropertyFieldId::ArtworkWeightMappingBias,
                                PropertyFieldId::ArtworkWeightStrength,
                                PropertyFieldId::ArtworkWeightResponse,
                            ] {
                                descriptors.push(descriptor_from_contract(field, target));
                            }
                        }
                    }
                    PatternMechanism::SiteExclusion { policy, .. } => {
                        let visible =
                            matches!(policy, SiteExclusionPolicy::VisibleMarkMargin { .. });
                        descriptors.push(descriptor_with_runtime_context(
                            PropertyFieldId::RandomExclusion,
                            target,
                            DescriptorRuntimeContext::Exclusion {
                                dependency: if visible { PropertyDependency::VisibleMarkExclusion } else { PropertyDependency::RandomProcess },
                                support: if visible { StructuralSupportConstraint::VisibleMarkMarginUsesMaximumRealizedSupport } else { StructuralSupportConstraint::None },
                            },
                        ));
                        match policy {
                            SiteExclusionPolicy::None => {}
                            SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                                descriptors.push(descriptor_from_contract(
                                    PropertyFieldId::ExclusionMinimumCenterDistance,
                                    target,
                                ))
                            }
                            SiteExclusionPolicy::VisibleMarkMargin { .. } => {
                                for field in [
                                    PropertyFieldId::VisibleMarkMargin,
                                    PropertyFieldId::VisibleMarkSizingPolicy,
                                ] {
                                    descriptors.push(descriptor_from_contract(field, target));
                                }
                            }
                        }
                    }
                    PatternMechanism::RandomSiteProduct { .. } => {
                        for field in [
                            PropertyFieldId::RandomMaximumAttempts,
                            PropertyFieldId::RandomMaximumNeighborChecks,
                        ] {
                            descriptors.push(descriptor_from_contract(field, target));
                        }
                    }
                    PatternMechanism::StraightGuides { .. }
                    | PatternMechanism::GuideIntersections { .. } => {}
                }
            }
            for layer in &definition.output_layers {
                let target = PropertyTarget::OutputLayer(definition.id, layer.id());
                if let PatternOutputLayer::ConnectionPaths { program, .. } = layer {
                    for field in [
                        PropertyFieldId::ConnectionProgram,
                        PropertyFieldId::ConnectionMaximumDegree,
                        PropertyFieldId::ConnectionMaximumDistance,
                    ] {
                        descriptors.push(descriptor_from_contract(field, target));
                    }
                    if matches!(program, ConnectionProgram::RandomLinks { .. }) {
                        descriptors.push(descriptor_from_contract(
                            PropertyFieldId::ConnectionMinimumDegree,
                            target,
                        ));
                    }
                    if program.seed().is_some() {
                        descriptors.push(descriptor_from_contract(
                            PropertyFieldId::ConnectionSeed,
                            target,
                        ));
                    }
                }
                if matches!(layer, PatternOutputLayer::MazeWalls { .. }) {
                    descriptors.push(descriptor_from_contract(PropertyFieldId::MazeSeed, target));
                }
                if matches!(
                    layer,
                    PatternOutputLayer::CircularMarks { .. }
                        | PatternOutputLayer::MarkPrototype { .. }
                ) {
                    descriptors.push(descriptor_from_contract(
                        PropertyFieldId::OutputSiteProduct,
                        target,
                    ));
                }
                if let PatternOutputLayer::MarkPrototype { orientation, .. } = layer {
                    for field in [
                        PropertyFieldId::OutputPrototype,
                        PropertyFieldId::OutputOrientation,
                    ] {
                        descriptors.push(descriptor_from_contract(field, target));
                    }
                    if matches!(
                        orientation,
                        MarkOrientation::GuideTangent { .. } | MarkOrientation::GuideNormal { .. }
                    ) {
                        descriptors.push(descriptor_from_contract(
                            PropertyFieldId::OutputOrientationDimension,
                            target,
                        ));
                    }
                    if matches!(
                        layer,
                        PatternOutputLayer::MarkPrototype {
                            prototype: MarkPrototype::AuthoredClosedShape { .. },
                            ..
                        }
                    ) {
                        descriptors.push(descriptor_from_contract(
                            PropertyFieldId::OutputAuthoredClosedShape,
                            target,
                        ));
                    }
                }
            }
        }
        descriptors
    }

    /// Returns the immutable current value for every active descriptor.  This
    /// deliberately mirrors `property_descriptors` rather than extending a
    /// descriptor with mutable document data: descriptor metadata and current
    /// authority remain two independently readable surfaces.
    pub fn property_values(&self) -> Vec<PropertyCurrentValue> {
        self.property_descriptors()
            .into_iter()
            .map(|descriptor| PropertyCurrentValue {
                value: self.property_value_for(&descriptor),
                authored_value: self.authored_property_value_for(&descriptor),
                inheritance: self.property_inheritance_for(&descriptor),
                descriptor,
            })
            .collect()
    }

    /// Begins a deliberate compound-variant transition from an active selector
    /// descriptor. The resulting draft has explicit domain-owned initial
    /// payload values and must be finalized before it becomes an existing
    /// `PatternDefinitionEdit`.
    pub fn variant_transition_draft(
        &self,
        selector: &PropertyDescriptor,
        choice: PropertyEnumChoice,
    ) -> Result<VariantTransitionDraft, ValidationError> {
        if !self.property_descriptors().contains(selector) {
            return Err(ValidationError::new(
                "transition_draft.selector",
                "transition selector is inactive or stale",
            ));
        }
        let base_choice = self
            .property_values()
            .into_iter()
            .find(|value| value.descriptor == *selector)
            .and_then(|value| match value.value {
                PropertyCurrentValueKind::EnumChoice(choice) => Some(choice),
                _ => None,
            })
            .ok_or_else(|| {
                ValidationError::new(
                    "transition_draft.selector",
                    "transition selector does not have an enum value",
                )
            })?;
        if !selector.choices.contains(&choice) {
            return Err(ValidationError::new(
                "transition_draft.choice",
                "transition choice is not supported by this selector",
            ));
        }
        let fields = transition_fields_for(self, selector, base_choice, choice)?;
        let definition_id = transition_definition_id(selector.target).ok_or_else(|| {
            ValidationError::new(
                "transition_draft.selector",
                "transition selector has no structural definition target",
            )
        })?;
        let base_definition = self.definition(definition_id).cloned().ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition definition is missing",
            )
        })?;
        Ok(VariantTransitionDraft {
            selector: selector.clone(),
            base_choice,
            choice,
            fields,
            base_definition,
        })
    }

    fn property_value_for(&self, descriptor: &PropertyDescriptor) -> PropertyCurrentValueKind {
        let channel = |id| {
            self.channel(id)
                .map(ChannelPropertyState::Legacy)
                .or_else(|| self.modeled_channel(id).map(ChannelPropertyState::Modeled))
        };
        match descriptor.target {
            PropertyTarget::Document => match descriptor.field {
                PropertyFieldId::SourceReference => PropertyCurrentValueKind::Reference(
                    PropertyReferenceValue::Source(self.source.clone()),
                ),
                PropertyFieldId::DefinitionSelection => PropertyCurrentValueKind::Reference(
                    PropertyReferenceValue::Definition(self.pattern_settings.definition_id),
                ),
                PropertyFieldId::DensityAcrossX => {
                    PropertyCurrentValueKind::FiniteF64(self.pattern_settings.density.across_x)
                }
                PropertyFieldId::DensityAcrossY => {
                    PropertyCurrentValueKind::FiniteF64(self.pattern_settings.density.across_y)
                }
                PropertyFieldId::DensityAspectLocked => {
                    PropertyCurrentValueKind::Boolean(self.pattern_settings.density.aspect_locked)
                }
                PropertyFieldId::RotationDegrees => PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.pattern_rotation_degrees,
                ),
                PropertyFieldId::ShapeRotationDegrees => PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.shape_rotation_degrees,
                ),
                PropertyFieldId::MarkMinimumFill
                | PropertyFieldId::MarkMaximumFill
                | PropertyFieldId::ConnectedMinimumThickness
                | PropertyFieldId::ConnectedMaximumThickness => {
                    unreachable!("document-wide response descriptors are not active")
                }
                _ => unreachable!("document descriptor is not a document-base field"),
            },
            PropertyTarget::Channel(channel_id) => {
                let channel = channel(channel_id).expect("active descriptor targets channel");
                let effective = self
                    .effective_channel_pattern(channel_id)
                    .expect("active descriptor resolves an effective pattern");
                match descriptor.field {
                    PropertyFieldId::DensityAcrossX => {
                        PropertyCurrentValueKind::FiniteF64(effective.density.across_x)
                    }
                    PropertyFieldId::DensityAcrossY => {
                        PropertyCurrentValueKind::FiniteF64(effective.density.across_y)
                    }
                    PropertyFieldId::DensityAspectLocked => {
                        PropertyCurrentValueKind::Boolean(effective.density.aspect_locked)
                    }
                    PropertyFieldId::RotationDegrees => {
                        PropertyCurrentValueKind::FiniteF64(effective.pattern_rotation_degrees)
                    }
                    PropertyFieldId::TranslationX => {
                        PropertyCurrentValueKind::FiniteF64(effective.translation_x)
                    }
                    PropertyFieldId::TranslationY => {
                        PropertyCurrentValueKind::FiniteF64(effective.translation_y)
                    }
                    PropertyFieldId::MarkMinimumFill => match &effective
                        .output_settings
                        .first()
                        .expect("active output setting")
                        .response
                    {
                        PatternGeometryResponse::Marks(response) => {
                            PropertyCurrentValueKind::FiniteF64(response.minimum_fill)
                        }
                        PatternGeometryResponse::Connected(_)
                        | PatternGeometryResponse::Regions(_) => {
                            unreachable!("inactive mark descriptor")
                        }
                    },
                    PropertyFieldId::MarkMaximumFill => match &effective
                        .output_settings
                        .first()
                        .expect("active output setting")
                        .response
                    {
                        PatternGeometryResponse::Marks(response) => {
                            PropertyCurrentValueKind::FiniteF64(response.maximum_fill)
                        }
                        PatternGeometryResponse::Connected(_)
                        | PatternGeometryResponse::Regions(_) => {
                            unreachable!("inactive mark descriptor")
                        }
                    },
                    PropertyFieldId::ConnectedMinimumThickness => match &effective
                        .output_settings
                        .first()
                        .expect("active output setting")
                        .response
                    {
                        PatternGeometryResponse::Connected(response) => {
                            PropertyCurrentValueKind::FiniteF64(response.minimum_thickness)
                        }
                        PatternGeometryResponse::Marks(_) | PatternGeometryResponse::Regions(_) => {
                            unreachable!("inactive connected descriptor")
                        }
                    },
                    PropertyFieldId::ConnectedMaximumThickness => match &effective
                        .output_settings
                        .first()
                        .expect("active output setting")
                        .response
                    {
                        PatternGeometryResponse::Connected(response) => {
                            PropertyCurrentValueKind::FiniteF64(response.maximum_thickness)
                        }
                        PatternGeometryResponse::Marks(_) | PatternGeometryResponse::Regions(_) => {
                            unreachable!("inactive connected descriptor")
                        }
                    },
                    PropertyFieldId::ShapeRotationDegrees => {
                        PropertyCurrentValueKind::FiniteF64(effective.shape_rotation_degrees)
                    }
                    PropertyFieldId::Opacity => {
                        PropertyCurrentValueKind::FiniteF64(channel.opacity())
                    }
                    PropertyFieldId::Visibility => {
                        PropertyCurrentValueKind::Boolean(channel.visible())
                    }
                    PropertyFieldId::DefinitionSelection => PropertyCurrentValueKind::Reference(
                        PropertyReferenceValue::Definition(effective.definition_id),
                    ),
                    PropertyFieldId::ColorRed => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().red)
                    }
                    PropertyFieldId::ColorGreen => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().green)
                    }
                    PropertyFieldId::ColorBlue => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().blue)
                    }
                    PropertyFieldId::ColorAlpha => {
                        PropertyCurrentValueKind::FiniteF64(channel.color().alpha)
                    }
                    PropertyFieldId::LegacyMappingComponent => {
                        PropertyCurrentValueKind::EnumChoice(
                            PropertyEnumChoice::SourceMappingComponent(
                                match channel
                                    .legacy_mapping()
                                    .expect("legacy descriptor")
                                    .component
                                {
                                    SourceComponent::Luminance => SourceMappingComponent::Luminance,
                                    SourceComponent::Alpha => SourceMappingComponent::Alpha,
                                },
                            ),
                        )
                    }
                    PropertyFieldId::LegacyMappingPlacement => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
                            channel
                                .legacy_mapping()
                                .expect("legacy descriptor")
                                .placement,
                        ))
                    }
                    PropertyFieldId::ModeledMappingComponent => {
                        PropertyCurrentValueKind::EnumChoice(
                            PropertyEnumChoice::SourceMappingComponent(
                                channel.mapping().expect("modeled descriptor").component,
                            ),
                        )
                    }
                    PropertyFieldId::ModeledMappingPlacement => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
                            channel.mapping().expect("modeled descriptor").placement,
                        ))
                    }
                    PropertyFieldId::ModeledMappingInverted => PropertyCurrentValueKind::Boolean(
                        channel.mapping().expect("modeled descriptor").inverted,
                    ),
                    PropertyFieldId::ModeledMappingGain => PropertyCurrentValueKind::FiniteF64(
                        channel.mapping().expect("modeled descriptor").gain,
                    ),
                    PropertyFieldId::ModeledMappingBias => PropertyCurrentValueKind::FiniteF64(
                        channel.mapping().expect("modeled descriptor").bias,
                    ),
                    PropertyFieldId::Paint => {
                        PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::Paint(
                            match channel.paint().expect("modeled descriptor") {
                                ChannelPaint::Solid(_) => PaintKind::Solid,
                                ChannelPaint::SampledSource => PaintKind::SampledSource,
                            },
                        ))
                    }
                    _ => unreachable!("channel descriptor field"),
                }
            }
            PropertyTarget::ChannelOutput(channel_id, output_layer_id) => {
                let effective = self
                    .effective_channel_pattern(channel_id)
                    .expect("active output descriptor resolves an effective pattern");
                let definition = self
                    .definition(effective.definition_id)
                    .expect("active output descriptor resolves a definition");
                let output = definition
                    .output_layers
                    .iter()
                    .find(|output| output.id() == output_layer_id)
                    .expect("active output descriptor targets a structural output");
                let response = effective
                    .output_settings
                    .iter()
                    .find(|setting| setting.output_layer_id == output_layer_id)
                    .map(|setting| &setting.response)
                    .expect("active output descriptor resolves an output setting");
                match (descriptor.field, output, response) {
                    (
                        PropertyFieldId::MarkMinimumFill,
                        PatternOutputLayer::CircularMarks { .. }
                        | PatternOutputLayer::MarkPrototype { .. },
                        PatternGeometryResponse::Marks(response),
                    ) => PropertyCurrentValueKind::FiniteF64(response.minimum_fill),
                    (
                        PropertyFieldId::MarkMaximumFill,
                        PatternOutputLayer::CircularMarks { .. }
                        | PatternOutputLayer::MarkPrototype { .. },
                        PatternGeometryResponse::Marks(response),
                    ) => PropertyCurrentValueKind::FiniteF64(response.maximum_fill),
                    (
                        PropertyFieldId::ConnectedMinimumThickness,
                        PatternOutputLayer::GuidePaths { .. }
                        | PatternOutputLayer::ParametricPaths { .. }
                        | PatternOutputLayer::ConnectionPaths { .. }
                        | PatternOutputLayer::MazeWalls { .. },
                        PatternGeometryResponse::Connected(response),
                    ) => PropertyCurrentValueKind::FiniteF64(response.minimum_thickness),
                    (
                        PropertyFieldId::ConnectedMaximumThickness,
                        PatternOutputLayer::GuidePaths { .. }
                        | PatternOutputLayer::ParametricPaths { .. }
                        | PatternOutputLayer::ConnectionPaths { .. }
                        | PatternOutputLayer::MazeWalls { .. },
                        PatternGeometryResponse::Connected(response),
                    ) => PropertyCurrentValueKind::FiniteF64(response.maximum_thickness),
                    _ => {
                        unreachable!("output response descriptor must match structural output kind")
                    }
                }
            }
            PropertyTarget::Definition(definition_id) => {
                let definition = self
                    .pattern_definition_bundles
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .expect("active definition descriptor");
                match descriptor.field {
                    PropertyFieldId::CoverageGuardSteps => {
                        PropertyCurrentValueKind::U32(definition.coverage.guard_steps)
                    }
                    PropertyFieldId::CoverageAdditionalMargin => {
                        PropertyCurrentValueKind::FiniteF64(definition.coverage.additional_margin)
                    }
                    _ => unreachable!("definition descriptor field"),
                }
            }
            PropertyTarget::GuideDimension(definition_id, mechanism_id, dimension_id) => {
                let mechanism = self
                    .pattern_definition_bundles
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .mechanisms
                            .iter()
                            .find(|mechanism| mechanism.id() == mechanism_id)
                    })
                    .expect("active guide dimension mechanism");
                match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
                        let dimension = dimensions
                            .iter()
                            .find(|dimension| dimension.id == dimension_id)
                            .expect("active straight guide dimension descriptor");
                        match descriptor.field {
                            PropertyFieldId::GuideBaselineAngle => {
                                PropertyCurrentValueKind::FiniteF64(
                                    dimension.baseline_angle_degrees,
                                )
                            }
                            PropertyFieldId::GuidePhase => {
                                PropertyCurrentValueKind::FiniteF64(dimension.phase)
                            }
                            PropertyFieldId::GuideSpacingMultiplier => {
                                PropertyCurrentValueKind::FiniteF64(
                                    dimension.repetition.spacing_multiplier,
                                )
                            }
                            _ => unreachable!("straight guide descriptor field"),
                        }
                    }
                    PatternMechanism::GuideDimensions { dimensions, .. } => {
                        let dimension = dimensions
                            .iter()
                            .find(|dimension| dimension.id == dimension_id)
                            .expect("active generic guide dimension descriptor");
                        match (
                            descriptor.field,
                            &dimension.prototype,
                            &dimension.repetition,
                        ) {
                            (
                                PropertyFieldId::GuidePrototype,
                                GuidePrototype::AuthoredOpenPath { .. },
                                _,
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::GuidePrototype(
                                    GuidePrototypeKind::AuthoredOpenPath,
                                ),
                            ),
                            (
                                PropertyFieldId::GuidePrototype,
                                GuidePrototype::CircularArc { .. },
                                _,
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::GuidePrototype(GuidePrototypeKind::CircularArc),
                            ),
                            (
                                PropertyFieldId::GuideAuthoredStructure,
                                GuidePrototype::AuthoredOpenPath { structure_id },
                                _,
                            ) => PropertyCurrentValueKind::Reference(
                                PropertyReferenceValue::AuthoredStructure(*structure_id),
                            ),
                            (
                                PropertyFieldId::GuideArcCenterX,
                                GuidePrototype::CircularArc { center, .. },
                                _,
                            ) => PropertyCurrentValueKind::FiniteF64(center.x),
                            (
                                PropertyFieldId::GuideArcCenterY,
                                GuidePrototype::CircularArc { center, .. },
                                _,
                            ) => PropertyCurrentValueKind::FiniteF64(center.y),
                            (
                                PropertyFieldId::GuideArcRadius,
                                GuidePrototype::CircularArc { radius, .. },
                                _,
                            ) => PropertyCurrentValueKind::FiniteF64(*radius),
                            (
                                PropertyFieldId::GuideArcStartAngle,
                                GuidePrototype::CircularArc {
                                    start_angle_degrees,
                                    ..
                                },
                                _,
                            ) => PropertyCurrentValueKind::FiniteF64(*start_angle_degrees),
                            (
                                PropertyFieldId::GuideArcSweepAngle,
                                GuidePrototype::CircularArc {
                                    sweep_angle_degrees,
                                    ..
                                },
                                _,
                            ) => PropertyCurrentValueKind::FiniteF64(*sweep_angle_degrees),
                            (PropertyFieldId::GuideBaselineAngle, _, _) => {
                                PropertyCurrentValueKind::FiniteF64(
                                    dimension.baseline_angle_degrees,
                                )
                            }
                            (PropertyFieldId::GuidePhase, _, _) => {
                                PropertyCurrentValueKind::FiniteF64(dimension.phase)
                            }
                            (PropertyFieldId::GuideRepetition, _, GuideRepetition::Single) => {
                                PropertyCurrentValueKind::EnumChoice(
                                    PropertyEnumChoice::GuideRepetition(
                                        GuideRepetitionKind::Single,
                                    ),
                                )
                            }
                            (
                                PropertyFieldId::GuideRepetition,
                                _,
                                GuideRepetition::TransformStack { .. },
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::GuideRepetition(
                                    GuideRepetitionKind::TransformStack,
                                ),
                            ),
                            (
                                PropertyFieldId::GuideRepetition,
                                _,
                                GuideRepetition::NormalOffset { .. },
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::GuideRepetition(
                                    GuideRepetitionKind::NormalOffset,
                                ),
                            ),
                            (
                                PropertyFieldId::GuideStackDirection,
                                _,
                                GuideRepetition::TransformStack {
                                    direction_degrees, ..
                                },
                            ) => PropertyCurrentValueKind::FiniteF64(*direction_degrees),
                            (
                                PropertyFieldId::GuideStackSpacingMultiplier,
                                _,
                                GuideRepetition::TransformStack {
                                    spacing_multiplier, ..
                                },
                            ) => PropertyCurrentValueKind::FiniteF64(*spacing_multiplier),
                            (
                                PropertyFieldId::GuideOffsetSpacing,
                                _,
                                GuideRepetition::NormalOffset { spacing, .. },
                            ) => PropertyCurrentValueKind::FiniteF64(*spacing),
                            (
                                PropertyFieldId::GuideOffsetSides,
                                _,
                                GuideRepetition::NormalOffset { sides, .. },
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::OffsetSides(*sides),
                            ),
                            (
                                PropertyFieldId::GuideOffsetCleanup,
                                _,
                                GuideRepetition::NormalOffset { cleanup, .. },
                            ) => PropertyCurrentValueKind::EnumChoice(
                                PropertyEnumChoice::OffsetCleanup(*cleanup),
                            ),
                            _ => unreachable!("inactive generic guide descriptor field"),
                        }
                    }
                    _ => unreachable!("guide dimension descriptor targets a guide root"),
                }
            }
            PropertyTarget::Mechanism(definition_id, mechanism_id) => {
                let mechanism = self
                    .pattern_definition_bundles
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .mechanisms
                            .iter()
                            .find(|mechanism| mechanism.id() == mechanism_id)
                    })
                    .expect("active mechanism descriptor");
                property_value_for_mechanism(descriptor.field, mechanism)
            }
            PropertyTarget::OutputLayer(definition_id, output_layer_id) => {
                let layer = self
                    .pattern_definition_bundles
                    .iter()
                    .find(|definition| definition.id == definition_id)
                    .and_then(|definition| {
                        definition
                            .output_layers
                            .iter()
                            .find(|layer| layer.id() == output_layer_id)
                    })
                    .expect("active output descriptor");
                match (descriptor.field, layer) {
                    (
                        PropertyFieldId::ConnectionProgram,
                        PatternOutputLayer::ConnectionPaths { program, .. },
                    ) => PropertyCurrentValueKind::EnumChoice(
                        PropertyEnumChoice::ConnectionProgram(program.kind()),
                    ),
                    (
                        PropertyFieldId::ConnectionMaximumDegree,
                        PatternOutputLayer::ConnectionPaths { program, .. },
                    ) => PropertyCurrentValueKind::U32(program.adjacency().maximum_degree),
                    (
                        PropertyFieldId::ConnectionMaximumDistance,
                        PatternOutputLayer::ConnectionPaths { program, .. },
                    ) => PropertyCurrentValueKind::FiniteF64(program.adjacency().maximum_distance),
                    (
                        PropertyFieldId::ConnectionMinimumDegree,
                        PatternOutputLayer::ConnectionPaths {
                            program: ConnectionProgram::RandomLinks { minimum_degree, .. },
                            ..
                        },
                    ) => PropertyCurrentValueKind::U32(*minimum_degree),
                    (
                        PropertyFieldId::ConnectionSeed,
                        PatternOutputLayer::ConnectionPaths { program, .. },
                    ) => PropertyCurrentValueKind::U32(
                        program.seed().expect("active connection seed descriptor"),
                    ),
                    (PropertyFieldId::MazeSeed, PatternOutputLayer::MazeWalls { program, .. }) => {
                        PropertyCurrentValueKind::U32(program.seed)
                    }
                    (
                        PropertyFieldId::OutputSiteProduct,
                        PatternOutputLayer::CircularMarks {
                            site_mechanism_id, ..
                        }
                        | PatternOutputLayer::MarkPrototype {
                            site_mechanism_id, ..
                        },
                    ) => PropertyCurrentValueKind::Reference(PropertyReferenceValue::Mechanism(
                        *site_mechanism_id,
                    )),
                    (
                        PropertyFieldId::OutputPrototype,
                        PatternOutputLayer::MarkPrototype { prototype, .. },
                    ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::MarkPrototype(
                        match prototype {
                            MarkPrototype::Circle => MarkPrototypeKind::Circle,
                            MarkPrototype::AuthoredClosedShape { .. } => {
                                MarkPrototypeKind::AuthoredClosedShape
                            }
                        },
                    )),
                    (
                        PropertyFieldId::OutputAuthoredClosedShape,
                        PatternOutputLayer::MarkPrototype {
                            prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                            ..
                        },
                    ) => PropertyCurrentValueKind::Reference(
                        PropertyReferenceValue::AuthoredStructure(*structure_id),
                    ),
                    (
                        PropertyFieldId::OutputOrientation,
                        PatternOutputLayer::MarkPrototype { orientation, .. },
                    ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::MarkOrientation(
                        match orientation {
                            MarkOrientation::Fixed => MarkOrientationKind::Fixed,
                            MarkOrientation::GuideTangent { .. } => {
                                MarkOrientationKind::GuideTangent
                            }
                            MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
                        },
                    )),
                    (
                        PropertyFieldId::OutputOrientationDimension,
                        PatternOutputLayer::MarkPrototype {
                            orientation:
                                MarkOrientation::GuideTangent { dimension_id }
                                | MarkOrientation::GuideNormal { dimension_id },
                            ..
                        },
                    ) => PropertyCurrentValueKind::Reference(
                        PropertyReferenceValue::GuideDimension(*dimension_id),
                    ),
                    _ => unreachable!("output descriptor field"),
                }
            }
        }
    }

    /// Reports whether a channel-facing effective value comes from the
    /// document base or an explicitly stored delta/override. It is metadata
    /// only and never changes the resolver's authority.
    fn property_inheritance_for(&self, descriptor: &PropertyDescriptor) -> PropertyInheritance {
        let channel_id = match descriptor.target {
            PropertyTarget::Channel(channel_id) | PropertyTarget::ChannelOutput(channel_id, _) => {
                channel_id
            }
            _ => return PropertyInheritance::NotApplicable,
        };
        let Some(instance) = self.channel_pattern_instance(channel_id) else {
            return PropertyInheritance::NotApplicable;
        };
        let output_delta = match descriptor.target {
            PropertyTarget::ChannelOutput(_, output_layer_id) => instance
                .output_response_deltas
                .iter()
                .find(|entry| entry.output_layer_id == output_layer_id)
                .map(|entry| &entry.delta),
            _ => None,
        };
        match descriptor.field {
            PropertyFieldId::DensityAcrossX | PropertyFieldId::DensityAcrossY => {
                if instance.layout_delta.density.is_some() {
                    PropertyInheritance::Explicit
                } else {
                    PropertyInheritance::Inherited
                }
            }
            PropertyFieldId::RotationDegrees => {
                if instance.layout_delta.rotation_degrees.is_some() {
                    PropertyInheritance::Explicit
                } else {
                    PropertyInheritance::Inherited
                }
            }
            PropertyFieldId::ShapeRotationDegrees => {
                if instance.shape_rotation_delta_degrees.is_some() {
                    PropertyInheritance::Explicit
                } else {
                    PropertyInheritance::Inherited
                }
            }
            PropertyFieldId::MarkMinimumFill => match output_delta {
                Some(ChannelGeometryResponseDelta::Marks(delta))
                    if delta.minimum_fill_delta.is_some() =>
                {
                    PropertyInheritance::Explicit
                }
                _ => PropertyInheritance::Inherited,
            },
            PropertyFieldId::MarkMaximumFill => match output_delta {
                Some(ChannelGeometryResponseDelta::Marks(delta))
                    if delta.maximum_fill_delta.is_some() =>
                {
                    PropertyInheritance::Explicit
                }
                _ => PropertyInheritance::Inherited,
            },
            PropertyFieldId::ConnectedMinimumThickness => match output_delta {
                Some(ChannelGeometryResponseDelta::Connected(delta))
                    if delta.minimum_thickness_delta.is_some() =>
                {
                    PropertyInheritance::Explicit
                }
                _ => PropertyInheritance::Inherited,
            },
            PropertyFieldId::ConnectedMaximumThickness => match output_delta {
                Some(ChannelGeometryResponseDelta::Connected(delta))
                    if delta.maximum_thickness_delta.is_some() =>
                {
                    PropertyInheritance::Explicit
                }
                _ => PropertyInheritance::Inherited,
            },
            PropertyFieldId::DefinitionSelection => {
                if instance.definition_override.is_some() {
                    PropertyInheritance::Explicit
                } else {
                    PropertyInheritance::Inherited
                }
            }
            _ => PropertyInheritance::NotApplicable,
        }
    }

    /// Projects optional raw Stage 20G intent without replacing the effective
    /// display value. `None` represents inherited absence and is therefore
    /// suitable for reset affordances without frontend subtraction.
    fn authored_property_value_for(
        &self,
        descriptor: &PropertyDescriptor,
    ) -> Option<PropertyCurrentValueKind> {
        match descriptor.target {
            PropertyTarget::Document => match descriptor.field {
                PropertyFieldId::DefinitionSelection => Some(PropertyCurrentValueKind::Reference(
                    PropertyReferenceValue::Definition(self.pattern_settings.definition_id),
                )),
                PropertyFieldId::DensityAcrossX => Some(PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.density.across_x,
                )),
                PropertyFieldId::DensityAcrossY => Some(PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.density.across_y,
                )),
                PropertyFieldId::DensityAspectLocked => Some(PropertyCurrentValueKind::Boolean(
                    self.pattern_settings.density.aspect_locked,
                )),
                PropertyFieldId::RotationDegrees => Some(PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.pattern_rotation_degrees,
                )),
                PropertyFieldId::ShapeRotationDegrees => Some(PropertyCurrentValueKind::FiniteF64(
                    self.pattern_settings.shape_rotation_degrees,
                )),
                PropertyFieldId::MarkMinimumFill
                | PropertyFieldId::MarkMaximumFill
                | PropertyFieldId::ConnectedMinimumThickness
                | PropertyFieldId::ConnectedMaximumThickness => None,
                _ => None,
            },
            PropertyTarget::Channel(channel_id) => {
                let instance = self.channel_pattern_instance(channel_id)?;
                match descriptor.field {
                    PropertyFieldId::DefinitionSelection => {
                        instance.definition_override.map(|value| {
                            PropertyCurrentValueKind::Reference(PropertyReferenceValue::Definition(
                                value,
                            ))
                        })
                    }
                    PropertyFieldId::DensityAcrossX => instance
                        .layout_delta
                        .density
                        .as_ref()
                        .map(|value| PropertyCurrentValueKind::FiniteF64(value.across_x_delta)),
                    PropertyFieldId::DensityAcrossY => instance
                        .layout_delta
                        .density
                        .as_ref()
                        .map(|value| PropertyCurrentValueKind::FiniteF64(value.across_y_delta)),
                    PropertyFieldId::RotationDegrees => instance
                        .layout_delta
                        .rotation_degrees
                        .map(PropertyCurrentValueKind::FiniteF64),
                    PropertyFieldId::ShapeRotationDegrees => instance
                        .shape_rotation_delta_degrees
                        .map(PropertyCurrentValueKind::FiniteF64),
                    PropertyFieldId::MarkMinimumFill | PropertyFieldId::MarkMaximumFill => None,
                    PropertyFieldId::ConnectedMinimumThickness => {
                        match None::<&ChannelGeometryResponseDelta> {
                            Some(ChannelGeometryResponseDelta::Connected(delta)) => delta
                                .minimum_thickness_delta
                                .map(PropertyCurrentValueKind::FiniteF64),
                            _ => None,
                        }
                    }
                    PropertyFieldId::ConnectedMaximumThickness => {
                        match None::<&ChannelGeometryResponseDelta> {
                            Some(ChannelGeometryResponseDelta::Connected(delta)) => delta
                                .maximum_thickness_delta
                                .map(PropertyCurrentValueKind::FiniteF64),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Mechanical bidirectional completeness gate for the active schema.
    pub fn validate_property_descriptors(&self) -> Result<(), ValidationError> {
        let descriptors = self.property_descriptors();
        let mut seen = HashSet::new();
        for descriptor in &descriptors {
            if !seen.insert((descriptor.field, descriptor.target)) {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "duplicate descriptor field target",
                ));
            }
            let contract = property_field_contract(descriptor.field);
            if descriptor.command_kind() != contract.command_kind
                || descriptor.value_kind != contract.value_kind
                || descriptor.bounds != contract.bounds
                || descriptor.unit != contract.unit
                || descriptor.invalidation != contract.invalidation
                || descriptor.copy_on_edit_escalates_to_family
                    != contract.copy_on_edit_escalates_to_family
                || descriptor.reference_constraint != contract.reference_constraint
                || descriptor.choice_policy != contract.choice_policy
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor diverges from its field contract",
                ));
            }
            if contract.choice_policy == PropertyChoicePolicy::Static
                && descriptor.choices != contract.choices
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor choices diverge from its field contract",
                ));
            }
            if !matches!(
                contract.applicability,
                PropertyApplicability::CurrentExclusion
            ) && descriptor.structural_support != contract.structural_support
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor structural support diverges from its field contract",
                ));
            }
            if dependency_for_contract(contract.applicability, descriptor.dependency)
                != descriptor.dependency
            {
                return Err(ValidationError::new(
                    "capabilities.descriptors",
                    "descriptor dependency diverges from its field contract",
                ));
            }
        }
        if descriptors != self.property_descriptors() {
            return Err(ValidationError::new(
                "capabilities.descriptors",
                "descriptor order is nondeterministic",
            ));
        }
        Ok(())
    }

    fn channel_ids(&self) -> Vec<ChannelId> {
        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels.iter().map(|channel| channel.id).collect()
            }
            ChannelConfiguration::Topology { topology, .. } => {
                topology.channels.iter().map(|channel| channel.id).collect()
            }
        }
    }

    fn has_channel(&self, channel_id: ChannelId) -> bool {
        self.channel_ids().contains(&channel_id)
    }

    fn pattern_definition_id_for(&self, channel_id: ChannelId) -> Option<PatternDefinitionId> {
        self.effective_channel_pattern(channel_id)
            .ok()
            .map(|effective| effective.definition_id)
    }

    fn bundle(&self, id: PatternDefinitionId) -> Option<&PatternDefinitionBundle> {
        self.pattern_definition_bundles
            .iter()
            .find(|bundle| bundle.definition.id == id)
    }

    fn definition(&self, id: PatternDefinitionId) -> Option<&PatternDefinition> {
        self.bundle(id).map(|bundle| &bundle.definition)
    }

    /// Returns the channels targeting one definition in authoritative document
    /// order. Read-only callers use this to disclose a shared edit before its
    /// history-backed command is confirmed.
    pub fn linked_channels(&self, definition_id: PatternDefinitionId) -> Vec<ChannelId> {
        self.channel_ids()
            .into_iter()
            .filter(|channel_id| self.pattern_definition_id_for(*channel_id) == Some(definition_id))
            .collect()
    }

    fn allocate_definition_id(&self) -> Result<PatternDefinitionId, ValidationError> {
        next_id(
            self.pattern_definition_bundles
                .iter()
                .map(|bundle| bundle.definition.id.0),
            "pattern_definitions.id",
        )
        .map(PatternDefinitionId)
    }

    fn allocate_mechanism_id(&self) -> Result<PatternMechanismId, ValidationError> {
        next_id(
            self.pattern_definition_bundles.iter().flat_map(|bundle| {
                bundle
                    .definition
                    .mechanisms
                    .iter()
                    .map(|mechanism| mechanism.id().0)
            }),
            "pattern_definitions.mechanisms.id",
        )
        .map(PatternMechanismId)
    }

    fn allocate_output_layer_id(&self) -> Result<PatternOutputLayerId, ValidationError> {
        next_id(
            self.pattern_definition_bundles.iter().flat_map(|bundle| {
                bundle
                    .definition
                    .output_layers
                    .iter()
                    .map(|layer| layer.id().0)
            }),
            "pattern_definitions.output_layers.id",
        )
        .map(PatternOutputLayerId)
    }

    fn allocate_dimension_id(&self) -> Result<GuideDimensionId, ValidationError> {
        next_id(
            self.pattern_definition_bundles
                .iter()
                .flat_map(|bundle| bundle.definition.mechanisms.iter())
                .flat_map(|mechanism| match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                        .iter()
                        .map(|dimension| dimension.id.0)
                        .collect::<Vec<_>>(),
                    PatternMechanism::GuideDimensions { dimensions, .. } => dimensions
                        .iter()
                        .map(|dimension| dimension.id.0)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                }),
            "pattern_definitions.mechanisms.dimensions.id",
        )
        .map(GuideDimensionId)
    }

    /// Materializes the one supported selected-channel transition from the ordinary
    /// straight-grid topology to an authored open path with along-guide sites.
    ///
    /// The caller supplies an already-added open-path resource. The returned definition has
    /// fresh document-wide IDs, preserves the ordinary grid's coverage and mark-output
    /// semantics, and does not mutate this document. Only the grouped history transition
    /// may publish it, so linked channels retain the original ordinary definition.
    ///
    /// # Errors
    ///
    /// Returns a stable topology, output, allocation, or validation diagnostic when the
    /// selected definition is not the ordinary single-output straight grid or an ID space is
    /// exhausted. No partial definition is returned.
    fn custom_authored_along_guide_definition(
        &self,
        base: &PatternDefinition,
        structure_id: AuthoredStructureId,
    ) -> Result<PatternDefinition, ValidationError> {
        let [
            PatternMechanism::StraightGuides { .. },
            PatternMechanism::GuideIntersections { .. },
        ] = base.mechanisms.as_slice()
        else {
            return Err(ValidationError::new(
                "pattern_definitions.family",
                "custom authored guides require an ordinary straight grid",
            ));
        };
        let (prototype, orientation) = match base.output_layers.as_slice() {
            [PatternOutputLayer::CircularMarks { .. }] => {
                (MarkPrototype::Circle, MarkOrientation::Fixed)
            }
            [
                PatternOutputLayer::MarkPrototype {
                    prototype,
                    orientation,
                    ..
                },
            ] => (prototype.clone(), orientation.clone()),
            _ => {
                return Err(ValidationError::new(
                    "pattern_definitions.output_layers",
                    "custom authored guides require one selected mark output",
                ));
            }
        };
        let definition_id = self.allocate_definition_id()?;
        let guide_id = self.allocate_mechanism_id()?;
        let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.mechanisms.id",
                "document mechanism ID space is exhausted",
            )
        })?);
        let output_id = self.allocate_output_layer_id()?;
        let dimension_id = self.allocate_dimension_id()?;
        let mut definition = PatternDefinition::generalized_guides(
            definition_id,
            format!("{} custom guide layout", base.name),
            guide_id,
            site_id,
            output_id,
            vec![GuideDimension {
                id: dimension_id,
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath { structure_id },
                repetition: GuideRepetition::Single,
            }],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![dimension_id],
                interval_multiplier: 1.0,
                phase: 0.0,
            },
            orientation,
            base.coverage.clone(),
        );
        let [
            PatternOutputLayer::MarkPrototype {
                prototype: target_prototype,
                ..
            },
        ] = definition.output_layers.as_mut_slice()
        else {
            unreachable!("generic guide constructor owns one mark output")
        };
        *target_prototype = prototype;
        Ok(definition)
    }

    fn allocate_definition_from_draft(
        &self,
        draft: &PatternDefinitionDraft,
    ) -> Result<PatternDefinition, ValidationError> {
        validate_definition_draft(draft)?;
        let id = self.allocate_definition_id()?;
        let guide_id = self.allocate_mechanism_id()?;
        let intersection_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.mechanisms.id",
                "document mechanism ID space is exhausted",
            )
        })?);
        if self
            .pattern_definition_bundles
            .iter()
            .flat_map(|definition| definition.mechanisms.iter())
            .any(|mechanism| mechanism.id() == intersection_id)
        {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.id",
                "document mechanism ID allocation collided",
            ));
        }
        let output_id = self.allocate_output_layer_id()?;
        Ok(PatternDefinition::supported_straight_grid(
            id,
            draft.name.clone(),
            guide_id,
            intersection_id,
            output_id,
            draft.coverage.clone(),
        ))
    }

    /// Materializes an ID-free recipe through the existing descriptor, variant
    /// draft, edit, and validation boundaries. The neutral topology is first
    /// attached to a private candidate either as the document base or as one
    /// channel override; payload-bearing variants are assembled only by
    /// `VariantTransitionDraft` before any command can publish it.
    ///
    /// # Errors
    ///
    /// Returns stable allocation, recipe-topology, transition, authored-shape,
    /// or definition validation errors without publishing the private candidate.
    fn allocate_definition_from_recipe(
        &self,
        channel_id: Option<ChannelId>,
        recipe: &PatternDefinitionRecipe,
    ) -> Result<MaterializedPatternDefinitionRecipe, ValidationError> {
        let (definition_recipe, shape_draft, connection, maze, voronoi, guide_faces) =
            match &recipe.structure {
                PatternStructureRecipe::AuthoredClosedShapeMarks { definition, shape } => {
                    (definition.as_ref(), Some(shape), None, None, false, None)
                }
                PatternStructureRecipe::ConnectionPaths {
                    definition,
                    program,
                    style,
                } => {
                    validate_connection_recipe(definition, program)?;
                    (
                        definition.as_ref(),
                        None,
                        Some((program, *style)),
                        None,
                        false,
                        None,
                    )
                }
                PatternStructureRecipe::MazeWalls {
                    definition,
                    program,
                    style,
                } => {
                    validate_maze_recipe(definition, program)?;
                    (
                        definition.as_ref(),
                        None,
                        None,
                        Some((program, *style)),
                        false,
                        None,
                    )
                }
                PatternStructureRecipe::VoronoiRegions { definition } => {
                    (definition.as_ref(), None, None, None, true, None)
                }
                PatternStructureRecipe::GuideFaceRegions {
                    definition,
                    dimension_indices,
                } => (
                    definition.as_ref(),
                    None,
                    None,
                    None,
                    false,
                    Some(dimension_indices.as_slice()),
                ),
                _ => (&recipe.structure, None, None, None, false, None),
            };
        let neutral = self.allocate_neutral_definition_from_recipe(definition_recipe)?;
        let mut candidate = self.clone();
        candidate
            .pattern_definition_bundles
            .push(bundle_from_definition(neutral.clone()));
        if let Some(channel_id) = channel_id {
            candidate.retarget_channel(channel_id, neutral.id);
        } else {
            candidate.pattern_settings.definition_id = neutral.id;
        }
        candidate.apply_recipe_controls(neutral.id, definition_recipe)?;
        if let Some((program, style)) = connection {
            let definition = candidate
                .pattern_definition_bundles
                .iter_mut()
                .find(|definition| definition.id == neutral.id)
                .expect("fresh recipe definition");
            let (output_id, site_mechanism_id) = match definition.output_layers.as_slice() {
                [
                    PatternOutputLayer::CircularMarks {
                        id,
                        site_mechanism_id,
                    },
                ]
                | [
                    PatternOutputLayer::MarkPrototype {
                        id,
                        site_mechanism_id,
                        ..
                    },
                ] => (*id, *site_mechanism_id),
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.recipe.connection",
                        "connection recipe materialized an incompatible output",
                    ));
                }
            };
            definition.output_layers = vec![PatternOutputLayer::ConnectionPaths {
                id: output_id,
                site_mechanism_id,
                program: program.clone(),
                style,
            }];
            validate_definition(definition)?;
        }
        if let Some((program, style)) = maze {
            let definition = candidate
                .pattern_definition_bundles
                .iter_mut()
                .find(|definition| definition.id == neutral.id)
                .expect("fresh recipe definition");
            let (output_id, site_mechanism_id) = match definition.output_layers.as_slice() {
                [
                    PatternOutputLayer::CircularMarks {
                        id,
                        site_mechanism_id,
                    },
                ]
                | [
                    PatternOutputLayer::MarkPrototype {
                        id,
                        site_mechanism_id,
                        ..
                    },
                ] => (*id, *site_mechanism_id),
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.recipe.maze",
                        "maze recipe materialized an incompatible output",
                    ));
                }
            };
            definition.output_layers = vec![PatternOutputLayer::MazeWalls {
                id: output_id,
                site_mechanism_id,
                program: program.clone(),
                style,
            }];
            validate_definition(definition)?;
        }
        if voronoi {
            let definition = candidate
                .pattern_definition_bundles
                .iter_mut()
                .find(|definition| definition.id == neutral.id)
                .expect("fresh recipe definition");
            let (output_id, site_mechanism_id) = match definition.output_layers.as_slice() {
                [
                    PatternOutputLayer::CircularMarks {
                        id,
                        site_mechanism_id,
                    },
                ]
                | [
                    PatternOutputLayer::MarkPrototype {
                        id,
                        site_mechanism_id,
                        ..
                    },
                ] => (*id, *site_mechanism_id),
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.recipe.voronoi",
                        "Voronoi recipe materialized an incompatible output",
                    ));
                }
            };
            definition.output_layers = vec![PatternOutputLayer::Regions {
                id: output_id,
                source: RegionSourceIntent::VoronoiSites { site_mechanism_id },
            }];
            validate_definition(definition)?;
        }
        if let Some(dimension_indices) = guide_faces {
            let definition = candidate
                .pattern_definition_bundles
                .iter_mut()
                .find(|definition| definition.id == neutral.id)
                .expect("fresh recipe definition");
            let (output_id, guide_mechanism_id, dimensions) = match (
                definition.output_layers.as_slice(),
                definition.mechanisms.as_slice(),
            ) {
                (
                    [PatternOutputLayer::CircularMarks { id, .. }]
                    | [PatternOutputLayer::MarkPrototype { id, .. }],
                    [
                        PatternMechanism::StraightGuideDimensions {
                            id: guide_id,
                            dimensions,
                        },
                        ..,
                    ],
                ) => (*id, *guide_id, dimensions),
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.recipe.guide_faces",
                        "Guide Faces recipe materialized an incompatible straight-guide output",
                    ));
                }
            };
            let selected = dimension_indices
                .iter()
                .map(|index| dimensions.get(*index).map(|dimension| dimension.id))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.recipe.guide_faces",
                        "Guide Faces recipe dimension index is out of bounds",
                    )
                })?;
            definition.output_layers = vec![PatternOutputLayer::Regions {
                id: output_id,
                source: RegionSourceIntent::GuideFaces {
                    guide_mechanism_id,
                    dimensions: selected,
                },
            }];
            validate_definition(definition)?;
        }
        let authored_structure = if let Some(shape_draft) = shape_draft {
            let definition = candidate
                .pattern_definition_bundles
                .iter_mut()
                .find(|definition| definition.id == neutral.id)
                .expect("fresh recipe definition");
            if let [
                PatternOutputLayer::CircularMarks {
                    id,
                    site_mechanism_id,
                },
            ] = definition.output_layers.as_slice()
            {
                definition.output_layers = vec![PatternOutputLayer::MarkPrototype {
                    id: *id,
                    site_mechanism_id: *site_mechanism_id,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                }];
                validate_definition(definition)?;
            }
            let structure_id = next_authored_structure_id(&candidate.authored_structures)?;
            let structure = AuthoredStructure::new(
                structure_id,
                AuthoredStructureKind::ClosedShape,
                shape_draft.segments().to_vec(),
            )?;
            candidate.authored_structures.push(structure.clone());
            candidate.apply_recipe_transition(
                neutral.id,
                PropertyFieldId::OutputPrototype,
                PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::AuthoredClosedShape),
                vec![(
                    PropertyFieldId::OutputAuthoredClosedShape,
                    VariantTransitionValue::StableReference(Some(
                        PropertyReferenceValue::AuthoredStructure(structure_id),
                    )),
                )],
            )?;
            Some(structure)
        } else {
            None
        };
        let definition = candidate.definition(neutral.id).cloned().ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.recipe",
                "recipe materialization lost its fresh definition",
            )
        })?;
        let bundle = bind_recipe_output_settings(definition, &recipe.output_settings)?;
        Ok(MaterializedPatternDefinitionRecipe {
            bundle,
            authored_structure,
        })
    }

    /// Allocates only a valid neutral topology for an ID-free recipe. Variant
    /// payloads deliberately remain neutral here and are applied later through
    /// the public transition-draft authority.
    fn allocate_neutral_definition_from_recipe(
        &self,
        recipe: &PatternStructureRecipe,
    ) -> Result<PatternDefinition, ValidationError> {
        match recipe {
            PatternStructureRecipe::StraightGrid(draft) => {
                self.allocate_definition_from_draft(draft)
            }
            PatternStructureRecipe::GeneralizedStraightGuides {
                name,
                coverage,
                dimensions,
                product,
                orientation: _,
            } => {
                let id = self.allocate_definition_id()?;
                let guide_id = self.allocate_mechanism_id()?;
                let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.id",
                        "document mechanism ID space is exhausted",
                    )
                })?);
                let output_id = self.allocate_output_layer_id()?;
                let mut next_dimension = self.allocate_dimension_id()?.0;
                let mut materialized_dimensions = Vec::with_capacity(dimensions.len());
                for (index, _) in dimensions.iter().enumerate() {
                    materialized_dimensions.push(StraightGuideDimension {
                        id: GuideDimensionId(next_dimension),
                        baseline_angle_degrees: [0.0, 90.0, 45.0, 135.0]
                            .get(index)
                            .copied()
                            .unwrap_or(0.0),
                        phase: 0.0,
                        repetition: StraightGuideRepetition {
                            spacing_multiplier: 1.0,
                        },
                    });
                    next_dimension = next_dimension.checked_add(1).ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.mechanisms.dimensions.id",
                            "document dimension ID space is exhausted",
                        )
                    })?;
                }
                let dimension = |index: usize| {
                    materialized_dimensions
                        .get(index)
                        .map(|value| value.id)
                        .ok_or_else(|| {
                            ValidationError::new(
                                "pattern_definitions.recipe.dimensions",
                                "recipe dimension index is out of bounds",
                            )
                        })
                };
                let product = match product {
                    GeneralizedSiteProductDraft::Intersections { .. } => {
                        GeneralizedSiteProduct::Intersections {
                            dimensions: vec![dimension(0)?, dimension(1)?],
                            merge_epsilon: 0.0,
                        }
                    }
                    GeneralizedSiteProductDraft::AlongGuides { .. } => {
                        GeneralizedSiteProduct::AlongGuides {
                            dimensions: vec![dimension(0)?],
                            interval_multiplier: 1.0,
                            phase: 0.0,
                        }
                    }
                };
                let definition = PatternDefinition::generalized_straight_guides(
                    id,
                    name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    materialized_dimensions,
                    product,
                    MarkOrientation::Fixed,
                    coverage.clone(),
                );
                validate_definition(&definition)?;
                Ok(definition)
            }
            PatternStructureRecipe::RandomSites {
                name,
                coverage,
                character: _,
                seed: _,
                density_modulation: _,
                exclusion: _,
                maximum_attempts: _,
                maximum_neighbor_checks: _,
            } => {
                let id = self.allocate_definition_id()?;
                let base_id = self.allocate_mechanism_id()?;
                let modulation_id =
                    PatternMechanismId(base_id.0.checked_add(1).ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.mechanisms.id",
                            "document mechanism ID space is exhausted",
                        )
                    })?);
                let exclusion_id =
                    PatternMechanismId(modulation_id.0.checked_add(1).ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.mechanisms.id",
                            "document mechanism ID space is exhausted",
                        )
                    })?);
                let site_id =
                    PatternMechanismId(exclusion_id.0.checked_add(1).ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.mechanisms.id",
                            "document mechanism ID space is exhausted",
                        )
                    })?);
                let definition = PatternDefinition::random_sites(
                    id,
                    name.clone(),
                    base_id,
                    modulation_id,
                    exclusion_id,
                    site_id,
                    self.allocate_output_layer_id()?,
                    RandomSiteCharacter::RawUniform,
                    0,
                    SiteDensityModulation::Uniform,
                    SiteExclusionPolicy::None,
                    1,
                    1,
                    coverage.clone(),
                );
                validate_definition(&definition)?;
                Ok(definition)
            }
            PatternStructureRecipe::ConnectionPaths { .. }
            | PatternStructureRecipe::MazeWalls { .. }
            | PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
            | PatternStructureRecipe::VoronoiRegions { .. }
            | PatternStructureRecipe::GuideFaceRegions { .. } => {
                unreachable!("output recipe wrapper is removed before neutral allocation")
            }
        }
    }

    /// Applies a recipe's scalar controls and compound alternatives to a
    /// private candidate definition. All payload-bearing alternatives pass
    /// through `variant_transition_draft`; scalar leaves use existing edits.
    fn apply_recipe_controls(
        &mut self,
        definition_id: PatternDefinitionId,
        recipe: &PatternStructureRecipe,
    ) -> Result<(), ValidationError> {
        match recipe {
            PatternStructureRecipe::StraightGrid(_) => Ok(()),
            PatternStructureRecipe::GeneralizedStraightGuides {
                coverage,
                dimensions,
                product,
                orientation,
                ..
            } => {
                let definition = self
                    .definition(definition_id)
                    .cloned()
                    .expect("fresh recipe definition");
                let (guide_id, site_id, _output_id, stored_dimensions) = match (
                    definition.mechanisms.as_slice(),
                    definition.output_layers.as_slice(),
                ) {
                    (
                        [
                            PatternMechanism::StraightGuideDimensions { id, dimensions },
                            site,
                        ],
                        [output],
                    ) => (*id, site.id(), output.id(), dimensions.clone()),
                    _ => unreachable!("neutral generalized recipe has typed topology"),
                };
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetCoverageGuardSteps {
                        guard_steps: coverage.guard_steps,
                    },
                )?;
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetCoverageAdditionalMargin {
                        additional_margin: coverage.additional_margin,
                    },
                )?;
                for (index, authored) in dimensions.iter().enumerate() {
                    let dimension_id = stored_dimensions
                        .get(index)
                        .ok_or_else(|| {
                            ValidationError::new(
                                "pattern_definitions.recipe.dimensions",
                                "recipe dimension index is out of bounds",
                            )
                        })?
                        .id;
                    self.apply_recipe_edit(
                        definition_id,
                        PatternDefinitionEdit::SetGuideBaselineAngle {
                            mechanism_id: guide_id,
                            dimension_id,
                            baseline_angle_degrees: authored.baseline_angle_degrees,
                        },
                    )?;
                    self.apply_recipe_edit(
                        definition_id,
                        PatternDefinitionEdit::SetGuidePhase {
                            mechanism_id: guide_id,
                            dimension_id,
                            phase: authored.phase,
                        },
                    )?;
                    self.apply_recipe_edit(
                        definition_id,
                        PatternDefinitionEdit::SetGuideSpacingMultiplier {
                            mechanism_id: guide_id,
                            dimension_id,
                            spacing_multiplier: authored.spacing_multiplier,
                        },
                    )?;
                }
                let map_indices =
                    |indices: &[usize]| -> Result<Vec<GuideDimensionId>, ValidationError> {
                        indices
                            .iter()
                            .map(|index| {
                                stored_dimensions
                                    .get(*index)
                                    .map(|value| value.id)
                                    .ok_or_else(|| {
                                        ValidationError::new(
                                            "pattern_definitions.recipe.dimensions",
                                            "recipe dimension index is out of bounds",
                                        )
                                    })
                            })
                            .collect()
                    };
                match product {
                    GeneralizedSiteProductDraft::Intersections {
                        dimension_indices,
                        merge_epsilon,
                    } => {
                        self.apply_recipe_edit(
                            definition_id,
                            PatternDefinitionEdit::SetIntersectionDimensions {
                                mechanism_id: site_id,
                                dimensions: map_indices(dimension_indices)?,
                            },
                        )?;
                        self.apply_recipe_edit(
                            definition_id,
                            PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                                mechanism_id: site_id,
                                merge_epsilon: *merge_epsilon,
                            },
                        )?;
                    }
                    GeneralizedSiteProductDraft::AlongGuides {
                        dimension_indices,
                        interval_multiplier,
                        phase,
                    } => {
                        self.apply_recipe_edit(
                            definition_id,
                            PatternDefinitionEdit::SetAlongGuideDimensions {
                                mechanism_id: site_id,
                                dimensions: map_indices(dimension_indices)?,
                            },
                        )?;
                        self.apply_recipe_edit(
                            definition_id,
                            PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
                                mechanism_id: site_id,
                                interval_multiplier: *interval_multiplier,
                            },
                        )?;
                        self.apply_recipe_edit(
                            definition_id,
                            PatternDefinitionEdit::SetAlongGuidePhase {
                                mechanism_id: site_id,
                                phase: *phase,
                            },
                        )?;
                    }
                }
                let (choice, dimension_index) = match orientation {
                    MarkOrientationDraft::Fixed => return Ok(()),
                    MarkOrientationDraft::GuideTangent { dimension_index } => {
                        (MarkOrientationKind::GuideTangent, *dimension_index)
                    }
                    MarkOrientationDraft::GuideNormal { dimension_index } => {
                        (MarkOrientationKind::GuideNormal, *dimension_index)
                    }
                };
                let guide_dimension = stored_dimensions
                    .get(dimension_index)
                    .ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.recipe.orientation",
                            "recipe orientation dimension index is out of bounds",
                        )
                    })?
                    .id;
                self.apply_recipe_transition(
                    definition_id,
                    PropertyFieldId::OutputOrientation,
                    PropertyEnumChoice::MarkOrientation(choice),
                    vec![(
                        PropertyFieldId::OutputOrientationDimension,
                        VariantTransitionValue::StableReference(Some(
                            PropertyReferenceValue::GuideDimension(guide_dimension),
                        )),
                    )],
                )
            }
            PatternStructureRecipe::RandomSites {
                coverage,
                character,
                seed,
                density_modulation,
                exclusion,
                maximum_attempts,
                maximum_neighbor_checks,
                ..
            } => {
                let definition = self
                    .definition(definition_id)
                    .cloned()
                    .expect("fresh recipe definition");
                let [
                    PatternMechanism::RandomSiteProcess { id: random_id, .. },
                    PatternMechanism::SiteDensityModulation {
                        id: modulation_id, ..
                    },
                    PatternMechanism::SiteExclusion {
                        id: exclusion_id, ..
                    },
                    PatternMechanism::RandomSiteProduct { id: product_id, .. },
                ] = definition.mechanisms.as_slice()
                else {
                    unreachable!("neutral random recipe has typed topology")
                };
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetCoverageGuardSteps {
                        guard_steps: coverage.guard_steps,
                    },
                )?;
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetCoverageAdditionalMargin {
                        additional_margin: coverage.additional_margin,
                    },
                )?;
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetRandomSeed {
                        mechanism_id: *random_id,
                        seed: *seed,
                    },
                )?;
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetRandomMaximumAttempts {
                        mechanism_id: *product_id,
                        maximum_attempts: *maximum_attempts,
                    },
                )?;
                self.apply_recipe_edit(
                    definition_id,
                    PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                        mechanism_id: *product_id,
                        maximum_neighbor_checks: *maximum_neighbor_checks,
                    },
                )?;
                let (character_choice, character_updates) = recipe_random_transition(character);
                if character_choice != RandomCharacterKind::RawUniform {
                    self.apply_recipe_transition(
                        definition_id,
                        PropertyFieldId::RandomCharacter,
                        PropertyEnumChoice::RandomCharacter(character_choice),
                        character_updates,
                    )?;
                }
                let (modulation_choice, modulation_updates) =
                    recipe_modulation_transition(density_modulation);
                if modulation_choice != DensityModulationKind::Uniform {
                    self.apply_recipe_transition(
                        definition_id,
                        PropertyFieldId::RandomDensityModulation,
                        PropertyEnumChoice::DensityModulation(modulation_choice),
                        modulation_updates,
                    )?;
                }
                let (exclusion_choice, exclusion_updates) = recipe_exclusion_transition(exclusion);
                if exclusion_choice != ExclusionKind::None {
                    self.apply_recipe_transition(
                        definition_id,
                        PropertyFieldId::RandomExclusion,
                        PropertyEnumChoice::Exclusion(exclusion_choice),
                        exclusion_updates,
                    )?;
                }
                let _ = (modulation_id, exclusion_id);
                Ok(())
            }
            PatternStructureRecipe::ConnectionPaths { .. }
            | PatternStructureRecipe::MazeWalls { .. }
            | PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
            | PatternStructureRecipe::VoronoiRegions { .. }
            | PatternStructureRecipe::GuideFaceRegions { .. } => {
                unreachable!("output recipe wrapper is removed before control application")
            }
        }
    }

    /// Validates and applies one existing scalar edit to an unpublished recipe
    /// candidate, preserving the same leaf validation as normal commands.
    fn apply_recipe_edit(
        &mut self,
        definition_id: PatternDefinitionId,
        edit: PatternDefinitionEdit,
    ) -> Result<(), ValidationError> {
        let definition = self
            .definition(definition_id)
            .cloned()
            .expect("fresh recipe definition");
        validate_definition_edit(&definition, &edit)?;
        let target = self
            .pattern_definition_bundles
            .iter_mut()
            .find(|definition| definition.id == definition_id)
            .expect("fresh recipe definition");
        apply_definition_edit(target, &edit);
        validate_definition(target)
    }

    /// Finalizes and applies a Stage 17A transition draft to an unpublished
    /// recipe candidate. This is the sole recipe path for complete variants.
    fn apply_recipe_transition(
        &mut self,
        definition_id: PatternDefinitionId,
        field: PropertyFieldId,
        choice: PropertyEnumChoice,
        values: Vec<(PropertyFieldId, VariantTransitionValue)>,
    ) -> Result<(), ValidationError> {
        let selector = self
            .property_descriptors()
            .into_iter()
            .find(|descriptor| {
                descriptor.field == field
                    && transition_definition_id(descriptor.target) == Some(definition_id)
            })
            .ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.recipe",
                    "recipe transition selector is inactive",
                )
            })?;
        let draft = self.variant_transition_draft(&selector, choice)?;
        let updates = values
            .into_iter()
            .map(|(field, value)| {
                let target = draft
                    .fields()
                    .iter()
                    .find(|candidate| candidate.field == field)
                    .ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.recipe",
                            "recipe transition payload is inactive",
                        )
                    })?
                    .target;
                Ok(VariantTransitionFieldUpdate {
                    field,
                    target,
                    value,
                })
            })
            .collect::<Result<Vec<_>, ValidationError>>()?;
        let edit = draft.with_updates(&updates)?.finalize(self)?;
        self.apply_recipe_edit(definition_id, edit)
    }

    /// Allocates fresh definition-internal identities while retaining external authored-resource
    /// references and remapping every definition-owned link, including parametric source/site/output IDs.
    ///
    /// # Errors
    ///
    /// Returns stable ID-exhaustion or structural-family diagnostics without publishing a clone.
    fn duplicate_definition(
        &self,
        source: &PatternDefinition,
    ) -> Result<PatternDefinition, ValidationError> {
        if let (
            [
                PatternMechanism::StraightGuides { .. },
                PatternMechanism::GuideIntersections { .. },
            ],
            [PatternOutputLayer::ConnectionPaths { program, style, .. }],
        ) = (
            source.mechanisms.as_slice(),
            source.output_layers.as_slice(),
        ) {
            let id = self.allocate_definition_id()?;
            let guide_id = self.allocate_mechanism_id()?;
            let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            return Ok(PatternDefinition {
                id,
                name: source.name.clone(),
                family: PatternFamily::GuideIntersections {
                    guide_mechanism_id: guide_id,
                    site_mechanism_id: site_id,
                },
                mechanisms: vec![
                    PatternMechanism::StraightGuides { id: guide_id },
                    PatternMechanism::GuideIntersections {
                        id: site_id,
                        guide_mechanism_id: guide_id,
                    },
                ],
                output_layers: vec![PatternOutputLayer::ConnectionPaths {
                    id: output_id,
                    site_mechanism_id: site_id,
                    program: program.clone(),
                    style: *style,
                }],
                modulation: source.modulation.clone(),
                coverage: source.coverage.clone(),
            });
        }
        if let (
            [
                PatternMechanism::StraightGuides { .. },
                PatternMechanism::GuideIntersections { .. },
            ],
            [PatternOutputLayer::MazeWalls { program, style, .. }],
        ) = (
            source.mechanisms.as_slice(),
            source.output_layers.as_slice(),
        ) {
            let id = self.allocate_definition_id()?;
            let guide_id = self.allocate_mechanism_id()?;
            let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            return Ok(PatternDefinition {
                id,
                name: source.name.clone(),
                family: PatternFamily::GuideIntersections {
                    guide_mechanism_id: guide_id,
                    site_mechanism_id: site_id,
                },
                mechanisms: vec![
                    PatternMechanism::StraightGuides { id: guide_id },
                    PatternMechanism::GuideIntersections {
                        id: site_id,
                        guide_mechanism_id: guide_id,
                    },
                ],
                output_layers: vec![PatternOutputLayer::MazeWalls {
                    id: output_id,
                    site_mechanism_id: site_id,
                    program: program.clone(),
                    style: *style,
                }],
                modulation: source.modulation.clone(),
                coverage: source.coverage.clone(),
            });
        }
        if let [PatternMechanism::GuideDimensions { dimensions, .. }, site] =
            source.mechanisms.as_slice()
        {
            let id = self.allocate_definition_id()?;
            let guide_id = self.allocate_mechanism_id()?;
            let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            let mut next_dimension = self.allocate_dimension_id()?.0;
            let mut remapped = Vec::with_capacity(dimensions.len());
            for dimension in dimensions {
                let new_id = GuideDimensionId(next_dimension);
                next_dimension = next_dimension.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.guide_dimensions.id",
                        "document dimension ID space is exhausted",
                    )
                })?;
                remapped.push((
                    dimension.id,
                    GuideDimension {
                        id: new_id,
                        baseline_angle_degrees: dimension.baseline_angle_degrees,
                        phase: dimension.phase,
                        prototype: dimension.prototype.clone(),
                        repetition: dimension.repetition.clone(),
                    },
                ));
            }
            let remap = |old: GuideDimensionId| {
                remapped
                    .iter()
                    .find(|(candidate, _)| *candidate == old)
                    .map(|(_, value)| value.id)
                    .expect("source selection is validated")
            };
            let product = match site {
                PatternMechanism::SelectedGuideIntersections {
                    dimensions,
                    merge_epsilon,
                    ..
                } => GeneralizedSiteProduct::Intersections {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    merge_epsilon: *merge_epsilon,
                },
                PatternMechanism::AlongGuideSites {
                    dimensions,
                    interval_multiplier,
                    phase,
                    ..
                } => GeneralizedSiteProduct::AlongGuides {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    interval_multiplier: *interval_multiplier,
                    phase: *phase,
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family",
                        "generic definition has an incompatible site mechanism",
                    ));
                }
            };
            if let [PatternOutputLayer::ConnectionPaths { program, style, .. }] =
                source.output_layers.as_slice()
            {
                let mut duplicate = PatternDefinition::generalized_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.into_iter().map(|(_, value)| value).collect(),
                    product,
                    MarkOrientation::Fixed,
                    source.coverage.clone(),
                );
                duplicate.output_layers = vec![PatternOutputLayer::ConnectionPaths {
                    id: output_id,
                    site_mechanism_id: site_id,
                    program: program.clone(),
                    style: *style,
                }];
                return Ok(duplicate);
            }
            if let [
                PatternOutputLayer::Regions {
                    source:
                        RegionSourceIntent::GuideFaces {
                            dimensions: selected,
                            ..
                        },
                    ..
                },
            ] = source.output_layers.as_slice()
            {
                let mut duplicate = PatternDefinition::generalized_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.iter().map(|(_, value)| value.clone()).collect(),
                    product,
                    MarkOrientation::Fixed,
                    source.coverage.clone(),
                );
                duplicate.output_layers = vec![PatternOutputLayer::Regions {
                    id: output_id,
                    source: RegionSourceIntent::GuideFaces {
                        guide_mechanism_id: guide_id,
                        dimensions: selected.iter().copied().map(remap).collect(),
                    },
                }];
                return Ok(duplicate);
            }
            let orientation = match source.output_layers.as_slice() {
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::Fixed,
                        ..
                    },
                ] => MarkOrientation::Fixed,
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideTangent { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideTangent {
                    dimension_id: remap(*dimension_id),
                },
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideNormal { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideNormal {
                    dimension_id: remap(*dimension_id),
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "generic definition has an incompatible mark prototype",
                    ));
                }
            };
            return retain_duplicated_mark_prototype(
                source,
                PatternDefinition::generalized_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.into_iter().map(|(_, value)| value).collect(),
                    product,
                    orientation,
                    source.coverage.clone(),
                ),
            );
        }
        if let [
            PatternMechanism::StraightGuideDimensions { dimensions, .. },
            site,
        ] = source.mechanisms.as_slice()
        {
            let id = self.allocate_definition_id()?;
            let guide_id = self.allocate_mechanism_id()?;
            let site_id = PatternMechanismId(guide_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            let mut next_dimension = self.allocate_dimension_id()?.0;
            let mut remapped = Vec::with_capacity(dimensions.len());
            for dimension in dimensions {
                let new_id = GuideDimensionId(next_dimension);
                next_dimension = next_dimension.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.dimensions.id",
                        "document dimension ID space is exhausted",
                    )
                })?;
                remapped.push((
                    dimension.id,
                    StraightGuideDimension {
                        id: new_id,
                        baseline_angle_degrees: dimension.baseline_angle_degrees,
                        phase: dimension.phase,
                        repetition: dimension.repetition.clone(),
                    },
                ));
            }
            let remap = |old: GuideDimensionId| {
                remapped
                    .iter()
                    .find(|(candidate, _)| *candidate == old)
                    .map(|(_, value)| value.id)
                    .expect("source selection is validated")
            };
            let product = match site {
                PatternMechanism::SelectedGuideIntersections {
                    dimensions,
                    merge_epsilon,
                    ..
                } => GeneralizedSiteProduct::Intersections {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    merge_epsilon: *merge_epsilon,
                },
                PatternMechanism::AlongGuideSites {
                    dimensions,
                    interval_multiplier,
                    phase,
                    ..
                } => GeneralizedSiteProduct::AlongGuides {
                    dimensions: dimensions.iter().copied().map(remap).collect(),
                    interval_multiplier: *interval_multiplier,
                    phase: *phase,
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family",
                        "generalized definition has an incompatible site mechanism",
                    ));
                }
            };
            if let [PatternOutputLayer::ConnectionPaths { program, style, .. }] =
                source.output_layers.as_slice()
            {
                let mut duplicate = PatternDefinition::generalized_straight_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.into_iter().map(|(_, value)| value).collect(),
                    product,
                    MarkOrientation::Fixed,
                    source.coverage.clone(),
                );
                duplicate.output_layers = vec![PatternOutputLayer::ConnectionPaths {
                    id: output_id,
                    site_mechanism_id: site_id,
                    program: program.clone(),
                    style: *style,
                }];
                return Ok(duplicate);
            }
            if let [
                PatternOutputLayer::Regions {
                    source:
                        RegionSourceIntent::GuideFaces {
                            dimensions: selected,
                            ..
                        },
                    ..
                },
            ] = source.output_layers.as_slice()
            {
                let mut duplicate = PatternDefinition::generalized_straight_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.iter().map(|(_, value)| value.clone()).collect(),
                    product,
                    MarkOrientation::Fixed,
                    source.coverage.clone(),
                );
                duplicate.output_layers = vec![PatternOutputLayer::Regions {
                    id: output_id,
                    source: RegionSourceIntent::GuideFaces {
                        guide_mechanism_id: guide_id,
                        dimensions: selected.iter().copied().map(remap).collect(),
                    },
                }];
                return Ok(duplicate);
            }
            let orientation = match source.output_layers.as_slice() {
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::Fixed,
                        ..
                    },
                ] => MarkOrientation::Fixed,
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideTangent { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideTangent {
                    dimension_id: remap(*dimension_id),
                },
                [
                    PatternOutputLayer::MarkPrototype {
                        orientation: MarkOrientation::GuideNormal { dimension_id },
                        ..
                    },
                ] => MarkOrientation::GuideNormal {
                    dimension_id: remap(*dimension_id),
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "generalized definition has an incompatible mark prototype",
                    ));
                }
            };
            return retain_duplicated_mark_prototype(
                source,
                PatternDefinition::generalized_straight_guides(
                    id,
                    source.name.clone(),
                    guide_id,
                    site_id,
                    output_id,
                    remapped.into_iter().map(|(_, value)| value).collect(),
                    product,
                    orientation,
                    source.coverage.clone(),
                ),
            );
        }
        if let PatternFamily::ParametricCurve { .. } = source.family {
            let id = self.allocate_definition_id()?;
            let curve_id = self.allocate_mechanism_id()?;
            let output_id = self.allocate_output_layer_id()?;
            let [
                PatternMechanism::ParametricCurveSource {
                    curve, repetition, ..
                },
                rest @ ..,
            ] = source.mechanisms.as_slice()
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.family",
                    "parametric definition has an incompatible source mechanism",
                ));
            };
            let (site_id, mechanisms, site_mechanism_id) = match rest {
                [] => (
                    None,
                    vec![PatternMechanism::ParametricCurveSource {
                        id: curve_id,
                        curve: curve.clone(),
                        repetition: repetition.clone(),
                    }],
                    None,
                ),
                [
                    PatternMechanism::AlongParametricCurveSites {
                        interval, phase, ..
                    },
                ] => {
                    let site_id =
                        PatternMechanismId(curve_id.0.checked_add(1).ok_or_else(|| {
                            ValidationError::new(
                                "pattern_definitions.mechanisms.id",
                                "document mechanism ID space is exhausted",
                            )
                        })?);
                    (
                        Some(site_id),
                        vec![
                            PatternMechanism::ParametricCurveSource {
                                id: curve_id,
                                curve: curve.clone(),
                                repetition: repetition.clone(),
                            },
                            PatternMechanism::AlongParametricCurveSites {
                                id: site_id,
                                curve_mechanism_id: curve_id,
                                interval: *interval,
                                phase: *phase,
                            },
                        ],
                        Some(site_id),
                    )
                }
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family",
                        "parametric definition has an incompatible site mechanism",
                    ));
                }
            };
            let output_layers = match (source.output_layers.as_slice(), site_id) {
                ([PatternOutputLayer::ParametricPaths { style, .. }], None) => {
                    vec![PatternOutputLayer::ParametricPaths {
                        id: output_id,
                        curve_mechanism_id: curve_id,
                        style: *style,
                    }]
                }
                ([PatternOutputLayer::CircularMarks { .. }], Some(site_id)) => {
                    vec![PatternOutputLayer::CircularMarks {
                        id: output_id,
                        site_mechanism_id: site_id,
                    }]
                }
                (
                    [
                        PatternOutputLayer::MarkPrototype {
                            prototype,
                            orientation: MarkOrientation::Fixed,
                            ..
                        },
                    ],
                    Some(site_id),
                ) => vec![PatternOutputLayer::MarkPrototype {
                    id: output_id,
                    site_mechanism_id: site_id,
                    prototype: prototype.clone(),
                    orientation: MarkOrientation::Fixed,
                }],
                ([PatternOutputLayer::ConnectionPaths { program, style, .. }], Some(site_id)) => {
                    vec![PatternOutputLayer::ConnectionPaths {
                        id: output_id,
                        site_mechanism_id: site_id,
                        program: program.clone(),
                        style: *style,
                    }]
                }
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "parametric definition has an incompatible output layer",
                    ));
                }
            };
            return Ok(PatternDefinition {
                id,
                name: source.name.clone(),
                family: PatternFamily::ParametricCurve {
                    curve_mechanism_id: curve_id,
                    site_mechanism_id,
                },
                mechanisms,
                output_layers,
                modulation: source.modulation.clone(),
                coverage: source.coverage.clone(),
            });
        }
        if let PatternFamily::RandomSites { .. } = source.family {
            let id = self.allocate_definition_id()?;
            let base_id = self.allocate_mechanism_id()?;
            let modulation_id = PatternMechanismId(base_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let exclusion_id =
                PatternMechanismId(modulation_id.0.checked_add(1).ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.mechanisms.id",
                        "document mechanism ID space is exhausted",
                    )
                })?);
            let site_id = PatternMechanismId(exclusion_id.0.checked_add(1).ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.id",
                    "document mechanism ID space is exhausted",
                )
            })?);
            let output_id = self.allocate_output_layer_id()?;
            let [
                PatternMechanism::RandomSiteProcess {
                    character, seed, ..
                },
                PatternMechanism::SiteDensityModulation { modulation, .. },
                PatternMechanism::SiteExclusion { policy, .. },
                PatternMechanism::RandomSiteProduct {
                    maximum_attempts,
                    maximum_neighbor_checks,
                    ..
                },
            ] = source.mechanisms.as_slice()
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.family",
                    "random definition has an incompatible mechanism chain",
                ));
            };
            let mut duplicate = PatternDefinition::random_sites(
                id,
                source.name.clone(),
                base_id,
                modulation_id,
                exclusion_id,
                site_id,
                output_id,
                character.clone(),
                *seed,
                modulation.clone(),
                policy.clone(),
                *maximum_attempts,
                *maximum_neighbor_checks,
                source.coverage.clone(),
            );
            if let [PatternOutputLayer::ConnectionPaths { program, style, .. }] =
                source.output_layers.as_slice()
            {
                duplicate.output_layers = vec![PatternOutputLayer::ConnectionPaths {
                    id: output_id,
                    site_mechanism_id: site_id,
                    program: program.clone(),
                    style: *style,
                }];
                return Ok(duplicate);
            }
            return retain_duplicated_mark_prototype(source, duplicate);
        }
        let draft = PatternDefinitionDraft {
            name: source.name.clone(),
            coverage: source.coverage.clone(),
        };
        self.allocate_definition_from_draft(&draft)
    }

    fn retarget_channel(&mut self, channel_id: ChannelId, definition_id: PatternDefinitionId) {
        match &mut self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .expect("validated channel")
                    .pattern_instance
                    .definition_override = Some(definition_id)
            }
            ChannelConfiguration::Topology { topology, .. } => {
                topology
                    .channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .expect("validated channel")
                    .pattern_instance
                    .definition_override = Some(definition_id)
            }
        }
    }

    fn legacy_channel_mut(&mut self, channel_id: ChannelId) -> Option<&mut ChannelState> {
        match &mut self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels.iter_mut().find(|channel| channel.id == channel_id)
            }
            ChannelConfiguration::Topology { .. } => None,
        }
    }

    /// Returns one mutable channel instance while preserving the document as
    /// the sole owner of its base settings and ordered configuration.
    fn channel_pattern_instance_mut(
        &mut self,
        channel_id: ChannelId,
    ) -> Option<&mut ChannelPatternInstance> {
        match &mut self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter_mut()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &mut channel.pattern_instance),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter_mut()
                .find(|channel| channel.id == channel_id)
                .map(|channel| &mut channel.pattern_instance),
        }
    }

    /// Applies the retained absolute translation authority without creating a
    /// second pattern-layout store.
    fn apply_pattern_control(&mut self, command: &DocumentCommand) -> bool {
        let channel_id = command.channel_id();
        match command {
            DocumentCommand::SetTranslationAxis {
                edited_axis, value, ..
            } => {
                let instance = self
                    .channel_pattern_instance_mut(channel_id)
                    .expect("validated channel instance");
                match edited_axis {
                    TranslationEditedAxis::X => instance.layout_delta.translation_x = *value,
                    TranslationEditedAxis::Y => instance.layout_delta.translation_y = *value,
                }
                true
            }
            _ => false,
        }
    }

    /// Validates the complete persisted document contract, including its
    /// bounded authored-structure store before channel or pattern state.
    ///
    /// # Errors
    ///
    /// Returns stable authored-structure, canvas, definition, channel, or
    /// topology diagnostics without mutating the document.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_positive_finite(self.canvas.width, "canvas.width")?;
        validate_positive_finite(self.canvas.height, "canvas.height")?;
        validate_authored_structures(&self.authored_structures)?;

        let mut definition_ids = HashSet::new();
        let mut mechanism_ids = HashSet::new();
        let mut output_layer_ids = HashSet::new();
        let mut dimension_ids = HashSet::new();
        for bundle in &self.pattern_definition_bundles {
            bundle.validate()?;
            let definition = &bundle.definition;
            if !definition_ids.insert(definition.id) {
                return Err(ValidationError::new(
                    "pattern_definitions",
                    "document-owned pattern definition IDs must be unique",
                ));
            }
            if definition.name.trim().is_empty() {
                return Err(ValidationError::new(
                    "pattern_definitions.name",
                    "pattern definition name must not be empty",
                ));
            }
            validate_definition(definition)?;
            validate_definition_guide_references(definition, &self.authored_structures)?;
            for mechanism in &definition.mechanisms {
                if !mechanism_ids.insert(mechanism.id()) {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms",
                        "mechanism IDs must be unique document-wide",
                    ));
                }
                match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => {
                        for dimension in dimensions {
                            if !dimension_ids.insert(dimension.id) {
                                return Err(ValidationError::new(
                                    "pattern_definitions.mechanisms.dimensions",
                                    "straight-guide dimension IDs must be unique document-wide",
                                ));
                            }
                        }
                    }
                    PatternMechanism::GuideDimensions { dimensions, .. } => {
                        for dimension in dimensions {
                            if !dimension_ids.insert(dimension.id) {
                                return Err(ValidationError::new(
                                    "pattern_definitions.mechanisms.guide_dimensions.id",
                                    "guide dimension IDs must be nonzero and unique in stored order",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for layer in &definition.output_layers {
                if !output_layer_ids.insert(layer.id()) {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers",
                        "output layer IDs must be unique document-wide",
                    ));
                }
            }
        }

        validate_layout(&ChannelPatternLayout {
            density: self.pattern_settings.density.clone(),
            rotation_degrees: self.pattern_settings.pattern_rotation_degrees,
            translation_x: 0.0,
            translation_y: 0.0,
        })?;
        validate_finite(
            self.pattern_settings.shape_rotation_degrees,
            "document.pattern_settings.shape_rotation_degrees",
        )?;
        if self
            .definition(self.pattern_settings.definition_id)
            .is_none()
        {
            return Err(ValidationError::new(
                "document.pattern_settings.definition_id",
                "document base references a missing pattern definition",
            ));
        }

        match &self.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                let mut channel_ids = HashSet::new();
                for channel in channels {
                    if !channel_ids.insert(channel.id) {
                        return Err(ValidationError::new(
                            "channels",
                            "document-owned channel IDs must be unique",
                        ));
                    }
                    let definition =
                        self.pattern_definition_for(channel.id)
                            .ok_or(ValidationError::new(
                                "channel.pattern.definition_id",
                                "channel references a missing pattern definition",
                            ))?;
                    validate_channel(channel, definition)?;
                }
            }
            ChannelConfiguration::Topology { model, topology } => {
                validate_topology(*model, topology, &self.pattern_definition_bundles)?;
            }
        }
        for channel_id in self.channel_ids() {
            let effective = self.effective_channel_pattern(channel_id)?;
            if matches!(
                &self.channel_configuration,
                ChannelConfiguration::Topology {
                    model: HalftoneChannelModel::SourceColorAlpha,
                    ..
                }
            ) && self
                .definition(effective.definition_id)
                .is_some_and(|definition| {
                    matches!(
                        definition.output_layers.as_slice(),
                        [PatternOutputLayer::GuidePaths { .. }
                            | PatternOutputLayer::ConnectionPaths { .. }]
                    )
                })
            {
                return Err(ValidationError::new(
                    "channel.paint",
                    "guide-path output requires solid channel paint",
                ));
            }
        }
        Ok(())
    }

    /// Produces one fully validated document transition without mutating `self`.
    ///
    /// `toniator-engine` is responsible for atomically installing the returned
    /// candidate alongside the corresponding revision advance.
    ///
    /// # Errors
    ///
    /// Returns stable command or complete-candidate validation diagnostics;
    /// authored-structure failures leave `self` unchanged and publish neither
    /// a new resource ID nor a partial command result.
    pub fn apply_command(
        &self,
        command: &DocumentCommand,
    ) -> Result<(Self, CommandResult), ValidationError> {
        command.validate(self)?;
        let mut candidate = self.clone();
        command.apply_to_valid_document(&mut candidate);
        candidate.validate()?;
        if candidate == *self {
            return Err(ValidationError::new(
                "command",
                "command is a semantic no-op",
            ));
        }
        let mut result = command.result_for_transition(self, &candidate);
        if let DocumentCommand::EditSharedPatternDefinition { definition_id, .. }
        | DocumentCommand::ReplaceSharedPatternDefinitionRecipe { definition_id, .. } = command
        {
            result.affected_channels = self.linked_channels(*definition_id);
        }
        if let DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id,
            base_definition,
            edit,
        } = command
            && self.linked_channels(base_definition.id).len() > 1
            && definition_edit_invalidation(edit) == InvalidationLevel::Realization
        {
            // Copy-on-edit changes definition and internal stable IDs, so its
            // family key necessarily changes even for an output-only field.
            result.affected_channels = vec![*channel_id];
            result.invalidation = Some(InvalidationLevel::Family);
        } else if matches!(
            command,
            DocumentCommand::EditSelectedChannelPatternDefinition { .. }
                | DocumentCommand::EditSharedPatternDefinition { .. }
        ) {
            let edit = match command {
                DocumentCommand::EditSelectedChannelPatternDefinition { edit, .. }
                | DocumentCommand::EditSharedPatternDefinition { edit, .. } => edit,
                _ => unreachable!(),
            };
            result.invalidation = Some(definition_edit_invalidation(edit));
        }
        Ok((candidate, result))
    }
}

/// Copies the external mark resource selection onto a structurally remapped typed duplicate.
///
/// Definition-owned output IDs and guide-orientation references remain those allocated by the
/// duplicate constructor; only the prototype variant and its document-owned structure ID persist.
///
/// # Errors
///
/// Returns the stable output-layer diagnostic if either definition is not the one-layer typed
/// mark family already required by the duplication branch.
fn retain_duplicated_mark_prototype(
    source: &PatternDefinition,
    mut duplicate: PatternDefinition,
) -> Result<PatternDefinition, ValidationError> {
    let [
        PatternOutputLayer::MarkPrototype {
            prototype: source_prototype,
            ..
        },
    ] = source.output_layers.as_slice()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "typed definition duplication requires one mark-prototype output",
        ));
    };
    let [
        PatternOutputLayer::MarkPrototype {
            prototype: duplicate_prototype,
            ..
        },
    ] = duplicate.output_layers.as_mut_slice()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "typed definition duplication requires one mark-prototype output",
        ));
    };
    *duplicate_prototype = source_prototype.clone();
    Ok(duplicate)
}

/// Resolves every authored guide and mark reference through the owning document store.
///
/// # Errors
///
/// Returns stable missing-resource or wrong-kind diagnostics without changing either authority.
fn validate_definition_guide_references(
    definition: &PatternDefinition,
    structures: &[AuthoredStructure],
) -> Result<(), ValidationError> {
    for mechanism in &definition.mechanisms {
        let PatternMechanism::GuideDimensions { dimensions, .. } = mechanism else {
            continue;
        };
        for dimension in dimensions {
            let GuidePrototype::AuthoredOpenPath { structure_id } = dimension.prototype else {
                continue;
            };
            let structure = structures
                .iter()
                .find(|structure| structure.id == structure_id)
                .ok_or(ValidationError::new(
                    "pattern_definitions.mechanisms.guide_prototype.reference",
                    "authored guide prototype references a missing structure",
                ))?;
            if structure.kind != AuthoredStructureKind::OpenPath {
                return Err(ValidationError::new(
                    "pattern_definitions.mechanisms.guide_prototype.kind",
                    "authored guide prototypes require an open path",
                ));
            }
        }
    }
    for layer in &definition.output_layers {
        let PatternOutputLayer::MarkPrototype {
            prototype: MarkPrototype::AuthoredClosedShape { structure_id },
            ..
        } = layer
        else {
            continue;
        };
        let structure = structures
            .iter()
            .find(|structure| structure.id == *structure_id)
            .ok_or(ValidationError::new(
                "pattern_definitions.output_layers.prototype.reference",
                "authored closed-shape mark references a missing structure",
            ))?;
        if structure.kind != AuthoredStructureKind::ClosedShape {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers.prototype.kind",
                "authored closed-shape marks require a closed shape",
            ));
        }
    }
    Ok(())
}

fn next_id(values: impl Iterator<Item = u64>, path: &'static str) -> Result<u64, ValidationError> {
    let maximum = values.max().unwrap_or(0);
    maximum
        .checked_add(1)
        .ok_or_else(|| ValidationError::new(path, "document ID space is exhausted"))
}

/// Validates the document-owned authored-structure store and its aggregate bounds before commit.
///
/// # Errors
///
/// Returns stable store-limit, aggregate-segment-limit, ID, coordinate, or
/// topology diagnostics without reordering, repairing, or allocating data.
fn validate_authored_structures(structures: &[AuthoredStructure]) -> Result<(), ValidationError> {
    if structures.len() > 4_096 {
        return Err(ValidationError::new(
            "authored_structures.limit",
            "documents support at most 4096 authored structures",
        ));
    }
    let mut ids = HashSet::new();
    let mut total_segments = 0_usize;
    for structure in structures {
        validate_authored_structure_id(structure.id)?;
        if !ids.insert(structure.id) {
            return Err(ValidationError::new(
                "authored_structures.id",
                "authored structure IDs must be unique within a document",
            ));
        }
        validate_authored_structure_segments(structure.kind, &structure.segments)?;
        total_segments =
            total_segments
                .checked_add(structure.segments.len())
                .ok_or(ValidationError::new(
                    "authored_structures.segment_limit",
                    "document authored segment count exceeds the supported limit",
                ))?;
        if total_segments > 65_536 {
            return Err(ValidationError::new(
                "authored_structures.segment_limit",
                "documents support at most 65536 authored segments",
            ));
        }
    }
    Ok(())
}

/// Validates the nonzero stable identity of one document-owned authored structure.
///
/// # Errors
///
/// Returns `authored_structures.id` when the identity is zero.
fn validate_authored_structure_id(id: AuthoredStructureId) -> Result<(), ValidationError> {
    if id.0 == 0 {
        return Err(ValidationError::new(
            "authored_structures.id",
            "authored structure IDs must be nonzero",
        ));
    }
    Ok(())
}

/// Validates finite explicit segments and exact C0/closure topology without creating geometry.
///
/// # Errors
///
/// Returns stable empty, per-structure-limit, coordinate, continuity, or
/// closure diagnostics; it accepts finite degeneracies and never adds a seam.
fn validate_authored_structure_segments(
    kind: AuthoredStructureKind,
    segments: &[AuthoredCurveSegment],
) -> Result<(), ValidationError> {
    if segments.is_empty() {
        return Err(ValidationError::new(
            "authored_structures.segments.empty",
            "authored structures require at least one segment",
        ));
    }
    if segments.len() > 4_096 {
        return Err(ValidationError::new(
            "authored_structures.segments.limit",
            "authored structures support at most 4096 segments",
        ));
    }
    for segment in segments {
        validate_authored_curve_segment(segment)?;
    }
    for pair in segments.windows(2) {
        if pair[0].end() != pair[1].start() {
            return Err(ValidationError::new(
                "authored_structures.segments.continuity",
                "adjacent authored segment endpoints must be exactly equal",
            ));
        }
    }
    if kind == AuthoredStructureKind::ClosedShape
        && segments.last().expect("nonempty authored segments").end() != segments[0].start()
    {
        return Err(ValidationError::new(
            "authored_structures.closure",
            "closed authored shapes require the final endpoint to equal the initial start",
        ));
    }
    Ok(())
}

/// Validates every explicitly stored point of one authored segment without rejecting degeneracy.
///
/// # Errors
///
/// Returns `authored_structures.segments.coordinates` for any non-finite
/// endpoint or cubic control coordinate.
fn validate_authored_curve_segment(segment: &AuthoredCurveSegment) -> Result<(), ValidationError> {
    let points = match segment {
        AuthoredCurveSegment::Line { start, end } => vec![*start, *end],
        AuthoredCurveSegment::CubicBezier {
            start,
            control_1,
            control_2,
            end,
        } => vec![*start, *control_1, *control_2, *end],
    };
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(ValidationError::new(
            "authored_structures.segments.coordinates",
            "authored structure coordinates must be finite",
        ));
    }
    Ok(())
}

/// Allocates the next stable authored-structure ID with checked arithmetic.
///
/// # Errors
///
/// Returns `authored_structures.id` if the current maximum ID cannot advance;
/// it never reuses an ID or changes store order.
fn next_authored_structure_id(
    structures: &[AuthoredStructure],
) -> Result<AuthoredStructureId, ValidationError> {
    next_id(
        structures.iter().map(|structure| structure.id.0),
        "authored_structures.id",
    )
    .map(AuthoredStructureId)
}

/// Derives the earliest future cache boundary affected by an authored structure kind transition.
fn authored_structure_invalidation(kind: AuthoredStructureKind) -> InvalidationLevel {
    match kind {
        AuthoredStructureKind::OpenPath => InvalidationLevel::Family,
        AuthoredStructureKind::ClosedShape => InvalidationLevel::Realization,
    }
}

fn validate_definition_draft(draft: &PatternDefinitionDraft) -> Result<(), ValidationError> {
    if draft.name.trim().is_empty() {
        return Err(ValidationError::new(
            "pattern_definitions.name",
            "pattern definition name must not be empty",
        ));
    }
    validate_nonnegative_finite(
        draft.coverage.additional_margin,
        "pattern_definitions.coverage.additional_margin",
    )
}

/// Projects one random-character recipe payload onto the explicit Stage 17A
/// transition-field updates required to finalize that alternative.
fn recipe_random_transition(
    character: &RandomSiteCharacter,
) -> (
    RandomCharacterKind,
    Vec<(PropertyFieldId, VariantTransitionValue)>,
) {
    match character {
        RandomSiteCharacter::RawUniform => (RandomCharacterKind::RawUniform, Vec::new()),
        RandomSiteCharacter::Even {
            minimum_center_distance,
        } => (
            RandomCharacterKind::Even,
            vec![(
                PropertyFieldId::RandomEvenMinimumCenterDistance,
                VariantTransitionValue::FiniteF64(*minimum_center_distance),
            )],
        ),
        RandomSiteCharacter::Clustered {
            cluster_density,
            cluster_spread,
            cluster_strength,
        } => (
            RandomCharacterKind::Clustered,
            vec![
                (
                    PropertyFieldId::RandomClusterDensity,
                    VariantTransitionValue::FiniteF64(*cluster_density),
                ),
                (
                    PropertyFieldId::RandomClusterSpread,
                    VariantTransitionValue::FiniteF64(*cluster_spread),
                ),
                (
                    PropertyFieldId::RandomClusterStrength,
                    VariantTransitionValue::FiniteF64(*cluster_strength),
                ),
            ],
        ),
    }
}

/// Projects one density-modulation recipe payload onto the explicit Stage 17A
/// transition-field updates required to finalize that alternative.
fn recipe_modulation_transition(
    modulation: &SiteDensityModulation,
) -> (
    DensityModulationKind,
    Vec<(PropertyFieldId, VariantTransitionValue)>,
) {
    match modulation {
        SiteDensityModulation::Uniform => (DensityModulationKind::Uniform, Vec::new()),
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength,
            response,
        } => (
            DensityModulationKind::ArtworkWeighted,
            vec![
                (
                    PropertyFieldId::ArtworkWeightMappingComponent,
                    VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                        mapping.component,
                    )),
                ),
                (
                    PropertyFieldId::ArtworkWeightMappingPlacement,
                    VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourcePlacement(
                        mapping.placement,
                    )),
                ),
                (
                    PropertyFieldId::ArtworkWeightMappingInverted,
                    VariantTransitionValue::Boolean(mapping.inverted),
                ),
                (
                    PropertyFieldId::ArtworkWeightMappingGain,
                    VariantTransitionValue::FiniteF64(mapping.gain),
                ),
                (
                    PropertyFieldId::ArtworkWeightMappingBias,
                    VariantTransitionValue::FiniteF64(mapping.bias),
                ),
                (
                    PropertyFieldId::ArtworkWeightStrength,
                    VariantTransitionValue::FiniteF64(*strength),
                ),
                (
                    PropertyFieldId::ArtworkWeightResponse,
                    VariantTransitionValue::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(
                        *response,
                    )),
                ),
            ],
        ),
    }
}

/// Projects one exclusion recipe payload onto the explicit Stage 17A
/// transition-field updates required to finalize that alternative.
fn recipe_exclusion_transition(
    exclusion: &SiteExclusionPolicy,
) -> (
    ExclusionKind,
    Vec<(PropertyFieldId, VariantTransitionValue)>,
) {
    match exclusion {
        SiteExclusionPolicy::None => (ExclusionKind::None, Vec::new()),
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => (
            ExclusionKind::MinimumCenterDistance,
            vec![(
                PropertyFieldId::ExclusionMinimumCenterDistance,
                VariantTransitionValue::FiniteF64(*minimum),
            )],
        ),
        SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => (
            ExclusionKind::VisibleMarkMargin,
            vec![
                (
                    PropertyFieldId::VisibleMarkMargin,
                    VariantTransitionValue::FiniteF64(*margin),
                ),
                (
                    PropertyFieldId::VisibleMarkSizingPolicy,
                    VariantTransitionValue::EnumChoice(
                        PropertyEnumChoice::VisibleMarkSizingPolicy(*sizing),
                    ),
                ),
            ],
        ),
    }
}

/// Applies one already validated structural edit to a private definition candidate in place.
///
/// Callers retain publication and complete-document reference validation; inactive targets are
/// deliberately left unchanged so validation can reject them before this mutation seam is used.
fn apply_definition_edit(definition: &mut PatternDefinition, edit: &PatternDefinitionEdit) {
    match edit {
        PatternDefinitionEdit::SetParametricShape {
            mechanism_id,
            shape,
        } => apply_parametric_spiral(definition, *mechanism_id, |spiral| spiral.shape = *shape),
        PatternDefinitionEdit::SetParametricTurns {
            mechanism_id,
            turns,
        } => apply_parametric_spiral(definition, *mechanism_id, |spiral| spiral.turns = *turns),
        PatternDefinitionEdit::SetParametricRadialSpacing {
            mechanism_id,
            radial_spacing,
        } => apply_parametric_spiral(definition, *mechanism_id, |spiral| {
            spiral.radial_spacing = *radial_spacing
        }),
        PatternDefinitionEdit::SetParametricPhase {
            mechanism_id,
            phase_degrees,
        } => apply_parametric_spiral(definition, *mechanism_id, |spiral| {
            spiral.phase_degrees = *phase_degrees
        }),
        PatternDefinitionEdit::SetParametricWinding {
            mechanism_id,
            winding,
        } => apply_parametric_spiral(definition, *mechanism_id, |spiral| {
            spiral.winding = *winding
        }),
        PatternDefinitionEdit::SetParametricRepetition {
            mechanism_id,
            repetition,
        } => {
            if let Some(PatternMechanism::ParametricCurveSource {
                repetition: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|item| item.id() == *mechanism_id)
            {
                *current = repetition.clone();
            }
        }
        PatternDefinitionEdit::SetParametricOffsetSpacing {
            mechanism_id,
            spacing,
        } => apply_parametric_offset(definition, *mechanism_id, |value, _, _| *value = *spacing),
        PatternDefinitionEdit::SetParametricOffsetSides {
            mechanism_id,
            sides,
        } => apply_parametric_offset(definition, *mechanism_id, |_, value, _| *value = *sides),
        PatternDefinitionEdit::SetParametricOffsetCleanup {
            mechanism_id,
            cleanup,
        } => apply_parametric_offset(definition, *mechanism_id, |_, _, value| *value = *cleanup),
        PatternDefinitionEdit::SetParametricStackDirection {
            mechanism_id,
            value,
        } => apply_parametric_stack(
            definition,
            *mechanism_id,
            |direction, _, next| *direction = next,
            *value,
        ),
        PatternDefinitionEdit::SetParametricStackSpacingMultiplier {
            mechanism_id,
            value,
        } => apply_parametric_stack(
            definition,
            *mechanism_id,
            |_, spacing, next| *spacing = next,
            *value,
        ),
        PatternDefinitionEdit::SetAlongParametricInterval {
            mechanism_id,
            interval,
        } => {
            if let Some(PatternMechanism::AlongParametricCurveSites {
                interval: current, ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|item| item.id() == *mechanism_id)
            {
                *current = *interval;
            }
        }
        PatternDefinitionEdit::SetAlongParametricPhase {
            mechanism_id,
            phase,
        } => {
            if let Some(PatternMechanism::AlongParametricCurveSites { phase: current, .. }) =
                definition
                    .mechanisms
                    .iter_mut()
                    .find(|item| item.id() == *mechanism_id)
            {
                *current = *phase;
            }
        }
        PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps } => {
            definition.coverage.guard_steps = *guard_steps
        }
        PatternDefinitionEdit::SetCoverageAdditionalMargin { additional_margin } => {
            definition.coverage.additional_margin = *additional_margin
        }
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => {
            match definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) => {
                    if let Some(existing) = dimensions
                        .iter_mut()
                        .find(|value| value.id == *dimension_id)
                    {
                        existing.baseline_angle_degrees = *baseline_angle_degrees;
                    }
                }
                Some(PatternMechanism::GuideDimensions { dimensions, .. }) => {
                    if let Some(existing) = dimensions
                        .iter_mut()
                        .find(|value| value.id == *dimension_id)
                    {
                        existing.baseline_angle_degrees = *baseline_angle_degrees;
                    }
                }
                _ => {}
            }
        }
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => {
            match definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) => {
                    if let Some(existing) = dimensions
                        .iter_mut()
                        .find(|value| value.id == *dimension_id)
                    {
                        existing.phase = *phase;
                    }
                }
                Some(PatternMechanism::GuideDimensions { dimensions, .. }) => {
                    if let Some(existing) = dimensions
                        .iter_mut()
                        .find(|value| value.id == *dimension_id)
                    {
                        existing.phase = *phase;
                    }
                }
                _ => {}
            }
        }
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => {
            if let Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(existing) = dimensions
                    .iter_mut()
                    .find(|value| value.id == *dimension_id)
            {
                existing.repetition.spacing_multiplier = *spacing_multiplier;
            }
        }
        PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id,
            dimension_id,
            prototype,
        } => {
            if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(dimension) = dimensions
                    .iter_mut()
                    .find(|dimension| dimension.id == *dimension_id)
            {
                dimension.prototype = prototype.clone();
            }
        }
        PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id,
            dimension_id,
            structure_id,
        } => {
            if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(GuideDimension {
                    prototype:
                        GuidePrototype::AuthoredOpenPath {
                            structure_id: current,
                        },
                    ..
                }) = dimensions
                    .iter_mut()
                    .find(|dimension| dimension.id == *dimension_id)
            {
                *current = *structure_id;
            }
        }
        PatternDefinitionEdit::SetGuideArcCenterX {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_arc_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |arc, value| arc.0.x = value,
            );
        }
        PatternDefinitionEdit::SetGuideArcCenterY {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_arc_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |arc, value| arc.0.y = value,
            );
        }
        PatternDefinitionEdit::SetGuideArcRadius {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_arc_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |arc, value| *arc.1 = value,
            );
        }
        PatternDefinitionEdit::SetGuideArcStartAngle {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_arc_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |arc, value| *arc.2 = value,
            );
        }
        PatternDefinitionEdit::SetGuideArcSweepAngle {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_arc_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |arc, value| *arc.3 = value,
            );
        }
        PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id,
            dimension_id,
            repetition,
        } => {
            if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
                && let Some(dimension) = dimensions
                    .iter_mut()
                    .find(|dimension| dimension.id == *dimension_id)
            {
                dimension.repetition = repetition.clone();
            }
        }
        PatternDefinitionEdit::SetGuideOffsetSpacing {
            mechanism_id,
            dimension_id,
            spacing,
        } => apply_guide_offset_payload(
            definition,
            *mechanism_id,
            *dimension_id,
            |current_spacing, _sides, _cleanup| *current_spacing = *spacing,
        ),
        PatternDefinitionEdit::SetGuideOffsetSides {
            mechanism_id,
            dimension_id,
            sides,
        } => apply_guide_offset_payload(
            definition,
            *mechanism_id,
            *dimension_id,
            |_spacing, current_sides, _cleanup| *current_sides = *sides,
        ),
        PatternDefinitionEdit::SetGuideOffsetCleanup {
            mechanism_id,
            dimension_id,
            cleanup,
        } => apply_guide_offset_payload(
            definition,
            *mechanism_id,
            *dimension_id,
            |_spacing, _sides, current_cleanup| *current_cleanup = *cleanup,
        ),
        PatternDefinitionEdit::SetGuideStackDirection {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_stack_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |direction, _spacing, value| *direction = value,
            );
        }
        PatternDefinitionEdit::SetGuideStackSpacingMultiplier {
            mechanism_id,
            dimension_id,
            value,
        } => {
            apply_guide_stack_scalar(
                definition,
                *mechanism_id,
                *dimension_id,
                *value,
                |_direction, spacing, value| *spacing = value,
            );
        }
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => {
            if let Some(PatternMechanism::SelectedGuideIntersections {
                dimensions: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = dimensions.clone();
            }
        }
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => {
            if let Some(PatternMechanism::SelectedGuideIntersections {
                merge_epsilon: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *merge_epsilon;
            }
        }
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => {
            if let Some(PatternMechanism::AlongGuideSites {
                dimensions: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = dimensions.clone();
            }
        }
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => {
            if let Some(PatternMechanism::AlongGuideSites {
                interval_multiplier: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *interval_multiplier;
            }
        }
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => {
            if let Some(PatternMechanism::AlongGuideSites { phase: current, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *phase;
            }
        }
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character: current, ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = character.clone();
            }
        }
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, seed } => {
            if let Some(PatternMechanism::RandomSiteProcess { seed: current, .. }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *seed;
            }
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Even {
                        minimum_center_distance: current,
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *minimum_center_distance;
            }
        }
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_density: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_density;
            }
        }
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_spread: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_spread;
            }
        }
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => {
            if let Some(PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_strength: current,
                        ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *cluster_strength;
            }
        }
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation: current,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = modulation.clone();
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id,
            component,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.component = *component;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id,
            placement,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.placement = *placement;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id,
            inverted,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.inverted = *inverted;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.gain = *gain;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                current.bias = *bias;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        strength: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *strength;
            }
        }
        PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id,
            response,
        } => {
            if let Some(PatternMechanism::SiteDensityModulation {
                modulation:
                    SiteDensityModulation::ArtworkWeighted {
                        response: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *response;
            }
        }
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy: current, ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = policy.clone();
            }
        }
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: current },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *minimum_center_distance;
            }
        }
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy:
                    SiteExclusionPolicy::VisibleMarkMargin {
                        margin: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *margin;
            }
        }
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id,
            sizing,
        } => {
            if let Some(PatternMechanism::SiteExclusion {
                policy:
                    SiteExclusionPolicy::VisibleMarkMargin {
                        sizing: current, ..
                    },
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current = *sizing;
            }
        }
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => {
            if let Some(PatternMechanism::RandomSiteProduct {
                maximum_attempts: current_attempts,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current_attempts = *maximum_attempts;
            }
        }
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => {
            if let Some(PatternMechanism::RandomSiteProduct {
                maximum_neighbor_checks: current_checks,
                ..
            }) = definition
                .mechanisms
                .iter_mut()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                *current_checks = *maximum_neighbor_checks;
            }
        }
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => {
            if let Some(
                PatternOutputLayer::CircularMarks {
                    site_mechanism_id: current,
                    ..
                }
                | PatternOutputLayer::MarkPrototype {
                    site_mechanism_id: current,
                    ..
                },
            ) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = *site_mechanism_id;
            }
        }
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype,
        } => {
            if let Some(layer) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                match layer {
                    PatternOutputLayer::MarkPrototype {
                        prototype: current, ..
                    } => *current = prototype.clone(),
                    PatternOutputLayer::CircularMarks {
                        id,
                        site_mechanism_id,
                    } => {
                        *layer = PatternOutputLayer::MarkPrototype {
                            id: *id,
                            site_mechanism_id: *site_mechanism_id,
                            prototype: prototype.clone(),
                            orientation: MarkOrientation::Fixed,
                        };
                    }
                    PatternOutputLayer::GuidePaths { .. }
                    | PatternOutputLayer::ParametricPaths { .. }
                    | PatternOutputLayer::ConnectionPaths { .. }
                    | PatternOutputLayer::MazeWalls { .. }
                    | PatternOutputLayer::Regions { .. } => {}
                }
            }
        }
        PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id,
            structure_id,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype {
                prototype:
                    MarkPrototype::AuthoredClosedShape {
                        structure_id: current,
                    },
                ..
            }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = *structure_id;
            }
        }
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype {
                orientation: current,
                ..
            }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = orientation.clone();
            }
        }
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => {
            if let Some(PatternOutputLayer::MarkPrototype { orientation, .. }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                match orientation {
                    MarkOrientation::GuideTangent {
                        dimension_id: current,
                    }
                    | MarkOrientation::GuideNormal {
                        dimension_id: current,
                    } => *current = *dimension_id,
                    MarkOrientation::Fixed => {}
                }
            }
        }
        PatternDefinitionEdit::SetConnectionProgram {
            output_layer_id,
            program,
        } => apply_connection_program(definition, *output_layer_id, |current| {
            *current = program.clone()
        }),
        PatternDefinitionEdit::SetConnectionMaximumDegree {
            output_layer_id,
            maximum_degree,
        } => apply_connection_program(definition, *output_layer_id, |current| match current {
            ConnectionProgram::NearestLinks { adjacency }
            | ConnectionProgram::RandomLinks { adjacency, .. }
            | ConnectionProgram::GridSpanningTree { adjacency, .. } => {
                adjacency.maximum_degree = *maximum_degree;
            }
        }),
        PatternDefinitionEdit::SetConnectionMaximumDistance {
            output_layer_id,
            maximum_distance,
        } => apply_connection_program(definition, *output_layer_id, |current| match current {
            ConnectionProgram::NearestLinks { adjacency }
            | ConnectionProgram::RandomLinks { adjacency, .. }
            | ConnectionProgram::GridSpanningTree { adjacency, .. } => {
                adjacency.maximum_distance = *maximum_distance;
            }
        }),
        PatternDefinitionEdit::SetConnectionMinimumDegree {
            output_layer_id,
            minimum_degree,
        } => apply_connection_program(definition, *output_layer_id, |current| {
            if let ConnectionProgram::RandomLinks {
                minimum_degree: existing,
                ..
            } = current
            {
                *existing = *minimum_degree;
            }
        }),
        PatternDefinitionEdit::SetConnectionSeed {
            output_layer_id,
            seed,
        } => apply_connection_program(definition, *output_layer_id, |current| match current {
            ConnectionProgram::RandomLinks { seed: existing, .. }
            | ConnectionProgram::GridSpanningTree { seed: existing, .. } => *existing = *seed,
            ConnectionProgram::NearestLinks { .. } => {}
        }),
        PatternDefinitionEdit::SetMazeSeed {
            output_layer_id,
            seed,
        } => apply_maze_program(definition, *output_layer_id, |current| current.seed = *seed),
        PatternDefinitionEdit::SetGuideFaceDimensions {
            output_layer_id,
            dimensions,
        } => {
            if let Some(PatternOutputLayer::Regions {
                source:
                    RegionSourceIntent::GuideFaces {
                        dimensions: current,
                        ..
                    },
                ..
            }) = definition
                .output_layers
                .iter_mut()
                .find(|layer| layer.id() == *output_layer_id)
            {
                *current = dimensions.clone();
            }
        }
    }
}

/// Applies one validated connection-program mutation without altering the output's site mechanism.
///
/// The helper intentionally ignores absent or non-connection targets because validation rejects
/// them before candidate publication; it never introduces a new output variant.
fn apply_connection_program(
    definition: &mut PatternDefinition,
    output_layer_id: PatternOutputLayerId,
    mutate: impl FnOnce(&mut ConnectionProgram),
) {
    if let Some(PatternOutputLayer::ConnectionPaths { program, .. }) = definition
        .output_layers
        .iter_mut()
        .find(|layer| layer.id() == output_layer_id)
    {
        mutate(program);
    }
}

/// Applies one validated maze-program mutation without changing its output or site mechanism.
fn apply_maze_program(
    definition: &mut PatternDefinition,
    output_layer_id: PatternOutputLayerId,
    mutate: impl FnOnce(&mut MazeProgram),
) {
    if let Some(PatternOutputLayer::MazeWalls { program, .. }) = definition
        .output_layers
        .iter_mut()
        .find(|layer| layer.id() == output_layer_id)
    {
        mutate(program);
    }
}

/// Applies one active circular-arc payload leaf without inventing an inactive prototype.
///
/// The caller has already validated the target and field applicability, so a missing or inactive
/// payload remains unchanged here; candidate validation remains the authoritative publication gate.
fn apply_guide_arc_scalar(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
    value: f64,
    mutate: impl FnOnce((&mut AuthoredPoint2, &mut f64, &mut f64, &mut f64), f64),
) {
    if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
        .mechanisms
        .iter_mut()
        .find(|mechanism| mechanism.id() == mechanism_id)
        && let Some(GuideDimension {
            prototype:
                GuidePrototype::CircularArc {
                    center,
                    radius,
                    start_angle_degrees,
                    sweep_angle_degrees,
                },
            ..
        }) = dimensions
            .iter_mut()
            .find(|dimension| dimension.id == dimension_id)
    {
        mutate(
            (center, radius, start_angle_degrees, sweep_angle_degrees),
            value,
        );
    }
}

/// Mutates one validated parametric spiral source without deriving geometry or changing its repetition.
fn apply_parametric_spiral(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    mutate: impl FnOnce(&mut SpiralCurve),
) {
    if let Some(PatternMechanism::ParametricCurveSource {
        curve: ParametricCurve::Spiral(spiral),
        ..
    }) = definition
        .mechanisms
        .iter_mut()
        .find(|item| item.id() == mechanism_id)
    {
        mutate(spiral);
    }
}

/// Mutates one validated active parametric transform-stack payload without switching repetition topology.
fn apply_parametric_stack(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    mutate: impl FnOnce(&mut f64, &mut f64, f64),
    value: f64,
) {
    if let Some(PatternMechanism::ParametricCurveSource {
        repetition:
            CurveRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            },
        ..
    }) = definition
        .mechanisms
        .iter_mut()
        .find(|item| item.id() == mechanism_id)
    {
        mutate(direction_degrees, spacing_multiplier, value);
    }
}

/// Mutates one validated active parametric normal-offset payload without switching repetition topology.
fn apply_parametric_offset(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    mutate: impl FnOnce(&mut f64, &mut OffsetSides, &mut OffsetCleanup),
) {
    if let Some(PatternMechanism::ParametricCurveSource {
        repetition:
            CurveRepetition::NormalOffset {
                spacing,
                sides,
                cleanup,
            },
        ..
    }) = definition
        .mechanisms
        .iter_mut()
        .find(|item| item.id() == mechanism_id)
    {
        mutate(spacing, sides, cleanup);
    }
}

fn apply_guide_stack_scalar(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
    value: f64,
    mutate: impl FnOnce(&mut f64, &mut f64, f64),
) {
    if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
        .mechanisms
        .iter_mut()
        .find(|mechanism| mechanism.id() == mechanism_id)
        && let Some(GuideDimension {
            repetition:
                GuideRepetition::TransformStack {
                    direction_degrees,
                    spacing_multiplier,
                },
            ..
        }) = dimensions
            .iter_mut()
            .find(|dimension| dimension.id == dimension_id)
    {
        mutate(direction_degrees, spacing_multiplier, value);
    }
}

/// Applies one active normal-offset payload leaf without creating dormant repetition data.
///
/// The caller validates that the target is a generic normal-offset repetition before this local
/// mutation; inactive or missing targets remain unchanged for the candidate validation boundary.
fn apply_guide_offset_payload(
    definition: &mut PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
    mutate: impl FnOnce(&mut f64, &mut OffsetSides, &mut OffsetCleanup),
) {
    if let Some(PatternMechanism::GuideDimensions { dimensions, .. }) = definition
        .mechanisms
        .iter_mut()
        .find(|mechanism| mechanism.id() == mechanism_id)
        && let Some(GuideDimension {
            repetition:
                GuideRepetition::NormalOffset {
                    spacing,
                    sides,
                    cleanup,
                },
            ..
        }) = dimensions
            .iter_mut()
            .find(|dimension| dimension.id == dimension_id)
    {
        mutate(spacing, sides, cleanup);
    }
}

/// Remaps definition-owned IDs in one validated edit while retaining external resource references.
fn remap_definition_edit_for_duplicate(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> PatternDefinitionEdit {
    let mechanism = |id| remap_mechanism_id(source, duplicate, id);
    let dimension = |id| remap_dimension_id(source, duplicate, id);
    match edit {
        PatternDefinitionEdit::SetParametricShape {
            mechanism_id,
            shape,
        } => PatternDefinitionEdit::SetParametricShape {
            mechanism_id: mechanism(*mechanism_id),
            shape: *shape,
        },
        PatternDefinitionEdit::SetParametricTurns {
            mechanism_id,
            turns,
        } => PatternDefinitionEdit::SetParametricTurns {
            mechanism_id: mechanism(*mechanism_id),
            turns: *turns,
        },
        PatternDefinitionEdit::SetParametricRadialSpacing {
            mechanism_id,
            radial_spacing,
        } => PatternDefinitionEdit::SetParametricRadialSpacing {
            mechanism_id: mechanism(*mechanism_id),
            radial_spacing: *radial_spacing,
        },
        PatternDefinitionEdit::SetParametricPhase {
            mechanism_id,
            phase_degrees,
        } => PatternDefinitionEdit::SetParametricPhase {
            mechanism_id: mechanism(*mechanism_id),
            phase_degrees: *phase_degrees,
        },
        PatternDefinitionEdit::SetParametricWinding {
            mechanism_id,
            winding,
        } => PatternDefinitionEdit::SetParametricWinding {
            mechanism_id: mechanism(*mechanism_id),
            winding: *winding,
        },
        PatternDefinitionEdit::SetParametricRepetition {
            mechanism_id,
            repetition,
        } => PatternDefinitionEdit::SetParametricRepetition {
            mechanism_id: mechanism(*mechanism_id),
            repetition: repetition.clone(),
        },
        PatternDefinitionEdit::SetParametricOffsetSpacing {
            mechanism_id,
            spacing,
        } => PatternDefinitionEdit::SetParametricOffsetSpacing {
            mechanism_id: mechanism(*mechanism_id),
            spacing: *spacing,
        },
        PatternDefinitionEdit::SetParametricOffsetSides {
            mechanism_id,
            sides,
        } => PatternDefinitionEdit::SetParametricOffsetSides {
            mechanism_id: mechanism(*mechanism_id),
            sides: *sides,
        },
        PatternDefinitionEdit::SetParametricOffsetCleanup {
            mechanism_id,
            cleanup,
        } => PatternDefinitionEdit::SetParametricOffsetCleanup {
            mechanism_id: mechanism(*mechanism_id),
            cleanup: *cleanup,
        },
        PatternDefinitionEdit::SetParametricStackDirection {
            mechanism_id,
            value,
        } => PatternDefinitionEdit::SetParametricStackDirection {
            mechanism_id: mechanism(*mechanism_id),
            value: *value,
        },
        PatternDefinitionEdit::SetParametricStackSpacingMultiplier {
            mechanism_id,
            value,
        } => PatternDefinitionEdit::SetParametricStackSpacingMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            value: *value,
        },
        PatternDefinitionEdit::SetAlongParametricInterval {
            mechanism_id,
            interval,
        } => PatternDefinitionEdit::SetAlongParametricInterval {
            mechanism_id: mechanism(*mechanism_id),
            interval: *interval,
        },
        PatternDefinitionEdit::SetAlongParametricPhase {
            mechanism_id,
            phase,
        } => PatternDefinitionEdit::SetAlongParametricPhase {
            mechanism_id: mechanism(*mechanism_id),
            phase: *phase,
        },
        PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps } => {
            PatternDefinitionEdit::SetCoverageGuardSteps {
                guard_steps: *guard_steps,
            }
        }
        PatternDefinitionEdit::SetCoverageAdditionalMargin { additional_margin } => {
            PatternDefinitionEdit::SetCoverageAdditionalMargin {
                additional_margin: *additional_margin,
            }
        }
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            baseline_angle_degrees: *baseline_angle_degrees,
        },
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            phase: *phase,
        },
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            spacing_multiplier: *spacing_multiplier,
        },
        PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id,
            dimension_id,
            prototype,
        } => PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            prototype: prototype.clone(),
        },
        PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id,
            dimension_id,
            structure_id,
        } => PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            structure_id: *structure_id,
        },
        PatternDefinitionEdit::SetGuideArcCenterX {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideArcCenterX {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideArcCenterY {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideArcCenterY {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideArcRadius {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideArcRadius {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideArcStartAngle {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideArcStartAngle {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideArcSweepAngle {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideArcSweepAngle {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id,
            dimension_id,
            repetition,
        } => PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            repetition: repetition.clone(),
        },
        PatternDefinitionEdit::SetGuideOffsetSpacing {
            mechanism_id,
            dimension_id,
            spacing,
        } => PatternDefinitionEdit::SetGuideOffsetSpacing {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            spacing: *spacing,
        },
        PatternDefinitionEdit::SetGuideOffsetSides {
            mechanism_id,
            dimension_id,
            sides,
        } => PatternDefinitionEdit::SetGuideOffsetSides {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            sides: *sides,
        },
        PatternDefinitionEdit::SetGuideOffsetCleanup {
            mechanism_id,
            dimension_id,
            cleanup,
        } => PatternDefinitionEdit::SetGuideOffsetCleanup {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            cleanup: *cleanup,
        },
        PatternDefinitionEdit::SetGuideStackDirection {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideStackDirection {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetGuideStackSpacingMultiplier {
            mechanism_id,
            dimension_id,
            value,
        } => PatternDefinitionEdit::SetGuideStackSpacingMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            dimension_id: dimension(*dimension_id),
            value: *value,
        },
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id: mechanism(*mechanism_id),
            dimensions: dimensions.iter().copied().map(dimension).collect(),
        },
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id: mechanism(*mechanism_id),
            merge_epsilon: *merge_epsilon,
        },
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id: mechanism(*mechanism_id),
            dimensions: dimensions.iter().copied().map(dimension).collect(),
        },
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id: mechanism(*mechanism_id),
            interval_multiplier: *interval_multiplier,
        },
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: mechanism(*mechanism_id),
            phase: *phase,
        },
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id: mechanism(*mechanism_id),
            character: character.clone(),
        },
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, seed } => {
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: mechanism(*mechanism_id),
                seed: *seed,
            }
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: mechanism(*mechanism_id),
            minimum_center_distance: *minimum_center_distance,
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id: mechanism(*mechanism_id),
            cluster_density: *cluster_density,
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id: mechanism(*mechanism_id),
            cluster_spread: *cluster_spread,
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id: mechanism(*mechanism_id),
            cluster_strength: *cluster_strength,
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id: mechanism(*mechanism_id),
            modulation: modulation.clone(),
        },
        PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id,
            component,
        } => PatternDefinitionEdit::SetArtworkWeightMappingComponent {
            mechanism_id: mechanism(*mechanism_id),
            component: *component,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id,
            placement,
        } => PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id: mechanism(*mechanism_id),
            placement: *placement,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id,
            inverted,
        } => PatternDefinitionEdit::SetArtworkWeightMappingInverted {
            mechanism_id: mechanism(*mechanism_id),
            inverted: *inverted,
        },
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: mechanism(*mechanism_id),
                gain: *gain,
            }
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: mechanism(*mechanism_id),
                bias: *bias,
            }
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id: mechanism(*mechanism_id),
            strength: *strength,
        },
        PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id,
            response,
        } => PatternDefinitionEdit::SetArtworkWeightResponse {
            mechanism_id: mechanism(*mechanism_id),
            response: *response,
        },
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id: mechanism(*mechanism_id),
            policy: policy.clone(),
        },
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: mechanism(*mechanism_id),
            minimum_center_distance: *minimum_center_distance,
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: mechanism(*mechanism_id),
            margin: *margin,
        },
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id,
            sizing,
        } => PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
            mechanism_id: mechanism(*mechanism_id),
            sizing: *sizing,
        },
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id: mechanism(*mechanism_id),
            maximum_attempts: *maximum_attempts,
        },
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: mechanism(*mechanism_id),
            maximum_neighbor_checks: *maximum_neighbor_checks,
        },
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            site_mechanism_id: mechanism(*site_mechanism_id),
        },
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype,
        } => PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            prototype: prototype.clone(),
        },
        PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id,
            structure_id,
        } => PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            structure_id: *structure_id,
        },
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            orientation: remap_orientation(source, duplicate, orientation),
        },
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            dimension_id: dimension(*dimension_id),
        },
        PatternDefinitionEdit::SetConnectionProgram {
            output_layer_id,
            program,
        } => PatternDefinitionEdit::SetConnectionProgram {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            program: program.clone(),
        },
        PatternDefinitionEdit::SetConnectionMaximumDegree {
            output_layer_id,
            maximum_degree,
        } => PatternDefinitionEdit::SetConnectionMaximumDegree {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            maximum_degree: *maximum_degree,
        },
        PatternDefinitionEdit::SetConnectionMaximumDistance {
            output_layer_id,
            maximum_distance,
        } => PatternDefinitionEdit::SetConnectionMaximumDistance {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            maximum_distance: *maximum_distance,
        },
        PatternDefinitionEdit::SetConnectionMinimumDegree {
            output_layer_id,
            minimum_degree,
        } => PatternDefinitionEdit::SetConnectionMinimumDegree {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            minimum_degree: *minimum_degree,
        },
        PatternDefinitionEdit::SetConnectionSeed {
            output_layer_id,
            seed,
        } => PatternDefinitionEdit::SetConnectionSeed {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            seed: *seed,
        },
        PatternDefinitionEdit::SetMazeSeed {
            output_layer_id,
            seed,
        } => PatternDefinitionEdit::SetMazeSeed {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            seed: *seed,
        },
        PatternDefinitionEdit::SetGuideFaceDimensions {
            output_layer_id,
            dimensions,
        } => PatternDefinitionEdit::SetGuideFaceDimensions {
            output_layer_id: remap_output_layer_id(source, duplicate, *output_layer_id),
            dimensions: dimensions
                .iter()
                .map(|dimension_id| remap_dimension_id(source, duplicate, *dimension_id))
                .collect(),
        },
    }
}

fn remap_mechanism_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> PatternMechanismId {
    let index = source
        .mechanisms
        .iter()
        .position(|mechanism| mechanism.id() == mechanism_id)
        .expect("validated edit references a source mechanism");
    duplicate.mechanisms[index].id()
}

/// Remaps a selected-copy guide dimension to its duplicate definition without changing topology.
///
/// This helper accepts both straight and generic guide mechanisms because selected-copy edits
/// preserve the source mechanism family while allocating fresh dimension identities.
///
/// # Panics
///
/// Panics only when a caller violates the validated selected-copy invariant by naming a dimension
/// that is absent from both supported source mechanism families.
fn remap_dimension_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    dimension_id: GuideDimensionId,
) -> GuideDimensionId {
    for (source_mechanism, duplicate_mechanism) in
        source.mechanisms.iter().zip(&duplicate.mechanisms)
    {
        match (source_mechanism, duplicate_mechanism) {
            (
                PatternMechanism::StraightGuideDimensions {
                    dimensions: source_dimensions,
                    ..
                },
                PatternMechanism::StraightGuideDimensions {
                    dimensions: duplicate_dimensions,
                    ..
                },
            ) => {
                if let Some(index) = source_dimensions
                    .iter()
                    .position(|dimension| dimension.id == dimension_id)
                {
                    return duplicate_dimensions[index].id;
                }
            }
            (
                PatternMechanism::GuideDimensions {
                    dimensions: source_dimensions,
                    ..
                },
                PatternMechanism::GuideDimensions {
                    dimensions: duplicate_dimensions,
                    ..
                },
            ) => {
                if let Some(index) = source_dimensions
                    .iter()
                    .position(|dimension| dimension.id == dimension_id)
                {
                    return duplicate_dimensions[index].id;
                }
            }
            _ => {}
        }
    }
    panic!("validated edit references a source guide dimension")
}

fn remap_output_layer_id(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> PatternOutputLayerId {
    let index = source
        .output_layers
        .iter()
        .position(|layer| layer.id() == output_layer_id)
        .expect("validated edit references a source output layer");
    duplicate.output_layers[index].id()
}

fn remap_orientation(
    source: &PatternDefinition,
    duplicate: &PatternDefinition,
    orientation: &MarkOrientation,
) -> MarkOrientation {
    match orientation {
        MarkOrientation::Fixed => MarkOrientation::Fixed,
        MarkOrientation::GuideTangent { dimension_id } => MarkOrientation::GuideTangent {
            dimension_id: remap_dimension_id(source, duplicate, *dimension_id),
        },
        MarkOrientation::GuideNormal { dimension_id } => MarkOrientation::GuideNormal {
            dimension_id: remap_dimension_id(source, duplicate, *dimension_id),
        },
    }
}

/// Validates one structural edit against the active typed variant without mutating its definition.
///
/// # Errors
///
/// Returns a stable target, inactive-field, bound, or reference diagnostic before candidate edit.
fn validate_definition_edit(
    definition: &PatternDefinition,
    edit: &PatternDefinitionEdit,
) -> Result<(), ValidationError> {
    if !matches!(edit, PatternDefinitionEdit::SetGuideFaceDimensions { .. }) {
        validate_property_field_projection(edit.field_projection())?;
    }
    match edit {
        PatternDefinitionEdit::SetParametricShape { mechanism_id, .. }
        | PatternDefinitionEdit::SetParametricWinding { mechanism_id, .. } => {
            validate_parametric_source_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetParametricTurns {
            mechanism_id,
            turns,
        } => {
            validate_parametric_source_target(definition, *mechanism_id)?;
            if !turns.is_finite() || *turns <= 0.0 || *turns > 1024.0 {
                return Err(ValidationError::new(
                    "pattern_definitions.mechanisms.parametric.turns",
                    "spiral turns must be finite and in (0, 1024]",
                ));
            }
            Ok(())
        }
        PatternDefinitionEdit::SetParametricRadialSpacing {
            mechanism_id,
            radial_spacing,
        } => {
            validate_parametric_source_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *radial_spacing,
                "pattern_definitions.mechanisms.parametric.radial_spacing",
            )
        }
        PatternDefinitionEdit::SetParametricPhase {
            mechanism_id,
            phase_degrees,
        } => {
            validate_parametric_source_target(definition, *mechanism_id)?;
            validate_finite(
                *phase_degrees,
                "pattern_definitions.mechanisms.parametric.phase_degrees",
            )
        }
        PatternDefinitionEdit::SetParametricRepetition {
            mechanism_id,
            repetition,
        } => {
            validate_parametric_source_target(definition, *mechanism_id)?;
            validate_guide_repetition(repetition)
        }
        PatternDefinitionEdit::SetParametricOffsetSpacing {
            mechanism_id,
            spacing,
        } => {
            validate_active_parametric_offset_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *spacing,
                "pattern_definitions.mechanisms.parametric.repetition.offset.spacing",
            )
        }
        PatternDefinitionEdit::SetParametricOffsetSides { mechanism_id, .. }
        | PatternDefinitionEdit::SetParametricOffsetCleanup { mechanism_id, .. } => {
            validate_active_parametric_offset_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetParametricStackDirection {
            mechanism_id,
            value,
        } => {
            validate_active_parametric_stack_target(definition, *mechanism_id)?;
            validate_finite(
                *value,
                "pattern_definitions.mechanisms.parametric.repetition.direction",
            )
        }
        PatternDefinitionEdit::SetParametricStackSpacingMultiplier {
            mechanism_id,
            value,
        } => {
            validate_active_parametric_stack_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *value,
                "pattern_definitions.mechanisms.parametric.repetition.spacing_multiplier",
            )
        }
        PatternDefinitionEdit::SetAlongParametricInterval {
            mechanism_id,
            interval,
        } => {
            validate_along_parametric_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *interval,
                "pattern_definitions.mechanisms.along_parametric.interval",
            )
        }
        PatternDefinitionEdit::SetAlongParametricPhase {
            mechanism_id,
            phase,
        } => {
            validate_along_parametric_target(definition, *mechanism_id)?;
            validate_finite(
                *phase,
                "pattern_definitions.mechanisms.along_parametric.phase",
            )
        }
        PatternDefinitionEdit::SetCoverageGuardSteps { .. } => Ok(()),
        PatternDefinitionEdit::SetCoverageAdditionalMargin { additional_margin } => {
            validate_nonnegative_finite(
                *additional_margin,
                "pattern_definitions.coverage.additional_margin",
            )
        }
        PatternDefinitionEdit::SetGuideBaselineAngle {
            mechanism_id,
            dimension_id,
            baseline_angle_degrees,
        } => {
            validate_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(
                *baseline_angle_degrees,
                "pattern_definitions.mechanisms.dimensions.baseline_angle_degrees",
            )
        }
        PatternDefinitionEdit::SetGuidePrototype {
            mechanism_id,
            dimension_id,
            prototype,
        } => {
            validate_generic_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_guide_prototype(prototype)
        }
        PatternDefinitionEdit::SetGuideAuthoredStructure {
            mechanism_id,
            dimension_id,
            ..
        } => {
            match &validate_generic_guide_dimension_target(
                definition,
                *mechanism_id,
                *dimension_id,
            )?
            .prototype
            {
                GuidePrototype::AuthoredOpenPath { .. } => Ok(()),
                _ => Err(ValidationError::new(
                    "pattern_definitions.mechanisms.guide_prototype.reference",
                    "field is inactive for the current guide prototype",
                )),
            }
        }
        PatternDefinitionEdit::SetGuideArcCenterX {
            mechanism_id,
            dimension_id,
            value,
        }
        | PatternDefinitionEdit::SetGuideArcCenterY {
            mechanism_id,
            dimension_id,
            value,
        } => {
            validate_active_arc_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(
                *value,
                "pattern_definitions.mechanisms.guide_prototype.arc.center",
            )
        }
        PatternDefinitionEdit::SetGuideArcRadius {
            mechanism_id,
            dimension_id,
            value,
        } => {
            validate_active_arc_target(definition, *mechanism_id, *dimension_id)?;
            validate_positive_finite(
                *value,
                "pattern_definitions.mechanisms.guide_prototype.arc.radius",
            )
        }
        PatternDefinitionEdit::SetGuideArcStartAngle {
            mechanism_id,
            dimension_id,
            value,
        }
        | PatternDefinitionEdit::SetGuideArcSweepAngle {
            mechanism_id,
            dimension_id,
            value,
        } => {
            validate_active_arc_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(
                *value,
                "pattern_definitions.mechanisms.guide_prototype.arc.angles",
            )
        }
        PatternDefinitionEdit::SetGuideRepetition {
            mechanism_id,
            dimension_id,
            repetition,
        } => {
            validate_generic_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_guide_repetition(repetition)
        }
        PatternDefinitionEdit::SetGuideOffsetSpacing {
            mechanism_id,
            dimension_id,
            spacing,
        } => {
            validate_active_offset_target(definition, *mechanism_id, *dimension_id)?;
            validate_positive_finite(
                *spacing,
                "pattern_definitions.mechanisms.guide_repetition.offset.spacing",
            )
        }
        PatternDefinitionEdit::SetGuideOffsetSides {
            mechanism_id,
            dimension_id,
            ..
        }
        | PatternDefinitionEdit::SetGuideOffsetCleanup {
            mechanism_id,
            dimension_id,
            ..
        } => validate_active_offset_target(definition, *mechanism_id, *dimension_id).map(|_| ()),
        PatternDefinitionEdit::SetGuideStackDirection {
            mechanism_id,
            dimension_id,
            value,
        } => {
            validate_active_stack_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(
                *value,
                "pattern_definitions.mechanisms.guide_repetition.direction",
            )
        }
        PatternDefinitionEdit::SetGuideStackSpacingMultiplier {
            mechanism_id,
            dimension_id,
            value,
        } => {
            validate_active_stack_target(definition, *mechanism_id, *dimension_id)?;
            validate_positive_finite(
                *value,
                "pattern_definitions.mechanisms.guide_repetition.spacing_multiplier",
            )
        }
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id,
            dimension_id,
            phase,
        } => {
            validate_guide_dimension_target(definition, *mechanism_id, *dimension_id)?;
            validate_finite(*phase, "pattern_definitions.mechanisms.dimensions.phase")
        }
        PatternDefinitionEdit::SetGuideSpacingMultiplier {
            mechanism_id,
            dimension_id,
            spacing_multiplier,
        } => {
            match definition
                .mechanisms
                .iter()
                .find(|mechanism| mechanism.id() == *mechanism_id)
            {
                Some(PatternMechanism::StraightGuideDimensions { dimensions, .. })
                    if dimensions
                        .iter()
                        .any(|dimension| dimension.id == *dimension_id) => {}
                Some(PatternMechanism::StraightGuideDimensions { .. }) => {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.dimensions.id",
                        "command targets a missing guide dimension",
                    ));
                }
                Some(_) => {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.dimensions",
                        "field is inactive for the current guide repetition",
                    ));
                }
                None => {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.id",
                        "command targets a missing guide mechanism",
                    ));
                }
            }
            validate_positive_finite(
                *spacing_multiplier,
                "pattern_definitions.mechanisms.dimensions.repetition.spacing_multiplier",
            )
        }
        PatternDefinitionEdit::SetIntersectionDimensions {
            mechanism_id,
            dimensions,
        } => {
            let available = validate_selected_intersection_target(definition, *mechanism_id)?;
            validate_selection(
                dimensions,
                available,
                2,
                "pattern_definitions.mechanisms.intersections.dimensions",
            )
        }
        PatternDefinitionEdit::SetIntersectionMergeEpsilon {
            mechanism_id,
            merge_epsilon,
        } => {
            validate_selected_intersection_target(definition, *mechanism_id)?;
            validate_nonnegative_finite(
                *merge_epsilon,
                "pattern_definitions.mechanisms.intersections.merge_epsilon",
            )
        }
        PatternDefinitionEdit::SetAlongGuideDimensions {
            mechanism_id,
            dimensions,
        } => {
            let available = validate_along_guide_target(definition, *mechanism_id)?;
            validate_selection(
                dimensions,
                available,
                1,
                "pattern_definitions.mechanisms.along_guides.dimensions",
            )
        }
        PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
            mechanism_id,
            interval_multiplier,
        } => {
            validate_along_guide_target(definition, *mechanism_id)?;
            validate_positive_finite(
                *interval_multiplier,
                "pattern_definitions.mechanisms.along_guides.interval_multiplier",
            )
        }
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id,
            phase,
        } => {
            validate_along_guide_target(definition, *mechanism_id)?;
            validate_finite(*phase, "pattern_definitions.mechanisms.along_guides.phase")
        }
        PatternDefinitionEdit::SetRandomCharacter {
            mechanism_id,
            character,
        } => {
            validate_random_process_target(definition, *mechanism_id)?;
            validate_random_character(character)
        }
        PatternDefinitionEdit::SetRandomSeed { mechanism_id, .. } => {
            validate_random_process_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Even { .. } => validate_positive_finite(
                *minimum_center_distance,
                "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterDensity {
            mechanism_id,
            cluster_density,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_positive_finite(
                *cluster_density,
                "pattern_definitions.mechanisms.random_sites.cluster_density",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_density",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterSpread {
            mechanism_id,
            cluster_spread,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_positive_finite(
                *cluster_spread,
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetRandomClusterStrength {
            mechanism_id,
            cluster_strength,
        } => match validate_random_process_target(definition, *mechanism_id)? {
            RandomSiteCharacter::Clustered { .. } => validate_unit_component(
                *cluster_strength,
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
                "field is inactive for the current random character",
            )),
        },
        PatternDefinitionEdit::SetDensityModulationVariant {
            mechanism_id,
            modulation,
        } => {
            validate_density_modulation_target(definition, *mechanism_id)?;
            validate_site_density_modulation(modulation)
        }
        PatternDefinitionEdit::SetArtworkWeightMappingComponent { mechanism_id, .. }
        | PatternDefinitionEdit::SetArtworkWeightMappingPlacement { mechanism_id, .. }
        | PatternDefinitionEdit::SetArtworkWeightMappingInverted { mechanism_id, .. } => {
            validate_artwork_weighted_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetArtworkWeightMappingGain { mechanism_id, gain } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_nonnegative_finite(
                *gain,
                "pattern_definitions.mechanisms.site_density.mapping.gain",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightMappingBias { mechanism_id, bias } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_finite(
                *bias,
                "pattern_definitions.mechanisms.site_density.mapping.bias",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightStrength {
            mechanism_id,
            strength,
        } => {
            validate_artwork_weighted_target(definition, *mechanism_id)?;
            validate_unit_component(
                *strength,
                "pattern_definitions.mechanisms.site_density.strength",
            )
        }
        PatternDefinitionEdit::SetArtworkWeightResponse { mechanism_id, .. } => {
            validate_artwork_weighted_target(definition, *mechanism_id).map(|_| ())
        }
        PatternDefinitionEdit::SetExclusionVariant {
            mechanism_id,
            policy,
        } => {
            validate_exclusion_target(definition, *mechanism_id)?;
            validate_site_exclusion(policy)
        }
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id,
            minimum_center_distance,
        } => match validate_exclusion_target(definition, *mechanism_id)? {
            SiteExclusionPolicy::MinimumCenterDistance { .. } => validate_positive_finite(
                *minimum_center_distance,
                "pattern_definitions.mechanisms.site_exclusion.minimum",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.site_exclusion.minimum",
                "field is inactive for the current exclusion policy",
            )),
        },
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id,
            margin,
        } => match validate_exclusion_target(definition, *mechanism_id)? {
            SiteExclusionPolicy::VisibleMarkMargin { .. } => validate_nonnegative_finite(
                *margin,
                "pattern_definitions.mechanisms.site_exclusion.margin",
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.mechanisms.site_exclusion.margin",
                "field is inactive for the current exclusion policy",
            )),
        },
        PatternDefinitionEdit::SetVisibleMarkSizingPolicy { mechanism_id, .. } => {
            match validate_exclusion_target(definition, *mechanism_id)? {
                SiteExclusionPolicy::VisibleMarkMargin { .. } => Ok(()),
                _ => Err(ValidationError::new(
                    "pattern_definitions.mechanisms.site_exclusion.sizing",
                    "field is inactive for the current exclusion policy",
                )),
            }
        }
        PatternDefinitionEdit::SetRandomMaximumAttempts {
            mechanism_id,
            maximum_attempts,
        } => {
            validate_random_product_target(definition, *mechanism_id)?;
            if *maximum_attempts > 0 {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.mechanisms.random_sites.maximum_attempts",
                    "random product work limits must be nonzero",
                ))
            }
        }
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id,
            maximum_neighbor_checks,
        } => {
            validate_random_product_target(definition, *mechanism_id)?;
            if *maximum_neighbor_checks > 0 {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.mechanisms.random_sites.maximum_neighbor_checks",
                    "random product work limits must be nonzero",
                ))
            }
        }
        PatternDefinitionEdit::SetOutputSiteProduct {
            output_layer_id,
            site_mechanism_id,
        } => {
            validate_output_layer_target(definition, *output_layer_id)?;
            if definition
                .mechanisms
                .iter()
                .any(|mechanism| mechanism.id() == *site_mechanism_id)
            {
                Ok(())
            } else {
                Err(ValidationError::new(
                    "pattern_definitions.output_layers.site_mechanism_id",
                    "command references a missing site mechanism",
                ))
            }
        }
        PatternDefinitionEdit::SetOutputMarkPrototype {
            output_layer_id,
            prototype: _,
        } => {
            validate_output_layer_target(definition, *output_layer_id)?;
            Ok(())
        }
        PatternDefinitionEdit::SetOutputAuthoredClosedShape {
            output_layer_id, ..
        } => match validate_mark_prototype_output_target(definition, *output_layer_id)? {
            PatternOutputLayer::MarkPrototype {
                prototype: MarkPrototype::AuthoredClosedShape { .. },
                ..
            } => Ok(()),
            _ => Err(ValidationError::new(
                "pattern_definitions.output_layers.prototype.reference",
                "field is inactive for the current mark prototype",
            )),
        },
        PatternDefinitionEdit::SetOutputOrientation {
            output_layer_id,
            orientation,
        } => {
            validate_mark_prototype_output_target(definition, *output_layer_id)?;
            validate_output_orientation(definition, orientation)
        }
        PatternDefinitionEdit::SetOutputOrientationDimension {
            output_layer_id,
            dimension_id,
        } => match validate_mark_prototype_output_target(definition, *output_layer_id)? {
            PatternOutputLayer::MarkPrototype {
                orientation:
                    MarkOrientation::GuideTangent { .. } | MarkOrientation::GuideNormal { .. },
                ..
            } => validate_output_orientation(
                definition,
                &MarkOrientation::GuideTangent {
                    dimension_id: *dimension_id,
                },
            ),
            _ => Err(ValidationError::new(
                "pattern_definitions.output_layers.orientation.dimension_id",
                "field is inactive for fixed output orientation",
            )),
        },
        PatternDefinitionEdit::SetConnectionProgram {
            output_layer_id,
            program,
        } => {
            validate_connection_output_target(definition, *output_layer_id)?;
            program.validate()
        }
        PatternDefinitionEdit::SetConnectionMaximumDegree {
            output_layer_id,
            maximum_degree,
        } => {
            let mut candidate =
                validate_connection_output_target(definition, *output_layer_id)?.clone();
            match &mut candidate {
                ConnectionProgram::NearestLinks { adjacency }
                | ConnectionProgram::RandomLinks { adjacency, .. }
                | ConnectionProgram::GridSpanningTree { adjacency, .. } => {
                    adjacency.maximum_degree = *maximum_degree;
                }
            }
            candidate.validate()
        }
        PatternDefinitionEdit::SetConnectionMaximumDistance {
            output_layer_id,
            maximum_distance,
        } => {
            let mut candidate =
                validate_connection_output_target(definition, *output_layer_id)?.clone();
            match &mut candidate {
                ConnectionProgram::NearestLinks { adjacency }
                | ConnectionProgram::RandomLinks { adjacency, .. }
                | ConnectionProgram::GridSpanningTree { adjacency, .. } => {
                    adjacency.maximum_distance = *maximum_distance;
                }
            }
            candidate.validate()
        }
        PatternDefinitionEdit::SetConnectionMinimumDegree {
            output_layer_id,
            minimum_degree,
        } => {
            let mut candidate =
                validate_connection_output_target(definition, *output_layer_id)?.clone();
            let ConnectionProgram::RandomLinks {
                minimum_degree: current,
                ..
            } = &mut candidate
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.output_layers.connection.minimum_degree",
                    "field is inactive for the current connection program",
                ));
            };
            *current = *minimum_degree;
            candidate.validate()
        }
        PatternDefinitionEdit::SetConnectionSeed {
            output_layer_id,
            seed,
        } => {
            let mut candidate =
                validate_connection_output_target(definition, *output_layer_id)?.clone();
            match &mut candidate {
                ConnectionProgram::RandomLinks { seed: current, .. }
                | ConnectionProgram::GridSpanningTree { seed: current, .. } => {
                    *current = *seed;
                    candidate.validate()
                }
                ConnectionProgram::NearestLinks { .. } => Err(ValidationError::new(
                    "pattern_definitions.output_layers.connection.seed",
                    "field is inactive for the current connection program",
                )),
            }
        }
        PatternDefinitionEdit::SetMazeSeed {
            output_layer_id,
            seed,
        } => {
            let mut candidate = validate_maze_output_target(definition, *output_layer_id)?.clone();
            candidate.seed = *seed;
            candidate.validate()
        }
        PatternDefinitionEdit::SetGuideFaceDimensions {
            output_layer_id,
            dimensions,
        } => {
            let PatternOutputLayer::Regions {
                source:
                    RegionSourceIntent::GuideFaces {
                        guide_mechanism_id, ..
                    },
                ..
            } = validate_output_layer_target(definition, *output_layer_id)?
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.output_layers.guide_faces",
                    "command targets an output without Guide Faces configuration",
                ));
            };
            let selected_dimensions = definition
                .mechanisms
                .iter()
                .find_map(|mechanism| match mechanism {
                    PatternMechanism::StraightGuideDimensions { id, dimensions }
                        if *id == *guide_mechanism_id =>
                    {
                        Some(
                            dimensions
                                .iter()
                                .map(|dimension| dimension.id)
                                .collect::<Vec<_>>(),
                        )
                    }
                    PatternMechanism::GuideDimensions { id, dimensions }
                        if *id == *guide_mechanism_id =>
                    {
                        Some(
                            dimensions
                                .iter()
                                .map(|dimension| dimension.id)
                                .collect::<Vec<_>>(),
                        )
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    ValidationError::new(
                        "pattern_definitions.output_layers.guide_faces.mechanism",
                        "Guide Faces output references a missing guide mechanism",
                    )
                })?;
            validate_region_source_ids(
                &RegionSourceIntent::GuideFaces {
                    guide_mechanism_id: *guide_mechanism_id,
                    dimensions: dimensions.clone(),
                },
                *guide_mechanism_id,
                PatternMechanismId(u64::MAX),
                &selected_dimensions,
            )
        }
    }
}

fn validate_property_field_projection(
    projection: PropertyCommandFieldProjection,
) -> Result<(), ValidationError> {
    let contract = property_field_contract(projection.field);
    let kind_matches = matches!(
        (contract.value_kind, projection.value),
        (
            PropertyValueKind::FiniteF64,
            PropertyFieldValue::FiniteF64(_)
        ) | (PropertyValueKind::U32, PropertyFieldValue::U32(_))
            | (PropertyValueKind::Boolean, PropertyFieldValue::Boolean(_))
            | (
                PropertyValueKind::StableIdReference,
                PropertyFieldValue::StableIdReference
            )
            | (
                PropertyValueKind::StableIdReference,
                PropertyFieldValue::StableIdReferenceCollection(_)
            )
            | (
                PropertyValueKind::EnumChoice,
                PropertyFieldValue::EnumChoice(_)
            )
    );
    if !kind_matches {
        return Err(ValidationError::new(
            "command.field",
            "command value does not match its field contract",
        ));
    }
    let value = match projection.value {
        PropertyFieldValue::FiniteF64(value) => {
            if !value.is_finite() {
                return Err(ValidationError::new(
                    "command.field",
                    "value must be finite",
                ));
            }
            value
        }
        PropertyFieldValue::U32(value) => value as f64,
        PropertyFieldValue::EnumChoice(choice) => {
            if !contract.choices.is_empty() && !contract.choices.contains(&choice) {
                return Err(ValidationError::new(
                    "command.field",
                    "value is not a legal field enum choice",
                ));
            }
            return Ok(());
        }
        PropertyFieldValue::StableIdReferenceCollection(count) => {
            let PropertyReferenceConstraint::OrderedUniqueCollection {
                minimum_items,
                maximum_items,
            } = contract.reference_constraint
            else {
                return Err(ValidationError::new(
                    "command.field",
                    "field does not accept a reference collection",
                ));
            };
            if count < minimum_items as usize || count > maximum_items as usize {
                return Err(ValidationError::new(
                    "command.field",
                    "reference collection violates field cardinality",
                ));
            }
            return Ok(());
        }
        PropertyFieldValue::Boolean(_) | PropertyFieldValue::StableIdReference => return Ok(()),
    };
    if let Some(bounds) = contract.bounds {
        if let Some(minimum) = bounds.minimum
            && (value < minimum || (!bounds.minimum_inclusive && value == minimum))
        {
            return Err(ValidationError::new(
                "command.field",
                "value is below field minimum",
            ));
        }
        if let Some(maximum) = bounds.maximum
            && (value > maximum || (!bounds.maximum_inclusive && value == maximum))
        {
            return Err(ValidationError::new(
                "command.field",
                "value is above field maximum",
            ));
        }
    }
    Ok(())
}

fn validate_guide_dimension_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<(), ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::StraightGuideDimensions { dimensions, .. })
            if dimensions
                .iter()
                .any(|dimension| dimension.id == dimension_id) =>
        {
            Ok(())
        }
        Some(PatternMechanism::StraightGuideDimensions { .. }) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions.id",
            "command targets a missing guide dimension",
        )),
        Some(PatternMechanism::GuideDimensions { dimensions, .. })
            if dimensions
                .iter()
                .any(|dimension| dimension.id == dimension_id) =>
        {
            Ok(())
        }
        Some(PatternMechanism::GuideDimensions { .. }) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_dimensions.id",
            "command targets a missing guide dimension",
        )),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions",
            "command targets an incompatible guide mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing guide mechanism",
        )),
    }
}

/// Returns the active generic guide dimension or a stable target/applicability diagnostic.
fn validate_generic_guide_dimension_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<&GuideDimension, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::GuideDimensions { dimensions, .. }) => dimensions
            .iter()
            .find(|dimension| dimension.id == dimension_id)
            .ok_or_else(|| {
                ValidationError::new(
                    "pattern_definitions.mechanisms.guide_dimensions.id",
                    "command targets a missing guide dimension",
                )
            }),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_dimensions",
            "command targets an incompatible generic guide mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing guide mechanism",
        )),
    }
}

/// Validates that one payload edit addresses an active circular-arc prototype.
fn validate_active_arc_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<(), ValidationError> {
    match validate_generic_guide_dimension_target(definition, mechanism_id, dimension_id)?.prototype
    {
        GuidePrototype::CircularArc { .. } => Ok(()),
        _ => Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_prototype.arc",
            "field is inactive for the current guide prototype",
        )),
    }
}

/// Validates that one payload edit addresses an active transform-stack repetition.
fn validate_active_stack_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<(), ValidationError> {
    match validate_generic_guide_dimension_target(definition, mechanism_id, dimension_id)?
        .repetition
    {
        GuideRepetition::TransformStack { .. } => Ok(()),
        _ => Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_repetition",
            "field is inactive for the current guide repetition",
        )),
    }
}

/// Validates that one payload edit addresses an active normal-offset repetition.
///
/// # Errors
///
/// Returns the generic-guide target diagnostic or a stable inactive-field error without mutation.
fn validate_active_offset_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
    dimension_id: GuideDimensionId,
) -> Result<(), ValidationError> {
    match validate_generic_guide_dimension_target(definition, mechanism_id, dimension_id)?
        .repetition
    {
        GuideRepetition::NormalOffset { .. } => Ok(()),
        _ => Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_repetition",
            "field is inactive for the current guide repetition",
        )),
    }
}

fn straight_guide_root_dimensions(
    definition: &PatternDefinition,
) -> Result<&[StraightGuideDimension], ValidationError> {
    match definition.mechanisms.first() {
        Some(PatternMechanism::StraightGuideDimensions { dimensions, .. }) => Ok(dimensions),
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "definition has no straight guide root",
        )),
    }
}

fn validate_selected_intersection_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&[StraightGuideDimension], ValidationError> {
    let available = straight_guide_root_dimensions(definition)?;
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SelectedGuideIntersections { .. }) => Ok(available),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible intersection mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing intersection mechanism",
        )),
    }
}

fn validate_along_guide_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&[StraightGuideDimension], ValidationError> {
    let available = straight_guide_root_dimensions(definition)?;
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::AlongGuideSites { .. }) => Ok(available),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible along-guide mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing along-guide mechanism",
        )),
    }
}

fn validate_random_process_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&RandomSiteCharacter, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::RandomSiteProcess { character, .. }) => Ok(character),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible random process",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing random process",
        )),
    }
}

fn validate_density_modulation_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SiteDensityModulation, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SiteDensityModulation { modulation, .. }) => Ok(modulation),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible density modulation",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing density modulation",
        )),
    }
}

fn validate_artwork_weighted_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SourceMapping, ValidationError> {
    match validate_density_modulation_target(definition, mechanism_id)? {
        SiteDensityModulation::ArtworkWeighted { mapping, .. } => Ok(mapping),
        SiteDensityModulation::Uniform => Err(ValidationError::new(
            "pattern_definitions.mechanisms.site_density",
            "field is inactive for uniform density modulation",
        )),
    }
}

fn validate_exclusion_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<&SiteExclusionPolicy, ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::SiteExclusion { policy, .. }) => Ok(policy),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible exclusion mechanism",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing exclusion mechanism",
        )),
    }
}

fn validate_random_product_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    match definition
        .mechanisms
        .iter()
        .find(|mechanism| mechanism.id() == mechanism_id)
    {
        Some(PatternMechanism::RandomSiteProduct { .. }) => Ok(()),
        Some(_) => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets an incompatible random product",
        )),
        None => Err(ValidationError::new(
            "pattern_definitions.mechanisms.id",
            "command targets a missing random product",
        )),
    }
}

fn validate_output_layer_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&PatternOutputLayer, ValidationError> {
    definition
        .output_layers
        .iter()
        .find(|layer| layer.id() == output_layer_id)
        .ok_or_else(|| {
            ValidationError::new(
                "pattern_definitions.output_layers.id",
                "command targets a missing output layer",
            )
        })
}

fn validate_mark_prototype_output_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&PatternOutputLayer, ValidationError> {
    match validate_output_layer_target(definition, output_layer_id)? {
        layer @ PatternOutputLayer::MarkPrototype { .. } => Ok(layer),
        PatternOutputLayer::CircularMarks { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets an output without a mark-prototype configuration",
        )),
        PatternOutputLayer::GuidePaths { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets a guide-path output without a mark-prototype configuration",
        )),
        PatternOutputLayer::ParametricPaths { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets a parametric-path output without a mark-prototype configuration",
        )),
        PatternOutputLayer::ConnectionPaths { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets a connection-path output without a mark-prototype configuration",
        )),
        PatternOutputLayer::MazeWalls { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets a maze-wall output without a mark-prototype configuration",
        )),
        PatternOutputLayer::Regions { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "command targets a region output without a mark-prototype configuration",
        )),
    }
}

/// Returns the authored connection program for one connection-path output target.
///
/// # Errors
///
/// Returns a stable missing-target or incompatible-output diagnostic without mutating the definition.
fn validate_connection_output_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&ConnectionProgram, ValidationError> {
    match validate_output_layer_target(definition, output_layer_id)? {
        PatternOutputLayer::ConnectionPaths { program, .. } => Ok(program),
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.connection",
            "command targets an output without a connection-program configuration",
        )),
    }
}

/// Returns the authored maze program only for a maze-wall output target.
///
/// # Errors
///
/// Returns a stable target diagnostic without mutating the definition.
fn validate_maze_output_target(
    definition: &PatternDefinition,
    output_layer_id: PatternOutputLayerId,
) -> Result<&MazeProgram, ValidationError> {
    match validate_output_layer_target(definition, output_layer_id)? {
        PatternOutputLayer::MazeWalls { program, .. } => Ok(program),
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.maze",
            "command targets an output without a maze-program configuration",
        )),
    }
}

fn validate_output_orientation(
    definition: &PatternDefinition,
    orientation: &MarkOrientation,
) -> Result<(), ValidationError> {
    match orientation {
        MarkOrientation::Fixed => Ok(()),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id }
            if definition
                .mechanisms
                .iter()
                .any(|mechanism| match mechanism {
                    PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                        .iter()
                        .any(|dimension| dimension.id == *dimension_id),
                    PatternMechanism::GuideDimensions { dimensions, .. } => dimensions
                        .iter()
                        .any(|dimension| dimension.id == *dimension_id),
                    _ => false,
                }) =>
        {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.orientation",
            "orientation is incompatible with definition dimensions",
        )),
    }
}

fn definition_edit_invalidation(edit: &PatternDefinitionEdit) -> InvalidationLevel {
    if matches!(edit, PatternDefinitionEdit::SetGuideFaceDimensions { .. }) {
        InvalidationLevel::Family
    } else {
        property_field_contract(edit.field_projection().field).invalidation
    }
}

fn affected_topology_channels(
    old_channel_ids: Vec<ChannelId>,
    new_topology: &ChannelTopology,
) -> Vec<ChannelId> {
    let mut affected = old_channel_ids;
    let introduced: Vec<_> = new_topology
        .channels
        .iter()
        .map(|channel| channel.id)
        .filter(|id| !affected.contains(id))
        .collect();
    affected.extend(introduced);
    affected
}

fn validate_channel(
    channel: &ChannelState,
    _definition: &PatternDefinition,
) -> Result<(), ValidationError> {
    validate_channel_pattern_instance(&channel.pattern_instance)?;
    validate_unit_component(channel.appearance.color.red, "channel.appearance.color.red")?;
    validate_unit_component(
        channel.appearance.color.green,
        "channel.appearance.color.green",
    )?;
    validate_unit_component(
        channel.appearance.color.blue,
        "channel.appearance.color.blue",
    )?;
    validate_unit_component(
        channel.appearance.color.alpha,
        "channel.appearance.color.alpha",
    )?;
    validate_unit_component(channel.appearance.opacity, "channel.appearance.opacity")?;
    Ok(())
}

/// Validates and projects one complete typed pattern definition through the shared current vocabulary.
///
/// # Errors
///
/// Returns the first stable family, mechanism, output, coverage, or payload diagnostic.
fn validate_definition(definition: &PatternDefinition) -> Result<(), ValidationError> {
    validate_and_project_definition(definition).map(|_| ())
}

/// Validates one typed recipe then derives its active structural workflow projection.
///
/// # Errors
///
/// Returns the established validation diagnostic before exposing any partial projection.
fn validate_and_project_definition(
    definition: &PatternDefinition,
) -> Result<PatternCapabilityProjection, ValidationError> {
    validate_definition_structure(definition)?;
    let settings = bundle_from_definition(definition.clone())
        .output_settings
        .into_iter()
        .map(|value| EffectivePatternOutputSettings {
            output_layer_id: value.output_layer_id,
            response: value.response,
        })
        .collect::<Vec<_>>();
    project_validated_pattern_definition(definition, &settings)
}

/// Validates one complete typed pattern definition and its ordered family/output capability chain.
/// The companion projection is derived only after this exhaustive structural validation succeeds.
///
/// # Errors
///
/// Returns the first stable family, mechanism, output, coverage, or payload diagnostic.
fn validate_definition_structure(definition: &PatternDefinition) -> Result<(), ValidationError> {
    validate_nonnegative_finite(
        definition.coverage.additional_margin,
        "pattern_definitions.coverage.additional_margin",
    )?;
    let mut mechanism_ids = HashSet::new();
    for mechanism in &definition.mechanisms {
        if !mechanism_ids.insert(mechanism.id()) {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms",
                "mechanism IDs must be unique and deterministically ordered",
            ));
        }
    }
    let mut output_ids = HashSet::new();
    for layer in &definition.output_layers {
        if !output_ids.insert(layer.id()) {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers",
                "output layer IDs must be unique and deterministically ordered",
            ));
        }
        if let PatternOutputLayer::ConnectionPaths { program, .. } = layer {
            program.validate()?;
        }
        if let PatternOutputLayer::MazeWalls { program, .. } = layer {
            program.validate()?;
        }
    }
    if let PatternFamily::RandomSites {
        base_site_process_id,
        density_modulation_id,
        exclusion_id,
        site_product_id,
    } = definition.family
    {
        return validate_random_site_definition(
            definition,
            base_site_process_id,
            density_modulation_id,
            exclusion_id,
            site_product_id,
        );
    }
    if let PatternFamily::ParametricCurve {
        curve_mechanism_id,
        site_mechanism_id,
    } = definition.family
    {
        return validate_parametric_curve_definition(
            definition,
            curve_mechanism_id,
            site_mechanism_id,
        );
    }
    let PatternFamily::GuideIntersections {
        guide_mechanism_id,
        site_mechanism_id: root_site_id,
    } = definition.family
    else {
        unreachable!("all pattern families are handled above");
    };
    if let [
        PatternMechanism::StraightGuideDimensions { id, dimensions },
        site,
    ] = definition.mechanisms.as_slice()
    {
        if *id != guide_mechanism_id {
            return Err(ValidationError::new(
                "pattern_definitions.family.guide_mechanism_id",
                "family root must reference the ordered straight-guide mechanism",
            ));
        }
        validate_straight_dimensions(dimensions)?;
        validate_site_mechanism(site, *id, root_site_id, dimensions)?;
        validate_generalized_output_layers(
            &definition.output_layers,
            *id,
            root_site_id,
            dimensions,
            site,
        )?;
        return Ok(());
    }
    if let [PatternMechanism::GuideDimensions { id, dimensions }, site] =
        definition.mechanisms.as_slice()
    {
        if *id != guide_mechanism_id {
            return Err(ValidationError::new(
                "pattern_definitions.family.guide_mechanism_id",
                "family root must reference the ordered guide-dimensions mechanism",
            ));
        }
        validate_guide_dimensions(dimensions)?;
        let ids: Vec<_> = dimensions.iter().map(|dimension| dimension.id).collect();
        validate_site_mechanism_ids(site, *id, root_site_id, &ids)?;
        validate_generalized_output_layers_ids(&definition.output_layers, *id, root_site_id, &ids)?;
        if let [
            PatternOutputLayer::Regions {
                source:
                    RegionSourceIntent::GuideFaces {
                        dimensions: selected,
                        ..
                    },
                ..
            },
        ] = definition.output_layers.as_slice()
            && selected.iter().any(|selected_id| {
                !dimensions.iter().any(|dimension| {
                    dimension.id == *selected_id
                        && matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. })
                })
            })
        {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers.guide_faces.prototype",
                "guide-face regions require selected authored open-path guides",
            ));
        }
        return Ok(());
    }
    if !matches!(definition.mechanisms.first(), Some(PatternMechanism::StraightGuides { id }) if *id == guide_mechanism_id)
        || !matches!(definition.mechanisms.get(1), Some(PatternMechanism::GuideIntersections { id, guide_mechanism_id: parent }) if *id == root_site_id && *parent == guide_mechanism_id)
        || definition.mechanisms.len() != 2
    {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "family root requires ordered straight-guide and intersection mechanisms",
        ));
    }
    let has_compatible_output = match definition.output_layers.as_slice() {
        [
            PatternOutputLayer::CircularMarks {
                site_mechanism_id, ..
            },
        ] => *site_mechanism_id == root_site_id,
        [
            PatternOutputLayer::MarkPrototype {
                site_mechanism_id,
                orientation: MarkOrientation::Fixed,
                ..
            },
        ] => *site_mechanism_id == root_site_id,
        [
            PatternOutputLayer::GuidePaths {
                guide_mechanism_id: output_guide_id,
                ..
            },
        ] => *output_guide_id == guide_mechanism_id,
        [
            PatternOutputLayer::ConnectionPaths {
                site_mechanism_id,
                program,
                ..
            },
        ] => *site_mechanism_id == root_site_id && program.validate().is_ok(),
        [
            PatternOutputLayer::MazeWalls {
                site_mechanism_id,
                program,
                ..
            },
        ] => *site_mechanism_id == root_site_id && program.validate().is_ok(),
        [PatternOutputLayer::Regions { source, .. }] => {
            source.site_mechanism_id() == Some(root_site_id)
        }
        _ => false,
    };
    if !has_compatible_output {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "ordered mark output requires the family intersection mechanism",
        ));
    }
    Ok(())
}

/// Validates the bounded Stage 20K parametric source chain and homogeneous output.
///
/// # Errors
///
/// Returns a stable structural, numeric, or output-authority diagnostic before installation.
/// Validates that an edit addresses an active analytic parametric source.
///
/// # Errors
///
/// Returns the stable missing-or-nonparametric source diagnostic without mutating a definition.
fn validate_parametric_source_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    matches!(
        definition
            .mechanisms
            .iter()
            .find(|item| item.id() == mechanism_id),
        Some(PatternMechanism::ParametricCurveSource { .. })
    )
    .then_some(())
    .ok_or_else(|| {
        ValidationError::new(
            "pattern_definitions.mechanisms.parametric",
            "command targets a missing parametric source",
        )
    })
}

/// Validates that an edit addresses the active transform-stack payload.
///
/// # Errors
///
/// Returns the stable inactive-repetition diagnostic without mutating a definition.
fn validate_active_parametric_stack_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    matches!(
        definition
            .mechanisms
            .iter()
            .find(|item| item.id() == mechanism_id),
        Some(PatternMechanism::ParametricCurveSource {
            repetition: CurveRepetition::TransformStack { .. },
            ..
        })
    )
    .then_some(())
    .ok_or_else(|| {
        ValidationError::new(
            "pattern_definitions.mechanisms.parametric.repetition",
            "field is inactive for the current parametric repetition",
        )
    })
}

/// Validates that an edit addresses the active normal-offset payload.
///
/// # Errors
///
/// Returns the stable inactive-repetition diagnostic without mutating a definition.
fn validate_active_parametric_offset_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    matches!(
        definition
            .mechanisms
            .iter()
            .find(|item| item.id() == mechanism_id),
        Some(PatternMechanism::ParametricCurveSource {
            repetition: CurveRepetition::NormalOffset { .. },
            ..
        })
    )
    .then_some(())
    .ok_or_else(|| {
        ValidationError::new(
            "pattern_definitions.mechanisms.parametric.repetition",
            "field is inactive for the current parametric repetition",
        )
    })
}

/// Validates that an edit addresses the active equal-arc parametric site product.
///
/// # Errors
///
/// Returns the stable missing-site-product diagnostic without mutating a definition.
fn validate_along_parametric_target(
    definition: &PatternDefinition,
    mechanism_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    matches!(
        definition
            .mechanisms
            .iter()
            .find(|item| item.id() == mechanism_id),
        Some(PatternMechanism::AlongParametricCurveSites { .. })
    )
    .then_some(())
    .ok_or_else(|| {
        ValidationError::new(
            "pattern_definitions.mechanisms.along_parametric",
            "command targets a missing parametric site product",
        )
    })
}

/// Validates one complete parametric source, optional site product, and homogeneous output chain.
///
/// # Errors
///
/// Returns structural, numeric, or output-authority diagnostics before a definition can publish.
fn validate_parametric_curve_definition(
    definition: &PatternDefinition,
    curve_id: PatternMechanismId,
    site_id: Option<PatternMechanismId>,
) -> Result<(), ValidationError> {
    let Some(PatternMechanism::ParametricCurveSource {
        id,
        curve,
        repetition,
    }) = definition.mechanisms.first()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "parametric family requires a source mechanism first",
        ));
    };
    if *id != curve_id {
        return Err(ValidationError::new(
            "pattern_definitions.family.curve_mechanism_id",
            "parametric family root must reference its source mechanism",
        ));
    }
    validate_parametric_curve(curve)?;
    validate_guide_repetition(repetition)?;
    match (
        site_id,
        definition.mechanisms.as_slice(),
        definition.output_layers.as_slice(),
    ) {
        (
            None,
            [PatternMechanism::ParametricCurveSource { .. }],
            [
                PatternOutputLayer::ParametricPaths {
                    curve_mechanism_id, ..
                },
            ],
        ) if *curve_mechanism_id == curve_id => Ok(()),
        (
            Some(site_id),
            [
                PatternMechanism::ParametricCurveSource { .. },
                PatternMechanism::AlongParametricCurveSites {
                    id,
                    curve_mechanism_id,
                    interval,
                    phase,
                },
            ],
            [
                PatternOutputLayer::CircularMarks {
                    site_mechanism_id, ..
                },
            ],
        ) if *id == site_id && *curve_mechanism_id == curve_id && *site_mechanism_id == site_id => {
            validate_positive_finite(
                *interval,
                "pattern_definitions.mechanisms.along_parametric.interval",
            )?;
            validate_finite(
                *phase,
                "pattern_definitions.mechanisms.along_parametric.phase",
            )
        }
        (
            Some(site_id),
            [
                PatternMechanism::ParametricCurveSource { .. },
                PatternMechanism::AlongParametricCurveSites {
                    id,
                    curve_mechanism_id,
                    interval,
                    phase,
                },
            ],
            [PatternOutputLayer::Regions { source, .. }],
        ) if *id == site_id
            && *curve_mechanism_id == curve_id
            && source.site_mechanism_id() == Some(site_id) =>
        {
            validate_positive_finite(
                *interval,
                "pattern_definitions.mechanisms.along_parametric.interval",
            )?;
            validate_finite(
                *phase,
                "pattern_definitions.mechanisms.along_parametric.phase",
            )
        }
        (
            Some(site_id),
            [
                PatternMechanism::ParametricCurveSource { .. },
                PatternMechanism::AlongParametricCurveSites {
                    id,
                    curve_mechanism_id,
                    interval,
                    phase,
                },
            ],
            [
                PatternOutputLayer::MarkPrototype {
                    site_mechanism_id,
                    orientation: MarkOrientation::Fixed,
                    ..
                },
            ],
        ) if *id == site_id && *curve_mechanism_id == curve_id && *site_mechanism_id == site_id => {
            validate_positive_finite(
                *interval,
                "pattern_definitions.mechanisms.along_parametric.interval",
            )?;
            validate_finite(
                *phase,
                "pattern_definitions.mechanisms.along_parametric.phase",
            )
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "parametric families require exactly one homogeneous raw-path or fixed-mark output",
        )),
    }
}

/// Validates finite authored spiral intent without deriving geometry or normalizing angles.
///
/// # Errors
///
/// Returns the stable parametric numeric diagnostic for invalid finite source intent.
fn validate_parametric_curve(curve: &ParametricCurve) -> Result<(), ValidationError> {
    match curve {
        ParametricCurve::Spiral(spiral) => {
            if !spiral.turns.is_finite() || spiral.turns <= 0.0 || spiral.turns > 1024.0 {
                return Err(ValidationError::new(
                    "pattern_definitions.mechanisms.parametric.turns",
                    "spiral turns must be finite and in (0, 1024]",
                ));
            }
            validate_positive_finite(
                spiral.radial_spacing,
                "pattern_definitions.mechanisms.parametric.radial_spacing",
            )?;
            validate_finite(
                spiral.phase_degrees,
                "pattern_definitions.mechanisms.parametric.phase_degrees",
            )
        }
    }
}

/// Projects one already validated definition through the current structural capability vocabulary.
/// It preserves stored output and generic-prototype order and never examines display metadata.
///
/// # Errors
///
/// Returns a stable internal-capability diagnostic only if a caller bypasses the shared validator.
fn project_validated_pattern_definition(
    definition: &PatternDefinition,
    settings: &[EffectivePatternOutputSettings],
) -> Result<PatternCapabilityProjection, ValidationError> {
    if definition.output_layers.len() != settings.len()
        || definition
            .output_layers
            .iter()
            .zip(settings)
            .any(|(output, setting)| {
                output.id() != setting.output_layer_id
                    || validate_response_for_output(output, &setting.response).is_err()
            })
    {
        return Err(ValidationError::new(
            "pattern.capability.outputs",
            "capability output settings must match definition order and kind",
        ));
    }
    let outputs = definition
        .output_layers
        .iter()
        .zip(settings)
        .map(|(output, setting)| {
            Ok(PatternOutputCapabilityRecord {
                output_layer_id: output.id(),
                structural: project_output_capability(output)?,
                response: setting.response.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let family = match &definition.family {
        PatternFamily::GuideIntersections { .. } => {
            let [guide_mechanism, site_mechanism] = definition.mechanisms.as_slice() else {
                return Err(ValidationError::new(
                    "pattern_definitions.family.capability",
                    "validated guide family is missing its ordered mechanisms",
                ));
            };
            let guides = match guide_mechanism {
                PatternMechanism::StraightGuides { .. } => GuideCapabilities {
                    count: 2,
                    spacing: false,
                    phase: false,
                    editable_curve: false,
                    prototype_kinds: Vec::new(),
                },
                PatternMechanism::StraightGuideDimensions { dimensions, .. } => GuideCapabilities {
                    count: dimensions.len() as u8,
                    spacing: true,
                    phase: true,
                    editable_curve: false,
                    prototype_kinds: Vec::new(),
                },
                PatternMechanism::GuideDimensions { dimensions, .. } => GuideCapabilities {
                    count: dimensions.len() as u8,
                    spacing: true,
                    phase: true,
                    editable_curve: dimensions.iter().any(|dimension| {
                        matches!(dimension.prototype, GuidePrototype::AuthoredOpenPath { .. })
                    }),
                    prototype_kinds: dimensions
                        .iter()
                        .map(|dimension| guide_prototype_kind(&dimension.prototype))
                        .collect(),
                },
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family.capability",
                        "validated guide family has an unsupported guide mechanism",
                    ));
                }
            };
            let site_product = match site_mechanism {
                PatternMechanism::GuideIntersections { .. }
                | PatternMechanism::SelectedGuideIntersections { .. } => {
                    GuideSiteProductCapability::Intersections
                }
                PatternMechanism::AlongGuideSites { .. } => GuideSiteProductCapability::AlongGuides,
                _ => {
                    return Err(ValidationError::new(
                        "pattern_definitions.family.capability",
                        "validated guide family has an unsupported site mechanism",
                    ));
                }
            };
            PatternFamilyCapabilityProjection::Grid(GridCapabilityProjection {
                generator: GeneratorCapabilities {
                    density: true,
                    seed: false,
                },
                guides,
                site_product,
            })
        }
        PatternFamily::RandomSites { .. } => {
            let [
                PatternMechanism::RandomSiteProcess { character, .. },
                PatternMechanism::SiteDensityModulation { modulation, .. },
                PatternMechanism::SiteExclusion { policy, .. },
                PatternMechanism::RandomSiteProduct { .. },
            ] = definition.mechanisms.as_slice()
            else {
                return Err(ValidationError::new(
                    "pattern_definitions.family.capability",
                    "validated dispersion family is missing its ordered mechanisms",
                ));
            };
            PatternFamilyCapabilityProjection::Dispersion(DispersionCapabilityProjection {
                generator: GeneratorCapabilities {
                    density: true,
                    seed: true,
                },
                character: random_character_kind(character),
                density_modulation: density_modulation_kind(modulation),
                exclusion: exclusion_kind(policy),
            })
        }
        PatternFamily::ParametricCurve {
            site_mechanism_id, ..
        } => PatternFamilyCapabilityProjection::Parametric(ParametricCapabilityProjection {
            generator: GeneratorCapabilities {
                density: matches!(
                    definition.mechanisms.first(),
                    Some(PatternMechanism::ParametricCurveSource {
                        repetition: GuideRepetition::TransformStack { .. },
                        ..
                    })
                ),
                seed: false,
            },
            supports_round_spiral: true,
            supports_square_spiral: true,
            raw_paths: site_mechanism_id.is_none(),
            sites_along_curve: site_mechanism_id.is_some(),
            normal_offset: matches!(
                definition.mechanisms.first(),
                Some(PatternMechanism::ParametricCurveSource {
                    repetition: GuideRepetition::NormalOffset { .. },
                    ..
                })
            ),
        }),
    };
    Ok(PatternCapabilityProjection {
        definition_id: definition.id,
        family,
        outputs,
    })
}

/// Projects one accepted mark output without exposing renderer behavior or payload IDs.
///
/// # Errors
///
/// Returns a stable diagnostic if an unchecked definition contains a future output branch.
fn project_output_capability(
    output: &PatternOutputLayer,
) -> Result<PatternOutputCapabilityProjection, ValidationError> {
    if let PatternOutputLayer::Regions { source, .. } = output {
        return Ok(PatternOutputCapabilityProjection::Regions(
            RegionOutputCapabilityProjection {
                source: match source {
                    RegionSourceIntent::VoronoiSites { site_mechanism_id } => {
                        RegionSourceCapabilityKind::OrdinaryVoronoi {
                            site_mechanism_id: *site_mechanism_id,
                        }
                    }
                    RegionSourceIntent::GuideFaces {
                        guide_mechanism_id,
                        dimensions,
                    } => RegionSourceCapabilityKind::GuideFaces {
                        guide_mechanism_id: *guide_mechanism_id,
                        dimensions: dimensions.clone(),
                    },
                },
                full_treatment_only: true,
                sampled_paint: false,
            },
        ));
    }
    if matches!(
        output,
        PatternOutputLayer::GuidePaths { .. } | PatternOutputLayer::ParametricPaths { .. }
    ) {
        return Ok(PatternOutputCapabilityProjection::GuidePaths(
            GuidePathOutputCapabilityProjection {
                round_join: true,
                round_cap: true,
                thickness_range: true,
            },
        ));
    }
    if let PatternOutputLayer::ConnectionPaths { program, .. } = output {
        let (program_kind, minimum_degree, seed) = match program {
            ConnectionProgram::NearestLinks { .. } => {
                (ConnectionProgramKind::NearestLinks, None, None)
            }
            ConnectionProgram::RandomLinks {
                minimum_degree,
                seed,
                ..
            } => (
                ConnectionProgramKind::RandomLinks,
                Some(*minimum_degree),
                Some(*seed),
            ),
            ConnectionProgram::GridSpanningTree { seed, .. } => {
                (ConnectionProgramKind::GridSpanningTree, None, Some(*seed))
            }
        };
        let adjacency = program.adjacency();
        return Ok(PatternOutputCapabilityProjection::ConnectionPaths(
            ConnectionPathOutputCapabilityProjection {
                program_kind,
                maximum_degree: adjacency.maximum_degree,
                maximum_distance: adjacency.maximum_distance,
                minimum_degree,
                seed,
                round_join: true,
                round_cap: true,
                thickness_range: true,
            },
        ));
    }
    if let PatternOutputLayer::MazeWalls { program, .. } = output {
        program.validate()?;
        return Ok(PatternOutputCapabilityProjection::MazeWalls(
            MazeWallOutputCapabilityProjection {
                seeded_recursive_backtracker: matches!(
                    program.algorithm,
                    GridMazeAlgorithm::RecursiveBacktracker
                ),
                seed: program.seed,
                round_join: true,
                round_cap: true,
                thickness_range: true,
            },
        ));
    }
    let (prototype, orientation) = match output {
        PatternOutputLayer::CircularMarks { .. } => {
            (MarkPrototypeKind::Circle, MarkOrientationKind::Fixed)
        }
        PatternOutputLayer::MarkPrototype {
            prototype,
            orientation,
            ..
        } => (
            mark_prototype_kind(prototype),
            mark_orientation_kind(orientation),
        ),
        PatternOutputLayer::GuidePaths { .. } | PatternOutputLayer::ParametricPaths { .. } => {
            unreachable!("handled above")
        }
        PatternOutputLayer::ConnectionPaths { .. } | PatternOutputLayer::MazeWalls { .. } => {
            unreachable!("handled above")
        }
        PatternOutputLayer::Regions { .. } => unreachable!("regions have a dedicated capability"),
    };
    Ok(PatternOutputCapabilityProjection::Marks(
        MarkOutputCapabilityProjection {
            prototype,
            orientation,
            fill_range: true,
        },
    ))
}

/// Maps the active random-process variant to its payload-free capability discriminant.
fn random_character_kind(character: &RandomSiteCharacter) -> RandomCharacterKind {
    match character {
        RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
        RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
        RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
    }
}

/// Maps the active density-modulation variant to its payload-free capability discriminant.
fn density_modulation_kind(modulation: &SiteDensityModulation) -> DensityModulationKind {
    match modulation {
        SiteDensityModulation::Uniform => DensityModulationKind::Uniform,
        SiteDensityModulation::ArtworkWeighted { .. } => DensityModulationKind::ArtworkWeighted,
    }
}

/// Maps the active exclusion variant to its payload-free capability discriminant.
fn exclusion_kind(policy: &SiteExclusionPolicy) -> ExclusionKind {
    match policy {
        SiteExclusionPolicy::None => ExclusionKind::None,
        SiteExclusionPolicy::MinimumCenterDistance { .. } => ExclusionKind::MinimumCenterDistance,
        SiteExclusionPolicy::VisibleMarkMargin { .. } => ExclusionKind::VisibleMarkMargin,
    }
}

/// Maps one active generic guide prototype to its stable descriptor discriminant.
fn guide_prototype_kind(prototype: &GuidePrototype) -> GuidePrototypeKind {
    match prototype {
        GuidePrototype::AuthoredOpenPath { .. } => GuidePrototypeKind::AuthoredOpenPath,
        GuidePrototype::CircularArc { .. } => GuidePrototypeKind::CircularArc,
    }
}

/// Maps one active mark prototype to its stable descriptor discriminant.
fn mark_prototype_kind(prototype: &MarkPrototype) -> MarkPrototypeKind {
    match prototype {
        MarkPrototype::Circle => MarkPrototypeKind::Circle,
        MarkPrototype::AuthoredClosedShape { .. } => MarkPrototypeKind::AuthoredClosedShape,
    }
}

/// Maps one active mark orientation to its stable descriptor discriminant.
fn mark_orientation_kind(orientation: &MarkOrientation) -> MarkOrientationKind {
    match orientation {
        MarkOrientation::Fixed => MarkOrientationKind::Fixed,
        MarkOrientation::GuideTangent { .. } => MarkOrientationKind::GuideTangent,
        MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
    }
}

/// Validates one random-site mechanism chain and its compatible typed mark output in stored order.
///
/// # Errors
///
/// Returns a stable identity, ordering, payload, work-limit, or output-capability diagnostic.
fn validate_random_site_definition(
    definition: &PatternDefinition,
    base_id: PatternMechanismId,
    modulation_id: PatternMechanismId,
    exclusion_id: PatternMechanismId,
    site_id: PatternMechanismId,
) -> Result<(), ValidationError> {
    let [
        PatternMechanism::RandomSiteProcess {
            id: declared_base,
            character,
            ..
        },
        PatternMechanism::SiteDensityModulation {
            id: declared_modulation,
            base_site_process_id,
            modulation,
        },
        PatternMechanism::SiteExclusion {
            id: declared_exclusion,
            density_modulation_id,
            policy,
        },
        PatternMechanism::RandomSiteProduct {
            id: declared_site,
            exclusion_id: product_exclusion_id,
            maximum_attempts,
            maximum_neighbor_checks,
        },
    ] = definition.mechanisms.as_slice()
    else {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "random-site family requires base, modulation, exclusion, and site product in stored order",
        ));
    };
    if *declared_base != base_id
        || *declared_modulation != modulation_id
        || *declared_exclusion != exclusion_id
        || *declared_site != site_id
        || *base_site_process_id != base_id
        || *density_modulation_id != modulation_id
        || *product_exclusion_id != exclusion_id
    {
        return Err(ValidationError::new(
            "pattern_definitions.family",
            "random-site family root must reference its ordered mechanism chain",
        ));
    }
    validate_random_character(character)?;
    validate_site_density_modulation(modulation)?;
    validate_site_exclusion(policy)?;
    if *maximum_attempts == 0 {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.random_sites.maximum_attempts",
            "random-site maximum attempts must be nonzero",
        ));
    }
    if *maximum_neighbor_checks == 0 {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.random_sites.maximum_neighbor_checks",
            "random-site maximum neighbor checks must be nonzero",
        ));
    }
    let [output] = definition.output_layers.as_slice() else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "random-site products require exactly one fixed mark prototype layer",
        ));
    };
    let source_id = match output {
        PatternOutputLayer::MarkPrototype {
            site_mechanism_id,
            orientation: MarkOrientation::Fixed,
            ..
        } => *site_mechanism_id,
        PatternOutputLayer::ConnectionPaths {
            site_mechanism_id,
            program,
            ..
        } => {
            program.validate()?;
            *site_mechanism_id
        }
        PatternOutputLayer::Regions { source, .. } => {
            source.site_mechanism_id().ok_or_else(|| {
                ValidationError::new(
                    "pattern.output_layers.region_source",
                    "this output shape requires a site-backed region source",
                )
            })?
        }
        _ => {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers",
                "random-site products require one fixed mark or connection layer",
            ));
        }
    };
    if source_id != site_id {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "output layer must consume the declared random-site product",
        ));
    }
    Ok(())
}

fn validate_random_character(character: &RandomSiteCharacter) -> Result<(), ValidationError> {
    match character {
        RandomSiteCharacter::RawUniform => Ok(()),
        RandomSiteCharacter::Even {
            minimum_center_distance,
        } => validate_positive_finite(
            *minimum_center_distance,
            "pattern_definitions.mechanisms.random_sites.minimum_center_distance",
        ),
        RandomSiteCharacter::Clustered {
            cluster_density,
            cluster_spread,
            cluster_strength,
        } => {
            validate_positive_finite(
                *cluster_density,
                "pattern_definitions.mechanisms.random_sites.cluster_density",
            )?;
            validate_positive_finite(
                *cluster_spread,
                "pattern_definitions.mechanisms.random_sites.cluster_spread",
            )?;
            validate_unit_component(
                *cluster_strength,
                "pattern_definitions.mechanisms.random_sites.cluster_strength",
            )
        }
    }
}

fn validate_site_density_modulation(
    modulation: &SiteDensityModulation,
) -> Result<(), ValidationError> {
    match modulation {
        SiteDensityModulation::Uniform => Ok(()),
        SiteDensityModulation::ArtworkWeighted {
            mapping,
            strength,
            response,
        } => {
            validate_source_mapping(*mapping)?;
            validate_unit_component(
                *strength,
                "pattern_definitions.mechanisms.site_density.strength",
            )?;
            match response {
                ArtworkWeightResponse::Linear | ArtworkWeightResponse::Smoothstep => Ok(()),
            }
        }
    }
}

fn validate_site_exclusion(policy: &SiteExclusionPolicy) -> Result<(), ValidationError> {
    match policy {
        SiteExclusionPolicy::None => Ok(()),
        SiteExclusionPolicy::MinimumCenterDistance { minimum } => validate_positive_finite(
            *minimum,
            "pattern_definitions.mechanisms.site_exclusion.minimum",
        ),
        SiteExclusionPolicy::VisibleMarkMargin { margin, .. } => validate_nonnegative_finite(
            *margin,
            "pattern_definitions.mechanisms.site_exclusion.margin",
        ),
    }
}

/// Validates one standalone typed definition before it is installed in an
/// authoritative document.  Document construction additionally validates
/// document-wide definition/channel references and IDs.
pub fn validate_pattern_definition(definition: &PatternDefinition) -> Result<(), ValidationError> {
    validate_definition(definition)
}

fn validate_straight_dimensions(
    dimensions: &[StraightGuideDimension],
) -> Result<(), ValidationError> {
    if !(1..=4).contains(&dimensions.len()) {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.dimensions",
            "straight-guide dimensions must contain one through four entries",
        ));
    }
    let mut ids = HashSet::new();
    for dimension in dimensions {
        if !ids.insert(dimension.id) {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.dimensions",
                "straight-guide dimension IDs must be unique in stored order",
            ));
        }
        validate_finite(
            dimension.baseline_angle_degrees,
            "pattern_definitions.mechanisms.dimensions.baseline_angle_degrees",
        )?;
        validate_finite(
            dimension.phase,
            "pattern_definitions.mechanisms.dimensions.phase",
        )?;
        validate_positive_finite(
            dimension.repetition.spacing_multiplier,
            "pattern_definitions.mechanisms.dimensions.repetition.spacing_multiplier",
        )?;
        if dimension.id.0 == 0 {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.dimensions.id",
                "straight-guide dimension IDs must be nonzero stable IDs",
            ));
        }
    }
    Ok(())
}

fn validate_selection(
    selection: &[GuideDimensionId],
    dimensions: &[StraightGuideDimension],
    minimum: usize,
    path: &'static str,
) -> Result<(), ValidationError> {
    if selection.len() < minimum {
        return Err(ValidationError::new(
            path,
            "selection has too few dimensions",
        ));
    }
    let expected: Vec<_> = dimensions.iter().map(|dimension| dimension.id).collect();
    let mut previous = None;
    for id in selection {
        let Some(position) = expected.iter().position(|candidate| candidate == id) else {
            return Err(ValidationError::new(
                path,
                "selection references a missing dimension ID",
            ));
        };
        if previous.is_some_and(|value| position <= value) {
            return Err(ValidationError::new(
                path,
                "selection must be unique and follow dimension stored order",
            ));
        }
        previous = Some(position);
    }
    Ok(())
}

/// Validates ordered site selections against stable generic dimension IDs.
fn validate_selection_ids(
    selection: &[GuideDimensionId],
    dimensions: &[GuideDimensionId],
    minimum: usize,
    path: &'static str,
) -> Result<(), ValidationError> {
    if selection.len() < minimum {
        return Err(ValidationError::new(
            path,
            "selection has too few dimensions",
        ));
    }
    let mut previous = None;
    for id in selection {
        let Some(position) = dimensions.iter().position(|candidate| candidate == id) else {
            return Err(ValidationError::new(
                path,
                "selection references a missing dimension ID",
            ));
        };
        if previous.is_some_and(|value| position <= value) {
            return Err(ValidationError::new(
                path,
                "selection must be unique and follow dimension stored order",
            ));
        }
        previous = Some(position);
    }
    Ok(())
}

/// Validates bounded generic guide dimensions without resolving their document resources.
fn validate_guide_dimensions(dimensions: &[GuideDimension]) -> Result<(), ValidationError> {
    if !(1..=4).contains(&dimensions.len()) {
        return Err(ValidationError::new(
            "pattern_definitions.mechanisms.guide_dimensions",
            "guide dimensions must contain one through four entries",
        ));
    }
    let mut ids = HashSet::new();
    for dimension in dimensions {
        if dimension.id.0 == 0 || !ids.insert(dimension.id) {
            return Err(ValidationError::new(
                "pattern_definitions.mechanisms.guide_dimensions.id",
                "guide dimension IDs must be nonzero and unique in stored order",
            ));
        }
        validate_finite(
            dimension.baseline_angle_degrees,
            "pattern_definitions.mechanisms.guide_dimensions.baseline_angle",
        )?;
        validate_finite(
            dimension.phase,
            "pattern_definitions.mechanisms.guide_dimensions.phase",
        )?;
        match &dimension.prototype {
            GuidePrototype::AuthoredOpenPath { .. } => {}
            GuidePrototype::CircularArc {
                center,
                radius,
                start_angle_degrees,
                sweep_angle_degrees,
            } => {
                if !center.x.is_finite() || !center.y.is_finite() {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_prototype.arc.center",
                        "circular-arc centers must be finite",
                    ));
                }
                if !radius.is_finite() || *radius <= 0.0 {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_prototype.arc.radius",
                        "circular-arc radius must be positive and finite",
                    ));
                }
                if !start_angle_degrees.is_finite()
                    || !sweep_angle_degrees.is_finite()
                    || *sweep_angle_degrees == 0.0
                    || sweep_angle_degrees.abs() > 360.0
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_prototype.arc.angles",
                        "circular-arc angles must be finite with a nonzero sweep of at most 360 degrees",
                    ));
                }
            }
        }
        match dimension.repetition {
            GuideRepetition::Single => {}
            GuideRepetition::TransformStack {
                direction_degrees,
                spacing_multiplier,
            } => {
                validate_finite(
                    direction_degrees,
                    "pattern_definitions.mechanisms.guide_repetition.direction",
                )?;
                if !spacing_multiplier.is_finite() || spacing_multiplier <= 0.0 {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_repetition.spacing_multiplier",
                        "guide stack spacing multiplier must be positive and finite",
                    ));
                }
            }
            GuideRepetition::NormalOffset { spacing, .. } => {
                if dimension.phase != 0.0 {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_repetition.phase",
                        "normal-offset guide dimensions require zero phase",
                    ));
                }
                if !spacing.is_finite() || spacing <= 0.0 {
                    return Err(ValidationError::new(
                        "pattern_definitions.mechanisms.guide_repetition.spacing",
                        "normal-offset spacing must be positive and finite",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Validates one complete generic prototype payload before an edit installs it.
fn validate_guide_prototype(prototype: &GuidePrototype) -> Result<(), ValidationError> {
    let probe = GuideDimension {
        id: GuideDimensionId(1),
        baseline_angle_degrees: 0.0,
        phase: 0.0,
        prototype: prototype.clone(),
        repetition: GuideRepetition::Single,
    };
    validate_guide_dimensions(&[probe])
}

/// Validates one complete bounded generic repetition payload before an edit installs it.
fn validate_guide_repetition(repetition: &GuideRepetition) -> Result<(), ValidationError> {
    let probe = GuideDimension {
        id: GuideDimensionId(1),
        baseline_angle_degrees: 0.0,
        phase: 0.0,
        prototype: GuidePrototype::CircularArc {
            center: AuthoredPoint2 { x: 0.0, y: 0.0 },
            radius: 1.0,
            start_angle_degrees: 0.0,
            sweep_angle_degrees: 90.0,
        },
        repetition: repetition.clone(),
    };
    validate_guide_dimensions(&[probe])
}

/// Validates existing site products against a generic guide root without changing their semantics.
fn validate_site_mechanism_ids(
    mechanism: &PatternMechanism,
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[GuideDimensionId],
) -> Result<(), ValidationError> {
    match mechanism {
        PatternMechanism::SelectedGuideIntersections {
            id,
            guide_mechanism_id,
            dimensions: selection,
            merge_epsilon,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection_ids(
                selection,
                dimensions,
                2,
                "pattern_definitions.mechanisms.intersections.dimensions",
            )?;
            validate_nonnegative_finite(
                *merge_epsilon,
                "pattern_definitions.mechanisms.intersections.merge_epsilon",
            )
        }
        PatternMechanism::AlongGuideSites {
            id,
            guide_mechanism_id,
            dimensions: selection,
            interval_multiplier,
            phase,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection_ids(
                selection,
                dimensions,
                1,
                "pattern_definitions.mechanisms.along_guides.dimensions",
            )?;
            validate_positive_finite(
                *interval_multiplier,
                "pattern_definitions.mechanisms.along_guides.interval_multiplier",
            )?;
            validate_finite(*phase, "pattern_definitions.mechanisms.along_guides.phase")
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "family root requires a compatible declared guide site mechanism",
        )),
    }
}

/// Validates the unchanged mark output against generic stable guide dimension IDs.
///
/// # Errors
///
/// Returns a stable output-layer or orientation-reference diagnostic.
fn validate_generalized_output_layers_ids(
    layers: &[PatternOutputLayer],
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[GuideDimensionId],
) -> Result<(), ValidationError> {
    if let [
        PatternOutputLayer::ConnectionPaths {
            site_mechanism_id,
            program,
            ..
        },
    ] = layers
    {
        program.validate()?;
        return (*site_mechanism_id == site_id)
            .then_some(())
            .ok_or(ValidationError::new(
                "pattern_definitions.output_layers.site_mechanism_id",
                "connection output must consume its declared site mechanism",
            ));
    }
    if let [PatternOutputLayer::Regions { source, .. }] = layers {
        return validate_region_source_ids(source, guide_id, site_id, dimensions);
    }
    if let [
        PatternOutputLayer::GuidePaths {
            guide_mechanism_id, ..
        },
    ] = layers
    {
        return (*guide_mechanism_id == guide_id)
            .then_some(())
            .ok_or(ValidationError::new(
                "pattern_definitions.output_layers.guide_mechanism_id",
                "guide-path output must consume its declared guide mechanism",
            ));
    }
    let [
        PatternOutputLayer::MarkPrototype {
            site_mechanism_id,
            orientation,
            ..
        },
    ] = layers
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "generalized guide products require exactly one mark prototype layer",
        ));
    };
    if *site_mechanism_id != site_id {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "output layer must consume its declared site product",
        ));
    }
    match orientation {
        MarkOrientation::Fixed => Ok(()),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id }
            if dimensions.contains(dimension_id) =>
        {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.orientation",
            "orientation references a missing guide dimension",
        )),
    }
}

/// Validates the typed source binding for one fixed full region output.
///
/// Guide-face selections are ordered in the owning guide mechanism's stored
/// order so a renderer or evaluator never needs to infer a second selection.
///
/// # Errors
///
/// Returns a stable source, mechanism, cardinality, ordering, or foreign-ID
/// diagnostic without changing the structural definition.
fn validate_region_source_ids(
    source: &RegionSourceIntent,
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[GuideDimensionId],
) -> Result<(), ValidationError> {
    match source {
        RegionSourceIntent::VoronoiSites { site_mechanism_id } if *site_mechanism_id == site_id => {
            Ok(())
        }
        RegionSourceIntent::GuideFaces {
            guide_mechanism_id,
            dimensions: selected,
        } if *guide_mechanism_id == guide_id => {
            if !(2..=3).contains(&selected.len()) {
                return Err(ValidationError::new(
                    "pattern_definitions.output_layers.guide_faces.dimensions",
                    "guide-face regions require two or three dimensions",
                ));
            }
            let mut previous = None;
            for dimension in selected {
                let Some(index) = dimensions
                    .iter()
                    .position(|candidate| candidate == dimension)
                else {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers.guide_faces.dimensions",
                        "guide-face regions require owned guide dimensions",
                    ));
                };
                if previous.is_some_and(|previous| index <= previous) {
                    return Err(ValidationError::new(
                        "pattern_definitions.output_layers.guide_faces.dimensions",
                        "guide-face dimensions must be unique in stored guide order",
                    ));
                }
                previous = Some(index);
            }
            Ok(())
        }
        RegionSourceIntent::VoronoiSites { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "region output must consume its declared site mechanism",
        )),
        RegionSourceIntent::GuideFaces { .. } => Err(ValidationError::new(
            "pattern_definitions.output_layers.guide_faces.mechanism",
            "guide-face regions must consume their declared guide mechanism",
        )),
    }
}

fn validate_site_mechanism(
    mechanism: &PatternMechanism,
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[StraightGuideDimension],
) -> Result<(), ValidationError> {
    match mechanism {
        PatternMechanism::SelectedGuideIntersections {
            id,
            guide_mechanism_id,
            dimensions: selection,
            merge_epsilon,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection(
                selection,
                dimensions,
                2,
                "pattern_definitions.mechanisms.intersections.dimensions",
            )?;
            validate_nonnegative_finite(
                *merge_epsilon,
                "pattern_definitions.mechanisms.intersections.merge_epsilon",
            )
        }
        PatternMechanism::AlongGuideSites {
            id,
            guide_mechanism_id,
            dimensions: selection,
            interval_multiplier,
            phase,
        } if *id == site_id && *guide_mechanism_id == guide_id => {
            validate_selection(
                selection,
                dimensions,
                1,
                "pattern_definitions.mechanisms.along_guides.dimensions",
            )?;
            validate_positive_finite(
                *interval_multiplier,
                "pattern_definitions.mechanisms.along_guides.interval_multiplier",
            )?;
            validate_finite(*phase, "pattern_definitions.mechanisms.along_guides.phase")
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.family",
            "family root requires a compatible declared straight-guide site mechanism",
        )),
    }
}

/// Validates one straight-guide family output against the selected product and dimension IDs.
///
/// # Errors
///
/// Returns a stable output-layer, site-product, or orientation-reference diagnostic.
fn validate_generalized_output_layers(
    layers: &[PatternOutputLayer],
    guide_id: PatternMechanismId,
    site_id: PatternMechanismId,
    dimensions: &[StraightGuideDimension],
    site_mechanism: &PatternMechanism,
) -> Result<(), ValidationError> {
    if let [
        PatternOutputLayer::ConnectionPaths {
            site_mechanism_id,
            program,
            ..
        },
    ] = layers
    {
        program.validate()?;
        return (*site_mechanism_id == site_id)
            .then_some(())
            .ok_or(ValidationError::new(
                "pattern_definitions.output_layers.site_mechanism_id",
                "connection output must consume its declared site mechanism",
            ));
    }
    if let [PatternOutputLayer::Regions { source, .. }] = layers {
        let ids = dimensions
            .iter()
            .map(|dimension| dimension.id)
            .collect::<Vec<_>>();
        return validate_region_source_ids(source, guide_id, site_id, &ids);
    }
    if let [
        PatternOutputLayer::GuidePaths {
            guide_mechanism_id, ..
        },
    ] = layers
    {
        return (*guide_mechanism_id == guide_id)
            .then_some(())
            .ok_or(ValidationError::new(
                "pattern_definitions.output_layers.guide_mechanism_id",
                "guide-path output must consume its declared guide mechanism",
            ));
    }
    if let [
        PatternOutputLayer::MazeWalls {
            site_mechanism_id,
            program,
            ..
        },
    ] = layers
    {
        program.validate()?;
        if *site_mechanism_id != site_id {
            return Err(ValidationError::new(
                "pattern_definitions.output_layers.site_mechanism_id",
                "maze output must consume its declared site mechanism",
            ));
        }
        return match site_mechanism {
            PatternMechanism::SelectedGuideIntersections {
                dimensions: selection,
                ..
            } if (2..=3).contains(&selection.len()) => Ok(()),
            _ => Err(ValidationError::new(
                "pattern_definitions.output_layers.maze",
                "maze-wall output requires a two- or three-guide intersection site product",
            )),
        };
    }
    let [
        PatternOutputLayer::MarkPrototype {
            site_mechanism_id,
            orientation,
            ..
        },
    ] = layers
    else {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers",
            "generalized straight-guide products require exactly one mark prototype layer",
        ));
    };
    if *site_mechanism_id != site_id {
        return Err(ValidationError::new(
            "pattern_definitions.output_layers.site_mechanism_id",
            "output layer must consume its declared site product",
        ));
    }
    match orientation {
        MarkOrientation::Fixed => Ok(()),
        MarkOrientation::GuideTangent { dimension_id }
        | MarkOrientation::GuideNormal { dimension_id }
            if dimensions
                .iter()
                .any(|dimension| dimension.id == *dimension_id) =>
        {
            Ok(())
        }
        _ => Err(ValidationError::new(
            "pattern_definitions.output_layers.orientation",
            "orientation references a missing straight-guide dimension",
        )),
    }
}

fn derive_density_axis(
    canvas: &CanvasSpec,
    authoritative_value: f64,
    edited_axis: DensityEditedAxis,
) -> Result<f64, ValidationError> {
    validate_positive_finite(authoritative_value, "channel.pattern.layout.density")?;
    validate_positive_finite(canvas.width, "canvas.width")?;
    validate_positive_finite(canvas.height, "canvas.height")?;
    let result = match edited_axis {
        DensityEditedAxis::AcrossX => authoritative_value * canvas.height / canvas.width,
        DensityEditedAxis::AcrossY => authoritative_value * canvas.width / canvas.height,
    };
    validate_positive_finite(result, "channel.pattern.layout.density.derived_axis")?;
    Ok(result)
}

fn validate_layout(layout: &ChannelPatternLayout) -> Result<(), ValidationError> {
    validate_positive_finite(
        layout.density.across_x,
        "channel.pattern.layout.density.across_x",
    )?;
    validate_positive_finite(
        layout.density.across_y,
        "channel.pattern.layout.density.across_y",
    )?;
    validate_finite(
        layout.rotation_degrees,
        "channel.pattern.layout.rotation_degrees",
    )?;
    validate_finite(layout.translation_x, "channel.pattern.layout.translation_x")?;
    validate_finite(layout.translation_y, "channel.pattern.layout.translation_y")
}

/// Validates stored channel-relative intent without resolving a document base.
/// Every optional delta remains finite; positivity and mark bounds are checked
/// only after additive composition by `effective_channel_pattern`.
fn validate_channel_pattern_instance(
    instance: &ChannelPatternInstance,
) -> Result<(), ValidationError> {
    validate_finite(
        instance.layout_delta.translation_x,
        "channel.pattern.layout_delta.translation_x",
    )?;
    validate_finite(
        instance.layout_delta.translation_y,
        "channel.pattern.layout_delta.translation_y",
    )?;
    if let Some(density) = &instance.layout_delta.density {
        validate_finite(
            density.across_x_delta,
            "channel.pattern.layout_delta.density.across_x_delta",
        )?;
        validate_finite(
            density.across_y_delta,
            "channel.pattern.layout_delta.density.across_y_delta",
        )?;
    }
    if let Some(rotation) = instance.layout_delta.rotation_degrees {
        validate_finite(rotation, "channel.pattern.layout_delta.rotation_degrees")?;
    }
    if let Some(rotation) = instance.shape_rotation_delta_degrees {
        validate_finite(rotation, "channel.pattern.shape_rotation_delta_degrees")?;
    }
    for entry in &instance.output_response_deltas {
        let (minimum, maximum) = match &entry.delta {
            ChannelGeometryResponseDelta::Marks(value) => {
                (value.minimum_fill_delta, value.maximum_fill_delta)
            }
            ChannelGeometryResponseDelta::Connected(value) => {
                (value.minimum_thickness_delta, value.maximum_thickness_delta)
            }
        };
        if let Some(value) = minimum {
            validate_finite(
                value,
                "channel.pattern.geometry_response_delta.minimum_delta",
            )?;
        }
        if let Some(value) = maximum {
            validate_finite(
                value,
                "channel.pattern.geometry_response_delta.maximum_delta",
            )?;
        }
    }
    Ok(())
}

/// Validates a fully resolved Stage 20G instance after the document performs
/// all finite additions.  This boundary intentionally neither clamps fill nor
/// normalizes rotations, preserving authored intent exactly.
fn validate_effective_pattern(
    effective: &EffectiveChannelPatternInstance,
) -> Result<(), ValidationError> {
    validate_positive_finite(
        effective.density.across_x,
        "effective_pattern.density.across_x",
    )?;
    validate_positive_finite(
        effective.density.across_y,
        "effective_pattern.density.across_y",
    )?;
    validate_finite(
        effective.pattern_rotation_degrees,
        "effective_pattern.pattern_rotation_degrees",
    )?;
    validate_finite(effective.translation_x, "effective_pattern.translation_x")?;
    validate_finite(effective.translation_y, "effective_pattern.translation_y")?;
    validate_finite(
        effective.shape_rotation_degrees,
        "effective_pattern.shape_rotation_degrees",
    )?;
    for output in &effective.output_settings {
        validate_pattern_geometry_response(&output.response)?;
    }
    Ok(())
}

/// Confirms that the resolved definition can realize the persisted response branch.
///
/// # Errors
///
/// Returns a validation error without mutation when the response branch and
/// the one homogeneous output kind do not agree.
fn validate_effective_response_compatibility(
    definition: &PatternDefinition,
    settings: &[EffectivePatternOutputSettings],
) -> Result<(), ValidationError> {
    if definition.output_layers.len() != settings.len() {
        return Err(ValidationError::new(
            "effective_pattern.output_settings.cardinality",
            "effective settings must cover every structural output",
        ));
    }
    for (output, setting) in definition.output_layers.iter().zip(settings) {
        if output.id() != setting.output_layer_id {
            return Err(ValidationError::new(
                "effective_pattern.output_settings.order",
                "effective settings must retain structural output order",
            ));
        }
        validate_response_for_output(output, &setting.response)?;
    }
    Ok(())
}

fn validate_mark_response(response: &MarkGeometryResponse) -> Result<(), ValidationError> {
    validate_range_finite(
        response.minimum_fill,
        0.0,
        2.0,
        "channel.pattern.mark_geometry_response.minimum_fill",
    )?;
    validate_range_finite(
        response.maximum_fill,
        0.0,
        2.0,
        "channel.pattern.mark_geometry_response.maximum_fill",
    )?;
    if response.minimum_fill > response.maximum_fill {
        return Err(ValidationError::new(
            "channel.pattern.mark_geometry_response",
            "minimum_fill must not exceed maximum_fill",
        ));
    }
    Ok(())
}

/// Validates the normalized stroke-width response after document authority has composed deltas.
fn validate_connected_response(
    response: &ConnectedGeometryResponse,
) -> Result<(), ValidationError> {
    for value in [response.minimum_thickness, response.maximum_thickness] {
        if !value.is_finite() || !(0.0..=2.0).contains(&value) {
            return Err(ValidationError::new(
                "channel.pattern.geometry_response",
                "stroke thickness must be finite and within 0.0..=2.0",
            ));
        }
    }
    if response.minimum_thickness > response.maximum_thickness {
        return Err(ValidationError::new(
            "channel.pattern.geometry_response",
            "minimum stroke thickness must not exceed maximum stroke thickness",
        ));
    }
    Ok(())
}

fn validate_topology(
    model: HalftoneChannelModel,
    topology: &ChannelTopology,
    bundles: &[PatternDefinitionBundle],
) -> Result<(), ValidationError> {
    let required_roles = model.canonical_roles();
    if topology.channels.len() != required_roles.len() {
        return Err(ValidationError::new(
            "channel_topology.roles",
            "topology must contain every required role exactly once",
        ));
    }
    let mut ids = HashSet::new();
    for (channel, required_role) in topology.channels.iter().zip(required_roles.iter()) {
        if channel.role != *required_role {
            return Err(ValidationError::new(
                "channel_topology.roles",
                "topology roles must use the model's canonical order without extras",
            ));
        }
        if !ids.insert(channel.id) {
            return Err(ValidationError::new(
                "channel_topology.channels",
                "topology channel IDs must be unique",
            ));
        }
        if let Some(definition_id) = channel.pattern_instance.definition_override
            && !bundles
                .iter()
                .any(|bundle| bundle.definition.id == definition_id)
        {
            return Err(ValidationError::new(
                "channel.pattern.definition_id",
                "channel override references a missing pattern definition",
            ));
        }
        validate_channel_pattern_instance(&channel.pattern_instance)?;
        validate_unit_component(channel.opacity, "channel.appearance.opacity")?;
        validate_source_mapping(channel.mapping)?;
        validate_paint(model, channel.role, &channel.paint)?;
    }
    Ok(())
}

fn validate_source_mapping(mapping: SourceMapping) -> Result<(), ValidationError> {
    validate_nonnegative_finite(mapping.gain, "channel.source_mapping.gain")?;
    validate_finite(mapping.bias, "channel.source_mapping.bias")
}

fn validate_paint(
    model: HalftoneChannelModel,
    role: HalftoneChannelRole,
    paint: &ChannelPaint,
) -> Result<(), ValidationError> {
    match (model, role, paint) {
        (
            HalftoneChannelModel::SourceColorAlpha,
            HalftoneChannelRole::SourceColor,
            ChannelPaint::SampledSource,
        ) => Ok(()),
        (HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk, _, ChannelPaint::Solid(color)) => {
            validate_unit_component(color.red, "channel.paint.solid.red")?;
            validate_unit_component(color.green, "channel.paint.solid.green")?;
            validate_unit_component(color.blue, "channel.paint.solid.blue")?;
            validate_unit_component(color.alpha, "channel.paint.solid.alpha")
        }
        _ => Err(ValidationError::new(
            "channel.paint",
            "paint is incompatible with the channel role and model",
        )),
    }
}

fn validate_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be finite"))
    }
}

fn validate_positive_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be greater than zero",
        ))
    }
}

fn validate_nonnegative_finite(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must not be negative"))
    }
}

/// Validates one bounded finite scalar without assigning geometric meaning.
fn validate_range_finite(
    value: f64,
    minimum: f64,
    maximum: f64,
    path: &'static str,
) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be within the declared bounds",
        ))
    }
}

fn validate_unit_component(value: f64, path: &'static str) -> Result<(), ValidationError> {
    validate_finite(value, path)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(path, "value must be within 0.0..=1.0"))
    }
}

fn set_color_component(color: &mut ColorValue, component: ColorComponent, value: f64) {
    match component {
        ColorComponent::Red => color.red = value,
        ColorComponent::Green => color.green = value,
        ColorComponent::Blue => color.blue = value,
        ColorComponent::Alpha => color.alpha = value,
    }
}

/// A schema-scoped validation failure suitable for a frontend error display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    path: &'static str,
    message: &'static str,
}

impl ValidationError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for ValidationError {}

/// The pipeline layer invalidated by a committed document command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidationLevel {
    Presentation,
    Realization,
    Family,
    Source,
    ChannelTopology,
}

/// The committed effect of a document command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandResult {
    pub affected_channels: Vec<ChannelId>,
    /// `None` records a meaningful authority-only edit whose resolved current
    /// evaluation inputs are unchanged.
    pub invalidation: Option<InvalidationLevel>,
    /// A newly allocated authored structure ID, when this command creates a reusable structure.
    pub created_authored_structure_id: Option<AuthoredStructureId>,
}

/// The explicit density control which supplied a replacement value.  Keeping
/// this intent in the command (rather than inferring it from two values) is
/// what makes aspect-locked updates deterministic and replayable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DensityEditedAxis {
    AcrossX,
    AcrossY,
}

/// The explicit translation coordinate supplied by a channel-frame edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationEditedAxis {
    X,
    Y,
}

/// A compact, schema-only capability vocabulary.  It deliberately contains
/// neither current values nor UI policy: callers read those from `Document`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyFieldId {
    SourceReference,
    DensityAcrossX,
    DensityAcrossY,
    DensityAspectLocked,
    RotationDegrees,
    TranslationX,
    TranslationY,
    MarkMinimumFill,
    MarkMaximumFill,
    ConnectedMinimumThickness,
    ConnectedMaximumThickness,
    ShapeRotationDegrees,
    LegacyMappingComponent,
    LegacyMappingPlacement,
    ModeledMappingComponent,
    ModeledMappingPlacement,
    ModeledMappingInverted,
    ModeledMappingGain,
    ModeledMappingBias,
    Paint,
    ColorRed,
    ColorGreen,
    ColorBlue,
    ColorAlpha,
    Opacity,
    Visibility,
    DefinitionSelection,
    CoverageGuardSteps,
    CoverageAdditionalMargin,
    GuideBaselineAngle,
    GuidePhase,
    GuideSpacingMultiplier,
    GuidePrototype,
    GuideAuthoredStructure,
    GuideArcCenterX,
    GuideArcCenterY,
    GuideArcRadius,
    GuideArcStartAngle,
    GuideArcSweepAngle,
    GuideRepetition,
    GuideOffsetSpacing,
    GuideOffsetSides,
    GuideOffsetCleanup,
    GuideStackDirection,
    GuideStackSpacingMultiplier,
    IntersectionDimensions,
    IntersectionMergeEpsilon,
    AlongGuideDimensions,
    AlongGuideIntervalMultiplier,
    AlongGuidePhase,
    RandomCharacter,
    RandomEvenMinimumCenterDistance,
    RandomClusterDensity,
    RandomClusterSpread,
    RandomClusterStrength,
    RandomSeed,
    RandomDensityModulation,
    ArtworkWeightMappingComponent,
    ArtworkWeightMappingPlacement,
    ArtworkWeightMappingInverted,
    ArtworkWeightMappingGain,
    ArtworkWeightMappingBias,
    ArtworkWeightStrength,
    ArtworkWeightResponse,
    RandomExclusion,
    ExclusionMinimumCenterDistance,
    VisibleMarkMargin,
    VisibleMarkSizingPolicy,
    RandomMaximumAttempts,
    RandomMaximumNeighborChecks,
    OutputSiteProduct,
    OutputPrototype,
    OutputAuthoredClosedShape,
    OutputOrientation,
    OutputOrientationDimension,
    ConnectionProgram,
    ConnectionMaximumDegree,
    ConnectionMaximumDistance,
    ConnectionMinimumDegree,
    ConnectionSeed,
    MazeSeed,
    ParametricShape,
    ParametricTurns,
    ParametricRadialSpacing,
    ParametricPhase,
    ParametricWinding,
    ParametricRepetition,
    ParametricOffsetSpacing,
    ParametricOffsetSides,
    ParametricOffsetCleanup,
    ParametricStackDirection,
    ParametricStackSpacingMultiplier,
    AlongParametricInterval,
    AlongParametricPhase,
}

/// The authoritative descriptor order.  Keeping this list beside the field
/// enum makes missing or duplicate command-facing schema fields testable
/// without deriving any ordering from frontend state or allocation order.
pub const PROPERTY_FIELD_IDS: &[PropertyFieldId] = &[
    PropertyFieldId::SourceReference,
    PropertyFieldId::DensityAcrossX,
    PropertyFieldId::DensityAcrossY,
    PropertyFieldId::DensityAspectLocked,
    PropertyFieldId::RotationDegrees,
    PropertyFieldId::TranslationX,
    PropertyFieldId::TranslationY,
    PropertyFieldId::MarkMinimumFill,
    PropertyFieldId::MarkMaximumFill,
    PropertyFieldId::ConnectedMinimumThickness,
    PropertyFieldId::ConnectedMaximumThickness,
    PropertyFieldId::ShapeRotationDegrees,
    PropertyFieldId::LegacyMappingComponent,
    PropertyFieldId::LegacyMappingPlacement,
    PropertyFieldId::ModeledMappingComponent,
    PropertyFieldId::ModeledMappingPlacement,
    PropertyFieldId::ModeledMappingInverted,
    PropertyFieldId::ModeledMappingGain,
    PropertyFieldId::ModeledMappingBias,
    PropertyFieldId::Paint,
    PropertyFieldId::ColorRed,
    PropertyFieldId::ColorGreen,
    PropertyFieldId::ColorBlue,
    PropertyFieldId::ColorAlpha,
    PropertyFieldId::Opacity,
    PropertyFieldId::Visibility,
    PropertyFieldId::DefinitionSelection,
    PropertyFieldId::CoverageGuardSteps,
    PropertyFieldId::CoverageAdditionalMargin,
    PropertyFieldId::GuideBaselineAngle,
    PropertyFieldId::GuidePhase,
    PropertyFieldId::GuideSpacingMultiplier,
    PropertyFieldId::GuidePrototype,
    PropertyFieldId::GuideAuthoredStructure,
    PropertyFieldId::GuideArcCenterX,
    PropertyFieldId::GuideArcCenterY,
    PropertyFieldId::GuideArcRadius,
    PropertyFieldId::GuideArcStartAngle,
    PropertyFieldId::GuideArcSweepAngle,
    PropertyFieldId::GuideRepetition,
    PropertyFieldId::GuideOffsetSpacing,
    PropertyFieldId::GuideOffsetSides,
    PropertyFieldId::GuideOffsetCleanup,
    PropertyFieldId::GuideStackDirection,
    PropertyFieldId::GuideStackSpacingMultiplier,
    PropertyFieldId::IntersectionDimensions,
    PropertyFieldId::IntersectionMergeEpsilon,
    PropertyFieldId::AlongGuideDimensions,
    PropertyFieldId::AlongGuideIntervalMultiplier,
    PropertyFieldId::AlongGuidePhase,
    PropertyFieldId::RandomCharacter,
    PropertyFieldId::RandomEvenMinimumCenterDistance,
    PropertyFieldId::RandomClusterDensity,
    PropertyFieldId::RandomClusterSpread,
    PropertyFieldId::RandomClusterStrength,
    PropertyFieldId::RandomSeed,
    PropertyFieldId::RandomDensityModulation,
    PropertyFieldId::ArtworkWeightMappingComponent,
    PropertyFieldId::ArtworkWeightMappingPlacement,
    PropertyFieldId::ArtworkWeightMappingInverted,
    PropertyFieldId::ArtworkWeightMappingGain,
    PropertyFieldId::ArtworkWeightMappingBias,
    PropertyFieldId::ArtworkWeightStrength,
    PropertyFieldId::ArtworkWeightResponse,
    PropertyFieldId::RandomExclusion,
    PropertyFieldId::ExclusionMinimumCenterDistance,
    PropertyFieldId::VisibleMarkMargin,
    PropertyFieldId::VisibleMarkSizingPolicy,
    PropertyFieldId::RandomMaximumAttempts,
    PropertyFieldId::RandomMaximumNeighborChecks,
    PropertyFieldId::OutputSiteProduct,
    PropertyFieldId::OutputPrototype,
    PropertyFieldId::OutputAuthoredClosedShape,
    PropertyFieldId::OutputOrientation,
    PropertyFieldId::OutputOrientationDimension,
    PropertyFieldId::ConnectionProgram,
    PropertyFieldId::ConnectionMaximumDegree,
    PropertyFieldId::ConnectionMaximumDistance,
    PropertyFieldId::ConnectionMinimumDegree,
    PropertyFieldId::ConnectionSeed,
    PropertyFieldId::MazeSeed,
    PropertyFieldId::ParametricShape,
    PropertyFieldId::ParametricTurns,
    PropertyFieldId::ParametricRadialSpacing,
    PropertyFieldId::ParametricPhase,
    PropertyFieldId::ParametricWinding,
    PropertyFieldId::ParametricRepetition,
    PropertyFieldId::ParametricOffsetSpacing,
    PropertyFieldId::ParametricOffsetSides,
    PropertyFieldId::ParametricOffsetCleanup,
    PropertyFieldId::ParametricStackDirection,
    PropertyFieldId::ParametricStackSpacingMultiplier,
    PropertyFieldId::AlongParametricInterval,
    PropertyFieldId::AlongParametricPhase,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyTarget {
    Document,
    Channel(ChannelId),
    /// Addresses one channel's inherited/additive response for an explicit structural output.
    ChannelOutput(ChannelId, PatternOutputLayerId),
    Definition(PatternDefinitionId),
    Mechanism(PatternDefinitionId, PatternMechanismId),
    OutputLayer(PatternDefinitionId, PatternOutputLayerId),
    GuideDimension(PatternDefinitionId, PatternMechanismId, GuideDimensionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyValueKind {
    FiniteF64,
    U32,
    Boolean,
    StableIdReference,
    EnumChoice,
}

/// Stable finite discriminants for descriptor enum fields. Payload values are
/// addressed by their own descriptors and never encoded as display strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyEnumChoice {
    SourceMappingComponent(SourceMappingComponent),
    SourcePlacement(SourcePlacement),
    Paint(PaintKind),
    RandomCharacter(RandomCharacterKind),
    DensityModulation(DensityModulationKind),
    ArtworkWeightResponse(ArtworkWeightResponse),
    Exclusion(ExclusionKind),
    VisibleMarkSizingPolicy(VisibleMarkSizingPolicy),
    MarkPrototype(MarkPrototypeKind),
    MarkOrientation(MarkOrientationKind),
    ConnectionProgram(ConnectionProgramKind),
    GuidePrototype(GuidePrototypeKind),
    GuideRepetition(GuideRepetitionKind),
    SpiralShape(SpiralShape),
    CurveWinding(CurveWinding),
    CurveRepetition(GuideRepetitionKind),
    OffsetSides(OffsetSides),
    OffsetCleanup(OffsetCleanup),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintKind {
    Solid,
    SampledSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomCharacterKind {
    RawUniform,
    Even,
    Clustered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DensityModulationKind {
    Uniform,
    ArtworkWeighted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionKind {
    None,
    MinimumCenterDistance,
    VisibleMarkMargin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkPrototypeKind {
    Circle,
    AuthoredClosedShape,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkOrientationKind {
    Fixed,
    GuideTangent,
    GuideNormal,
}
/// Stable generic-guide prototype choices; payloads have separate fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidePrototypeKind {
    AuthoredOpenPath,
    CircularArc,
}
/// Stable bounded generic-guide repetition choices; payloads have separate fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideRepetitionKind {
    Single,
    TransformStack,
    NormalOffset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyBounds {
    pub minimum: Option<f64>,
    pub minimum_inclusive: bool,
    pub maximum: Option<f64>,
    pub maximum_inclusive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyUnit {
    None,
    Density,
    Degrees,
    Phase,
    DocumentDistance,
    NormalizedComponent,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyDependency {
    Always,
    ModeledChannel,
    SolidPaint,
    SampledPaint,
    StraightGuideDimension,
    GenericGuideDimension,
    AuthoredPathPrototype,
    CircularArc,
    TransformStack,
    NormalOffset,
    IntersectionProduct,
    AlongGuideProduct,
    RandomProcess,
    EvenRandomProcess,
    ClusteredRandomProcess,
    ArtworkWeightedDensity,
    MinimumCenterExclusion,
    VisibleMarkExclusion,
    MarkPrototypeOutput,
    AuthoredClosedShapeMark,
    GuidedOutputOrientation,
}

/// The schema predicate that decides whether an otherwise stable field is
/// active.  This is capability metadata, not a stored value or UI policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyApplicability {
    Always,
    ModeledChannel,
    SolidPaint,
    StraightGuideDimension,
    GenericGuideDimension,
    AuthoredPathPrototype,
    CircularArc,
    TransformStack,
    NormalOffset,
    IntersectionProduct,
    AlongGuideProduct,
    RandomProcess,
    EvenRandomProcess,
    ClusteredRandomProcess,
    ArtworkWeightedDensity,
    MinimumCenterExclusion,
    VisibleMarkExclusion,
    MarkPrototypeOutput,
    AuthoredClosedShapeMark,
    GuidedOutputOrientation,
    ConnectionPathOutput,
    RandomLinkProgram,
    SeededConnectionProgram,
    MazeWallProgram,
    CurrentPaint,
    CurrentDensityModulation,
    CurrentExclusion,
}

/// Schema constraint explanation only; this never calculates geometry or
/// captures a persisted value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralSupportConstraint {
    None,
    /// The active maximum fill derives the conservative family coverage envelope.
    MaximumFillDefinesCoverage,
    /// Visible-mark exclusion derives its separation from active realized support.
    VisibleMarkMarginUsesMaximumRealizedSupport,
}

/// Validation shape for stable references. Collection fields carry their
/// cardinality here; exact ID existence, canonical stored order, and
/// duplicate rejection remain document-relative typed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyReferenceConstraint {
    NotReference,
    Singular,
    OrderedUniqueCollection {
        minimum_items: u8,
        maximum_items: u8,
    },
}

/// Choice resolution is explicit when legal choices depend on the active
/// channel model/role; all other enum fields use `PropertyFieldContract::choices`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyChoicePolicy {
    Static,
    ModelRolePaint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyCommandKind {
    SetParametricCurve,
    SetSourceReference,
    SetChannelDensityDelta,
    SetDocumentPatternSettings,
    SetChannelPatternRotationDelta,
    SetChannelShapeRotationDelta,
    SetTranslationAxis,
    SetChannelGeometryResponseDelta,
    SetLegacyMappingField,
    SetModeledMappingField,
    SetPaint,
    SetColorComponent,
    SetOpacity,
    SetVisibility,
    SetChannelPatternDefinitionOverride,
    SetGuideBaselineAngle,
    SetGuidePhase,
    SetGuideSpacingMultiplier,
    SetGuidePrototype,
    SetGuideAuthoredStructure,
    SetGuideArcCenterX,
    SetGuideArcCenterY,
    SetGuideArcRadius,
    SetGuideArcStartAngle,
    SetGuideArcSweepAngle,
    SetGuideRepetition,
    SetGuideOffsetSpacing,
    SetGuideOffsetSides,
    SetGuideOffsetCleanup,
    SetGuideStackDirection,
    SetGuideStackSpacingMultiplier,
    SetIntersectionDimensions,
    SetIntersectionMergeEpsilon,
    SetAlongGuideDimensions,
    SetAlongGuideIntervalMultiplier,
    SetAlongGuidePhase,
    SetRandomCharacter,
    SetRandomSeed,
    SetRandomEvenMinimumCenterDistance,
    SetRandomClusterDensity,
    SetRandomClusterSpread,
    SetRandomClusterStrength,
    SetDensityModulationVariant,
    SetArtworkWeightMappingComponent,
    SetArtworkWeightMappingPlacement,
    SetArtworkWeightMappingInverted,
    SetArtworkWeightMappingGain,
    SetArtworkWeightMappingBias,
    SetArtworkWeightStrength,
    SetArtworkWeightResponse,
    SetExclusionVariant,
    SetExclusionMinimumCenterDistance,
    SetVisibleMarkMargin,
    SetVisibleMarkSizingPolicy,
    SetRandomMaximumAttempts,
    SetRandomMaximumNeighborChecks,
    SetOutputSiteProduct,
    SetOutputMarkPrototype,
    SetOutputAuthoredClosedShape,
    SetOutputOrientation,
    SetOutputOrientationDimension,
    SetConnectionProgram,
    SetConnectionMaximumDegree,
    SetConnectionMaximumDistance,
    SetConnectionMinimumDegree,
    SetConnectionSeed,
    SetMazeSeed,
    SetCoverageGuardSteps,
    SetCoverageAdditionalMargin,
}

/// Identifies the sole persisted authority edited through a descriptor.
/// Effective values remain projections and are never an additional store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyAuthority {
    DocumentBase,
    ChannelDelta,
    ChannelSpecific,
    StructuralDefinition,
}

/// Reports whether a displayed channel value is inherited, explicitly
/// overridden, or unrelated to Stage 20G pattern authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInheritance {
    NotApplicable,
    Inherited,
    Explicit,
}

/// A deterministic read-only descriptor derived from the validated current
/// schema. `choices` uses stable discriminants, never localized labels.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDescriptor {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub value_kind: PropertyValueKind,
    pub choices: &'static [PropertyEnumChoice],
    pub bounds: Option<PropertyBounds>,
    pub unit: PropertyUnit,
    pub dependency: PropertyDependency,
    pub applicability: PropertyApplicability,
    pub invalidation: InvalidationLevel,
    pub copy_on_edit_escalates_to_family: bool,
    pub structural_support: StructuralSupportConstraint,
    pub reference_constraint: PropertyReferenceConstraint,
    pub choice_policy: PropertyChoicePolicy,
    pub authority: PropertyAuthority,
    pub reset_capable: bool,
}

/// An immutable typed value paired with one active descriptor.  This is a
/// read boundary for frontends and tooling, not a second document shape.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyCurrentValue {
    pub descriptor: PropertyDescriptor,
    pub value: PropertyCurrentValueKind,
    pub authored_value: Option<PropertyCurrentValueKind>,
    pub inheritance: PropertyInheritance,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyCurrentValueKind {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    EnumChoice(PropertyEnumChoice),
    Reference(PropertyReferenceValue),
    ReferenceCollection(Vec<PropertyReferenceValue>),
}

/// Stable identifiers remain typed at the read boundary; presentation code
/// may format them, but cannot turn them into positional aliases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyReferenceValue {
    Source(SourceReference),
    Definition(PatternDefinitionId),
    Mechanism(PatternMechanismId),
    GuideDimension(GuideDimensionId),
    AuthoredStructure(AuthoredStructureId),
}

/// A non-persisted, immutable proposal for the complete payload needed when a
/// selector changes to a payload-bearing alternative.  It is intentionally
/// separate from active property descriptors: these fields are transition
/// inputs, not current document capabilities.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionDraft {
    selector: PropertyDescriptor,
    /// Private immutable structural base. Like command bases, this is
    /// lifecycle-only stale detection and never document/persistence state.
    base_definition: PatternDefinition,
    base_choice: PropertyEnumChoice,
    choice: PropertyEnumChoice,
    fields: Vec<VariantTransitionField>,
}

impl VariantTransitionDraft {
    pub fn selector(&self) -> &PropertyDescriptor {
        &self.selector
    }

    pub fn choice(&self) -> PropertyEnumChoice {
        self.choice
    }

    pub fn fields(&self) -> &[VariantTransitionField] {
        &self.fields
    }

    /// Returns a new draft after validating the complete update set.  The
    /// update list is intentionally keyed by stable field and target, never
    /// list index; duplicate, missing, or foreign updates are rejected.
    pub fn with_updates(
        &self,
        updates: &[VariantTransitionFieldUpdate],
    ) -> Result<Self, ValidationError> {
        let mut next = self.clone();
        let mut seen = HashSet::new();
        for update in updates {
            let key = (update.field, update.target);
            if !seen.insert(key) {
                return Err(ValidationError::new(
                    "transition_draft.update",
                    "transition draft contains a duplicate field update",
                ));
            }
            let field = next
                .fields
                .iter_mut()
                .find(|field| field.field == update.field && field.target == update.target)
                .ok_or_else(|| {
                    ValidationError::new(
                        "transition_draft.update",
                        "transition draft update targets an absent field",
                    )
                })?;
            validate_transition_field_value(field, &update.value)?;
            field.value = update.value.clone();
        }
        Ok(next)
    }

    /// Validates this transient draft against the current immutable document
    /// and returns the existing typed edit.  It never mutates a document or
    /// history owner.
    pub fn finalize(&self, document: &Document) -> Result<PatternDefinitionEdit, ValidationError> {
        validate_transition_draft_shape(self, document)?;
        let edit = transition_draft_edit(self)?;
        let definition_id =
            transition_definition_id(self.selector.target).expect("validated selector target");
        let definition = document.definition(definition_id).ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition definition is missing",
            )
        })?;
        validate_definition_edit(definition, &edit)?;
        let mut candidate = definition.clone();
        apply_definition_edit(&mut candidate, &edit);
        if &candidate == definition {
            return Err(ValidationError::new(
                "transition_draft.confirm",
                "transition draft is a semantic no-op",
            ));
        }
        validate_definition(&candidate)?;
        Ok(edit)
    }
}

/// One complete typed transition payload field. `contract` is copied from the
/// immutable schema field contract, never from a frontend policy or document value.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionField {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub contract: PropertyFieldContract,
    pub value: VariantTransitionValue,
    pub reference_choices: Vec<PropertyReferenceValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VariantTransitionValue {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    EnumChoice(PropertyEnumChoice),
    /// `None` is an explicit incomplete required reference, used only where
    /// transition policy cannot choose a stable identity on the user's behalf.
    StableReference(Option<PropertyReferenceValue>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariantTransitionFieldUpdate {
    pub field: PropertyFieldId,
    pub target: PropertyTarget,
    pub value: VariantTransitionValue,
}

enum ChannelPropertyState<'a> {
    Legacy(&'a ChannelState),
    Modeled(&'a ModeledChannelState),
}

impl ChannelPropertyState<'_> {
    fn opacity(&self) -> f64 {
        match self {
            Self::Legacy(channel) => channel.appearance.opacity,
            Self::Modeled(channel) => channel.opacity,
        }
    }
    fn visible(&self) -> bool {
        match self {
            Self::Legacy(channel) => channel.appearance.visible,
            Self::Modeled(channel) => channel.visible,
        }
    }
    fn color(&self) -> &ColorValue {
        match self {
            Self::Legacy(channel) => &channel.appearance.color,
            Self::Modeled(channel) => match &channel.paint {
                ChannelPaint::Solid(color) => color,
                ChannelPaint::SampledSource => {
                    unreachable!("sampled paint has no color descriptors")
                }
            },
        }
    }
    fn legacy_mapping(&self) -> Option<ChannelSourceMapping> {
        match self {
            Self::Legacy(channel) => Some(channel.source_mapping),
            Self::Modeled(_) => None,
        }
    }
    fn mapping(&self) -> Option<SourceMapping> {
        match self {
            Self::Legacy(_) => None,
            Self::Modeled(channel) => Some(channel.mapping),
        }
    }
    fn paint(&self) -> Option<&ChannelPaint> {
        match self {
            Self::Legacy(_) => None,
            Self::Modeled(channel) => Some(&channel.paint),
        }
    }
}

fn property_value_for_mechanism(
    field: PropertyFieldId,
    mechanism: &PatternMechanism,
) -> PropertyCurrentValueKind {
    match (field, mechanism) {
        (
            PropertyFieldId::ParametricShape,
            PatternMechanism::ParametricCurveSource {
                curve: ParametricCurve::Spiral(spiral),
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SpiralShape(spiral.shape)),
        (
            PropertyFieldId::ParametricTurns,
            PatternMechanism::ParametricCurveSource {
                curve: ParametricCurve::Spiral(spiral),
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(spiral.turns),
        (
            PropertyFieldId::ParametricRadialSpacing,
            PatternMechanism::ParametricCurveSource {
                curve: ParametricCurve::Spiral(spiral),
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(spiral.radial_spacing),
        (
            PropertyFieldId::ParametricPhase,
            PatternMechanism::ParametricCurveSource {
                curve: ParametricCurve::Spiral(spiral),
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(spiral.phase_degrees),
        (
            PropertyFieldId::ParametricWinding,
            PatternMechanism::ParametricCurveSource {
                curve: ParametricCurve::Spiral(spiral),
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::CurveWinding(spiral.winding)),
        (
            PropertyFieldId::ParametricRepetition,
            PatternMechanism::ParametricCurveSource { repetition, .. },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::CurveRepetition(
            match repetition {
                CurveRepetition::Single => GuideRepetitionKind::Single,
                CurveRepetition::TransformStack { .. } => GuideRepetitionKind::TransformStack,
                CurveRepetition::NormalOffset { .. } => GuideRepetitionKind::NormalOffset,
            },
        )),
        (
            PropertyFieldId::ParametricStackDirection,
            PatternMechanism::ParametricCurveSource {
                repetition:
                    CurveRepetition::TransformStack {
                        direction_degrees, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*direction_degrees),
        (
            PropertyFieldId::ParametricStackSpacingMultiplier,
            PatternMechanism::ParametricCurveSource {
                repetition:
                    CurveRepetition::TransformStack {
                        spacing_multiplier, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*spacing_multiplier),
        (
            PropertyFieldId::ParametricOffsetSpacing,
            PatternMechanism::ParametricCurveSource {
                repetition: CurveRepetition::NormalOffset { spacing, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*spacing),
        (
            PropertyFieldId::ParametricOffsetSides,
            PatternMechanism::ParametricCurveSource {
                repetition: CurveRepetition::NormalOffset { sides, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::OffsetSides(*sides)),
        (
            PropertyFieldId::ParametricOffsetCleanup,
            PatternMechanism::ParametricCurveSource {
                repetition: CurveRepetition::NormalOffset { .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::OffsetCleanup(
            OffsetCleanup::DissolveCrossings,
        )),
        (
            PropertyFieldId::AlongParametricInterval,
            PatternMechanism::AlongParametricCurveSites { interval, .. },
        ) => PropertyCurrentValueKind::FiniteF64(*interval),
        (
            PropertyFieldId::AlongParametricPhase,
            PatternMechanism::AlongParametricCurveSites { phase, .. },
        ) => PropertyCurrentValueKind::FiniteF64(*phase),
        (
            PropertyFieldId::IntersectionDimensions,
            PatternMechanism::SelectedGuideIntersections { dimensions, .. },
        ) => PropertyCurrentValueKind::ReferenceCollection(
            dimensions
                .iter()
                .copied()
                .map(PropertyReferenceValue::GuideDimension)
                .collect(),
        ),
        (
            PropertyFieldId::IntersectionMergeEpsilon,
            PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. },
        ) => PropertyCurrentValueKind::FiniteF64(*merge_epsilon),
        (
            PropertyFieldId::AlongGuideDimensions,
            PatternMechanism::AlongGuideSites { dimensions, .. },
        ) => PropertyCurrentValueKind::ReferenceCollection(
            dimensions
                .iter()
                .copied()
                .map(PropertyReferenceValue::GuideDimension)
                .collect(),
        ),
        (
            PropertyFieldId::AlongGuideIntervalMultiplier,
            PatternMechanism::AlongGuideSites {
                interval_multiplier,
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*interval_multiplier),
        (PropertyFieldId::AlongGuidePhase, PatternMechanism::AlongGuideSites { phase, .. }) => {
            PropertyCurrentValueKind::FiniteF64(*phase)
        }
        (
            PropertyFieldId::RandomCharacter,
            PatternMechanism::RandomSiteProcess { character, .. },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::RandomCharacter(
            match character {
                RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
                RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
                RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
            },
        )),
        (PropertyFieldId::RandomSeed, PatternMechanism::RandomSiteProcess { seed, .. }) => {
            PropertyCurrentValueKind::U32(*seed)
        }
        (
            PropertyFieldId::RandomEvenMinimumCenterDistance,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Even {
                        minimum_center_distance,
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*minimum_center_distance),
        (
            PropertyFieldId::RandomClusterDensity,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_density, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_density),
        (
            PropertyFieldId::RandomClusterSpread,
            PatternMechanism::RandomSiteProcess {
                character: RandomSiteCharacter::Clustered { cluster_spread, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_spread),
        (
            PropertyFieldId::RandomClusterStrength,
            PatternMechanism::RandomSiteProcess {
                character:
                    RandomSiteCharacter::Clustered {
                        cluster_strength, ..
                    },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*cluster_strength),
        (
            PropertyFieldId::RandomDensityModulation,
            PatternMechanism::SiteDensityModulation { modulation, .. },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::DensityModulation(
            match modulation {
                SiteDensityModulation::Uniform => DensityModulationKind::Uniform,
                SiteDensityModulation::ArtworkWeighted { .. } => {
                    DensityModulationKind::ArtworkWeighted
                }
            },
        )),
        (
            PropertyFieldId::ArtworkWeightMappingComponent,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
            mapping.component,
        )),
        (
            PropertyFieldId::ArtworkWeightMappingPlacement,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::SourcePlacement(
            mapping.placement,
        )),
        (
            PropertyFieldId::ArtworkWeightMappingInverted,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::Boolean(mapping.inverted),
        (
            PropertyFieldId::ArtworkWeightMappingGain,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(mapping.gain),
        (
            PropertyFieldId::ArtworkWeightMappingBias,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(mapping.bias),
        (
            PropertyFieldId::ArtworkWeightStrength,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { strength, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*strength),
        (
            PropertyFieldId::ArtworkWeightResponse,
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { response, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(
            *response,
        )),
        (PropertyFieldId::RandomExclusion, PatternMechanism::SiteExclusion { policy, .. }) => {
            PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::Exclusion(match policy {
                SiteExclusionPolicy::None => ExclusionKind::None,
                SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                    ExclusionKind::MinimumCenterDistance
                }
                SiteExclusionPolicy::VisibleMarkMargin { .. } => ExclusionKind::VisibleMarkMargin,
            }))
        }
        (
            PropertyFieldId::ExclusionMinimumCenterDistance,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*minimum),
        (
            PropertyFieldId::VisibleMarkMargin,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::VisibleMarkMargin { margin, .. },
                ..
            },
        ) => PropertyCurrentValueKind::FiniteF64(*margin),
        (
            PropertyFieldId::VisibleMarkSizingPolicy,
            PatternMechanism::SiteExclusion {
                policy: SiteExclusionPolicy::VisibleMarkMargin { sizing, .. },
                ..
            },
        ) => PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::VisibleMarkSizingPolicy(
            *sizing,
        )),
        (
            PropertyFieldId::RandomMaximumAttempts,
            PatternMechanism::RandomSiteProduct {
                maximum_attempts, ..
            },
        ) => PropertyCurrentValueKind::U32(*maximum_attempts),
        (
            PropertyFieldId::RandomMaximumNeighborChecks,
            PatternMechanism::RandomSiteProduct {
                maximum_neighbor_checks,
                ..
            },
        ) => PropertyCurrentValueKind::U32(*maximum_neighbor_checks),
        _ => unreachable!("active mechanism descriptor field"),
    }
}

fn transition_definition_id(target: PropertyTarget) -> Option<PatternDefinitionId> {
    match target {
        PropertyTarget::Mechanism(definition_id, _)
        | PropertyTarget::OutputLayer(definition_id, _) => Some(definition_id),
        _ => None,
    }
}

fn transition_field(
    field: PropertyFieldId,
    target: PropertyTarget,
    value: VariantTransitionValue,
    reference_choices: Vec<PropertyReferenceValue>,
) -> VariantTransitionField {
    VariantTransitionField {
        field,
        target,
        contract: property_field_contract(field),
        value,
        reference_choices,
    }
}

/// Resolves the complete explicit payload fields for one supported compound selector transition.
///
/// # Errors
///
/// Returns stable selector, target, or variant diagnostics without mutating the document.
fn transition_fields_for(
    document: &Document,
    selector: &PropertyDescriptor,
    base_choice: PropertyEnumChoice,
    choice: PropertyEnumChoice,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    match (selector.field, selector.target, base_choice, choice) {
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::RandomCharacter(base),
            PropertyEnumChoice::RandomCharacter(choice),
        ) => random_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::DensityModulation(base),
            PropertyEnumChoice::DensityModulation(choice),
        ) => modulation_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(definition_id, mechanism_id),
            PropertyEnumChoice::Exclusion(base),
            PropertyEnumChoice::Exclusion(choice),
        ) => exclusion_transition_fields(document, definition_id, mechanism_id, base, choice),
        (
            PropertyFieldId::OutputOrientation,
            PropertyTarget::OutputLayer(definition_id, output_layer_id),
            PropertyEnumChoice::MarkOrientation(base),
            PropertyEnumChoice::MarkOrientation(choice),
        ) => orientation_transition_fields(document, definition_id, output_layer_id, base, choice),
        (
            PropertyFieldId::OutputPrototype,
            PropertyTarget::OutputLayer(definition_id, output_layer_id),
            PropertyEnumChoice::MarkPrototype(base),
            PropertyEnumChoice::MarkPrototype(choice),
        ) => {
            mark_prototype_transition_fields(document, definition_id, output_layer_id, base, choice)
        }
        _ => Err(ValidationError::new(
            "transition_draft.selector",
            "selector target or choice does not support compound transition drafts",
        )),
    }
}

fn transition_mechanism(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
) -> Result<&PatternMechanism, ValidationError> {
    document
        .definition(definition_id)
        .and_then(|definition| {
            definition
                .mechanisms
                .iter()
                .find(|mechanism| mechanism.id() == mechanism_id)
        })
        .ok_or_else(|| {
            ValidationError::new("transition_draft.target", "transition mechanism is missing")
        })
}

fn random_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: RandomCharacterKind,
    choice: RandomCharacterKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::RandomSiteProcess { character, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not a random process",
        ));
    };
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    match choice {
        RandomCharacterKind::RawUniform => Ok(Vec::new()),
        RandomCharacterKind::Even => {
            let value = if base == choice {
                match character {
                    RandomSiteCharacter::Even {
                        minimum_center_distance,
                    } => *minimum_center_distance,
                    _ => unreachable!("base selector is current"),
                }
            } else {
                1.0
            };
            Ok(vec![transition_field(
                PropertyFieldId::RandomEvenMinimumCenterDistance,
                target,
                VariantTransitionValue::FiniteF64(value),
                Vec::new(),
            )])
        }
        RandomCharacterKind::Clustered => {
            let (density, spread, strength) = if base == choice {
                match character {
                    RandomSiteCharacter::Clustered {
                        cluster_density,
                        cluster_spread,
                        cluster_strength,
                    } => (*cluster_density, *cluster_spread, *cluster_strength),
                    _ => unreachable!("base selector is current"),
                }
            } else {
                (1.0, 1.0, 1.0)
            };
            Ok(vec![
                transition_field(
                    PropertyFieldId::RandomClusterDensity,
                    target,
                    VariantTransitionValue::FiniteF64(density),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::RandomClusterSpread,
                    target,
                    VariantTransitionValue::FiniteF64(spread),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::RandomClusterStrength,
                    target,
                    VariantTransitionValue::FiniteF64(strength),
                    Vec::new(),
                ),
            ])
        }
    }
}

fn modulation_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: DensityModulationKind,
    choice: DensityModulationKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::SiteDensityModulation { modulation, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not density modulation",
        ));
    };
    if choice == DensityModulationKind::Uniform {
        return Ok(Vec::new());
    }
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    let (mapping, strength, response) = if base == choice {
        match modulation {
            SiteDensityModulation::ArtworkWeighted {
                mapping,
                strength,
                response,
            } => (*mapping, *strength, *response),
            _ => unreachable!("base selector is current"),
        }
    } else {
        (
            SourceMapping::canonical(SourceMappingComponent::Luminance),
            1.0,
            ArtworkWeightResponse::Linear,
        )
    };
    Ok(vec![
        transition_field(
            PropertyFieldId::ArtworkWeightMappingComponent,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                mapping.component,
            )),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingPlacement,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::SourcePlacement(
                mapping.placement,
            )),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingInverted,
            target,
            VariantTransitionValue::Boolean(mapping.inverted),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingGain,
            target,
            VariantTransitionValue::FiniteF64(mapping.gain),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightMappingBias,
            target,
            VariantTransitionValue::FiniteF64(mapping.bias),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightStrength,
            target,
            VariantTransitionValue::FiniteF64(strength),
            Vec::new(),
        ),
        transition_field(
            PropertyFieldId::ArtworkWeightResponse,
            target,
            VariantTransitionValue::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(response)),
            Vec::new(),
        ),
    ])
}

fn exclusion_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    mechanism_id: PatternMechanismId,
    base: ExclusionKind,
    choice: ExclusionKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let mechanism = transition_mechanism(document, definition_id, mechanism_id)?;
    let PatternMechanism::SiteExclusion { policy, .. } = mechanism else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not exclusion policy",
        ));
    };
    let target = PropertyTarget::Mechanism(definition_id, mechanism_id);
    match choice {
        ExclusionKind::None => Ok(Vec::new()),
        ExclusionKind::MinimumCenterDistance => {
            let minimum = if base == choice {
                match policy {
                    SiteExclusionPolicy::MinimumCenterDistance { minimum } => *minimum,
                    _ => unreachable!("base selector is current"),
                }
            } else {
                1.0
            };
            Ok(vec![transition_field(
                PropertyFieldId::ExclusionMinimumCenterDistance,
                target,
                VariantTransitionValue::FiniteF64(minimum),
                Vec::new(),
            )])
        }
        ExclusionKind::VisibleMarkMargin => {
            let (margin, sizing) = if base == choice {
                match policy {
                    SiteExclusionPolicy::VisibleMarkMargin { margin, sizing } => (*margin, *sizing),
                    _ => unreachable!("base selector is current"),
                }
            } else {
                (0.0, VisibleMarkSizingPolicy::MaximumSupportRadius)
            };
            Ok(vec![
                transition_field(
                    PropertyFieldId::VisibleMarkMargin,
                    target,
                    VariantTransitionValue::FiniteF64(margin),
                    Vec::new(),
                ),
                transition_field(
                    PropertyFieldId::VisibleMarkSizingPolicy,
                    target,
                    VariantTransitionValue::EnumChoice(
                        PropertyEnumChoice::VisibleMarkSizingPolicy(sizing),
                    ),
                    Vec::new(),
                ),
            ])
        }
    }
}

/// Builds the optional guide-dimension reference required by one orientation transition.
///
/// # Errors
///
/// Returns stable definition/output target diagnostics for an incompatible selector.
fn orientation_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    output_layer_id: PatternOutputLayerId,
    base: MarkOrientationKind,
    choice: MarkOrientationKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let definition = document.definition(definition_id).ok_or_else(|| {
        ValidationError::new(
            "transition_draft.target",
            "transition definition is missing",
        )
    })?;
    let layer = definition
        .output_layers
        .iter()
        .find(|layer| layer.id() == output_layer_id)
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition output layer is missing",
            )
        })?;
    if !matches!(layer, PatternOutputLayer::MarkPrototype { .. }) {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not a mark-prototype output",
        ));
    }
    if choice == MarkOrientationKind::Fixed {
        return Ok(Vec::new());
    }
    let reference_choices = definition
        .mechanisms
        .iter()
        .flat_map(|mechanism| match mechanism {
            PatternMechanism::StraightGuideDimensions { dimensions, .. } => dimensions
                .iter()
                .map(|dimension| PropertyReferenceValue::GuideDimension(dimension.id))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    let existing = if base == choice {
        match layer {
            PatternOutputLayer::MarkPrototype {
                orientation:
                    MarkOrientation::GuideTangent { dimension_id }
                    | MarkOrientation::GuideNormal { dimension_id },
                ..
            } => Some(PropertyReferenceValue::GuideDimension(*dimension_id)),
            _ => unreachable!("base selector is current"),
        }
    } else {
        None
    };
    Ok(vec![transition_field(
        PropertyFieldId::OutputOrientationDimension,
        PropertyTarget::OutputLayer(definition_id, output_layer_id),
        VariantTransitionValue::StableReference(existing),
        reference_choices,
    )])
}

/// Builds the explicit stable-reference payload required when a mark output switches variants.
///
/// # Errors
///
/// Returns stable target diagnostics when the output layer is absent or not a typed mark layer.
fn mark_prototype_transition_fields(
    document: &Document,
    definition_id: PatternDefinitionId,
    output_layer_id: PatternOutputLayerId,
    base: MarkPrototypeKind,
    choice: MarkPrototypeKind,
) -> Result<Vec<VariantTransitionField>, ValidationError> {
    let definition = document.definition(definition_id).ok_or_else(|| {
        ValidationError::new(
            "transition_draft.target",
            "transition definition is missing",
        )
    })?;
    let layer = definition
        .output_layers
        .iter()
        .find(|layer| layer.id() == output_layer_id)
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.target",
                "transition output layer is missing",
            )
        })?;
    let PatternOutputLayer::MarkPrototype { prototype, .. } = layer else {
        return Err(ValidationError::new(
            "transition_draft.target",
            "selector is not a mark-prototype output",
        ));
    };
    if choice == MarkPrototypeKind::Circle {
        return Ok(Vec::new());
    }
    let existing = if base == choice {
        match prototype {
            MarkPrototype::AuthoredClosedShape { structure_id } => {
                Some(PropertyReferenceValue::AuthoredStructure(*structure_id))
            }
            MarkPrototype::Circle => unreachable!("base selector is current"),
        }
    } else {
        None
    };
    let reference_choices = document
        .authored_structures()
        .iter()
        .filter(|structure| structure.kind() == AuthoredStructureKind::ClosedShape)
        .map(|structure| PropertyReferenceValue::AuthoredStructure(structure.id()))
        .collect();
    Ok(vec![transition_field(
        PropertyFieldId::OutputAuthoredClosedShape,
        PropertyTarget::OutputLayer(definition_id, output_layer_id),
        VariantTransitionValue::StableReference(existing),
        reference_choices,
    )])
}

fn validate_transition_field_value(
    field: &VariantTransitionField,
    value: &VariantTransitionValue,
) -> Result<(), ValidationError> {
    let projection = match (field.contract.value_kind, value) {
        (PropertyValueKind::FiniteF64, VariantTransitionValue::FiniteF64(value)) => {
            PropertyFieldValue::FiniteF64(*value)
        }
        (PropertyValueKind::U32, VariantTransitionValue::U32(value)) => {
            PropertyFieldValue::U32(*value)
        }
        (PropertyValueKind::Boolean, VariantTransitionValue::Boolean(value)) => {
            PropertyFieldValue::Boolean(*value)
        }
        (PropertyValueKind::EnumChoice, VariantTransitionValue::EnumChoice(value)) => {
            if !field.contract.choices.contains(value) {
                return Err(ValidationError::new(
                    "transition_draft.value",
                    "transition enum choice is unsupported",
                ));
            }
            PropertyFieldValue::EnumChoice(*value)
        }
        (
            PropertyValueKind::StableIdReference,
            VariantTransitionValue::StableReference(Some(reference)),
        ) => {
            if !field.reference_choices.contains(reference) {
                return Err(ValidationError::new(
                    "transition_draft.reference",
                    "transition reference is missing or incompatible",
                ));
            }
            PropertyFieldValue::StableIdReference
        }
        (PropertyValueKind::StableIdReference, VariantTransitionValue::StableReference(None)) => {
            return Err(ValidationError::new(
                "transition_draft.reference",
                "transition requires an explicit stable reference",
            ));
        }
        _ => {
            return Err(ValidationError::new(
                "transition_draft.value",
                "transition value has the wrong kind",
            ));
        }
    };
    validate_property_field_projection(PropertyCommandFieldProjection {
        field: field.field,
        value: projection,
    })
}

fn validate_transition_draft_shape(
    draft: &VariantTransitionDraft,
    document: &Document,
) -> Result<(), ValidationError> {
    let definition_id = transition_definition_id(draft.selector.target).ok_or_else(|| {
        ValidationError::new(
            "transition_draft.selector",
            "transition selector has no structural definition target",
        )
    })?;
    if document.definition(definition_id) != Some(&draft.base_definition) {
        return Err(ValidationError::new(
            "transition_draft.stale",
            "transition draft definition base is stale",
        ));
    }
    let active = document.property_descriptors();
    if !active.contains(&draft.selector) {
        return Err(ValidationError::new(
            "transition_draft.selector",
            "transition selector is inactive or stale",
        ));
    }
    let current = document
        .property_values()
        .into_iter()
        .find(|value| value.descriptor == draft.selector)
        .and_then(|value| match value.value {
            PropertyCurrentValueKind::EnumChoice(choice) => Some(choice),
            _ => None,
        })
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.selector",
                "transition selector no longer has an enum value",
            )
        })?;
    if current != draft.base_choice {
        return Err(ValidationError::new(
            "transition_draft.stale",
            "transition draft base is stale",
        ));
    }
    let expected =
        transition_fields_for(document, &draft.selector, draft.base_choice, draft.choice)?;
    if expected.len() != draft.fields.len()
        || expected
            .iter()
            .zip(&draft.fields)
            .any(|(expected, actual)| {
                expected.field != actual.field
                    || expected.target != actual.target
                    || expected.contract != actual.contract
                    || expected.reference_choices != actual.reference_choices
            })
    {
        return Err(ValidationError::new(
            "transition_draft.fields",
            "transition draft field order or shape is invalid",
        ));
    }
    for field in &draft.fields {
        validate_transition_field_value(field, &field.value)?;
    }
    Ok(())
}

fn transition_value(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<&VariantTransitionValue, ValidationError> {
    draft
        .fields
        .iter()
        .find(|candidate| candidate.field == field)
        .map(|field| &field.value)
        .ok_or_else(|| {
            ValidationError::new(
                "transition_draft.fields",
                "transition draft is missing a required field",
            )
        })
}

fn transition_f64(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::FiniteF64(value) => Ok(*value),
        _ => Err(ValidationError::new(
            "transition_draft.value",
            "transition field has the wrong kind",
        )),
    }
}

fn transition_choice(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<PropertyEnumChoice, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::EnumChoice(value) => Ok(*value),
        _ => Err(ValidationError::new(
            "transition_draft.value",
            "transition field has the wrong kind",
        )),
    }
}

fn transition_reference(
    draft: &VariantTransitionDraft,
    field: PropertyFieldId,
) -> Result<PropertyReferenceValue, ValidationError> {
    match transition_value(draft, field)? {
        VariantTransitionValue::StableReference(Some(value)) => Ok(value.clone()),
        _ => Err(ValidationError::new(
            "transition_draft.reference",
            "transition requires an explicit stable reference",
        )),
    }
}

/// Converts one complete validated transition draft into the existing typed structural edit.
///
/// # Errors
///
/// Returns a stable selector, payload-kind, or missing-reference diagnostic.
fn transition_draft_edit(
    draft: &VariantTransitionDraft,
) -> Result<PatternDefinitionEdit, ValidationError> {
    match (draft.selector.field, draft.selector.target, draft.choice) {
        (
            PropertyFieldId::RandomCharacter,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::RandomCharacter(choice),
        ) => {
            let character = match choice {
                RandomCharacterKind::RawUniform => RandomSiteCharacter::RawUniform,
                RandomCharacterKind::Even => RandomSiteCharacter::Even {
                    minimum_center_distance: transition_f64(
                        draft,
                        PropertyFieldId::RandomEvenMinimumCenterDistance,
                    )?,
                },
                RandomCharacterKind::Clustered => RandomSiteCharacter::Clustered {
                    cluster_density: transition_f64(draft, PropertyFieldId::RandomClusterDensity)?,
                    cluster_spread: transition_f64(draft, PropertyFieldId::RandomClusterSpread)?,
                    cluster_strength: transition_f64(
                        draft,
                        PropertyFieldId::RandomClusterStrength,
                    )?,
                },
            };
            Ok(PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id,
                character,
            })
        }
        (
            PropertyFieldId::RandomDensityModulation,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::DensityModulation(choice),
        ) => {
            let modulation = match choice {
                DensityModulationKind::Uniform => SiteDensityModulation::Uniform,
                DensityModulationKind::ArtworkWeighted => {
                    let component = match transition_choice(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingComponent,
                    )? {
                        PropertyEnumChoice::SourceMappingComponent(value) => value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping component choice is invalid",
                            ));
                        }
                    };
                    let placement = match transition_choice(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingPlacement,
                    )? {
                        PropertyEnumChoice::SourcePlacement(value) => value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping placement choice is invalid",
                            ));
                        }
                    };
                    let inverted = match transition_value(
                        draft,
                        PropertyFieldId::ArtworkWeightMappingInverted,
                    )? {
                        VariantTransitionValue::Boolean(value) => *value,
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.value",
                                "mapping inverted value is invalid",
                            ));
                        }
                    };
                    let response =
                        match transition_choice(draft, PropertyFieldId::ArtworkWeightResponse)? {
                            PropertyEnumChoice::ArtworkWeightResponse(value) => value,
                            _ => {
                                return Err(ValidationError::new(
                                    "transition_draft.value",
                                    "artwork response choice is invalid",
                                ));
                            }
                        };
                    SiteDensityModulation::ArtworkWeighted {
                        mapping: SourceMapping {
                            component,
                            placement,
                            inverted,
                            gain: transition_f64(draft, PropertyFieldId::ArtworkWeightMappingGain)?,
                            bias: transition_f64(draft, PropertyFieldId::ArtworkWeightMappingBias)?,
                        },
                        strength: transition_f64(draft, PropertyFieldId::ArtworkWeightStrength)?,
                        response,
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id,
                modulation,
            })
        }
        (
            PropertyFieldId::RandomExclusion,
            PropertyTarget::Mechanism(_, mechanism_id),
            PropertyEnumChoice::Exclusion(choice),
        ) => {
            let policy = match choice {
                ExclusionKind::None => SiteExclusionPolicy::None,
                ExclusionKind::MinimumCenterDistance => {
                    SiteExclusionPolicy::MinimumCenterDistance {
                        minimum: transition_f64(
                            draft,
                            PropertyFieldId::ExclusionMinimumCenterDistance,
                        )?,
                    }
                }
                ExclusionKind::VisibleMarkMargin => {
                    let sizing =
                        match transition_choice(draft, PropertyFieldId::VisibleMarkSizingPolicy)? {
                            PropertyEnumChoice::VisibleMarkSizingPolicy(value) => value,
                            _ => {
                                return Err(ValidationError::new(
                                    "transition_draft.value",
                                    "visible-mark sizing choice is invalid",
                                ));
                            }
                        };
                    SiteExclusionPolicy::VisibleMarkMargin {
                        margin: transition_f64(draft, PropertyFieldId::VisibleMarkMargin)?,
                        sizing,
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id,
                policy,
            })
        }
        (
            PropertyFieldId::OutputOrientation,
            PropertyTarget::OutputLayer(_, output_layer_id),
            PropertyEnumChoice::MarkOrientation(choice),
        ) => {
            let orientation = match choice {
                MarkOrientationKind::Fixed => MarkOrientation::Fixed,
                MarkOrientationKind::GuideTangent => {
                    match transition_reference(draft, PropertyFieldId::OutputOrientationDimension)?
                    {
                        PropertyReferenceValue::GuideDimension(dimension_id) => {
                            MarkOrientation::GuideTangent { dimension_id }
                        }
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.reference",
                                "orientation requires a guide dimension",
                            ));
                        }
                    }
                }
                MarkOrientationKind::GuideNormal => {
                    match transition_reference(draft, PropertyFieldId::OutputOrientationDimension)?
                    {
                        PropertyReferenceValue::GuideDimension(dimension_id) => {
                            MarkOrientation::GuideNormal { dimension_id }
                        }
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.reference",
                                "orientation requires a guide dimension",
                            ));
                        }
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id,
                orientation,
            })
        }
        (
            PropertyFieldId::OutputPrototype,
            PropertyTarget::OutputLayer(_, output_layer_id),
            PropertyEnumChoice::MarkPrototype(choice),
        ) => {
            let prototype = match choice {
                MarkPrototypeKind::Circle => MarkPrototype::Circle,
                MarkPrototypeKind::AuthoredClosedShape => {
                    match transition_reference(draft, PropertyFieldId::OutputAuthoredClosedShape)? {
                        PropertyReferenceValue::AuthoredStructure(structure_id) => {
                            MarkPrototype::AuthoredClosedShape { structure_id }
                        }
                        _ => {
                            return Err(ValidationError::new(
                                "transition_draft.reference",
                                "authored mark prototype requires a closed-shape structure",
                            ));
                        }
                    }
                }
            };
            Ok(PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id,
                prototype,
            })
        }
        _ => Err(ValidationError::new(
            "transition_draft.selector",
            "transition draft selector/choice is invalid",
        )),
    }
}

/// Exhaustive command-facing schema contract for one editable field.  It is
/// intentionally value-free; descriptors supply active-variant applicability,
/// while command validation uses the same field identity and invalidation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyFieldContract {
    pub field: PropertyFieldId,
    pub command_kind: PropertyCommandKind,
    pub value_kind: PropertyValueKind,
    pub choices: &'static [PropertyEnumChoice],
    pub bounds: Option<PropertyBounds>,
    pub unit: PropertyUnit,
    pub applicability: PropertyApplicability,
    pub invalidation: InvalidationLevel,
    pub copy_on_edit_escalates_to_family: bool,
    pub structural_support: StructuralSupportConstraint,
    pub reference_constraint: PropertyReferenceConstraint,
    pub choice_policy: PropertyChoicePolicy,
}

pub const fn property_field_contract(field: PropertyFieldId) -> PropertyFieldContract {
    PropertyFieldContract {
        field,
        command_kind: match field {
            PropertyFieldId::ParametricShape
            | PropertyFieldId::ParametricTurns
            | PropertyFieldId::ParametricRadialSpacing
            | PropertyFieldId::ParametricPhase
            | PropertyFieldId::ParametricWinding
            | PropertyFieldId::ParametricRepetition
            | PropertyFieldId::ParametricOffsetSpacing
            | PropertyFieldId::ParametricOffsetSides
            | PropertyFieldId::ParametricOffsetCleanup
            | PropertyFieldId::ParametricStackDirection
            | PropertyFieldId::ParametricStackSpacingMultiplier
            | PropertyFieldId::AlongParametricInterval
            | PropertyFieldId::AlongParametricPhase => PropertyCommandKind::SetParametricCurve,
            PropertyFieldId::SourceReference => PropertyCommandKind::SetSourceReference,
            PropertyFieldId::DensityAcrossX | PropertyFieldId::DensityAcrossY => {
                PropertyCommandKind::SetChannelDensityDelta
            }
            PropertyFieldId::DensityAspectLocked => PropertyCommandKind::SetDocumentPatternSettings,
            PropertyFieldId::RotationDegrees => PropertyCommandKind::SetChannelPatternRotationDelta,
            PropertyFieldId::TranslationX | PropertyFieldId::TranslationY => {
                PropertyCommandKind::SetTranslationAxis
            }
            PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness => {
                PropertyCommandKind::SetChannelGeometryResponseDelta
            }
            PropertyFieldId::ShapeRotationDegrees => {
                PropertyCommandKind::SetChannelShapeRotationDelta
            }
            PropertyFieldId::LegacyMappingComponent | PropertyFieldId::LegacyMappingPlacement => {
                PropertyCommandKind::SetLegacyMappingField
            }
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias => PropertyCommandKind::SetModeledMappingField,
            PropertyFieldId::Paint => PropertyCommandKind::SetPaint,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha => PropertyCommandKind::SetColorComponent,
            PropertyFieldId::Opacity => PropertyCommandKind::SetOpacity,
            PropertyFieldId::Visibility => PropertyCommandKind::SetVisibility,
            PropertyFieldId::DefinitionSelection => {
                PropertyCommandKind::SetChannelPatternDefinitionOverride
            }
            PropertyFieldId::GuideBaselineAngle => PropertyCommandKind::SetGuideBaselineAngle,
            PropertyFieldId::GuidePhase => PropertyCommandKind::SetGuidePhase,
            PropertyFieldId::GuideSpacingMultiplier => {
                PropertyCommandKind::SetGuideSpacingMultiplier
            }
            PropertyFieldId::GuidePrototype => PropertyCommandKind::SetGuidePrototype,
            PropertyFieldId::GuideAuthoredStructure => {
                PropertyCommandKind::SetGuideAuthoredStructure
            }
            PropertyFieldId::GuideArcCenterX => PropertyCommandKind::SetGuideArcCenterX,
            PropertyFieldId::GuideArcCenterY => PropertyCommandKind::SetGuideArcCenterY,
            PropertyFieldId::GuideArcRadius => PropertyCommandKind::SetGuideArcRadius,
            PropertyFieldId::GuideArcStartAngle => PropertyCommandKind::SetGuideArcStartAngle,
            PropertyFieldId::GuideArcSweepAngle => PropertyCommandKind::SetGuideArcSweepAngle,
            PropertyFieldId::GuideRepetition => PropertyCommandKind::SetGuideRepetition,
            PropertyFieldId::GuideOffsetSpacing => PropertyCommandKind::SetGuideOffsetSpacing,
            PropertyFieldId::GuideOffsetSides => PropertyCommandKind::SetGuideOffsetSides,
            PropertyFieldId::GuideOffsetCleanup => PropertyCommandKind::SetGuideOffsetCleanup,
            PropertyFieldId::GuideStackDirection => PropertyCommandKind::SetGuideStackDirection,
            PropertyFieldId::GuideStackSpacingMultiplier => {
                PropertyCommandKind::SetGuideStackSpacingMultiplier
            }
            PropertyFieldId::IntersectionDimensions => {
                PropertyCommandKind::SetIntersectionDimensions
            }
            PropertyFieldId::IntersectionMergeEpsilon => {
                PropertyCommandKind::SetIntersectionMergeEpsilon
            }
            PropertyFieldId::AlongGuideDimensions => PropertyCommandKind::SetAlongGuideDimensions,
            PropertyFieldId::AlongGuideIntervalMultiplier => {
                PropertyCommandKind::SetAlongGuideIntervalMultiplier
            }
            PropertyFieldId::AlongGuidePhase => PropertyCommandKind::SetAlongGuidePhase,
            PropertyFieldId::RandomCharacter => PropertyCommandKind::SetRandomCharacter,
            PropertyFieldId::RandomSeed => PropertyCommandKind::SetRandomSeed,
            PropertyFieldId::RandomEvenMinimumCenterDistance => {
                PropertyCommandKind::SetRandomEvenMinimumCenterDistance
            }
            PropertyFieldId::RandomClusterDensity => PropertyCommandKind::SetRandomClusterDensity,
            PropertyFieldId::RandomClusterSpread => PropertyCommandKind::SetRandomClusterSpread,
            PropertyFieldId::RandomClusterStrength => PropertyCommandKind::SetRandomClusterStrength,
            PropertyFieldId::RandomDensityModulation => {
                PropertyCommandKind::SetDensityModulationVariant
            }
            PropertyFieldId::ArtworkWeightMappingComponent => {
                PropertyCommandKind::SetArtworkWeightMappingComponent
            }
            PropertyFieldId::ArtworkWeightMappingPlacement => {
                PropertyCommandKind::SetArtworkWeightMappingPlacement
            }
            PropertyFieldId::ArtworkWeightMappingInverted => {
                PropertyCommandKind::SetArtworkWeightMappingInverted
            }
            PropertyFieldId::ArtworkWeightMappingGain => {
                PropertyCommandKind::SetArtworkWeightMappingGain
            }
            PropertyFieldId::ArtworkWeightMappingBias => {
                PropertyCommandKind::SetArtworkWeightMappingBias
            }
            PropertyFieldId::ArtworkWeightStrength => PropertyCommandKind::SetArtworkWeightStrength,
            PropertyFieldId::ArtworkWeightResponse => PropertyCommandKind::SetArtworkWeightResponse,
            PropertyFieldId::RandomExclusion => PropertyCommandKind::SetExclusionVariant,
            PropertyFieldId::ExclusionMinimumCenterDistance => {
                PropertyCommandKind::SetExclusionMinimumCenterDistance
            }
            PropertyFieldId::VisibleMarkMargin => PropertyCommandKind::SetVisibleMarkMargin,
            PropertyFieldId::VisibleMarkSizingPolicy => {
                PropertyCommandKind::SetVisibleMarkSizingPolicy
            }
            PropertyFieldId::RandomMaximumAttempts => PropertyCommandKind::SetRandomMaximumAttempts,
            PropertyFieldId::RandomMaximumNeighborChecks => {
                PropertyCommandKind::SetRandomMaximumNeighborChecks
            }
            PropertyFieldId::OutputSiteProduct => PropertyCommandKind::SetOutputSiteProduct,
            PropertyFieldId::OutputPrototype => PropertyCommandKind::SetOutputMarkPrototype,
            PropertyFieldId::OutputAuthoredClosedShape => {
                PropertyCommandKind::SetOutputAuthoredClosedShape
            }
            PropertyFieldId::OutputOrientation => PropertyCommandKind::SetOutputOrientation,
            PropertyFieldId::OutputOrientationDimension => {
                PropertyCommandKind::SetOutputOrientationDimension
            }
            PropertyFieldId::ConnectionProgram => PropertyCommandKind::SetConnectionProgram,
            PropertyFieldId::ConnectionMaximumDegree => {
                PropertyCommandKind::SetConnectionMaximumDegree
            }
            PropertyFieldId::ConnectionMaximumDistance => {
                PropertyCommandKind::SetConnectionMaximumDistance
            }
            PropertyFieldId::ConnectionMinimumDegree => {
                PropertyCommandKind::SetConnectionMinimumDegree
            }
            PropertyFieldId::ConnectionSeed => PropertyCommandKind::SetConnectionSeed,
            PropertyFieldId::MazeSeed => PropertyCommandKind::SetMazeSeed,
            PropertyFieldId::CoverageGuardSteps => PropertyCommandKind::SetCoverageGuardSteps,
            PropertyFieldId::CoverageAdditionalMargin => {
                PropertyCommandKind::SetCoverageAdditionalMargin
            }
        },
        value_kind: match field {
            PropertyFieldId::SourceReference
            | PropertyFieldId::DefinitionSelection
            | PropertyFieldId::IntersectionDimensions
            | PropertyFieldId::AlongGuideDimensions
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputOrientationDimension
            | PropertyFieldId::GuideAuthoredStructure
            | PropertyFieldId::OutputAuthoredClosedShape => PropertyValueKind::StableIdReference,
            PropertyFieldId::DensityAspectLocked
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::Visibility => PropertyValueKind::Boolean,
            PropertyFieldId::CoverageGuardSteps
            | PropertyFieldId::RandomSeed
            | PropertyFieldId::ConnectionMaximumDegree
            | PropertyFieldId::ConnectionMinimumDegree
            | PropertyFieldId::ConnectionSeed
            | PropertyFieldId::MazeSeed
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyValueKind::U32,
            PropertyFieldId::LegacyMappingComponent
            | PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::Paint
            | PropertyFieldId::RandomCharacter
            | PropertyFieldId::RandomDensityModulation
            | PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightResponse
            | PropertyFieldId::RandomExclusion
            | PropertyFieldId::VisibleMarkSizingPolicy
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputOrientation
            | PropertyFieldId::ConnectionProgram
            | PropertyFieldId::GuidePrototype
            | PropertyFieldId::GuideRepetition
            | PropertyFieldId::GuideOffsetSides
            | PropertyFieldId::GuideOffsetCleanup
            | PropertyFieldId::ParametricShape
            | PropertyFieldId::ParametricWinding
            | PropertyFieldId::ParametricRepetition
            | PropertyFieldId::ParametricOffsetSides
            | PropertyFieldId::ParametricOffsetCleanup => PropertyValueKind::EnumChoice,
            _ => PropertyValueKind::FiniteF64,
        },
        choices: match field {
            PropertyFieldId::LegacyMappingComponent => LEGACY_COMPONENT_CHOICES,
            PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingPlacement => SOURCE_PLACEMENT_CHOICES,
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ArtworkWeightMappingComponent => SOURCE_MAPPING_CHOICES,
            PropertyFieldId::RandomCharacter => RANDOM_CHARACTER_CHOICES,
            PropertyFieldId::RandomDensityModulation => DENSITY_MODULATION_CHOICES,
            PropertyFieldId::ArtworkWeightResponse => ARTWORK_RESPONSE_CHOICES,
            PropertyFieldId::RandomExclusion => EXCLUSION_CHOICES,
            PropertyFieldId::VisibleMarkSizingPolicy => VISIBLE_MARK_SIZING_POLICY_CHOICES,
            PropertyFieldId::OutputPrototype => MARK_PROTOTYPE_CHOICES,
            PropertyFieldId::OutputOrientation => MARK_ORIENTATION_CHOICES,
            PropertyFieldId::GuidePrototype => GUIDE_PROTOTYPE_CHOICES,
            PropertyFieldId::GuideRepetition => GUIDE_REPETITION_CHOICES,
            PropertyFieldId::GuideOffsetSides => OFFSET_SIDES_CHOICES,
            PropertyFieldId::GuideOffsetCleanup => OFFSET_CLEANUP_CHOICES,
            PropertyFieldId::ParametricShape => SPIRAL_SHAPE_CHOICES,
            PropertyFieldId::ParametricWinding => CURVE_WINDING_CHOICES,
            PropertyFieldId::ParametricRepetition => CURVE_REPETITION_CHOICES,
            PropertyFieldId::ParametricOffsetSides => OFFSET_SIDES_CHOICES,
            PropertyFieldId::ParametricOffsetCleanup => OFFSET_CLEANUP_CHOICES,
            PropertyFieldId::ConnectionProgram => CONNECTION_PROGRAM_CHOICES,
            _ => &[],
        },
        bounds: match field {
            PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::GuideSpacingMultiplier
            | PropertyFieldId::GuideArcRadius
            | PropertyFieldId::GuideOffsetSpacing
            | PropertyFieldId::GuideStackSpacingMultiplier
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::RandomEvenMinimumCenterDistance
            | PropertyFieldId::RandomClusterDensity
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::ExclusionMinimumCenterDistance
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => positive_bounds(),
            PropertyFieldId::ConnectionMaximumDegree => Some(PropertyBounds {
                minimum: Some(1.0),
                minimum_inclusive: true,
                maximum: Some(32.0),
                maximum_inclusive: true,
            }),
            PropertyFieldId::ConnectionMinimumDegree => Some(PropertyBounds {
                minimum: Some(0.0),
                minimum_inclusive: true,
                maximum: Some(32.0),
                maximum_inclusive: true,
            }),
            PropertyFieldId::ConnectionMaximumDistance => positive_bounds(),
            PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness => fill_bounds(),
            PropertyFieldId::CoverageAdditionalMargin
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::VisibleMarkMargin
            | PropertyFieldId::IntersectionMergeEpsilon => nonnegative_bounds(),
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::RandomClusterStrength
            | PropertyFieldId::ArtworkWeightStrength => unit_bounds(),
            _ => None,
        },
        unit: match field {
            PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::GuideSpacingMultiplier
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::RandomClusterDensity => PropertyUnit::Density,
            PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::GuideBaselineAngle => PropertyUnit::Degrees,
            PropertyFieldId::GuideArcStartAngle
            | PropertyFieldId::GuideArcSweepAngle
            | PropertyFieldId::GuideStackDirection => PropertyUnit::Degrees,
            PropertyFieldId::GuidePhase | PropertyFieldId::AlongGuidePhase => PropertyUnit::Phase,
            PropertyFieldId::TranslationX
            | PropertyFieldId::TranslationY
            | PropertyFieldId::CoverageAdditionalMargin
            | PropertyFieldId::IntersectionMergeEpsilon
            | PropertyFieldId::RandomEvenMinimumCenterDistance
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::ExclusionMinimumCenterDistance
            | PropertyFieldId::VisibleMarkMargin => PropertyUnit::DocumentDistance,
            PropertyFieldId::ConnectionMaximumDistance => PropertyUnit::DocumentDistance,
            PropertyFieldId::GuideArcCenterX
            | PropertyFieldId::GuideArcCenterY
            | PropertyFieldId::GuideArcRadius
            | PropertyFieldId::GuideOffsetSpacing => PropertyUnit::DocumentDistance,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::RandomClusterStrength
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightStrength => PropertyUnit::NormalizedComponent,
            PropertyFieldId::CoverageGuardSteps
            | PropertyFieldId::RandomSeed
            | PropertyFieldId::ConnectionMaximumDegree
            | PropertyFieldId::ConnectionMinimumDegree
            | PropertyFieldId::ConnectionSeed
            | PropertyFieldId::MazeSeed
            | PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyUnit::Count,
            _ => PropertyUnit::None,
        },
        applicability: match field {
            PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias => PropertyApplicability::ModeledChannel,
            PropertyFieldId::Paint => PropertyApplicability::CurrentPaint,
            PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha => PropertyApplicability::SolidPaint,
            PropertyFieldId::GuideBaselineAngle
            | PropertyFieldId::GuidePhase
            | PropertyFieldId::GuideSpacingMultiplier => {
                PropertyApplicability::StraightGuideDimension
            }
            PropertyFieldId::GuidePrototype | PropertyFieldId::GuideRepetition => {
                PropertyApplicability::GenericGuideDimension
            }
            PropertyFieldId::GuideAuthoredStructure => PropertyApplicability::AuthoredPathPrototype,
            PropertyFieldId::GuideArcCenterX
            | PropertyFieldId::GuideArcCenterY
            | PropertyFieldId::GuideArcRadius
            | PropertyFieldId::GuideArcStartAngle
            | PropertyFieldId::GuideArcSweepAngle => PropertyApplicability::CircularArc,
            PropertyFieldId::GuideStackDirection | PropertyFieldId::GuideStackSpacingMultiplier => {
                PropertyApplicability::TransformStack
            }
            PropertyFieldId::GuideOffsetSpacing
            | PropertyFieldId::GuideOffsetSides
            | PropertyFieldId::GuideOffsetCleanup => PropertyApplicability::NormalOffset,
            PropertyFieldId::IntersectionDimensions | PropertyFieldId::IntersectionMergeEpsilon => {
                PropertyApplicability::IntersectionProduct
            }
            PropertyFieldId::AlongGuideDimensions
            | PropertyFieldId::AlongGuideIntervalMultiplier
            | PropertyFieldId::AlongGuidePhase => PropertyApplicability::AlongGuideProduct,
            PropertyFieldId::RandomCharacter | PropertyFieldId::RandomSeed => {
                PropertyApplicability::RandomProcess
            }
            PropertyFieldId::RandomEvenMinimumCenterDistance => {
                PropertyApplicability::EvenRandomProcess
            }
            PropertyFieldId::RandomClusterDensity
            | PropertyFieldId::RandomClusterSpread
            | PropertyFieldId::RandomClusterStrength => {
                PropertyApplicability::ClusteredRandomProcess
            }
            PropertyFieldId::RandomDensityModulation => {
                PropertyApplicability::CurrentDensityModulation
            }
            PropertyFieldId::ArtworkWeightMappingComponent
            | PropertyFieldId::ArtworkWeightMappingPlacement
            | PropertyFieldId::ArtworkWeightMappingInverted
            | PropertyFieldId::ArtworkWeightMappingGain
            | PropertyFieldId::ArtworkWeightMappingBias
            | PropertyFieldId::ArtworkWeightStrength
            | PropertyFieldId::ArtworkWeightResponse => {
                PropertyApplicability::ArtworkWeightedDensity
            }
            PropertyFieldId::RandomExclusion => PropertyApplicability::CurrentExclusion,
            PropertyFieldId::ExclusionMinimumCenterDistance => {
                PropertyApplicability::MinimumCenterExclusion
            }
            PropertyFieldId::VisibleMarkMargin | PropertyFieldId::VisibleMarkSizingPolicy => {
                PropertyApplicability::VisibleMarkExclusion
            }
            PropertyFieldId::RandomMaximumAttempts
            | PropertyFieldId::RandomMaximumNeighborChecks => PropertyApplicability::RandomProcess,
            PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputOrientation => PropertyApplicability::MarkPrototypeOutput,
            PropertyFieldId::OutputAuthoredClosedShape => {
                PropertyApplicability::AuthoredClosedShapeMark
            }
            PropertyFieldId::OutputOrientationDimension => {
                PropertyApplicability::GuidedOutputOrientation
            }
            PropertyFieldId::ConnectionProgram
            | PropertyFieldId::ConnectionMaximumDegree
            | PropertyFieldId::ConnectionMaximumDistance => {
                PropertyApplicability::ConnectionPathOutput
            }
            PropertyFieldId::ConnectionMinimumDegree => PropertyApplicability::RandomLinkProgram,
            PropertyFieldId::ConnectionSeed => PropertyApplicability::SeededConnectionProgram,
            PropertyFieldId::MazeSeed => PropertyApplicability::MazeWallProgram,
            _ => PropertyApplicability::Always,
        },
        invalidation: match field {
            PropertyFieldId::SourceReference => InvalidationLevel::Source,
            PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::LegacyMappingComponent
            | PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputPrototype
            | PropertyFieldId::OutputAuthoredClosedShape
            | PropertyFieldId::OutputOrientation
            | PropertyFieldId::OutputOrientationDimension => InvalidationLevel::Realization,
            PropertyFieldId::Paint
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::Visibility => InvalidationLevel::Presentation,
            _ => InvalidationLevel::Family,
        },
        copy_on_edit_escalates_to_family: matches!(
            field,
            PropertyFieldId::CoverageGuardSteps
                | PropertyFieldId::CoverageAdditionalMargin
                | PropertyFieldId::GuideBaselineAngle
                | PropertyFieldId::GuidePhase
                | PropertyFieldId::GuideSpacingMultiplier
                | PropertyFieldId::GuidePrototype
                | PropertyFieldId::GuideAuthoredStructure
                | PropertyFieldId::GuideArcCenterX
                | PropertyFieldId::GuideArcCenterY
                | PropertyFieldId::GuideArcRadius
                | PropertyFieldId::GuideArcStartAngle
                | PropertyFieldId::GuideArcSweepAngle
                | PropertyFieldId::GuideRepetition
                | PropertyFieldId::GuideOffsetSpacing
                | PropertyFieldId::GuideOffsetSides
                | PropertyFieldId::GuideOffsetCleanup
                | PropertyFieldId::GuideStackDirection
                | PropertyFieldId::GuideStackSpacingMultiplier
                | PropertyFieldId::IntersectionDimensions
                | PropertyFieldId::IntersectionMergeEpsilon
                | PropertyFieldId::AlongGuideDimensions
                | PropertyFieldId::AlongGuideIntervalMultiplier
                | PropertyFieldId::AlongGuidePhase
                | PropertyFieldId::RandomCharacter
                | PropertyFieldId::RandomEvenMinimumCenterDistance
                | PropertyFieldId::RandomClusterDensity
                | PropertyFieldId::RandomClusterSpread
                | PropertyFieldId::RandomClusterStrength
                | PropertyFieldId::RandomSeed
                | PropertyFieldId::RandomDensityModulation
                | PropertyFieldId::ArtworkWeightMappingComponent
                | PropertyFieldId::ArtworkWeightMappingPlacement
                | PropertyFieldId::ArtworkWeightMappingInverted
                | PropertyFieldId::ArtworkWeightMappingGain
                | PropertyFieldId::ArtworkWeightMappingBias
                | PropertyFieldId::ArtworkWeightStrength
                | PropertyFieldId::ArtworkWeightResponse
                | PropertyFieldId::RandomExclusion
                | PropertyFieldId::ExclusionMinimumCenterDistance
                | PropertyFieldId::VisibleMarkMargin
                | PropertyFieldId::VisibleMarkSizingPolicy
                | PropertyFieldId::RandomMaximumAttempts
                | PropertyFieldId::RandomMaximumNeighborChecks
                | PropertyFieldId::OutputSiteProduct
                | PropertyFieldId::OutputPrototype
                | PropertyFieldId::OutputAuthoredClosedShape
                | PropertyFieldId::OutputOrientation
                | PropertyFieldId::OutputOrientationDimension
                | PropertyFieldId::ConnectionProgram
                | PropertyFieldId::ConnectionMaximumDegree
                | PropertyFieldId::ConnectionMaximumDistance
                | PropertyFieldId::ConnectionMinimumDegree
                | PropertyFieldId::ConnectionSeed
                | PropertyFieldId::MazeSeed
        ),
        structural_support: match field {
            PropertyFieldId::MarkMaximumFill => {
                StructuralSupportConstraint::MaximumFillDefinesCoverage
            }
            PropertyFieldId::RandomExclusion
            | PropertyFieldId::VisibleMarkMargin
            | PropertyFieldId::VisibleMarkSizingPolicy => {
                StructuralSupportConstraint::VisibleMarkMarginUsesMaximumRealizedSupport
            }
            _ => StructuralSupportConstraint::None,
        },
        reference_constraint: match field {
            PropertyFieldId::IntersectionDimensions => {
                PropertyReferenceConstraint::OrderedUniqueCollection {
                    minimum_items: 2,
                    maximum_items: 4,
                }
            }
            PropertyFieldId::AlongGuideDimensions => {
                PropertyReferenceConstraint::OrderedUniqueCollection {
                    minimum_items: 1,
                    maximum_items: 4,
                }
            }
            PropertyFieldId::SourceReference
            | PropertyFieldId::DefinitionSelection
            | PropertyFieldId::OutputSiteProduct
            | PropertyFieldId::OutputOrientationDimension
            | PropertyFieldId::GuideAuthoredStructure
            | PropertyFieldId::OutputAuthoredClosedShape => PropertyReferenceConstraint::Singular,
            _ => PropertyReferenceConstraint::NotReference,
        },
        choice_policy: match field {
            PropertyFieldId::Paint => PropertyChoicePolicy::ModelRolePaint,
            _ => PropertyChoicePolicy::Static,
        },
    }
}

pub fn property_field_contracts() -> impl ExactSizeIterator<Item = PropertyFieldContract> {
    PROPERTY_FIELD_IDS
        .iter()
        .copied()
        .map(property_field_contract)
}

impl PropertyDescriptor {
    pub const fn command_kind(&self) -> PropertyCommandKind {
        property_field_contract(self.field).command_kind
    }
}

const SOURCE_MAPPING_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Red),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Green),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Blue),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Cyan),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Magenta),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Yellow),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Black),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Alpha),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Luminance),
];
const LEGACY_COMPONENT_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Luminance),
    PropertyEnumChoice::SourceMappingComponent(SourceMappingComponent::Alpha),
];
const SOURCE_PLACEMENT_CHOICES: &[PropertyEnumChoice] = &[PropertyEnumChoice::SourcePlacement(
    SourcePlacement::StretchToCanvas,
)];
const SOLID_PAINT_CHOICES: &[PropertyEnumChoice] = &[PropertyEnumChoice::Paint(PaintKind::Solid)];
const SAMPLED_PAINT_CHOICES: &[PropertyEnumChoice] =
    &[PropertyEnumChoice::Paint(PaintKind::SampledSource)];
const RANDOM_CHARACTER_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::RawUniform),
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Even),
    PropertyEnumChoice::RandomCharacter(RandomCharacterKind::Clustered),
];
const DENSITY_MODULATION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::DensityModulation(DensityModulationKind::Uniform),
    PropertyEnumChoice::DensityModulation(DensityModulationKind::ArtworkWeighted),
];
const ARTWORK_RESPONSE_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::ArtworkWeightResponse(ArtworkWeightResponse::Linear),
    PropertyEnumChoice::ArtworkWeightResponse(ArtworkWeightResponse::Smoothstep),
];
const EXCLUSION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::Exclusion(ExclusionKind::None),
    PropertyEnumChoice::Exclusion(ExclusionKind::MinimumCenterDistance),
    PropertyEnumChoice::Exclusion(ExclusionKind::VisibleMarkMargin),
];
const VISIBLE_MARK_SIZING_POLICY_CHOICES: &[PropertyEnumChoice] =
    &[PropertyEnumChoice::VisibleMarkSizingPolicy(
        VisibleMarkSizingPolicy::MaximumSupportRadius,
    )];
const MARK_PROTOTYPE_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::Circle),
    PropertyEnumChoice::MarkPrototype(MarkPrototypeKind::AuthoredClosedShape),
];
const MARK_ORIENTATION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::Fixed),
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideTangent),
    PropertyEnumChoice::MarkOrientation(MarkOrientationKind::GuideNormal),
];
const CONNECTION_PROGRAM_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::ConnectionProgram(ConnectionProgramKind::NearestLinks),
    PropertyEnumChoice::ConnectionProgram(ConnectionProgramKind::RandomLinks),
    PropertyEnumChoice::ConnectionProgram(ConnectionProgramKind::GridSpanningTree),
];
const GUIDE_PROTOTYPE_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::GuidePrototype(GuidePrototypeKind::AuthoredOpenPath),
    PropertyEnumChoice::GuidePrototype(GuidePrototypeKind::CircularArc),
];
const GUIDE_REPETITION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::GuideRepetition(GuideRepetitionKind::Single),
    PropertyEnumChoice::GuideRepetition(GuideRepetitionKind::TransformStack),
    PropertyEnumChoice::GuideRepetition(GuideRepetitionKind::NormalOffset),
];
const SPIRAL_SHAPE_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::SpiralShape(SpiralShape::Round),
    PropertyEnumChoice::SpiralShape(SpiralShape::Square),
];
const CURVE_WINDING_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::CurveWinding(CurveWinding::Clockwise),
    PropertyEnumChoice::CurveWinding(CurveWinding::CounterClockwise),
];
const CURVE_REPETITION_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::CurveRepetition(GuideRepetitionKind::Single),
    PropertyEnumChoice::CurveRepetition(GuideRepetitionKind::TransformStack),
    PropertyEnumChoice::CurveRepetition(GuideRepetitionKind::NormalOffset),
];
const OFFSET_SIDES_CHOICES: &[PropertyEnumChoice] = &[
    PropertyEnumChoice::OffsetSides(OffsetSides::Left),
    PropertyEnumChoice::OffsetSides(OffsetSides::Right),
    PropertyEnumChoice::OffsetSides(OffsetSides::Both),
];
const OFFSET_CLEANUP_CHOICES: &[PropertyEnumChoice] = &[PropertyEnumChoice::OffsetCleanup(
    OffsetCleanup::DissolveCrossings,
)];

const fn dependency_for_contract(
    applicability: PropertyApplicability,
    dynamic: PropertyDependency,
) -> PropertyDependency {
    match applicability {
        PropertyApplicability::Always => PropertyDependency::Always,
        PropertyApplicability::ModeledChannel => PropertyDependency::ModeledChannel,
        PropertyApplicability::SolidPaint => PropertyDependency::SolidPaint,
        PropertyApplicability::StraightGuideDimension => PropertyDependency::StraightGuideDimension,
        PropertyApplicability::GenericGuideDimension => PropertyDependency::GenericGuideDimension,
        PropertyApplicability::AuthoredPathPrototype => PropertyDependency::AuthoredPathPrototype,
        PropertyApplicability::CircularArc => PropertyDependency::CircularArc,
        PropertyApplicability::TransformStack => PropertyDependency::TransformStack,
        PropertyApplicability::NormalOffset => PropertyDependency::NormalOffset,
        PropertyApplicability::IntersectionProduct => PropertyDependency::IntersectionProduct,
        PropertyApplicability::AlongGuideProduct => PropertyDependency::AlongGuideProduct,
        PropertyApplicability::RandomProcess => PropertyDependency::RandomProcess,
        PropertyApplicability::EvenRandomProcess => PropertyDependency::EvenRandomProcess,
        PropertyApplicability::ClusteredRandomProcess => PropertyDependency::ClusteredRandomProcess,
        PropertyApplicability::ArtworkWeightedDensity => PropertyDependency::ArtworkWeightedDensity,
        PropertyApplicability::MinimumCenterExclusion => PropertyDependency::MinimumCenterExclusion,
        PropertyApplicability::VisibleMarkExclusion => PropertyDependency::VisibleMarkExclusion,
        PropertyApplicability::MarkPrototypeOutput => PropertyDependency::MarkPrototypeOutput,
        PropertyApplicability::AuthoredClosedShapeMark => {
            PropertyDependency::AuthoredClosedShapeMark
        }
        PropertyApplicability::GuidedOutputOrientation => {
            PropertyDependency::GuidedOutputOrientation
        }
        PropertyApplicability::ConnectionPathOutput
        | PropertyApplicability::RandomLinkProgram
        | PropertyApplicability::SeededConnectionProgram
        | PropertyApplicability::MazeWallProgram => PropertyDependency::Always,
        PropertyApplicability::CurrentPaint
        | PropertyApplicability::CurrentDensityModulation
        | PropertyApplicability::CurrentExclusion => dynamic,
    }
}

#[derive(Clone, Copy)]
enum DescriptorRuntimeContext {
    Paint {
        choices: &'static [PropertyEnumChoice],
        dependency: PropertyDependency,
    },
    DensityModulation {
        dependency: PropertyDependency,
    },
    Exclusion {
        dependency: PropertyDependency,
        support: StructuralSupportConstraint,
    },
}

const fn descriptor_from_contract(
    field: PropertyFieldId,
    target: PropertyTarget,
) -> PropertyDescriptor {
    let contract = property_field_contract(field);
    PropertyDescriptor {
        field,
        target,
        value_kind: contract.value_kind,
        choices: contract.choices,
        bounds: contract.bounds,
        unit: contract.unit,
        dependency: dependency_for_contract(contract.applicability, PropertyDependency::Always),
        applicability: contract.applicability,
        invalidation: contract.invalidation,
        copy_on_edit_escalates_to_family: contract.copy_on_edit_escalates_to_family,
        structural_support: contract.structural_support,
        reference_constraint: contract.reference_constraint,
        choice_policy: contract.choice_policy,
        authority: property_authority(field, target),
        reset_capable: property_reset_capable(field, target),
    }
}

const fn descriptor_with_runtime_context(
    field: PropertyFieldId,
    target: PropertyTarget,
    context: DescriptorRuntimeContext,
) -> PropertyDescriptor {
    let contract = property_field_contract(field);
    let (choices, dependency, structural_support) = match context {
        DescriptorRuntimeContext::Paint {
            choices,
            dependency,
        } => (choices, dependency, contract.structural_support),
        DescriptorRuntimeContext::DensityModulation { dependency } => {
            (contract.choices, dependency, contract.structural_support)
        }
        DescriptorRuntimeContext::Exclusion {
            dependency,
            support,
        } => (contract.choices, dependency, support),
    };
    PropertyDescriptor {
        field,
        target,
        value_kind: contract.value_kind,
        choices,
        bounds: contract.bounds,
        unit: contract.unit,
        dependency,
        applicability: contract.applicability,
        invalidation: contract.invalidation,
        copy_on_edit_escalates_to_family: contract.copy_on_edit_escalates_to_family,
        structural_support,
        reference_constraint: contract.reference_constraint,
        choice_policy: contract.choice_policy,
        authority: property_authority(field, target),
        reset_capable: property_reset_capable(field, target),
    }
}

/// Identifies the document base, optional channel delta, or independent
/// authority represented by one stable field.
const fn property_authority(field: PropertyFieldId, target: PropertyTarget) -> PropertyAuthority {
    match target {
        PropertyTarget::Document
            if matches!(
                field,
                PropertyFieldId::DefinitionSelection
                    | PropertyFieldId::DensityAcrossX
                    | PropertyFieldId::DensityAcrossY
                    | PropertyFieldId::DensityAspectLocked
                    | PropertyFieldId::RotationDegrees
                    | PropertyFieldId::ShapeRotationDegrees
                    | PropertyFieldId::MarkMinimumFill
                    | PropertyFieldId::MarkMaximumFill
            ) =>
        {
            PropertyAuthority::DocumentBase
        }
        _ => match field {
            PropertyFieldId::DensityAspectLocked => PropertyAuthority::DocumentBase,
            PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::DefinitionSelection => PropertyAuthority::ChannelDelta,
            PropertyFieldId::TranslationX
            | PropertyFieldId::TranslationY
            | PropertyFieldId::Opacity
            | PropertyFieldId::Visibility
            | PropertyFieldId::Paint
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::SourceReference
            | PropertyFieldId::LegacyMappingComponent
            | PropertyFieldId::LegacyMappingPlacement
            | PropertyFieldId::ModeledMappingComponent
            | PropertyFieldId::ModeledMappingPlacement
            | PropertyFieldId::ModeledMappingInverted
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias => PropertyAuthority::ChannelSpecific,
            _ => PropertyAuthority::StructuralDefinition,
        },
    }
}

/// States whether reset removes optional Stage 20G channel intent for one
/// field rather than copying its current effective value.
const fn property_reset_capable(field: PropertyFieldId, target: PropertyTarget) -> bool {
    matches!(
        target,
        PropertyTarget::Channel(_) | PropertyTarget::ChannelOutput(_, _)
    ) && matches!(
        field,
        PropertyFieldId::DensityAcrossX
            | PropertyFieldId::DensityAcrossY
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::DefinitionSelection
    )
}

const fn positive_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: false,
        maximum: None,
        maximum_inclusive: false,
    })
}
const fn nonnegative_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: true,
        maximum: None,
        maximum_inclusive: false,
    })
}

/// Returns the exact normalized fill interval exposed by descriptor metadata.
const fn fill_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: true,
        maximum: Some(2.0),
        maximum_inclusive: true,
    })
}
const fn unit_bounds() -> Option<PropertyBounds> {
    Some(PropertyBounds {
        minimum: Some(0.0),
        minimum_inclusive: true,
        maximum: Some(1.0),
        maximum_inclusive: true,
    })
}

/// A typed, ID-free definition proposal.  Stage 14 intentionally exposes only
/// the accepted mechanism composition; IDs are allocated by the document.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinitionDraft {
    pub name: String,
    pub coverage: CoveragePolicy,
}

/// Stable metadata for a pure-schema preset shortcut.
///
/// It is intentionally independent from evaluation: removing this metadata
/// never removes the ordinary pattern definition capability it describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetMetadata {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub thumbnail: Option<String>,
}

/// An ID-free straight-guide dimension authored through the typed pattern
/// boundary. The document allocates its stable ID when materializing a recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideDimensionDraft {
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub spacing_multiplier: f64,
}

/// An ID-free site-product choice whose dimension references are stored-order
/// indices into the recipe's `dimensions` collection.
#[derive(Clone, Debug, PartialEq)]
pub enum GeneralizedSiteProductDraft {
    Intersections {
        dimension_indices: Vec<usize>,
        merge_epsilon: f64,
    },
    AlongGuides {
        dimension_indices: Vec<usize>,
        interval_multiplier: f64,
        phase: f64,
    },
}

/// An ID-free output-orientation choice whose reference, when present, is a
/// stored-order index into the recipe's `dimensions` collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkOrientationDraft {
    Fixed,
    GuideTangent { dimension_index: usize },
    GuideNormal { dimension_index: usize },
}

/// The structural, ID-free portion of a complete pattern recipe.
///
/// All fields map directly to current typed schema controls; this recipe
/// contains neither a preset discriminator nor evaluator/cache/render state.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternStructureRecipe {
    StraightGrid(PatternDefinitionDraft),
    GeneralizedStraightGuides {
        name: String,
        coverage: CoveragePolicy,
        dimensions: Vec<GuideDimensionDraft>,
        product: GeneralizedSiteProductDraft,
        orientation: MarkOrientationDraft,
    },
    RandomSites {
        name: String,
        coverage: CoveragePolicy,
        character: RandomSiteCharacter,
        seed: u32,
        density_modulation: SiteDensityModulation,
        exclusion: SiteExclusionPolicy,
        maximum_attempts: u32,
        maximum_neighbor_checks: u32,
    },
    /// Wraps one eligible ID-free site-family recipe with authored connection intent.
    /// Materialization retains the allocated output and site-product IDs while replacing only
    /// its mark output; graphs, paths, diagnostics, and limits remain derived-only.
    ConnectionPaths {
        definition: Box<PatternStructureRecipe>,
        program: ConnectionProgram,
        style: PathStrokeStyle,
    },
    /// Wraps a two- or three-guide straight intersection recipe with conventional wall-maze intent.
    MazeWalls {
        definition: Box<PatternStructureRecipe>,
        program: MazeProgram,
        style: PathStrokeStyle,
    },
    /// Wraps one ordinary ID-free family recipe with a validated closed-shape payload.
    /// Materialization allocates both the document-owned structure and the definition
    /// atomically, then installs the ordinary typed output reference.
    AuthoredClosedShapeMarks {
        definition: Box<PatternStructureRecipe>,
        shape: AuthoredStructureDraft,
    },
    /// Wraps one ID-free site family with ordinary Stage 20O Voronoi intent.
    VoronoiRegions {
        definition: Box<PatternStructureRecipe>,
    },
    /// Wraps a two- or three-dimension straight-guide recipe with Stage
    /// 20P arrangement-face region intent. Indices retain the authored recipe
    /// order and materialize atomically into stable guide-dimension IDs.
    GuideFaceRegions {
        definition: Box<PatternStructureRecipe>,
        dimension_indices: Vec<usize>,
    },
}

/// One ordered ID-free base response authored alongside a structural output.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternOutputSettingsRecipe {
    pub response: PatternGeometryResponse,
}

/// Complete ID-free pattern definition authority.
///
/// The structure never carries document IDs, while settings are ordered to
/// bind atomically to output IDs allocated during materialization. Current
/// Stage 20N validation deliberately retains the one-output authoring gate;
/// the ordered form is the authority Stage 20R will generalize.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefinitionRecipe {
    pub structure: PatternStructureRecipe,
    pub output_settings: Vec<PatternOutputSettingsRecipe>,
}

impl PatternDefinitionRecipe {
    /// Builds the complete current one-mark-output recipe without assigning any document ID.
    pub fn marks(structure: PatternStructureRecipe) -> Self {
        Self {
            structure,
            output_settings: vec![PatternOutputSettingsRecipe {
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            }],
        }
    }

    /// Builds the complete current one-connected-output recipe without assigning any document ID.
    pub fn connected(structure: PatternStructureRecipe) -> Self {
        Self {
            structure,
            output_settings: vec![PatternOutputSettingsRecipe {
                response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.0,
                    maximum_thickness: 1.0,
                }),
            }],
        }
    }

    /// Builds the complete current one-region-output recipe without assigning document IDs.
    pub fn regions(structure: PatternStructureRecipe) -> Self {
        Self {
            structure: PatternStructureRecipe::VoronoiRegions {
                definition: Box::new(structure),
            },
            output_settings: vec![PatternOutputSettingsRecipe {
                response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            }],
        }
    }

    /// Builds the complete current Guide Faces region recipe without assigning document IDs.
    pub fn guide_faces(structure: PatternStructureRecipe, dimension_indices: Vec<usize>) -> Self {
        Self {
            structure: PatternStructureRecipe::GuideFaceRegions {
                definition: Box::new(structure),
                dimension_indices,
            },
            output_settings: vec![PatternOutputSettingsRecipe {
                response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            }],
        }
    }
}

/// One unpublished recipe result whose optional resource and definition must
/// be installed together before authoritative validation can succeed.
#[derive(Clone, Debug, PartialEq)]
struct MaterializedPatternDefinitionRecipe {
    bundle: PatternDefinitionBundle,
    authored_structure: Option<AuthoredStructure>,
}

/// A metadata/recipe pair crossing registry and persistence boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetRecord {
    pub metadata: PresetMetadata,
    pub recipe: PatternDefinitionRecipe,
}

/// Validates pure preset metadata and ID-free recipe inputs without allocating
/// IDs, mutating a document, or publishing history. Full reconstruction still
/// reuses command validation when a recipe is applied to a document.
pub fn validate_preset_record(record: &PresetRecord) -> Result<(), ValidationError> {
    for (path, value) in [
        ("preset.metadata.id", record.metadata.id.as_str()),
        ("preset.metadata.name", record.metadata.name.as_str()),
        (
            "preset.metadata.category",
            record.metadata.category.as_str(),
        ),
        (
            "preset.metadata.description",
            record.metadata.description.as_str(),
        ),
    ] {
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(ValidationError::new(
                path,
                "must be nonempty printable text",
            ));
        }
    }
    if record
        .metadata
        .thumbnail
        .as_deref()
        .is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control))
    {
        return Err(ValidationError::new(
            "preset.metadata.thumbnail",
            "must be printable text when present",
        ));
    }
    validate_recipe_output_settings(&record.recipe)?;
    validate_pattern_structure_recipe(&record.recipe.structure)
}

/// Validates one ID-free structural recipe without allocating output IDs.
fn validate_pattern_structure_recipe(
    recipe: &PatternStructureRecipe,
) -> Result<(), ValidationError> {
    match recipe {
        PatternStructureRecipe::StraightGrid(draft) => validate_definition_draft(draft),
        PatternStructureRecipe::GeneralizedStraightGuides {
            name,
            coverage,
            dimensions,
            product,
            orientation,
        } => {
            validate_definition_draft(&PatternDefinitionDraft {
                name: name.clone(),
                coverage: coverage.clone(),
            })?;
            if !(1..=4).contains(&dimensions.len()) {
                return Err(ValidationError::new(
                    "preset.recipe.dimensions",
                    "must contain one through four guide dimensions",
                ));
            }
            for dimension in dimensions {
                validate_finite(
                    dimension.baseline_angle_degrees,
                    "preset.recipe.dimensions.baseline_angle_degrees",
                )?;
                validate_finite(dimension.phase, "preset.recipe.dimensions.phase")?;
                validate_positive_finite(
                    dimension.spacing_multiplier,
                    "preset.recipe.dimensions.spacing_multiplier",
                )?;
            }
            let validate_indices = |indices: &[usize], minimum: usize| {
                if !(minimum..=4).contains(&indices.len()) {
                    return Err(ValidationError::new(
                        "preset.recipe.product.dimension_indices",
                        "has invalid cardinality",
                    ));
                }
                let mut seen = HashSet::new();
                for index in indices {
                    if *index >= dimensions.len() || !seen.insert(*index) {
                        return Err(ValidationError::new(
                            "preset.recipe.product.dimension_indices",
                            "must be unique in bounds indices",
                        ));
                    }
                }
                Ok(())
            };
            match product {
                GeneralizedSiteProductDraft::Intersections {
                    dimension_indices,
                    merge_epsilon,
                } => {
                    validate_indices(dimension_indices, 2)?;
                    validate_nonnegative_finite(
                        *merge_epsilon,
                        "preset.recipe.product.merge_epsilon",
                    )?;
                }
                GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices,
                    interval_multiplier,
                    phase,
                } => {
                    validate_indices(dimension_indices, 1)?;
                    validate_positive_finite(
                        *interval_multiplier,
                        "preset.recipe.product.interval_multiplier",
                    )?;
                    validate_finite(*phase, "preset.recipe.product.phase")?;
                }
            }
            match orientation {
                MarkOrientationDraft::Fixed => Ok(()),
                MarkOrientationDraft::GuideTangent { dimension_index }
                | MarkOrientationDraft::GuideNormal { dimension_index }
                    if *dimension_index < dimensions.len() =>
                {
                    Ok(())
                }
                _ => Err(ValidationError::new(
                    "preset.recipe.orientation.dimension_index",
                    "must address an in-bounds guide dimension",
                )),
            }
        }
        PatternStructureRecipe::RandomSites {
            name,
            coverage,
            character,
            seed: _,
            density_modulation,
            exclusion,
            maximum_attempts,
            maximum_neighbor_checks,
        } => {
            validate_definition_draft(&PatternDefinitionDraft {
                name: name.clone(),
                coverage: coverage.clone(),
            })?;
            match character {
                RandomSiteCharacter::RawUniform => {}
                RandomSiteCharacter::Even {
                    minimum_center_distance,
                } => validate_positive_finite(
                    *minimum_center_distance,
                    "preset.recipe.character.minimum_center_distance",
                )?,
                RandomSiteCharacter::Clustered {
                    cluster_density,
                    cluster_spread,
                    cluster_strength,
                } => {
                    validate_positive_finite(
                        *cluster_density,
                        "preset.recipe.character.cluster_density",
                    )?;
                    validate_positive_finite(
                        *cluster_spread,
                        "preset.recipe.character.cluster_spread",
                    )?;
                    validate_unit_component(
                        *cluster_strength,
                        "preset.recipe.character.cluster_strength",
                    )?;
                }
            }
            if let SiteDensityModulation::ArtworkWeighted {
                mapping, strength, ..
            } = density_modulation
            {
                validate_nonnegative_finite(mapping.gain, "preset.recipe.modulation.mapping.gain")?;
                validate_finite(mapping.bias, "preset.recipe.modulation.mapping.bias")?;
                validate_unit_component(*strength, "preset.recipe.modulation.strength")?;
            }
            match exclusion {
                SiteExclusionPolicy::None => {}
                SiteExclusionPolicy::MinimumCenterDistance { minimum } => {
                    validate_positive_finite(*minimum, "preset.recipe.exclusion.minimum")?
                }
                SiteExclusionPolicy::VisibleMarkMargin { margin, .. } => {
                    validate_nonnegative_finite(*margin, "preset.recipe.exclusion.margin")?
                }
            }
            if *maximum_attempts == 0 || *maximum_neighbor_checks == 0 {
                return Err(ValidationError::new(
                    "preset.recipe.random_work",
                    "maximum attempts and neighbor checks must be nonzero",
                ));
            }
            Ok(())
        }
        PatternStructureRecipe::ConnectionPaths {
            definition,
            program,
            ..
        } => {
            validate_connection_recipe(definition, program)?;
            validate_pattern_structure_recipe(definition)
        }
        PatternStructureRecipe::MazeWalls {
            definition,
            program,
            ..
        } => {
            validate_maze_recipe(definition, program)?;
            validate_pattern_structure_recipe(definition)
        }
        PatternStructureRecipe::AuthoredClosedShapeMarks { definition, shape } => {
            if shape.kind() != AuthoredStructureKind::ClosedShape {
                return Err(ValidationError::new(
                    "preset.recipe.shape.kind",
                    "authored mark recipes require a closed-shape payload",
                ));
            }
            if matches!(
                definition.as_ref(),
                PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
                    | PatternStructureRecipe::ConnectionPaths { .. }
                    | PatternStructureRecipe::MazeWalls { .. }
                    | PatternStructureRecipe::GuideFaceRegions { .. }
            ) {
                return Err(ValidationError::new(
                    "preset.recipe.definition",
                    "authored mark recipes cannot wrap another output recipe wrapper",
                ));
            }
            validate_pattern_structure_recipe(definition)
        }
        PatternStructureRecipe::VoronoiRegions { definition } => {
            if matches!(
                definition.as_ref(),
                PatternStructureRecipe::ConnectionPaths { .. }
                    | PatternStructureRecipe::MazeWalls { .. }
                    | PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
                    | PatternStructureRecipe::VoronoiRegions { .. }
                    | PatternStructureRecipe::GuideFaceRegions { .. }
            ) {
                return Err(ValidationError::new(
                    "preset.recipe.definition",
                    "Voronoi recipes require an unwrapped site-family recipe",
                ));
            }
            validate_pattern_structure_recipe(definition)
        }
        PatternStructureRecipe::GuideFaceRegions {
            definition,
            dimension_indices,
        } => {
            let PatternStructureRecipe::GeneralizedStraightGuides { dimensions, .. } =
                definition.as_ref()
            else {
                return Err(ValidationError::new(
                    "preset.recipe.guide_faces.family",
                    "Guide Faces recipes require an unwrapped straight-guide recipe",
                ));
            };
            if !(2..=3).contains(&dimension_indices.len()) {
                return Err(ValidationError::new(
                    "preset.recipe.guide_faces.dimension_indices",
                    "must contain two or three selected dimensions",
                ));
            }
            let mut prior = None;
            for index in dimension_indices {
                if *index >= dimensions.len() || prior.is_some_and(|value| value >= *index) {
                    return Err(ValidationError::new(
                        "preset.recipe.guide_faces.dimension_indices",
                        "must be unique increasing in-bounds indices",
                    ));
                }
                prior = Some(*index);
            }
            validate_pattern_structure_recipe(definition)
        }
    }
}

/// Validates the ordered ID-free response collection against the current
/// one-output authoring gate and its structural response kind.
fn validate_recipe_output_settings(
    recipe: &PatternDefinitionRecipe,
) -> Result<(), ValidationError> {
    if recipe.output_settings.len() != 1 {
        return Err(ValidationError::new(
            "preset.recipe.output_settings.cardinality",
            "current pattern recipes require exactly one output setting",
        ));
    }
    let setting = recipe.output_settings.first().ok_or(ValidationError::new(
        "preset.recipe.output_settings.cardinality",
        "current pattern recipes require exactly one output setting",
    ))?;
    let response = &setting.response;
    let connected = matches!(
        recipe.structure,
        PatternStructureRecipe::ConnectionPaths { .. } | PatternStructureRecipe::MazeWalls { .. }
    );
    let regions = matches!(
        recipe.structure,
        PatternStructureRecipe::VoronoiRegions { .. }
            | PatternStructureRecipe::GuideFaceRegions { .. }
    );
    match (connected, regions, response) {
        (false, false, PatternGeometryResponse::Marks(_))
        | (true, false, PatternGeometryResponse::Connected(_))
        | (false, true, PatternGeometryResponse::Regions(_)) => {
            validate_pattern_geometry_response(response)
        }
        _ => Err(ValidationError::new(
            "preset.recipe.output_settings.kind",
            "recipe response kind must match its structural output",
        )),
    }
}

/// Validates whether one ID-free family recipe can own the supplied connection program.
///
/// # Errors
///
/// Returns stable wrapper, program, or family-eligibility diagnostics before any document IDs,
/// authored structures, or candidate definitions are allocated.
fn validate_connection_recipe(
    definition: &PatternStructureRecipe,
    program: &ConnectionProgram,
) -> Result<(), ValidationError> {
    program.validate()?;
    let nearest_or_random = matches!(
        program,
        ConnectionProgram::NearestLinks { .. } | ConnectionProgram::RandomLinks { .. }
    );
    match definition {
        PatternStructureRecipe::StraightGrid(_) => Ok(()),
        PatternStructureRecipe::GeneralizedStraightGuides { product, .. } => match product {
            GeneralizedSiteProductDraft::Intersections { .. } => Ok(()),
            GeneralizedSiteProductDraft::AlongGuides { .. } if nearest_or_random => Ok(()),
            GeneralizedSiteProductDraft::AlongGuides { .. } => Err(ValidationError::new(
                "preset.recipe.connection.program",
                "maze and spanning-tree programs require an intersection site recipe",
            )),
        },
        PatternStructureRecipe::RandomSites { .. } if nearest_or_random => Ok(()),
        PatternStructureRecipe::RandomSites { .. } => Err(ValidationError::new(
            "preset.recipe.connection.program",
            "maze and spanning-tree programs require an intersection site recipe",
        )),
        PatternStructureRecipe::ConnectionPaths { .. }
        | PatternStructureRecipe::MazeWalls { .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
        | PatternStructureRecipe::VoronoiRegions { .. }
        | PatternStructureRecipe::GuideFaceRegions { .. } => Err(ValidationError::new(
            "preset.recipe.definition",
            "connection recipes cannot wrap another output recipe wrapper",
        )),
    }
}

/// Validates the narrow straight-intersection recipe authority for conventional wall mazes.
///
/// # Errors
///
/// Rejects wrappers, nonintersection products, and invalid maze intent before any ID allocation.
fn validate_maze_recipe(
    definition: &PatternStructureRecipe,
    program: &MazeProgram,
) -> Result<(), ValidationError> {
    program.validate()?;
    match definition {
        PatternStructureRecipe::StraightGrid(_) => Ok(()),
        PatternStructureRecipe::GeneralizedStraightGuides {
            dimensions,
            product: GeneralizedSiteProductDraft::Intersections { .. },
            ..
        } if matches!(dimensions.len(), 2 | 3) => Ok(()),
        PatternStructureRecipe::GeneralizedStraightGuides { .. } => Err(ValidationError::new(
            "preset.recipe.maze.family",
            "maze recipes require two or three straight guide intersection dimensions",
        )),
        PatternStructureRecipe::RandomSites { .. }
        | PatternStructureRecipe::ConnectionPaths { .. }
        | PatternStructureRecipe::MazeWalls { .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { .. }
        | PatternStructureRecipe::VoronoiRegions { .. }
        | PatternStructureRecipe::GuideFaceRegions { .. } => Err(ValidationError::new(
            "preset.recipe.maze.family",
            "maze recipes require an unwrapped straight guide intersection family",
        )),
    }
}

/// A typed structural edit. It has no UI/editor state and can be applied only
/// through `DocumentHistory`, which records its exact inverse.
#[derive(Clone, Debug, PartialEq)]
pub enum PatternDefinitionEdit {
    SetParametricShape {
        mechanism_id: PatternMechanismId,
        shape: SpiralShape,
    },
    SetParametricTurns {
        mechanism_id: PatternMechanismId,
        turns: f64,
    },
    SetParametricRadialSpacing {
        mechanism_id: PatternMechanismId,
        radial_spacing: f64,
    },
    SetParametricPhase {
        mechanism_id: PatternMechanismId,
        phase_degrees: f64,
    },
    SetParametricWinding {
        mechanism_id: PatternMechanismId,
        winding: CurveWinding,
    },
    SetParametricRepetition {
        mechanism_id: PatternMechanismId,
        repetition: CurveRepetition,
    },
    SetParametricOffsetSpacing {
        mechanism_id: PatternMechanismId,
        spacing: f64,
    },
    SetParametricOffsetSides {
        mechanism_id: PatternMechanismId,
        sides: OffsetSides,
    },
    SetParametricOffsetCleanup {
        mechanism_id: PatternMechanismId,
        cleanup: OffsetCleanup,
    },
    SetParametricStackDirection {
        mechanism_id: PatternMechanismId,
        value: f64,
    },
    SetParametricStackSpacingMultiplier {
        mechanism_id: PatternMechanismId,
        value: f64,
    },
    SetAlongParametricInterval {
        mechanism_id: PatternMechanismId,
        interval: f64,
    },
    SetAlongParametricPhase {
        mechanism_id: PatternMechanismId,
        phase: f64,
    },
    SetCoverageGuardSteps {
        guard_steps: u32,
    },
    SetCoverageAdditionalMargin {
        additional_margin: f64,
    },
    SetGuideBaselineAngle {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        baseline_angle_degrees: f64,
    },
    SetGuidePhase {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        phase: f64,
    },
    SetGuideSpacingMultiplier {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        spacing_multiplier: f64,
    },
    SetGuidePrototype {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        prototype: GuidePrototype,
    },
    SetGuideAuthoredStructure {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        structure_id: AuthoredStructureId,
    },
    SetGuideArcCenterX {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideArcCenterY {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideArcRadius {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideArcStartAngle {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideArcSweepAngle {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideRepetition {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        repetition: GuideRepetition,
    },
    SetGuideOffsetSpacing {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        spacing: f64,
    },
    SetGuideOffsetSides {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        sides: OffsetSides,
    },
    SetGuideOffsetCleanup {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        cleanup: OffsetCleanup,
    },
    SetGuideStackDirection {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetGuideStackSpacingMultiplier {
        mechanism_id: PatternMechanismId,
        dimension_id: GuideDimensionId,
        value: f64,
    },
    SetIntersectionDimensions {
        mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
    SetIntersectionMergeEpsilon {
        mechanism_id: PatternMechanismId,
        merge_epsilon: f64,
    },
    SetAlongGuideDimensions {
        mechanism_id: PatternMechanismId,
        dimensions: Vec<GuideDimensionId>,
    },
    SetAlongGuideIntervalMultiplier {
        mechanism_id: PatternMechanismId,
        interval_multiplier: f64,
    },
    SetAlongGuidePhase {
        mechanism_id: PatternMechanismId,
        phase: f64,
    },
    SetRandomCharacter {
        mechanism_id: PatternMechanismId,
        character: RandomSiteCharacter,
    },
    SetRandomSeed {
        mechanism_id: PatternMechanismId,
        seed: u32,
    },
    SetRandomEvenMinimumCenterDistance {
        mechanism_id: PatternMechanismId,
        minimum_center_distance: f64,
    },
    SetRandomClusterDensity {
        mechanism_id: PatternMechanismId,
        cluster_density: f64,
    },
    SetRandomClusterSpread {
        mechanism_id: PatternMechanismId,
        cluster_spread: f64,
    },
    SetRandomClusterStrength {
        mechanism_id: PatternMechanismId,
        cluster_strength: f64,
    },
    SetDensityModulationVariant {
        mechanism_id: PatternMechanismId,
        modulation: SiteDensityModulation,
    },
    SetArtworkWeightMappingComponent {
        mechanism_id: PatternMechanismId,
        component: SourceMappingComponent,
    },
    SetArtworkWeightMappingPlacement {
        mechanism_id: PatternMechanismId,
        placement: SourcePlacement,
    },
    SetArtworkWeightMappingInverted {
        mechanism_id: PatternMechanismId,
        inverted: bool,
    },
    SetArtworkWeightMappingGain {
        mechanism_id: PatternMechanismId,
        gain: f64,
    },
    SetArtworkWeightMappingBias {
        mechanism_id: PatternMechanismId,
        bias: f64,
    },
    SetArtworkWeightStrength {
        mechanism_id: PatternMechanismId,
        strength: f64,
    },
    SetArtworkWeightResponse {
        mechanism_id: PatternMechanismId,
        response: ArtworkWeightResponse,
    },
    SetExclusionVariant {
        mechanism_id: PatternMechanismId,
        policy: SiteExclusionPolicy,
    },
    SetExclusionMinimumCenterDistance {
        mechanism_id: PatternMechanismId,
        minimum_center_distance: f64,
    },
    SetVisibleMarkMargin {
        mechanism_id: PatternMechanismId,
        margin: f64,
    },
    SetVisibleMarkSizingPolicy {
        mechanism_id: PatternMechanismId,
        sizing: VisibleMarkSizingPolicy,
    },
    SetRandomMaximumAttempts {
        mechanism_id: PatternMechanismId,
        maximum_attempts: u32,
    },
    SetRandomMaximumNeighborChecks {
        mechanism_id: PatternMechanismId,
        maximum_neighbor_checks: u32,
    },
    SetOutputSiteProduct {
        output_layer_id: PatternOutputLayerId,
        site_mechanism_id: PatternMechanismId,
    },
    SetOutputMarkPrototype {
        output_layer_id: PatternOutputLayerId,
        prototype: MarkPrototype,
    },
    /// Retargets the active authored closed-shape mark without changing its variant.
    SetOutputAuthoredClosedShape {
        output_layer_id: PatternOutputLayerId,
        structure_id: AuthoredStructureId,
    },
    SetOutputOrientation {
        output_layer_id: PatternOutputLayerId,
        orientation: MarkOrientation,
    },
    SetOutputOrientationDimension {
        output_layer_id: PatternOutputLayerId,
        dimension_id: GuideDimensionId,
    },
    SetConnectionProgram {
        output_layer_id: PatternOutputLayerId,
        program: ConnectionProgram,
    },
    SetConnectionMaximumDegree {
        output_layer_id: PatternOutputLayerId,
        maximum_degree: u32,
    },
    SetConnectionMaximumDistance {
        output_layer_id: PatternOutputLayerId,
        maximum_distance: f64,
    },
    SetConnectionMinimumDegree {
        output_layer_id: PatternOutputLayerId,
        minimum_degree: u32,
    },
    SetConnectionSeed {
        output_layer_id: PatternOutputLayerId,
        seed: u32,
    },
    /// Changes only the authored recursive-backtracker seed of one maze-wall output.
    SetMazeSeed {
        output_layer_id: PatternOutputLayerId,
        seed: u32,
    },
    /// Replaces the ordered two-or-three Guide Faces source dimensions of one region output.
    /// This typed structural operation deliberately has no property descriptor.
    SetGuideFaceDimensions {
        output_layer_id: PatternOutputLayerId,
        dimensions: Vec<GuideDimensionId>,
    },
}

/// The authoritative document command surface for channel edits and
/// document-scoped structural resources.
///
/// Commands carry only validated persisted intent. Document validation builds
/// an atomic candidate before publication, while reversible structural
/// transitions are owned by `DocumentHistory` rather than a frontend or an
/// evaluator.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommand {
    /// Replaces the document-owned settings after comparing an exact stale
    /// base; all channel instances are revalidated atomically before publish.
    SetDocumentPatternSettings {
        base: DocumentPatternSettings,
        settings: DocumentPatternSettings,
    },
    /// Materializes a fresh recipe definition and installs it as the document
    /// base without changing any channel override/delta intent.
    ReplaceDocumentPatternDefinitionRecipe {
        base: DocumentPatternSettings,
        base_definition: PatternDefinition,
        recipe: PatternDefinitionRecipe,
    },
    /// Installs or replaces one explicit channel definition override.
    SetChannelPatternDefinitionOverride {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        definition_id: PatternDefinitionId,
    },
    /// Removes one explicit channel definition override without copying an
    /// effective value into stored state.
    ResetChannelPatternDefinitionOverride {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
    },
    /// Materializes a fresh selected-channel override from an ID-free recipe.
    ReplaceChannelPatternDefinitionOverrideRecipe {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        base_definition: PatternDefinition,
        recipe: PatternDefinitionRecipe,
    },
    /// Stores deltas derived by the domain from a desired effective density.
    SetChannelDensityDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        density: DensityMetricDelta2D,
    },
    /// Removes stored density intent while retaining all other channel intent.
    ResetChannelDensityDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
    },
    /// Stores a delta derived by the domain from a desired effective rotation.
    SetChannelPatternRotationDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        rotation_degrees: f64,
    },
    /// Removes stored layout-rotation intent.
    ResetChannelPatternRotationDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
    },
    /// Stores a delta derived by the domain from a desired effective shape rotation.
    SetChannelShapeRotationDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        rotation_degrees: f64,
    },
    /// Removes stored shape-rotation intent.
    ResetChannelShapeRotationDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
    },
    /// Stores one output-keyed typed response delta derived by the domain.
    SetChannelOutputResponseDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        output_layer_id: PatternOutputLayerId,
        delta: ChannelGeometryResponseDelta,
    },
    /// Removes one output-keyed response delta without materializing its inherited response.
    ResetChannelOutputResponseDelta {
        base: DocumentPatternSettings,
        channel_id: ChannelId,
        output_layer_id: PatternOutputLayerId,
    },
    /// Adds one validated reusable authored structure with a fresh document-scoped ID.
    AddAuthoredStructure {
        draft: AuthoredStructureDraft,
    },
    /// Duplicates one existing reusable authored structure with a fresh document-scoped ID.
    DuplicateAuthoredStructure {
        structure_id: AuthoredStructureId,
    },
    /// Replaces one exact current authored structure while preserving its stable ID and store position.
    ReplaceAuthoredStructure {
        base_structure: AuthoredStructure,
        replacement: AuthoredStructureDraft,
    },
    /// Removes one currently unreferenced authored structure without retargeting any owner.
    RemoveUnreferencedAuthoredStructure {
        structure_id: AuthoredStructureId,
    },
    AddPatternDefinition {
        definition: PatternDefinitionDraft,
    },
    /// Installs a fully typed, stable-ID structural definition. This is the
    /// headless construction path for every accepted family; document-wide ID
    /// collision/order/reference validation still occurs before publication.
    AddTypedPatternDefinition {
        definition: PatternDefinition,
    },
    /// Atomically introduces a fresh structural definition for the bounded
    /// Stage 20F resource-editor path. Ordinary recipe replacement uses the
    /// explicit Stage 20G channel override command instead.
    ReplaceSelectedChannelDefinitionTopology {
        channel_id: ChannelId,
        base_definition: PatternDefinition,
        definition: PatternDefinition,
    },
    DuplicatePatternDefinition {
        definition_id: PatternDefinitionId,
    },
    RemoveUnreferencedPatternDefinition {
        definition_id: PatternDefinitionId,
    },
    /// The ordinary selected-channel structural path.  A stale base is
    /// rejected; shared definitions are cloned and only this channel retargets.
    EditSelectedChannelPatternDefinition {
        channel_id: ChannelId,
        /// Immutable editor base. This is command-only lifecycle input, never
        /// document/persistence state, and detects same-ID in-place edits.
        base_definition: PatternDefinition,
        edit: PatternDefinitionEdit,
    },
    /// The deliberate sharing path.  It keeps definition/internal IDs stable
    /// and reports every linked channel in document order.
    EditSharedPatternDefinition {
        definition_id: PatternDefinitionId,
        /// Immutable editor base; see selected-channel edit above.
        base_definition: PatternDefinition,
        edit: PatternDefinitionEdit,
    },
    /// Explicitly replaces one existing definition's topology for every
    /// linked channel. Callers must disclose the returned affected channels
    /// before applying this history-backed shared operation.
    ReplaceSharedPatternDefinitionRecipe {
        definition_id: PatternDefinitionId,
        /// Immutable editor base; this rejects stale shared replacements.
        base_definition: PatternDefinition,
        recipe: PatternDefinitionRecipe,
    },
    SetTranslationAxis {
        channel_id: ChannelId,
        edited_axis: TranslationEditedAxis,
        value: f64,
    },
    SetColorComponent {
        channel_id: ChannelId,
        component: ColorComponent,
        value: f64,
    },
    SetOpacity {
        channel_id: ChannelId,
        opacity: f64,
    },
    SetVisibility {
        channel_id: ChannelId,
        visible: bool,
    },
    SetSourceReference {
        source: SourceReference,
    },
    SetLegacyMappingField {
        channel_id: ChannelId,
        edit: LegacyMappingFieldEdit,
    },
    /// Replaces the complete model and its ordered channel topology together.
    ReplaceChannelTopology {
        model: HalftoneChannelModel,
        topology: ChannelTopology,
    },
    /// Replaces the full Stage 9 mapping for one modeled channel.
    SetModeledMappingField {
        channel_id: ChannelId,
        edit: ModeledMappingFieldEdit,
    },
    /// Replaces ordinary solid paint for a modeled channel.
    SetChannelPaint {
        channel_id: ChannelId,
        paint: ChannelPaint,
    },
}

/// One typed value supplied to a descriptor-backed command leaf.  Structural
/// references and relational payloads remain typed in their command variants;
/// this projection exists only to share the scalar/choice contract with the
/// read-only descriptor surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropertyFieldValue {
    FiniteF64(f64),
    U32(u32),
    Boolean(bool),
    StableIdReference,
    StableIdReferenceCollection(usize),
    EnumChoice(PropertyEnumChoice),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropertyCommandFieldProjection {
    pub field: PropertyFieldId,
    pub value: PropertyFieldValue,
}

/// Commands that intentionally do not address one editable property field.
/// These operations have their own typed authority and transition semantics;
/// keeping them explicit prevents new command variants from being silently
/// omitted from descriptor/command completeness checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonFieldCommandOperation {
    EffectivePatternAuthority,
    AddAuthoredStructure,
    DuplicateAuthoredStructure,
    ReplaceAuthoredStructure,
    RemoveUnreferencedAuthoredStructure,
    AddPatternDefinition,
    AddTypedPatternDefinition,
    ReplaceSelectedChannelDefinitionTopology,
    ReplaceSharedPatternDefinitionRecipe,
    DuplicatePatternDefinition,
    RemoveUnreferencedPatternDefinition,
    ReplaceChannelTopology,
    GuideFaceDimensions,
}

/// Exhaustive command classification at the descriptor boundary. A command is
/// either one or more descriptor-backed field leaves, or an explicitly typed
/// structural/topology operation. There is deliberately no implicit empty
/// fallback for future command variants.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentCommandFieldClassification {
    DescriptorBacked(Vec<PropertyCommandFieldProjection>),
    NonField(NonFieldCommandOperation),
}

impl PatternDefinitionEdit {
    /// Projects one typed structural edit into its descriptor contract without changing authority.
    pub fn field_projection(&self) -> PropertyCommandFieldProjection {
        use PatternDefinitionEdit as Edit;
        let (field, value) = match self {
            Edit::SetParametricShape { shape, .. } => (
                PropertyFieldId::ParametricShape,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::SpiralShape(*shape)),
            ),
            Edit::SetParametricTurns { turns, .. } => (
                PropertyFieldId::ParametricTurns,
                PropertyFieldValue::FiniteF64(*turns),
            ),
            Edit::SetParametricRadialSpacing { radial_spacing, .. } => (
                PropertyFieldId::ParametricRadialSpacing,
                PropertyFieldValue::FiniteF64(*radial_spacing),
            ),
            Edit::SetParametricPhase { phase_degrees, .. } => (
                PropertyFieldId::ParametricPhase,
                PropertyFieldValue::FiniteF64(*phase_degrees),
            ),
            Edit::SetParametricWinding { winding, .. } => (
                PropertyFieldId::ParametricWinding,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::CurveWinding(*winding)),
            ),
            Edit::SetParametricRepetition { repetition, .. } => (
                PropertyFieldId::ParametricRepetition,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::CurveRepetition(
                    match repetition {
                        CurveRepetition::Single => GuideRepetitionKind::Single,
                        CurveRepetition::TransformStack { .. } => {
                            GuideRepetitionKind::TransformStack
                        }
                        CurveRepetition::NormalOffset { .. } => GuideRepetitionKind::NormalOffset,
                    },
                )),
            ),
            Edit::SetParametricOffsetSpacing { spacing, .. } => (
                PropertyFieldId::ParametricOffsetSpacing,
                PropertyFieldValue::FiniteF64(*spacing),
            ),
            Edit::SetParametricOffsetSides { sides, .. } => (
                PropertyFieldId::ParametricOffsetSides,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::OffsetSides(*sides)),
            ),
            Edit::SetParametricOffsetCleanup { cleanup, .. } => (
                PropertyFieldId::ParametricOffsetCleanup,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::OffsetCleanup(*cleanup)),
            ),
            Edit::SetParametricStackDirection { value, .. } => (
                PropertyFieldId::ParametricStackDirection,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetParametricStackSpacingMultiplier { value, .. } => (
                PropertyFieldId::ParametricStackSpacingMultiplier,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetAlongParametricInterval { interval, .. } => (
                PropertyFieldId::AlongParametricInterval,
                PropertyFieldValue::FiniteF64(*interval),
            ),
            Edit::SetAlongParametricPhase { phase, .. } => (
                PropertyFieldId::AlongParametricPhase,
                PropertyFieldValue::FiniteF64(*phase),
            ),
            Edit::SetCoverageGuardSteps { guard_steps } => (
                PropertyFieldId::CoverageGuardSteps,
                PropertyFieldValue::U32(*guard_steps),
            ),
            Edit::SetCoverageAdditionalMargin { additional_margin } => (
                PropertyFieldId::CoverageAdditionalMargin,
                PropertyFieldValue::FiniteF64(*additional_margin),
            ),
            Edit::SetGuideBaselineAngle {
                baseline_angle_degrees,
                ..
            } => (
                PropertyFieldId::GuideBaselineAngle,
                PropertyFieldValue::FiniteF64(*baseline_angle_degrees),
            ),
            Edit::SetGuidePhase { phase, .. } => (
                PropertyFieldId::GuidePhase,
                PropertyFieldValue::FiniteF64(*phase),
            ),
            Edit::SetGuideSpacingMultiplier {
                spacing_multiplier, ..
            } => (
                PropertyFieldId::GuideSpacingMultiplier,
                PropertyFieldValue::FiniteF64(*spacing_multiplier),
            ),
            Edit::SetGuidePrototype { prototype, .. } => (
                PropertyFieldId::GuidePrototype,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::GuidePrototype(
                    match prototype {
                        GuidePrototype::AuthoredOpenPath { .. } => {
                            GuidePrototypeKind::AuthoredOpenPath
                        }
                        GuidePrototype::CircularArc { .. } => GuidePrototypeKind::CircularArc,
                    },
                )),
            ),
            Edit::SetGuideAuthoredStructure { .. } => (
                PropertyFieldId::GuideAuthoredStructure,
                PropertyFieldValue::StableIdReference,
            ),
            Edit::SetGuideArcCenterX { value, .. } => (
                PropertyFieldId::GuideArcCenterX,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideArcCenterY { value, .. } => (
                PropertyFieldId::GuideArcCenterY,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideArcRadius { value, .. } => (
                PropertyFieldId::GuideArcRadius,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideArcStartAngle { value, .. } => (
                PropertyFieldId::GuideArcStartAngle,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideArcSweepAngle { value, .. } => (
                PropertyFieldId::GuideArcSweepAngle,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideRepetition { repetition, .. } => (
                PropertyFieldId::GuideRepetition,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::GuideRepetition(
                    match repetition {
                        GuideRepetition::Single => GuideRepetitionKind::Single,
                        GuideRepetition::TransformStack { .. } => {
                            GuideRepetitionKind::TransformStack
                        }
                        GuideRepetition::NormalOffset { .. } => GuideRepetitionKind::NormalOffset,
                    },
                )),
            ),
            Edit::SetGuideOffsetSpacing { spacing, .. } => (
                PropertyFieldId::GuideOffsetSpacing,
                PropertyFieldValue::FiniteF64(*spacing),
            ),
            Edit::SetGuideOffsetSides { sides, .. } => (
                PropertyFieldId::GuideOffsetSides,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::OffsetSides(*sides)),
            ),
            Edit::SetGuideOffsetCleanup { cleanup, .. } => (
                PropertyFieldId::GuideOffsetCleanup,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::OffsetCleanup(*cleanup)),
            ),
            Edit::SetGuideStackDirection { value, .. } => (
                PropertyFieldId::GuideStackDirection,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetGuideStackSpacingMultiplier { value, .. } => (
                PropertyFieldId::GuideStackSpacingMultiplier,
                PropertyFieldValue::FiniteF64(*value),
            ),
            Edit::SetIntersectionDimensions { dimensions, .. } => (
                PropertyFieldId::IntersectionDimensions,
                PropertyFieldValue::StableIdReferenceCollection(dimensions.len()),
            ),
            Edit::SetIntersectionMergeEpsilon { merge_epsilon, .. } => (
                PropertyFieldId::IntersectionMergeEpsilon,
                PropertyFieldValue::FiniteF64(*merge_epsilon),
            ),
            Edit::SetAlongGuideDimensions { dimensions, .. } => (
                PropertyFieldId::AlongGuideDimensions,
                PropertyFieldValue::StableIdReferenceCollection(dimensions.len()),
            ),
            Edit::SetAlongGuideIntervalMultiplier {
                interval_multiplier,
                ..
            } => (
                PropertyFieldId::AlongGuideIntervalMultiplier,
                PropertyFieldValue::FiniteF64(*interval_multiplier),
            ),
            Edit::SetAlongGuidePhase { phase, .. } => (
                PropertyFieldId::AlongGuidePhase,
                PropertyFieldValue::FiniteF64(*phase),
            ),
            Edit::SetRandomCharacter { character, .. } => (
                PropertyFieldId::RandomCharacter,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::RandomCharacter(
                    match character {
                        RandomSiteCharacter::RawUniform => RandomCharacterKind::RawUniform,
                        RandomSiteCharacter::Even { .. } => RandomCharacterKind::Even,
                        RandomSiteCharacter::Clustered { .. } => RandomCharacterKind::Clustered,
                    },
                )),
            ),
            Edit::SetRandomSeed { seed, .. } => {
                (PropertyFieldId::RandomSeed, PropertyFieldValue::U32(*seed))
            }
            Edit::SetRandomEvenMinimumCenterDistance {
                minimum_center_distance,
                ..
            } => (
                PropertyFieldId::RandomEvenMinimumCenterDistance,
                PropertyFieldValue::FiniteF64(*minimum_center_distance),
            ),
            Edit::SetRandomClusterDensity {
                cluster_density, ..
            } => (
                PropertyFieldId::RandomClusterDensity,
                PropertyFieldValue::FiniteF64(*cluster_density),
            ),
            Edit::SetRandomClusterSpread { cluster_spread, .. } => (
                PropertyFieldId::RandomClusterSpread,
                PropertyFieldValue::FiniteF64(*cluster_spread),
            ),
            Edit::SetRandomClusterStrength {
                cluster_strength, ..
            } => (
                PropertyFieldId::RandomClusterStrength,
                PropertyFieldValue::FiniteF64(*cluster_strength),
            ),
            Edit::SetDensityModulationVariant { modulation, .. } => (
                PropertyFieldId::RandomDensityModulation,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::DensityModulation(
                    match modulation {
                        SiteDensityModulation::Uniform => DensityModulationKind::Uniform,
                        SiteDensityModulation::ArtworkWeighted { .. } => {
                            DensityModulationKind::ArtworkWeighted
                        }
                    },
                )),
            ),
            Edit::SetArtworkWeightMappingComponent { component, .. } => (
                PropertyFieldId::ArtworkWeightMappingComponent,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourceMappingComponent(
                    *component,
                )),
            ),
            Edit::SetArtworkWeightMappingPlacement { placement, .. } => (
                PropertyFieldId::ArtworkWeightMappingPlacement,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourcePlacement(*placement)),
            ),
            Edit::SetArtworkWeightMappingInverted { inverted, .. } => (
                PropertyFieldId::ArtworkWeightMappingInverted,
                PropertyFieldValue::Boolean(*inverted),
            ),
            Edit::SetArtworkWeightMappingGain { gain, .. } => (
                PropertyFieldId::ArtworkWeightMappingGain,
                PropertyFieldValue::FiniteF64(*gain),
            ),
            Edit::SetArtworkWeightMappingBias { bias, .. } => (
                PropertyFieldId::ArtworkWeightMappingBias,
                PropertyFieldValue::FiniteF64(*bias),
            ),
            Edit::SetArtworkWeightStrength { strength, .. } => (
                PropertyFieldId::ArtworkWeightStrength,
                PropertyFieldValue::FiniteF64(*strength),
            ),
            Edit::SetArtworkWeightResponse { response, .. } => (
                PropertyFieldId::ArtworkWeightResponse,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::ArtworkWeightResponse(
                    *response,
                )),
            ),
            Edit::SetExclusionVariant { policy, .. } => (
                PropertyFieldId::RandomExclusion,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::Exclusion(match policy {
                    SiteExclusionPolicy::None => ExclusionKind::None,
                    SiteExclusionPolicy::MinimumCenterDistance { .. } => {
                        ExclusionKind::MinimumCenterDistance
                    }
                    SiteExclusionPolicy::VisibleMarkMargin { .. } => {
                        ExclusionKind::VisibleMarkMargin
                    }
                })),
            ),
            Edit::SetExclusionMinimumCenterDistance {
                minimum_center_distance,
                ..
            } => (
                PropertyFieldId::ExclusionMinimumCenterDistance,
                PropertyFieldValue::FiniteF64(*minimum_center_distance),
            ),
            Edit::SetVisibleMarkMargin { margin, .. } => (
                PropertyFieldId::VisibleMarkMargin,
                PropertyFieldValue::FiniteF64(*margin),
            ),
            Edit::SetVisibleMarkSizingPolicy { sizing, .. } => (
                PropertyFieldId::VisibleMarkSizingPolicy,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::VisibleMarkSizingPolicy(
                    *sizing,
                )),
            ),
            Edit::SetRandomMaximumAttempts {
                maximum_attempts, ..
            } => (
                PropertyFieldId::RandomMaximumAttempts,
                PropertyFieldValue::U32(*maximum_attempts),
            ),
            Edit::SetRandomMaximumNeighborChecks {
                maximum_neighbor_checks,
                ..
            } => (
                PropertyFieldId::RandomMaximumNeighborChecks,
                PropertyFieldValue::U32(*maximum_neighbor_checks),
            ),
            Edit::SetOutputSiteProduct { .. } => (
                PropertyFieldId::OutputSiteProduct,
                PropertyFieldValue::StableIdReference,
            ),
            Edit::SetOutputMarkPrototype { prototype, .. } => (
                PropertyFieldId::OutputPrototype,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::MarkPrototype(
                    match prototype {
                        MarkPrototype::Circle => MarkPrototypeKind::Circle,
                        MarkPrototype::AuthoredClosedShape { .. } => {
                            MarkPrototypeKind::AuthoredClosedShape
                        }
                    },
                )),
            ),
            Edit::SetOutputAuthoredClosedShape { .. } => (
                PropertyFieldId::OutputAuthoredClosedShape,
                PropertyFieldValue::StableIdReference,
            ),
            Edit::SetOutputOrientation { orientation, .. } => (
                PropertyFieldId::OutputOrientation,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::MarkOrientation(
                    match orientation {
                        MarkOrientation::Fixed => MarkOrientationKind::Fixed,
                        MarkOrientation::GuideTangent { .. } => MarkOrientationKind::GuideTangent,
                        MarkOrientation::GuideNormal { .. } => MarkOrientationKind::GuideNormal,
                    },
                )),
            ),
            Edit::SetOutputOrientationDimension { .. } => (
                PropertyFieldId::OutputOrientationDimension,
                PropertyFieldValue::StableIdReference,
            ),
            Edit::SetConnectionProgram { program, .. } => (
                PropertyFieldId::ConnectionProgram,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::ConnectionProgram(
                    program.kind(),
                )),
            ),
            Edit::SetConnectionMaximumDegree { maximum_degree, .. } => (
                PropertyFieldId::ConnectionMaximumDegree,
                PropertyFieldValue::U32(*maximum_degree),
            ),
            Edit::SetConnectionMaximumDistance {
                maximum_distance, ..
            } => (
                PropertyFieldId::ConnectionMaximumDistance,
                PropertyFieldValue::FiniteF64(*maximum_distance),
            ),
            Edit::SetConnectionMinimumDegree { minimum_degree, .. } => (
                PropertyFieldId::ConnectionMinimumDegree,
                PropertyFieldValue::U32(*minimum_degree),
            ),
            Edit::SetConnectionSeed { seed, .. } => (
                PropertyFieldId::ConnectionSeed,
                PropertyFieldValue::U32(*seed),
            ),
            Edit::SetMazeSeed { seed, .. } => {
                (PropertyFieldId::MazeSeed, PropertyFieldValue::U32(*seed))
            }
            Edit::SetGuideFaceDimensions { .. } => (
                // This projection is intentionally unreachable from command classification;
                // it retains a total public method for generic callers.
                PropertyFieldId::OutputSiteProduct,
                PropertyFieldValue::StableIdReference,
            ),
        };
        PropertyCommandFieldProjection { field, value }
    }
}

impl DocumentCommand {
    /// Exhaustively classifies every command at the descriptor boundary.
    /// Compound coordinate commands deliberately yield one independently
    /// addressable field leaf.
    pub fn field_classification(&self) -> DocumentCommandFieldClassification {
        use DocumentCommand as Command;
        let one = |field, value| {
            DocumentCommandFieldClassification::DescriptorBacked(vec![
                PropertyCommandFieldProjection { field, value },
            ])
        };
        match self {
            Command::SetDocumentPatternSettings { .. }
            | Command::ReplaceDocumentPatternDefinitionRecipe { .. }
            | Command::SetChannelPatternDefinitionOverride { .. }
            | Command::ResetChannelPatternDefinitionOverride { .. }
            | Command::ReplaceChannelPatternDefinitionOverrideRecipe { .. }
            | Command::SetChannelDensityDelta { .. }
            | Command::ResetChannelDensityDelta { .. }
            | Command::SetChannelPatternRotationDelta { .. }
            | Command::ResetChannelPatternRotationDelta { .. }
            | Command::SetChannelShapeRotationDelta { .. }
            | Command::ResetChannelShapeRotationDelta { .. }
            | Command::SetChannelOutputResponseDelta { .. }
            | Command::ResetChannelOutputResponseDelta { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::EffectivePatternAuthority,
                )
            }
            Command::AddAuthoredStructure { .. } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::AddAuthoredStructure,
            ),
            Command::DuplicateAuthoredStructure { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::DuplicateAuthoredStructure,
                )
            }
            Command::ReplaceAuthoredStructure { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::ReplaceAuthoredStructure,
                )
            }
            Command::RemoveUnreferencedAuthoredStructure { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::RemoveUnreferencedAuthoredStructure,
                )
            }
            Command::AddPatternDefinition { .. } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::AddPatternDefinition,
            ),
            Command::AddTypedPatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::AddTypedPatternDefinition,
                )
            }
            Command::ReplaceSelectedChannelDefinitionTopology { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::ReplaceSelectedChannelDefinitionTopology,
                )
            }
            Command::ReplaceSharedPatternDefinitionRecipe { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::ReplaceSharedPatternDefinitionRecipe,
                )
            }
            Command::DuplicatePatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::DuplicatePatternDefinition,
                )
            }
            Command::RemoveUnreferencedPatternDefinition { .. } => {
                DocumentCommandFieldClassification::NonField(
                    NonFieldCommandOperation::RemoveUnreferencedPatternDefinition,
                )
            }
            Command::ReplaceChannelTopology { .. } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::ReplaceChannelTopology,
            ),
            Command::SetTranslationAxis {
                edited_axis, value, ..
            } => one(
                match edited_axis {
                    TranslationEditedAxis::X => PropertyFieldId::TranslationX,
                    TranslationEditedAxis::Y => PropertyFieldId::TranslationY,
                },
                PropertyFieldValue::FiniteF64(*value),
            ),
            Command::SetColorComponent {
                component, value, ..
            } => one(
                match component {
                    ColorComponent::Red => PropertyFieldId::ColorRed,
                    ColorComponent::Green => PropertyFieldId::ColorGreen,
                    ColorComponent::Blue => PropertyFieldId::ColorBlue,
                    ColorComponent::Alpha => PropertyFieldId::ColorAlpha,
                },
                PropertyFieldValue::FiniteF64(*value),
            ),
            Command::SetOpacity { opacity, .. } => one(
                PropertyFieldId::Opacity,
                PropertyFieldValue::FiniteF64(*opacity),
            ),
            Command::SetVisibility { visible, .. } => one(
                PropertyFieldId::Visibility,
                PropertyFieldValue::Boolean(*visible),
            ),
            Command::SetSourceReference { .. } => one(
                PropertyFieldId::SourceReference,
                PropertyFieldValue::StableIdReference,
            ),
            Command::SetLegacyMappingField { edit, .. } => one(
                match edit {
                    LegacyMappingFieldEdit::Component(_) => PropertyFieldId::LegacyMappingComponent,
                    LegacyMappingFieldEdit::Placement(_) => PropertyFieldId::LegacyMappingPlacement,
                },
                match edit {
                    LegacyMappingFieldEdit::Component(component) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourceMappingComponent(match component {
                            SourceComponent::Luminance => SourceMappingComponent::Luminance,
                            SourceComponent::Alpha => SourceMappingComponent::Alpha,
                        }),
                    ),
                    LegacyMappingFieldEdit::Placement(placement) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourcePlacement(*placement),
                    ),
                },
            ),
            Command::SetModeledMappingField { edit, .. } => one(
                match edit {
                    ModeledMappingFieldEdit::Component(_) => {
                        PropertyFieldId::ModeledMappingComponent
                    }
                    ModeledMappingFieldEdit::Placement(_) => {
                        PropertyFieldId::ModeledMappingPlacement
                    }
                    ModeledMappingFieldEdit::Inverted(_) => PropertyFieldId::ModeledMappingInverted,
                    ModeledMappingFieldEdit::Gain(_) => PropertyFieldId::ModeledMappingGain,
                    ModeledMappingFieldEdit::Bias(_) => PropertyFieldId::ModeledMappingBias,
                },
                match edit {
                    ModeledMappingFieldEdit::Component(value) => PropertyFieldValue::EnumChoice(
                        PropertyEnumChoice::SourceMappingComponent(*value),
                    ),
                    ModeledMappingFieldEdit::Placement(value) => {
                        PropertyFieldValue::EnumChoice(PropertyEnumChoice::SourcePlacement(*value))
                    }
                    ModeledMappingFieldEdit::Inverted(value) => PropertyFieldValue::Boolean(*value),
                    ModeledMappingFieldEdit::Gain(value) | ModeledMappingFieldEdit::Bias(value) => {
                        PropertyFieldValue::FiniteF64(*value)
                    }
                },
            ),
            Command::SetChannelPaint { paint, .. } => one(
                PropertyFieldId::Paint,
                PropertyFieldValue::EnumChoice(PropertyEnumChoice::Paint(match paint {
                    ChannelPaint::Solid(_) => PaintKind::Solid,
                    ChannelPaint::SampledSource => PaintKind::SampledSource,
                })),
            ),
            Command::EditSelectedChannelPatternDefinition {
                edit: PatternDefinitionEdit::SetGuideFaceDimensions { .. },
                ..
            }
            | Command::EditSharedPatternDefinition {
                edit: PatternDefinitionEdit::SetGuideFaceDimensions { .. },
                ..
            } => DocumentCommandFieldClassification::NonField(
                NonFieldCommandOperation::GuideFaceDimensions,
            ),
            Command::EditSelectedChannelPatternDefinition { edit, .. }
            | Command::EditSharedPatternDefinition { edit, .. } => {
                DocumentCommandFieldClassification::DescriptorBacked(vec![edit.field_projection()])
            }
        }
    }

    /// Projects only descriptor-backed command leaves to their contract
    /// fields. Non-field structural operations intentionally have no property
    /// projections, but are exhaustively represented by `field_classification`.
    pub fn field_projections(&self) -> Vec<PropertyCommandFieldProjection> {
        match self.field_classification() {
            DocumentCommandFieldClassification::DescriptorBacked(projections) => projections,
            DocumentCommandFieldClassification::NonField(_) => Vec::new(),
        }
    }

    /// Returns whether a document-owned structural resource or definition
    /// transition requires reversible `DocumentHistory` ownership.
    ///
    /// The public `DocumentSession` boundary intentionally rejects these
    /// commands; only history records their before/after authoritative
    /// snapshots after validation succeeds.
    fn requires_history(&self) -> bool {
        matches!(
            self,
            Self::SetDocumentPatternSettings { .. }
                | Self::ReplaceDocumentPatternDefinitionRecipe { .. }
                | Self::ReplaceChannelPatternDefinitionOverrideRecipe { .. }
                | Self::AddAuthoredStructure { .. }
                | Self::DuplicateAuthoredStructure { .. }
                | Self::ReplaceAuthoredStructure { .. }
                | Self::RemoveUnreferencedAuthoredStructure { .. }
                | Self::AddPatternDefinition { .. }
                | Self::AddTypedPatternDefinition { .. }
                | Self::ReplaceSelectedChannelDefinitionTopology { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. }
                | Self::EditSelectedChannelPatternDefinition { .. }
                | Self::EditSharedPatternDefinition { .. }
                | Self::ReplaceSharedPatternDefinitionRecipe { .. }
        )
    }

    /// Returns a command's channel target or a neutral ID for document-scoped structural commands.
    ///
    /// The neutral value is used only after command classification exempts non-channel operations
    /// from channel lookup; it is never an implicit document reference.
    fn channel_id(&self) -> ChannelId {
        match self {
            Self::ReplaceSelectedChannelDefinitionTopology { channel_id, .. }
            | Self::SetChannelPatternDefinitionOverride { channel_id, .. }
            | Self::ResetChannelPatternDefinitionOverride { channel_id, .. }
            | Self::ReplaceChannelPatternDefinitionOverrideRecipe { channel_id, .. }
            | Self::SetChannelDensityDelta { channel_id, .. }
            | Self::ResetChannelDensityDelta { channel_id, .. }
            | Self::SetChannelPatternRotationDelta { channel_id, .. }
            | Self::ResetChannelPatternRotationDelta { channel_id, .. }
            | Self::SetChannelShapeRotationDelta { channel_id, .. }
            | Self::ResetChannelShapeRotationDelta { channel_id, .. }
            | Self::SetChannelOutputResponseDelta { channel_id, .. }
            | Self::ResetChannelOutputResponseDelta { channel_id, .. }
            | Self::SetTranslationAxis { channel_id, .. }
            | Self::SetColorComponent { channel_id, .. }
            | Self::SetOpacity { channel_id, .. }
            | Self::SetVisibility { channel_id, .. }
            | Self::SetLegacyMappingField { channel_id, .. }
            | Self::SetModeledMappingField { channel_id, .. }
            | Self::SetChannelPaint { channel_id, .. }
            | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => *channel_id,
            Self::SetSourceReference { .. }
            | Self::SetDocumentPatternSettings { .. }
            | Self::ReplaceDocumentPatternDefinitionRecipe { .. }
            | Self::ReplaceChannelTopology { .. }
            | Self::AddAuthoredStructure { .. }
            | Self::DuplicateAuthoredStructure { .. }
            | Self::ReplaceAuthoredStructure { .. }
            | Self::RemoveUnreferencedAuthoredStructure { .. }
            | Self::AddPatternDefinition { .. }
            | Self::AddTypedPatternDefinition { .. }
            | Self::DuplicatePatternDefinition { .. }
            | Self::RemoveUnreferencedPatternDefinition { .. }
            | Self::EditSharedPatternDefinition { .. }
            | Self::ReplaceSharedPatternDefinitionRecipe { .. } => ChannelId(0),
        }
    }

    /// Validates one command completely before an authoritative candidate document is mutated.
    ///
    /// # Errors
    ///
    /// Returns stable command, authored-structure, or existing document diagnostics while leaving
    /// both the document and history unchanged.
    fn validate(&self, document: &Document) -> Result<(), ValidationError> {
        if !matches!(
            self,
            Self::SetSourceReference { .. }
                | Self::SetDocumentPatternSettings { .. }
                | Self::ReplaceDocumentPatternDefinitionRecipe { .. }
                | Self::ReplaceChannelTopology { .. }
                | Self::AddAuthoredStructure { .. }
                | Self::DuplicateAuthoredStructure { .. }
                | Self::ReplaceAuthoredStructure { .. }
                | Self::RemoveUnreferencedAuthoredStructure { .. }
                | Self::AddPatternDefinition { .. }
                | Self::AddTypedPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. }
                | Self::EditSharedPatternDefinition { .. }
                | Self::ReplaceSharedPatternDefinitionRecipe { .. }
        ) && !document.has_channel(self.channel_id())
        {
            return Err(ValidationError::new(
                "command.channel_id",
                "command targets a missing channel",
            ));
        }

        for projection in self.field_projections() {
            validate_property_field_projection(projection)?;
        }

        match self {
            Self::SetDocumentPatternSettings { base, settings } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "document pattern settings base is stale",
                    ));
                }
                validate_layout(&ChannelPatternLayout {
                    density: settings.density.clone(),
                    rotation_degrees: settings.pattern_rotation_degrees,
                    translation_x: 0.0,
                    translation_y: 0.0,
                })?;
                validate_finite(
                    settings.shape_rotation_degrees,
                    "document.pattern_settings.shape_rotation_degrees",
                )?;
                Ok(())
            }
            Self::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe,
            } => {
                if &document.pattern_settings != base
                    || document.definition(base.definition_id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "document recipe replacement base is stale",
                    ));
                }
                let _ = document.allocate_definition_from_recipe(None, recipe)?;
                Ok(())
            }
            Self::SetChannelPatternDefinitionOverride {
                base,
                definition_id,
                ..
            } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ));
                }
                document
                    .definition(*definition_id)
                    .map(|_| ())
                    .ok_or(ValidationError::new(
                        "channel.pattern.definition_override",
                        "override references a missing pattern definition",
                    ))
            }
            Self::ResetChannelPatternDefinitionOverride { base, .. }
            | Self::ResetChannelDensityDelta { base, .. }
            | Self::ResetChannelPatternRotationDelta { base, .. }
            | Self::ResetChannelShapeRotationDelta { base, .. } => {
                if &document.pattern_settings == base {
                    Ok(())
                } else {
                    Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ))
                }
            }
            Self::ResetChannelOutputResponseDelta {
                base, channel_id, ..
            } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ));
                }
                document
                    .channel_pattern_instance(*channel_id)
                    .map(|_| ())
                    .ok_or(ValidationError::new(
                        "command.channel_id",
                        "command targets a missing channel",
                    ))
            }
            Self::ReplaceChannelPatternDefinitionOverrideRecipe {
                base,
                base_definition,
                recipe,
                channel_id,
            } => {
                if &document.pattern_settings != base
                    || document.definition(base_definition.id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "channel override recipe base is stale",
                    ));
                }
                let _ = document.allocate_definition_from_recipe(Some(*channel_id), recipe)?;
                Ok(())
            }
            Self::SetChannelDensityDelta { base, density, .. } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ));
                }
                validate_finite(
                    density.across_x_delta,
                    "channel.pattern.density_delta.across_x_delta",
                )?;
                validate_finite(
                    density.across_y_delta,
                    "channel.pattern.density_delta.across_y_delta",
                )
            }
            Self::SetChannelPatternRotationDelta {
                base,
                rotation_degrees,
                ..
            }
            | Self::SetChannelShapeRotationDelta {
                base,
                rotation_degrees,
                ..
            } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ));
                }
                validate_finite(*rotation_degrees, "channel.pattern.rotation_delta")
            }
            Self::SetChannelOutputResponseDelta {
                base,
                output_layer_id,
                delta,
                channel_id,
                ..
            } => {
                if &document.pattern_settings != base {
                    return Err(ValidationError::new(
                        "document.pattern_settings.base",
                        "channel command base is stale",
                    ));
                }
                let definition_id = document
                    .effective_channel_pattern(*channel_id)?
                    .definition_id;
                let bundle = document
                    .bundle(definition_id)
                    .expect("effective definition bundle");
                let response = bundle
                    .output_settings
                    .iter()
                    .find(|setting| setting.output_layer_id == *output_layer_id)
                    .ok_or(ValidationError::new(
                        "channel.pattern.output_layer_id",
                        "response delta targets a missing output",
                    ))?;
                validate_response_delta_kind(&response.response, delta)?;
                match delta {
                    ChannelGeometryResponseDelta::Marks(delta) => {
                        if let Some(value) = delta.minimum_fill_delta {
                            validate_finite(value, "channel.pattern.mark_delta.minimum_fill")?;
                        }
                        if let Some(value) = delta.maximum_fill_delta {
                            validate_finite(value, "channel.pattern.mark_delta.maximum_fill")?;
                        }
                        Ok(())
                    }
                    ChannelGeometryResponseDelta::Connected(delta) => {
                        if let Some(value) = delta.minimum_thickness_delta {
                            validate_finite(
                                value,
                                "channel.pattern.stroke_delta.minimum_thickness",
                            )?;
                        }
                        if let Some(value) = delta.maximum_thickness_delta {
                            validate_finite(
                                value,
                                "channel.pattern.stroke_delta.maximum_thickness",
                            )?;
                        }
                        Ok(())
                    }
                }
            }
            Self::AddAuthoredStructure { draft } => {
                next_authored_structure_id(&document.authored_structures)?;
                validate_authored_structure_segments(draft.kind, &draft.segments)
            }
            Self::DuplicateAuthoredStructure { structure_id } => {
                document
                    .authored_structure(*structure_id)
                    .ok_or(ValidationError::new(
                        "authored_structures.reference",
                        "authored structure to duplicate does not exist",
                    ))?;
                next_authored_structure_id(&document.authored_structures)?;
                Ok(())
            }
            Self::ReplaceAuthoredStructure {
                base_structure,
                replacement,
            } => {
                let current = document.authored_structure(base_structure.id()).ok_or(
                    ValidationError::new(
                        "authored_structures.reference",
                        "authored structure to replace does not exist",
                    ),
                )?;
                if current != base_structure {
                    return Err(ValidationError::new(
                        "authored_structures.edit.stale",
                        "authored structure replacement base is stale",
                    ));
                }
                let replaced = current.replace_with(replacement)?;
                if &replaced == current {
                    return Err(ValidationError::new(
                        "authored_structures.edit.noop",
                        "authored structure replacement is a semantic no-op",
                    ));
                }
                Ok(())
            }
            Self::RemoveUnreferencedAuthoredStructure { structure_id } => {
                document
                    .authored_structure(*structure_id)
                    .ok_or(ValidationError::new(
                        "authored_structures.remove.missing",
                        "authored structure to remove does not exist",
                    ))?;
                if document.authored_structure_is_referenced(*structure_id) {
                    return Err(ValidationError::new(
                        "authored_structures.remove.referenced",
                        "referenced authored structures cannot be removed",
                    ));
                }
                Ok(())
            }
            Self::AddPatternDefinition { definition } => {
                document.allocate_definition_from_draft(definition)?;
                Ok(())
            }
            Self::AddTypedPatternDefinition { definition } => {
                validate_definition(definition)?;
                if document.definition(definition.id).is_some() {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "definition ID already exists",
                    ));
                }
                // Candidate validation performs the document-wide collision
                // check over mechanisms, output layers, and dimensions.
                Ok(())
            }
            Self::ReplaceSelectedChannelDefinitionTopology {
                channel_id,
                base_definition,
                definition,
            } => {
                if document.pattern_definition_id_for(*channel_id) != Some(base_definition.id)
                    || document.definition(base_definition.id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "selected definition base is stale",
                    ));
                }
                if definition.id == base_definition.id
                    || document.definition(definition.id).is_some()
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "topology replacement requires a fresh definition ID",
                    ));
                }
                validate_definition(definition)
            }
            Self::DuplicatePatternDefinition { definition_id } => {
                let source = document
                    .definition(*definition_id)
                    .ok_or(ValidationError::new(
                        "pattern_definitions.id",
                        "definition to duplicate does not exist",
                    ))?;
                document.duplicate_definition(source)?;
                Ok(())
            }
            Self::RemoveUnreferencedPatternDefinition { definition_id } => {
                if document.definition(*definition_id).is_none() {
                    return Err(ValidationError::new(
                        "pattern_definitions.id",
                        "definition to remove does not exist",
                    ));
                }
                if !document.linked_channels(*definition_id).is_empty() {
                    return Err(ValidationError::new(
                        "pattern_definitions",
                        "referenced pattern definitions cannot be removed",
                    ));
                }
                Ok(())
            }
            Self::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            } => {
                if document.pattern_definition_id_for(*channel_id) != Some(base_definition.id)
                    || document.definition(base_definition.id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "selected-channel definition base is stale",
                    ));
                }
                let definition = document
                    .definition(base_definition.id)
                    .expect("validated reference");
                validate_definition_edit(definition, edit)?;
                let mut edited = definition.clone();
                apply_definition_edit(&mut edited, edit);
                if &edited == definition {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)?;
                if document.linked_channels(base_definition.id).len() > 1 {
                    // Copy-on-edit must allocate every fresh stable ID before
                    // publication. Validate the exact duplicate/remapped
                    // candidate here so exhaustion and remap failures remain
                    // ordinary atomic command errors rather than apply-time
                    // assertions.
                    let duplicate = document.duplicate_definition(definition)?;
                    let remapped_edit =
                        remap_definition_edit_for_duplicate(definition, &duplicate, edit);
                    let mut remapped = duplicate;
                    apply_definition_edit(&mut remapped, &remapped_edit);
                    validate_definition(&remapped)?;
                }
                Ok(())
            }
            Self::EditSharedPatternDefinition {
                definition_id,
                base_definition,
                edit,
            } => {
                if *definition_id != base_definition.id
                    || document.definition(*definition_id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "shared definition base is stale",
                    ));
                }
                let mut edited = document
                    .definition(*definition_id)
                    .expect("validated reference")
                    .clone();
                validate_definition_edit(&edited, edit)?;
                apply_definition_edit(&mut edited, edit);
                if document.definition(*definition_id) == Some(&edited) {
                    return Err(ValidationError::new(
                        "pattern_definitions.edit",
                        "structural edit is a semantic no-op",
                    ));
                }
                validate_definition(&edited)
            }
            Self::ReplaceSharedPatternDefinitionRecipe {
                definition_id,
                base_definition,
                recipe,
            } => {
                if *definition_id != base_definition.id
                    || document.definition(*definition_id) != Some(base_definition)
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.base",
                        "shared definition base is stale",
                    ));
                }
                let channel_id = document
                    .linked_channels(*definition_id)
                    .first()
                    .copied()
                    .ok_or_else(|| {
                        ValidationError::new(
                            "pattern_definitions.id",
                            "shared replacement targets an unlinked definition",
                        )
                    })?;
                let materialized =
                    document.allocate_definition_from_recipe(Some(channel_id), recipe)?;
                let mut replacement = materialized.bundle;
                replacement.definition.id = *definition_id;
                replacement.validate()?;
                if replacement.definition == *base_definition
                    && materialized.authored_structure.is_none()
                {
                    return Err(ValidationError::new(
                        "pattern_definitions.recipe",
                        "shared replacement is a semantic no-op",
                    ));
                }
                Ok(())
            }
            Self::SetTranslationAxis { .. } => Ok(()),
            Self::SetColorComponent {
                channel_id, value, ..
            } => {
                validate_unit_component(*value, "channel.appearance.color")?;
                if let Some(channel) = document.modeled_channel(*channel_id)
                    && !matches!(channel.paint, ChannelPaint::Solid(_))
                {
                    return Err(ValidationError::new(
                        "channel.paint",
                        "sampled-source paint has no editable solid components",
                    ));
                }
                Ok(())
            }
            Self::SetOpacity { opacity, .. } => {
                validate_unit_component(*opacity, "channel.appearance.opacity")
            }
            Self::SetVisibility { .. } => Ok(()),
            Self::SetSourceReference { source } => match source {
                SourceReference::Unassigned => Ok(()),
                SourceReference::Assigned(id) if id.as_str().trim().is_empty() => {
                    Err(ValidationError::new(
                        "source.reference_id",
                        "source reference ID must not be empty",
                    ))
                }
                SourceReference::Assigned(_) => Ok(()),
            },
            Self::SetLegacyMappingField { .. } if document.channel_topology().is_some() => {
                Err(ValidationError::new(
                    "channel.source_mapping",
                    "modeled channels require a complete Stage 9 source mapping",
                ))
            }
            Self::SetLegacyMappingField { .. } => Ok(()),
            Self::ReplaceChannelTopology { model, topology } => {
                validate_topology(*model, topology, &document.pattern_definition_bundles)
            }
            Self::SetModeledMappingField { channel_id, edit } => {
                let topology = document.channel_topology().ok_or(ValidationError::new(
                    "channel_topology",
                    "complete source mappings require an installed channel topology",
                ))?;
                if !topology
                    .channels
                    .iter()
                    .any(|channel| channel.id == *channel_id)
                {
                    return Err(ValidationError::new(
                        "command.channel_id",
                        "command targets a missing modeled channel",
                    ));
                }
                let mut mapping = topology
                    .channels
                    .iter()
                    .find(|channel| channel.id == *channel_id)
                    .expect("validated channel")
                    .mapping;
                match edit {
                    ModeledMappingFieldEdit::Component(value) => mapping.component = *value,
                    ModeledMappingFieldEdit::Placement(value) => mapping.placement = *value,
                    ModeledMappingFieldEdit::Inverted(value) => mapping.inverted = *value,
                    ModeledMappingFieldEdit::Gain(value) => mapping.gain = *value,
                    ModeledMappingFieldEdit::Bias(value) => mapping.bias = *value,
                }
                validate_source_mapping(mapping)
            }
            Self::SetChannelPaint { channel_id, paint } => {
                let model = document.channel_model().ok_or(ValidationError::new(
                    "channel_topology",
                    "paint changes require an installed channel topology",
                ))?;
                let topology = document
                    .channel_topology()
                    .expect("model and topology validate together");
                let channel = topology
                    .channels
                    .iter()
                    .find(|channel| channel.id == *channel_id)
                    .ok_or(ValidationError::new(
                        "command.channel_id",
                        "command targets a missing modeled channel",
                    ))?;
                validate_paint(model, channel.role, paint)
            }
        }
    }

    /// Applies a command only after validation has established all allocation and topology invariants.
    ///
    /// This private mutation helper relies on its caller to validate first and never publishes a
    /// partial candidate; authoritative session/history code installs the completed clone atomically.
    fn apply_to_valid_document(&self, document: &mut Document) {
        match self {
            Self::SetDocumentPatternSettings { settings, .. } => {
                document.pattern_settings = settings.clone();
                document.prune_document_base_output_response_deltas();
                return;
            }
            Self::ReplaceDocumentPatternDefinitionRecipe { recipe, .. } => {
                let materialized = document
                    .allocate_definition_from_recipe(None, recipe)
                    .expect("validated document recipe");
                if let Some(structure) = materialized.authored_structure {
                    document.authored_structures.push(structure);
                }
                let definition_id = materialized.bundle.definition.id;
                document
                    .pattern_definition_bundles
                    .push(materialized.bundle);
                document.pattern_settings.definition_id = definition_id;
                document.prune_document_base_output_response_deltas();
                return;
            }
            Self::SetChannelPatternDefinitionOverride {
                channel_id,
                definition_id,
                ..
            } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .definition_override = Some(*definition_id);
                document.prune_incompatible_output_response_deltas(*channel_id);
                return;
            }
            Self::ResetChannelPatternDefinitionOverride { channel_id, .. } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .definition_override = None;
                document.prune_incompatible_output_response_deltas(*channel_id);
                return;
            }
            Self::ReplaceChannelPatternDefinitionOverrideRecipe {
                channel_id, recipe, ..
            } => {
                let materialized = document
                    .allocate_definition_from_recipe(Some(*channel_id), recipe)
                    .expect("validated override recipe");
                if let Some(structure) = materialized.authored_structure {
                    document.authored_structures.push(structure);
                }
                let definition_id = materialized.bundle.definition.id;
                document
                    .pattern_definition_bundles
                    .push(materialized.bundle);
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .definition_override = Some(definition_id);
                document.prune_incompatible_output_response_deltas(*channel_id);
                return;
            }
            Self::SetChannelDensityDelta {
                channel_id,
                density,
                ..
            } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .layout_delta
                    .density = Some(density.clone());
                return;
            }
            Self::ResetChannelDensityDelta { channel_id, .. } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .layout_delta
                    .density = None;
                return;
            }
            Self::SetChannelPatternRotationDelta {
                channel_id,
                rotation_degrees,
                ..
            } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .layout_delta
                    .rotation_degrees = Some(*rotation_degrees);
                return;
            }
            Self::ResetChannelPatternRotationDelta { channel_id, .. } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .layout_delta
                    .rotation_degrees = None;
                return;
            }
            Self::SetChannelShapeRotationDelta {
                channel_id,
                rotation_degrees,
                ..
            } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .shape_rotation_delta_degrees = Some(*rotation_degrees);
                return;
            }
            Self::ResetChannelShapeRotationDelta { channel_id, .. } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .shape_rotation_delta_degrees = None;
                return;
            }
            Self::SetChannelOutputResponseDelta {
                channel_id,
                output_layer_id,
                delta,
                ..
            } => {
                let definition_id = document
                    .effective_channel_pattern(*channel_id)
                    .expect("validated channel definition")
                    .definition_id;
                let bundle = document
                    .bundle(definition_id)
                    .expect("validated channel definition bundle")
                    .clone();
                let deltas = &mut document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .output_response_deltas;
                upsert_pattern_output_delta_in_order(
                    &bundle,
                    deltas,
                    PatternOutputResponseDelta {
                        output_layer_id: *output_layer_id,
                        delta: delta.clone(),
                    },
                )
                .expect("validated response delta insertion");
                return;
            }
            Self::ResetChannelOutputResponseDelta {
                channel_id,
                output_layer_id,
                ..
            } => {
                document
                    .channel_pattern_instance_mut(*channel_id)
                    .expect("validated channel")
                    .output_response_deltas
                    .retain(|entry| entry.output_layer_id != *output_layer_id);
                return;
            }
            Self::AddAuthoredStructure { draft } => {
                let id = next_authored_structure_id(&document.authored_structures)
                    .expect("command validation allocated an authored structure ID");
                document.authored_structures.push(
                    AuthoredStructure::new(id, draft.kind, draft.segments.clone())
                        .expect("command validation retained a valid authored structure draft"),
                );
                return;
            }
            Self::DuplicateAuthoredStructure { structure_id } => {
                let source = document
                    .authored_structure(*structure_id)
                    .expect("validated authored structure")
                    .clone();
                let id = next_authored_structure_id(&document.authored_structures)
                    .expect("command validation allocated an authored structure ID");
                document.authored_structures.push(
                    AuthoredStructure::new(id, source.kind, source.segments)
                        .expect("validated authored structure duplication remains valid"),
                );
                return;
            }
            Self::ReplaceAuthoredStructure {
                base_structure,
                replacement,
            } => {
                let structure = document
                    .authored_structures
                    .iter_mut()
                    .find(|structure| structure.id == base_structure.id)
                    .expect("validated authored structure replacement target");
                *structure = structure
                    .replace_with(replacement)
                    .expect("validated authored structure replacement");
                return;
            }
            Self::RemoveUnreferencedAuthoredStructure { structure_id } => {
                document
                    .authored_structures
                    .retain(|structure| structure.id != *structure_id);
                return;
            }
            Self::AddPatternDefinition { definition } => {
                let definition = document
                    .allocate_definition_from_draft(definition)
                    .expect("command validation allocated a definition");
                document
                    .pattern_definition_bundles
                    .push(bundle_from_definition(definition));
                return;
            }
            Self::AddTypedPatternDefinition { definition } => {
                document
                    .pattern_definition_bundles
                    .push(bundle_from_definition(definition.clone()));
                return;
            }
            Self::ReplaceSelectedChannelDefinitionTopology {
                channel_id,
                definition,
                ..
            } => {
                document
                    .pattern_definition_bundles
                    .push(bundle_from_definition(definition.clone()));
                document.retarget_channel(*channel_id, definition.id);
                return;
            }
            Self::DuplicatePatternDefinition { definition_id } => {
                let source = document
                    .definition(*definition_id)
                    .expect("validated definition")
                    .clone();
                let definition = document
                    .duplicate_definition(&source)
                    .expect("validated duplicate allocation");
                document
                    .pattern_definition_bundles
                    .push(bundle_from_definition(definition));
                return;
            }
            Self::RemoveUnreferencedPatternDefinition { definition_id } => {
                document
                    .pattern_definition_bundles
                    .retain(|definition| definition.id != *definition_id);
                return;
            }
            Self::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            } => {
                if document.linked_channels(base_definition.id).len() > 1 {
                    let source = document
                        .bundle(base_definition.id)
                        .expect("validated definition")
                        .clone();
                    let mut clone = document
                        .duplicate_definition(&source.definition)
                        .expect("validated clone allocation");
                    let remapped_edit =
                        remap_definition_edit_for_duplicate(&source.definition, &clone, edit);
                    apply_definition_edit(&mut clone, &remapped_edit);
                    let clone = document
                        .bundle_for_duplicate(&source, clone)
                        .expect("validated output correspondence");
                    let clone_id = clone.definition.id;
                    document.pattern_definition_bundles.push(clone.clone());
                    document.retarget_channel(*channel_id, clone_id);
                    document.remap_channel_output_response_deltas(*channel_id, &source, &clone);
                } else {
                    let definition = document
                        .pattern_definition_bundles
                        .iter_mut()
                        .find(|definition| definition.id == base_definition.id)
                        .expect("validated definition");
                    apply_definition_edit(definition, edit);
                }
                return;
            }
            Self::EditSharedPatternDefinition {
                definition_id,
                edit,
                ..
            } => {
                let definition = document
                    .pattern_definition_bundles
                    .iter_mut()
                    .find(|definition| definition.id == *definition_id)
                    .expect("validated definition");
                apply_definition_edit(definition, edit);
                return;
            }
            Self::ReplaceSharedPatternDefinitionRecipe {
                definition_id,
                recipe,
                ..
            } => {
                let linked_channels = document.linked_channels(*definition_id);
                let materialized = document
                    .allocate_definition_from_recipe(
                        Some(
                            *linked_channels
                                .first()
                                .expect("validated shared definition link"),
                        ),
                        recipe,
                    )
                    .expect("command validation materialized a recipe");
                if let Some(structure) = materialized.authored_structure {
                    document.authored_structures.push(structure);
                }
                let mut replacement = materialized.bundle;
                replacement.definition.id = *definition_id;
                let definition = document
                    .pattern_definition_bundles
                    .iter_mut()
                    .find(|definition| definition.id == *definition_id)
                    .expect("validated definition");
                *definition = replacement;
                for channel_id in linked_channels {
                    document.prune_incompatible_output_response_deltas(channel_id);
                }
                return;
            }
            _ => {}
        }
        if let Self::SetSourceReference { source } = self {
            document.source = source.clone();
            return;
        }
        if let Self::ReplaceChannelTopology { model, topology } = self {
            document.channel_configuration = ChannelConfiguration::Topology {
                model: *model,
                topology: topology.clone(),
            };
            return;
        }
        if document.apply_pattern_control(self) {
            return;
        }
        match &mut document.channel_configuration {
            ChannelConfiguration::Legacy(_) => {
                let channel = document
                    .legacy_channel_mut(self.channel_id())
                    .expect("validated command must target an existing legacy channel");
                match self {
                    Self::SetColorComponent {
                        component, value, ..
                    } => set_color_component(&mut channel.appearance.color, *component, *value),
                    Self::SetOpacity { opacity, .. } => channel.appearance.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.appearance.visible = *visible,
                    Self::SetLegacyMappingField { edit, .. } => match edit {
                        LegacyMappingFieldEdit::Component(value) => {
                            channel.source_mapping.component = *value
                        }
                        LegacyMappingFieldEdit::Placement(value) => {
                            channel.source_mapping.placement = *value
                        }
                    },
                    _ => unreachable!("modeled-only command was validated against legacy state"),
                }
            }
            ChannelConfiguration::Topology { topology, .. } => {
                let channel = topology
                    .channels
                    .iter_mut()
                    .find(|channel| channel.id == self.channel_id())
                    .expect("validated command must target an existing modeled channel");
                match self {
                    Self::SetColorComponent {
                        component, value, ..
                    } => {
                        let ChannelPaint::Solid(color) = &mut channel.paint else {
                            unreachable!("validated solid paint");
                        };
                        set_color_component(color, *component, *value);
                    }
                    Self::SetOpacity { opacity, .. } => channel.opacity = *opacity,
                    Self::SetVisibility { visible, .. } => channel.visible = *visible,
                    Self::SetModeledMappingField { edit, .. } => match edit {
                        ModeledMappingFieldEdit::Component(value) => {
                            channel.mapping.component = *value
                        }
                        ModeledMappingFieldEdit::Placement(value) => {
                            channel.mapping.placement = *value
                        }
                        ModeledMappingFieldEdit::Inverted(value) => {
                            channel.mapping.inverted = *value
                        }
                        ModeledMappingFieldEdit::Gain(value) => channel.mapping.gain = *value,
                        ModeledMappingFieldEdit::Bias(value) => channel.mapping.bias = *value,
                    },
                    Self::SetChannelPaint { paint, .. } => channel.paint = paint.clone(),
                    Self::SetLegacyMappingField { .. } => {
                        unreachable!("legacy mapping is rejected for modeled state")
                    }
                    _ => unreachable!("handled before configuration mutation"),
                }
            }
        }
    }

    /// Derives the authoritative invalidation and affected-consumer result from an exact transition.
    ///
    /// Authored resource replacements enumerate every current generic-guide consumer in channel
    /// order, while unreferenced resource operations retain the established empty result.
    fn result_for_transition(&self, before: &Document, after: &Document) -> CommandResult {
        if matches!(
            self,
            Self::SetDocumentPatternSettings { .. }
                | Self::ReplaceDocumentPatternDefinitionRecipe { .. }
                | Self::SetChannelPatternDefinitionOverride { .. }
                | Self::ResetChannelPatternDefinitionOverride { .. }
                | Self::ReplaceChannelPatternDefinitionOverrideRecipe { .. }
                | Self::SetChannelDensityDelta { .. }
                | Self::ResetChannelDensityDelta { .. }
                | Self::SetChannelPatternRotationDelta { .. }
                | Self::ResetChannelPatternRotationDelta { .. }
                | Self::SetChannelShapeRotationDelta { .. }
                | Self::ResetChannelShapeRotationDelta { .. }
                | Self::SetChannelOutputResponseDelta { .. }
                | Self::ResetChannelOutputResponseDelta { .. }
        ) {
            let mut level = None;
            let affected_channels = after
                .channel_ids()
                .into_iter()
                .filter(|channel_id| {
                    let old = before.effective_channel_pattern(*channel_id).ok();
                    let new = after.effective_channel_pattern(*channel_id).ok();
                    if old == new {
                        return false;
                    }
                    let family = old.as_ref().zip(new.as_ref()).is_none_or(|(old, new)| {
                        old.definition_id != new.definition_id
                            || old.density != new.density
                            || old.pattern_rotation_degrees != new.pattern_rotation_degrees
                            || old.translation_x != new.translation_x
                            || old.translation_y != new.translation_y
                    });
                    level = strongest_invalidation(
                        level,
                        if family {
                            InvalidationLevel::Family
                        } else {
                            InvalidationLevel::Realization
                        },
                    );
                    true
                })
                .collect();
            return CommandResult {
                affected_channels,
                invalidation: level,
                created_authored_structure_id: None,
            };
        }
        let invalidation = match self.field_classification() {
            DocumentCommandFieldClassification::DescriptorBacked(projections) => {
                let projection = projections
                    .first()
                    .expect("descriptor-backed command has one field projection");
                property_field_contract(projection.field).invalidation
            }
            DocumentCommandFieldClassification::NonField(operation) => match operation {
                NonFieldCommandOperation::EffectivePatternAuthority => InvalidationLevel::Family,
                NonFieldCommandOperation::AddAuthoredStructure
                | NonFieldCommandOperation::DuplicateAuthoredStructure
                | NonFieldCommandOperation::RemoveUnreferencedAuthoredStructure
                | NonFieldCommandOperation::AddPatternDefinition
                | NonFieldCommandOperation::AddTypedPatternDefinition
                | NonFieldCommandOperation::ReplaceSelectedChannelDefinitionTopology
                | NonFieldCommandOperation::ReplaceSharedPatternDefinitionRecipe
                | NonFieldCommandOperation::DuplicatePatternDefinition
                | NonFieldCommandOperation::RemoveUnreferencedPatternDefinition => {
                    InvalidationLevel::Family
                }
                NonFieldCommandOperation::GuideFaceDimensions => InvalidationLevel::Family,
                NonFieldCommandOperation::ReplaceChannelTopology => {
                    InvalidationLevel::ChannelTopology
                }
                NonFieldCommandOperation::ReplaceAuthoredStructure => {
                    let DocumentCommand::ReplaceAuthoredStructure { base_structure, .. } = self
                    else {
                        unreachable!("authored replacement classification matches its command")
                    };
                    let replacement = after
                        .authored_structure(base_structure.id)
                        .expect("validated authored replacement preserves its target");
                    if base_structure.kind == AuthoredStructureKind::OpenPath
                        || replacement.kind == AuthoredStructureKind::OpenPath
                    {
                        InvalidationLevel::Family
                    } else {
                        InvalidationLevel::Realization
                    }
                }
            },
        };
        CommandResult {
            affected_channels: match self {
                Self::AddAuthoredStructure { .. }
                | Self::DuplicateAuthoredStructure { .. }
                | Self::RemoveUnreferencedAuthoredStructure { .. }
                | Self::AddPatternDefinition { .. }
                | Self::AddTypedPatternDefinition { .. }
                | Self::DuplicatePatternDefinition { .. }
                | Self::RemoveUnreferencedPatternDefinition { .. } => Vec::new(),
                Self::ReplaceAuthoredStructure { base_structure, .. } => before
                    .channel_ids()
                    .into_iter()
                    .filter(|channel_id| {
                        before
                            .pattern_definition_id_for(*channel_id)
                            .is_some_and(|definition_id| {
                                before.definition(definition_id).is_some_and(|definition| {
                                    definition.mechanisms.iter().any(|mechanism| matches!(mechanism,
                                PatternMechanism::GuideDimensions { dimensions, .. }
                                    if dimensions.iter().any(|dimension| matches!(
                                        dimension.prototype,
                                        GuidePrototype::AuthoredOpenPath { structure_id }
                                            if structure_id == base_structure.id()
                                    ))
                            )) || definition.output_layers.iter().any(|layer| matches!(
                                layer,
                                PatternOutputLayer::MarkPrototype {
                                    prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                                    ..
                                } if *structure_id == base_structure.id()
                            ))
                                })
                            })
                    })
                    .collect(),
                Self::ReplaceSelectedChannelDefinitionTopology { channel_id, .. }
                | Self::EditSelectedChannelPatternDefinition { channel_id, .. } => {
                    vec![*channel_id]
                }
                Self::EditSharedPatternDefinition { .. }
                | Self::ReplaceSharedPatternDefinitionRecipe { .. } => Vec::new(),
                Self::SetSourceReference { .. } => after.channel_ids(),
                Self::ReplaceChannelTopology { topology, .. } => {
                    affected_topology_channels(before.channel_ids(), topology)
                }
                _ => vec![self.channel_id()],
            },
            invalidation: if matches!(
                self,
                Self::SetDocumentPatternSettings { .. }
                    | Self::ReplaceDocumentPatternDefinitionRecipe { .. }
                    | Self::SetChannelPatternDefinitionOverride { .. }
                    | Self::ResetChannelPatternDefinitionOverride { .. }
                    | Self::ReplaceChannelPatternDefinitionOverrideRecipe { .. }
                    | Self::SetChannelDensityDelta { .. }
                    | Self::ResetChannelDensityDelta { .. }
                    | Self::SetChannelPatternRotationDelta { .. }
                    | Self::ResetChannelPatternRotationDelta { .. }
                    | Self::SetChannelShapeRotationDelta { .. }
                    | Self::ResetChannelShapeRotationDelta { .. }
                    | Self::SetChannelOutputResponseDelta { .. }
                    | Self::ResetChannelOutputResponseDelta { .. }
            ) && before.channel_ids().iter().all(|channel_id| {
                before.effective_channel_pattern(*channel_id).ok()
                    == after.effective_channel_pattern(*channel_id).ok()
            }) {
                None
            } else {
                Some(invalidation)
            },
            created_authored_structure_id: match self {
                Self::AddAuthoredStructure { .. } | Self::DuplicateAuthoredStructure { .. } => {
                    after.authored_structures.last().map(AuthoredStructure::id)
                }
                _ => None,
            },
        }
    }
}

/// An immutable revision/channel token. Its fields are private so callers can
/// observe, but not manufacture, a different revision/channel pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationToken {
    revision: Revision,
    channel_id: ChannelId,
}

impl EvaluationToken {
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
}

/// One atomic read of the authority for a requested channel. A snapshot owns
/// both the document clone and its exact token and has no public constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationSnapshot {
    document: Document,
    token: EvaluationToken,
}

impl EvaluationSnapshot {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn token(&self) -> EvaluationToken {
        self.token
    }
}

/// An immutable revision token for complete-document evaluation. Its fields
/// are private so only a `DocumentSession` can mint a document/revision pair.
/// The token remains cheaply carryable by evaluators and schedulers.
///
/// ```compile_fail
/// use toniator_domain::{DocumentEvaluationToken, DocumentId, Revision};
///
/// let _forged = DocumentEvaluationToken {
///     document_id: DocumentId(1),
///     revision: Revision(0),
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentEvaluationToken {
    document_id: DocumentId,
    revision: Revision,
}

impl DocumentEvaluationToken {
    pub fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }
}

/// One atomic read of the complete document authority. A snapshot owns both
/// the document clone and its exact document-level token and has no public
/// constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentEvaluationSnapshot {
    document: Document,
    token: DocumentEvaluationToken,
}

impl DocumentEvaluationSnapshot {
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn token(&self) -> DocumentEvaluationToken {
        self.token
    }
}

/// The exclusive owner of mutable authoritative document state.
#[derive(Clone, Debug)]
pub struct DocumentSession {
    document: Document,
    revision: Revision,
}

impl DocumentSession {
    pub fn new(document: Document) -> Result<Self, ValidationError> {
        document.validate()?;
        Ok(Self {
            document,
            revision: Revision(0),
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn snapshot(&self) -> Document {
        self.document.clone()
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn apply(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        if command.requires_history() {
            return Err(DocumentSessionError::HistoryRequired);
        }
        self.apply_authoritative(command)
    }

    /// Applies a definition or legacy command through the only caller that
    /// records reversible definition transitions.
    fn apply_from_history(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        self.apply_authoritative(command)
    }

    fn apply_authoritative(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        let next_revision = self.next_revision()?;
        let (candidate, result) = self.document.apply_command(command)?;
        self.document = candidate;
        self.revision = next_revision;
        Ok(result)
    }

    /// Installs an already-validated authoritative snapshot while advancing the
    /// session revision. This is deliberately private: `DocumentHistory` is
    /// the only caller and records the snapshots from successful session
    /// transitions itself.
    fn restore_history_snapshot(&mut self, document: Document) -> Result<(), DocumentSessionError> {
        let next_revision = self.next_revision()?;
        self.document = document;
        self.revision = next_revision;
        Ok(())
    }

    fn next_revision(&self) -> Result<Revision, DocumentSessionError> {
        self.revision
            .0
            .checked_add(1)
            .map(Revision)
            .ok_or(DocumentSessionError::RevisionExhausted)
    }
    pub fn evaluation_snapshot(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationSnapshot, ValidationError> {
        if self.document.channel_model().is_some() {
            return Err(ValidationError::new(
                "evaluation.channel_topology",
                "modeled topology evaluation is not available before Stage 9D",
            ));
        }
        if self.document.channel(channel_id).is_none() {
            return Err(ValidationError::new(
                "evaluation.channel_id",
                "evaluation targets a missing channel",
            ));
        }
        Ok(EvaluationSnapshot {
            document: self.document.clone(),
            token: EvaluationToken {
                revision: self.revision,
                channel_id,
            },
        })
    }
    pub fn evaluation_token(
        &self,
        channel_id: ChannelId,
    ) -> Result<EvaluationToken, ValidationError> {
        Ok(self.evaluation_snapshot(channel_id)?.token())
    }

    /// Atomically captures the complete authoritative document with its
    /// session-minted revision token. Unlike the retained channel diagnostic
    /// snapshot, this supports both legacy and modeled document topologies.
    pub fn document_evaluation_snapshot(&self) -> DocumentEvaluationSnapshot {
        DocumentEvaluationSnapshot {
            document: self.document.clone(),
            token: self.document_evaluation_token(),
        }
    }

    pub fn document_evaluation_token(&self) -> DocumentEvaluationToken {
        DocumentEvaluationToken {
            document_id: self.document.id,
            revision: self.revision,
        }
    }

    pub fn accepts_evaluation(&self, token: EvaluationToken) -> bool {
        token.revision == self.revision
    }

    /// Returns whether `token` was minted by this document session for its
    /// still-current document revision.
    pub fn accepts_document_evaluation(&self, token: DocumentEvaluationToken) -> bool {
        token.document_id == self.document.id && token.revision == self.revision
    }
}

/// Session-lifetime reversible authoritative document transitions.
///
/// The history owns no source bytes, evaluator state, caches, or UI state.
/// Entries retain complete validated document snapshots so future document
/// fields participate automatically without history-specific reconstruction.
#[derive(Debug)]
pub struct DocumentHistory {
    session: DocumentSession,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    draft_root: Option<DraftRoot>,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: Document,
    after: Document,
    result: CommandResult,
}

/// Immutable main-history identity captured when a private editor draft begins.
#[derive(Clone, Debug)]
struct DraftRoot {
    document: Document,
    revision: Revision,
}

/// One successful or unchanged private-draft publication summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftSquashResult {
    /// Reports whether the draft equals its immutable root and therefore published no main revision.
    pub unchanged: bool,
    /// Lists affected channels in deterministic main-document order.
    pub affected_channels: Vec<ChannelId>,
    /// Reports the strongest invalidation required by the net published change.
    pub invalidation: Option<InvalidationLevel>,
}

impl DocumentHistory {
    pub fn new(session: DocumentSession) -> Self {
        Self {
            session,
            undo: Vec::new(),
            redo: Vec::new(),
            draft_root: None,
        }
    }

    /// Creates an isolated editable history rooted at this exact current document and revision.
    ///
    /// The returned history remains a normal command/undo/redo authority locally. Only
    /// `squash_draft` may publish it into a main history.
    pub fn new_draft(main: &Self) -> Self {
        Self {
            session: main.session.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
            draft_root: Some(DraftRoot {
                document: main.document().clone(),
                revision: main.revision(),
            }),
        }
    }

    pub fn session(&self) -> &DocumentSession {
        &self.session
    }

    pub fn document(&self) -> &Document {
        self.session.document()
    }

    pub fn revision(&self) -> Revision {
        self.session.revision()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn apply(
        &mut self,
        command: &DocumentCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        let before = self.session.snapshot();
        let result = self.session.apply_from_history(command)?;
        let after = self.session.snapshot();
        self.undo.push(HistoryEntry {
            before,
            after,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }

    /// Duplicates one selected authored resource and retargets exactly that use as one undoable history step.
    ///
    /// This reuses typed selected-copy definition semantics for a channel-local guide or mark reference.
    /// Both candidate transitions validate before either is published, and the allocated resource ID is retained.
    ///
    /// # Errors
    ///
    /// Returns stale-use, definition, validation, or revision failures without changing this history.
    pub fn duplicate_and_retarget_authored_structure(
        &mut self,
        selected_use: AuthoredStructureUse,
    ) -> Result<CommandResult, DocumentSessionError> {
        let before = self.session.snapshot();
        let selected_use = before
            .authored_structure_uses()
            .into_iter()
            .find(|candidate| *candidate == selected_use)
            .ok_or_else(|| {
                DocumentSessionError::Validation(ValidationError::new(
                    "authored_structures.use",
                    "selected authored structure use is stale",
                ))
            })?;
        let (candidate, duplicate_result) =
            before.apply_command(&DocumentCommand::DuplicateAuthoredStructure {
                structure_id: selected_use.structure_id(),
            })?;
        let structure_id = duplicate_result
            .created_authored_structure_id
            .expect("validated duplication allocates one resource ID");
        let (channel_id, definition_id, edit) = match selected_use {
            AuthoredStructureUse::Guide {
                channel_id,
                definition_id,
                mechanism_id,
                dimension_id,
                ..
            } => (
                channel_id,
                definition_id,
                PatternDefinitionEdit::SetGuideAuthoredStructure {
                    mechanism_id,
                    dimension_id,
                    structure_id,
                },
            ),
            AuthoredStructureUse::Mark {
                channel_id,
                definition_id,
                output_layer_id,
                ..
            } => (
                channel_id,
                definition_id,
                PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                    output_layer_id,
                    structure_id,
                },
            ),
        };
        let base_definition = candidate
            .definition(definition_id)
            .cloned()
            .ok_or_else(|| {
                DocumentSessionError::Validation(ValidationError::new(
                    "pattern_definitions.reference",
                    "selected authored structure use references a missing definition",
                ))
            })?;
        let (after, retarget_result) =
            candidate.apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            })?;
        self.session.restore_history_snapshot(after.clone())?;
        let result = CommandResult {
            affected_channels: retarget_result.affected_channels,
            invalidation: strongest_invalidation(
                duplicate_result.invalidation,
                retarget_result.invalidation.expect("retarget invalidates"),
            ),
            created_authored_structure_id: Some(structure_id),
        };
        self.undo.push(HistoryEntry {
            before,
            after,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }

    /// Adds a new authored resource and attaches it to one exact selected-channel descriptor as one undo entry.
    ///
    /// The caller supplies the already-disambiguated active target. Candidate add and selected-copy
    /// attachment both validate before this history advances, so a failure leaves no orphan resource.
    ///
    /// # Errors
    ///
    /// Returns add, selected-channel definition, attachment, validation, or revision failures without
    /// changing this history, its undo/redo stacks, or document-visible resource store.
    pub fn add_and_attach_authored_structure(
        &mut self,
        channel_id: ChannelId,
        attachment: AuthoredStructureAttachment,
        draft: AuthoredStructureDraft,
    ) -> Result<CommandResult, DocumentSessionError> {
        let before = self.session.snapshot();
        let (added, add_result) =
            before.apply_command(&DocumentCommand::AddAuthoredStructure { draft })?;
        let structure_id = add_result
            .created_authored_structure_id
            .expect("validated authored addition allocates one resource ID");
        let definition_id = added.pattern_definition_id_for(channel_id).ok_or_else(|| {
            DocumentSessionError::Validation(ValidationError::new(
                "pattern_definitions.reference",
                "selected channel has no active pattern definition",
            ))
        })?;
        let base_definition = added.definition(definition_id).cloned().ok_or_else(|| {
            DocumentSessionError::Validation(ValidationError::new(
                "pattern_definitions.reference",
                "selected channel definition is missing before authored attachment",
            ))
        })?;
        let (after, attach_result) = match attachment {
            AuthoredStructureAttachment::Guide {
                mechanism_id,
                dimension_id,
            } => added.apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit: PatternDefinitionEdit::SetGuidePrototype {
                    mechanism_id,
                    dimension_id,
                    prototype: GuidePrototype::AuthoredOpenPath { structure_id },
                },
            })?,
            AuthoredStructureAttachment::GuideCustomAlongLayout => {
                let definition =
                    added.custom_authored_along_guide_definition(&base_definition, structure_id)?;
                added.apply_command(&DocumentCommand::ReplaceSelectedChannelDefinitionTopology {
                    channel_id,
                    base_definition,
                    definition,
                })?
            }
            AuthoredStructureAttachment::Mark { output_layer_id } => {
                added.apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
                    channel_id,
                    base_definition,
                    edit: PatternDefinitionEdit::SetOutputMarkPrototype {
                        output_layer_id,
                        prototype: MarkPrototype::AuthoredClosedShape { structure_id },
                    },
                })?
            }
        };
        self.session.restore_history_snapshot(after.clone())?;
        let result = CommandResult {
            affected_channels: attach_result.affected_channels,
            invalidation: strongest_invalidation(
                add_result.invalidation,
                attach_result.invalidation.expect("attachment invalidates"),
            ),
            created_authored_structure_id: Some(structure_id),
        };
        self.undo.push(HistoryEntry {
            before,
            after,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }

    /// Duplicates, retargets one typed use, and replaces the new resource as one undoable history step.
    ///
    /// The method keeps all three structural transitions inside one candidate snapshot. It is the
    /// private-draft shared-resource boundary: the original shared resource remains untouched,
    /// the exact selected guide or mark use points at the allocated ID, and the replacement owns
    /// that same allocated ID before the history cursor advances.
    ///
    /// # Errors
    ///
    /// Returns stale-use, definition, replacement, validation, or revision failures without
    /// changing the document, undo stack, redo stack, or allocated-resource visibility.
    pub fn duplicate_retarget_and_replace_authored_structure(
        &mut self,
        selected_use: AuthoredStructureUse,
        replacement: AuthoredStructureDraft,
    ) -> Result<CommandResult, DocumentSessionError> {
        let before = self.session.snapshot();
        let selected_use = before
            .authored_structure_uses()
            .into_iter()
            .find(|candidate| *candidate == selected_use)
            .ok_or_else(|| {
                DocumentSessionError::Validation(ValidationError::new(
                    "authored_structures.use",
                    "selected authored structure use is stale",
                ))
            })?;
        let (duplicated, duplicate_result) =
            before.apply_command(&DocumentCommand::DuplicateAuthoredStructure {
                structure_id: selected_use.structure_id(),
            })?;
        let structure_id = duplicate_result
            .created_authored_structure_id
            .expect("validated duplication allocates one resource ID");
        let (channel_id, definition_id, edit) = match selected_use {
            AuthoredStructureUse::Guide {
                channel_id,
                definition_id,
                mechanism_id,
                dimension_id,
                ..
            } => (
                channel_id,
                definition_id,
                PatternDefinitionEdit::SetGuideAuthoredStructure {
                    mechanism_id,
                    dimension_id,
                    structure_id,
                },
            ),
            AuthoredStructureUse::Mark {
                channel_id,
                definition_id,
                output_layer_id,
                ..
            } => (
                channel_id,
                definition_id,
                PatternDefinitionEdit::SetOutputAuthoredClosedShape {
                    output_layer_id,
                    structure_id,
                },
            ),
        };
        let base_definition = duplicated
            .definition(definition_id)
            .cloned()
            .ok_or_else(|| {
                DocumentSessionError::Validation(ValidationError::new(
                    "pattern_definitions.reference",
                    "selected authored structure use references a missing definition",
                ))
            })?;
        let (retargeted, retarget_result) =
            duplicated.apply_command(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit,
            })?;
        let base_structure = retargeted
            .authored_structure(structure_id)
            .cloned()
            .ok_or_else(|| {
                DocumentSessionError::Validation(ValidationError::new(
                    "authored_structures.reference",
                    "duplicated authored structure is missing before replacement",
                ))
            })?;
        let (after, replace_result) =
            retargeted.apply_command(&DocumentCommand::ReplaceAuthoredStructure {
                base_structure,
                replacement,
            })?;
        self.session.restore_history_snapshot(after.clone())?;
        let result = CommandResult {
            affected_channels: retarget_result.affected_channels,
            invalidation: strongest_invalidation(
                strongest_invalidation(
                    duplicate_result.invalidation,
                    retarget_result.invalidation.expect("retarget invalidates"),
                ),
                replace_result
                    .invalidation
                    .expect("replacement invalidates"),
            ),
            created_authored_structure_id: Some(structure_id),
        };
        self.undo.push(HistoryEntry {
            before,
            after,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }

    pub fn undo(&mut self) -> Result<Option<CommandResult>, DocumentSessionError> {
        let Some(entry) = self.undo.last() else {
            return Ok(None);
        };
        self.session
            .restore_history_snapshot(entry.before.clone())?;
        let entry = self
            .undo
            .pop()
            .expect("history entry remains present after successful restoration");
        let result = entry.result.clone();
        self.redo.push(entry);
        Ok(Some(result))
    }

    pub fn redo(&mut self) -> Result<Option<CommandResult>, DocumentSessionError> {
        let Some(entry) = self.redo.last() else {
            return Ok(None);
        };
        self.session.restore_history_snapshot(entry.after.clone())?;
        let entry = self
            .redo
            .pop()
            .expect("history entry remains present after successful restoration");
        let result = entry.result.clone();
        self.undo.push(entry);
        Ok(Some(result))
    }

    /// Publishes a still-current private draft as one reversible main-history transition.
    ///
    /// # Errors
    ///
    /// Returns a stable draft-kind, stale-root, document-validation, or revision-exhaustion error
    /// without changing either history. A draft equal to its root is an unchanged no-op.
    pub fn squash_draft(
        &mut self,
        draft: &DocumentHistory,
    ) -> Result<DraftSquashResult, DocumentSessionError> {
        let root = draft.draft_root.as_ref().ok_or_else(|| {
            DocumentSessionError::Validation(ValidationError::new(
                "document.draft",
                "only a private draft history can be squashed",
            ))
        })?;
        if self.document() != &root.document || self.revision() != root.revision {
            return Err(DocumentSessionError::Validation(ValidationError::new(
                "document.draft",
                "private draft root is stale against the main history",
            )));
        }
        let final_document = draft.document();
        if final_document == &root.document {
            return Ok(DraftSquashResult {
                unchanged: true,
                affected_channels: Vec::new(),
                invalidation: None,
            });
        }
        final_document.validate()?;
        let result = squash_result(&root.document, final_document);
        let before = self.session.snapshot();
        self.session
            .restore_history_snapshot(final_document.clone())?;
        self.undo.push(HistoryEntry {
            before,
            after: final_document.clone(),
            result: CommandResult {
                affected_channels: result.affected_channels.clone(),
                invalidation: result.invalidation,
                created_authored_structure_id: None,
            },
        });
        self.redo.clear();
        Ok(result)
    }
}

/// Summarizes the net draft document difference without replaying draft command history.
fn squash_result(before: &Document, after: &Document) -> DraftSquashResult {
    let changed_structures = before
        .authored_structures
        .iter()
        .chain(after.authored_structures.iter())
        .filter_map(|structure| {
            let id = structure.id();
            (before.authored_structure(id) != after.authored_structure(id)).then_some(id)
        })
        .collect::<HashSet<_>>();
    let changed_definitions = before
        .pattern_definition_bundles
        .iter()
        .chain(after.pattern_definition_bundles.iter())
        .filter_map(|definition| {
            (before.definition(definition.id) != after.definition(definition.id))
                .then_some(definition.id)
        })
        .collect::<HashSet<_>>();
    let source_changed = before.source != after.source;
    let mut level = if source_changed {
        Some(InvalidationLevel::Source)
    } else {
        None
    };
    let before_channel_ids = before.channel_ids();
    let after_channel_ids = after.channel_ids();
    let topology_changed = before.channel_model() != after.channel_model()
        || before_channel_ids != after_channel_ids
        || after_channel_ids.iter().any(|id| {
            before.modeled_channel(*id).map(|value| value.role)
                != after.modeled_channel(*id).map(|value| value.role)
        });
    let mut changed_channels = HashSet::new();
    for channel_id in &after_channel_ids {
        let before_effective = before.effective_channel_pattern(*channel_id).ok();
        let after_effective = after.effective_channel_pattern(*channel_id).ok();
        if before_effective != after_effective {
            changed_channels.insert(*channel_id);
            let family = before_effective
                .as_ref()
                .zip(after_effective.as_ref())
                .is_none_or(|(old, new)| {
                    old.definition_id != new.definition_id
                        || old.density != new.density
                        || old.pattern_rotation_degrees != new.pattern_rotation_degrees
                        || old.translation_x != new.translation_x
                        || old.translation_y != new.translation_y
                });
            level = strongest_invalidation(
                level,
                if family {
                    InvalidationLevel::Family
                } else {
                    InvalidationLevel::Realization
                },
            );
            continue;
        }
        match (
            before.modeled_channel(*channel_id),
            after.modeled_channel(*channel_id),
        ) {
            (Some(old), Some(new)) if old != new => {
                if old.mapping != new.mapping {
                    changed_channels.insert(*channel_id);
                    level = strongest_invalidation(level, InvalidationLevel::Source);
                } else if old.paint != new.paint
                    || old.visible != new.visible
                    || old.opacity != new.opacity
                {
                    changed_channels.insert(*channel_id);
                    level = strongest_invalidation(level, InvalidationLevel::Presentation);
                }
            }
            (None, None) => {
                let old = before.channel(*channel_id);
                let new = after.channel(*channel_id);
                if old != new {
                    if old.map(|value| value.source_mapping)
                        != new.map(|value| value.source_mapping)
                    {
                        changed_channels.insert(*channel_id);
                        level = strongest_invalidation(level, InvalidationLevel::Source);
                    } else if old.map(|value| &value.appearance)
                        != new.map(|value| &value.appearance)
                    {
                        changed_channels.insert(*channel_id);
                        level = strongest_invalidation(level, InvalidationLevel::Presentation);
                    }
                }
            }
            _ => {}
        }
    }
    if topology_changed {
        level = strongest_invalidation(level, InvalidationLevel::ChannelTopology);
    }
    for id in &changed_structures {
        let kind = before
            .authored_structure(*id)
            .or_else(|| after.authored_structure(*id))
            .map(AuthoredStructure::kind);
        level = strongest_invalidation(
            level,
            match kind {
                Some(AuthoredStructureKind::ClosedShape) => InvalidationLevel::Realization,
                _ => InvalidationLevel::Family,
            },
        );
    }
    if !changed_definitions.is_empty() {
        level = strongest_invalidation(level, InvalidationLevel::Family);
    }
    let uses = after.authored_structure_uses();
    let affected_channels = after
        .channel_ids()
        .into_iter()
        .filter(|channel_id| {
            let definition_id = after.pattern_definition_id_for(*channel_id);
            topology_changed
                || source_changed
                || changed_channels.contains(channel_id)
                || definition_id.is_some_and(|id| changed_definitions.contains(&id))
                || uses.iter().any(|usage| match usage {
                    AuthoredStructureUse::Guide {
                        channel_id: owner,
                        structure_id,
                        ..
                    }
                    | AuthoredStructureUse::Mark {
                        channel_id: owner,
                        structure_id,
                        ..
                    } => owner == channel_id && changed_structures.contains(structure_id),
                })
        })
        .collect();
    DraftSquashResult {
        unchanged: false,
        affected_channels,
        invalidation: level,
    }
}

/// Chooses the strongest pipeline invalidation using the established authority ordering.
fn strongest_invalidation(
    current: Option<InvalidationLevel>,
    next: InvalidationLevel,
) -> Option<InvalidationLevel> {
    let rank = |value| match value {
        InvalidationLevel::Presentation => 0,
        InvalidationLevel::Realization => 1,
        InvalidationLevel::Family => 2,
        InvalidationLevel::Source => 3,
        InvalidationLevel::ChannelTopology => 4,
    };
    Some(match current {
        Some(value) if rank(value) >= rank(next) => value,
        _ => next,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentSessionError {
    Validation(ValidationError),
    RevisionExhausted,
    /// Stage 14 definition commands must retain their inverse in
    /// `DocumentHistory`; public session application is intentionally refused.
    HistoryRequired,
}
impl From<ValidationError> for DocumentSessionError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
impl fmt::Display for DocumentSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::RevisionExhausted => f.write_str("document revision is exhausted"),
            Self::HistoryRequired => {
                f.write_str("pattern definition commands require document history")
            }
        }
    }
}
impl Error for DocumentSessionError {}

#[cfg(test)]
mod history_tests {
    use super::*;

    /// Builds one reversible current-authority history for revision-exhaustion witnesses.
    fn history() -> DocumentHistory {
        let definition = PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 0,
                additional_margin: 5.0,
            },
        );
        let document = Document::new(
            DocumentId(1),
            CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            vec![PatternDefinitionBundle {
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: PatternOutputLayerId(1),
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                }],
                definition,
            }],
            DocumentPatternSettings {
                definition_id: PatternDefinitionId(1),
                density: DensityMetric2D {
                    across_x: 1.0,
                    across_y: 1.0,
                    aspect_locked: true,
                },
                pattern_rotation_degrees: 0.0,
                shape_rotation_degrees: 0.0,
            },
            vec![ChannelState {
                id: ChannelId(1),
                pattern_instance: ChannelPatternInstance {
                    definition_override: None,
                    layout_delta: ChannelPatternLayoutDelta {
                        density: None,
                        rotation_degrees: None,
                        translation_x: 0.0,
                        translation_y: 0.0,
                    },
                    shape_rotation_delta_degrees: None,
                    output_response_deltas: Vec::new(),
                },
                appearance: ChannelAppearance {
                    visible: true,
                    color: ColorValue {
                        red: 0.0,
                        green: 0.0,
                        blue: 0.0,
                        alpha: 1.0,
                    },
                    opacity: 1.0,
                },
                source_mapping: ChannelSourceMapping {
                    component: SourceComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                },
            }],
        )
        .unwrap();
        DocumentHistory::new(DocumentSession::new(document).unwrap())
    }

    /// Builds two ordered channels linked to one editable parametric definition.
    fn parametric_history() -> DocumentHistory {
        let curve_id = PatternMechanismId(41);
        let site_id = PatternMechanismId(42);
        let definition = PatternDefinition {
            id: PatternDefinitionId(40),
            name: "parametric history".into(),
            family: PatternFamily::ParametricCurve {
                curve_mechanism_id: curve_id,
                site_mechanism_id: Some(site_id),
            },
            mechanisms: vec![
                PatternMechanism::ParametricCurveSource {
                    id: curve_id,
                    curve: ParametricCurve::Spiral(SpiralCurve {
                        shape: SpiralShape::Round,
                        turns: 1.0,
                        radial_spacing: 8.0,
                        phase_degrees: 0.0,
                        winding: CurveWinding::CounterClockwise,
                    }),
                    repetition: CurveRepetition::Single,
                },
                PatternMechanism::AlongParametricCurveSites {
                    id: site_id,
                    curve_mechanism_id: curve_id,
                    interval: 6.0,
                    phase: 0.0,
                },
            ],
            output_layers: vec![PatternOutputLayer::MarkPrototype {
                id: PatternOutputLayerId(43),
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            }],
            modulation: PatternModulation,
            coverage: CoveragePolicy {
                guard_steps: 0,
                additional_margin: 1.0,
            },
        };
        let mut first = history()
            .document()
            .channels()
            .expect("legacy fixture has channels")[0]
            .clone();
        first.pattern_instance.definition_override = Some(definition.id);
        let mut second = first.clone();
        second.id = ChannelId(2);
        let document = Document::new(
            DocumentId(40),
            CanvasSpec {
                width: 80.0,
                height: 60.0,
            },
            vec![PatternDefinitionBundle {
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: PatternOutputLayerId(43),
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                }],
                definition,
            }],
            DocumentPatternSettings {
                definition_id: PatternDefinitionId(40),
                density: DensityMetric2D {
                    across_x: 8.0,
                    across_y: 6.0,
                    aspect_locked: false,
                },
                pattern_rotation_degrees: 0.0,
                shape_rotation_degrees: 0.0,
            },
            vec![first, second],
        )
        .expect("parametric history document validates");
        DocumentHistory::new(DocumentSession::new(document).expect("parametric session validates"))
    }

    /// Builds two channels linked to one connection-path definition with the supplied authored program.
    fn connection_history(program: ConnectionProgram) -> DocumentHistory {
        let definition_id = PatternDefinitionId(60);
        let guide_id = PatternMechanismId(61);
        let site_id = PatternMechanismId(62);
        let output_id = PatternOutputLayerId(63);
        let mut definition = PatternDefinition::supported_straight_grid(
            definition_id,
            "connection history",
            guide_id,
            site_id,
            output_id,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::ConnectionPaths {
            id: output_id,
            site_mechanism_id: site_id,
            program,
            style: PathStrokeStyle::default(),
        }];
        let mut first = history()
            .document()
            .channels()
            .expect("legacy fixture has a channel")[0]
            .clone();
        let mut second = first.clone();
        second.id = ChannelId(2);
        first.pattern_instance.definition_override = None;
        second.pattern_instance.definition_override = None;
        let document = Document::new(
            DocumentId(60),
            CanvasSpec {
                width: 80.0,
                height: 60.0,
            },
            vec![PatternDefinitionBundle {
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: output_id,
                    response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                        minimum_thickness: 0.1,
                        maximum_thickness: 1.0,
                    }),
                }],
                definition,
            }],
            DocumentPatternSettings {
                definition_id,
                density: DensityMetric2D {
                    across_x: 8.0,
                    across_y: 6.0,
                    aspect_locked: false,
                },
                pattern_rotation_degrees: 0.0,
                shape_rotation_degrees: 0.0,
            },
            vec![first, second],
        )
        .expect("connection history document validates");
        DocumentHistory::new(DocumentSession::new(document).expect("connection session validates"))
    }

    /// Builds two channels sharing one selected two-guide Guide Faces definition for copy-on-edit history checks.
    fn guide_face_history() -> DocumentHistory {
        let definition_id = PatternDefinitionId(210);
        let guide_id = PatternMechanismId(211);
        let site_id = PatternMechanismId(212);
        let output_id = PatternOutputLayerId(213);
        let first_dimension = GuideDimensionId(214);
        let second_dimension = GuideDimensionId(215);
        let third_dimension = GuideDimensionId(216);
        let mut definition = PatternDefinition::generalized_straight_guides(
            definition_id,
            "guide-face history",
            guide_id,
            site_id,
            output_id,
            vec![
                StraightGuideDimension {
                    id: first_dimension,
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: second_dimension,
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: third_dimension,
                    baseline_angle_degrees: 45.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![first_dimension, second_dimension],
                merge_epsilon: 0.0,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition.output_layers = vec![PatternOutputLayer::Regions {
            id: output_id,
            source: RegionSourceIntent::GuideFaces {
                guide_mechanism_id: guide_id,
                dimensions: vec![first_dimension, second_dimension],
            },
        }];
        let mut first = history().document().channels().expect("history channel")[0].clone();
        let mut second = first.clone();
        second.id = ChannelId(2);
        first.pattern_instance.definition_override = None;
        second.pattern_instance.definition_override = None;
        let document = Document::new(
            DocumentId(210),
            CanvasSpec {
                width: 80.0,
                height: 60.0,
            },
            vec![PatternDefinitionBundle {
                output_settings: vec![PatternOutputSettings {
                    output_layer_id: output_id,
                    response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
                }],
                definition,
            }],
            DocumentPatternSettings {
                definition_id,
                density: DensityMetric2D {
                    across_x: 8.0,
                    across_y: 6.0,
                    aspect_locked: false,
                },
                pattern_rotation_degrees: 0.0,
                shape_rotation_degrees: 0.0,
            },
            vec![first, second],
        )
        .expect("guide-face history document validates");
        DocumentHistory::new(
            DocumentSession::new(document).expect("guide-face history session validates"),
        )
    }

    /// Returns the output, guide mechanism, all owned dimensions, and selected dimensions for one Guide Faces definition.
    fn guide_face_binding(
        definition: &PatternDefinition,
    ) -> (
        PatternOutputLayerId,
        PatternMechanismId,
        Vec<GuideDimensionId>,
        Vec<GuideDimensionId>,
    ) {
        let (output_layer_id, guide_mechanism_id, selected) = definition
            .output_layers
            .iter()
            .find_map(|output| match output {
                PatternOutputLayer::Regions {
                    id,
                    source:
                        RegionSourceIntent::GuideFaces {
                            guide_mechanism_id,
                            dimensions,
                        },
                } => Some((*id, *guide_mechanism_id, dimensions.clone())),
                _ => None,
            })
            .expect("Guide Faces output exists");
        let dimensions = definition
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::StraightGuideDimensions { id, dimensions }
                    if *id == guide_mechanism_id =>
                {
                    Some(dimensions.iter().map(|dimension| dimension.id).collect())
                }
                _ => None,
            })
            .expect("Guide Faces guide mechanism exists");
        (output_layer_id, guide_mechanism_id, dimensions, selected)
    }

    /// Proves Guide Faces dimensions copy/remap on selected-channel edit and history restores exact shared authority.
    #[test]
    fn guide_face_dimension_edit_copies_remaps_and_undoes_exactly() {
        let mut history = guide_face_history();
        let before = history.document().clone();
        let base = before
            .definition(PatternDefinitionId(210))
            .expect("shared definition")
            .clone();
        let result = history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: base.clone(),
                edit: PatternDefinitionEdit::SetGuideFaceDimensions {
                    output_layer_id: PatternOutputLayerId(213),
                    dimensions: vec![GuideDimensionId(214), GuideDimensionId(216)],
                },
            })
            .expect("selected Guide Faces edit applies");
        assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
        assert_ne!(
            history.document().pattern_definition_id_for(ChannelId(1)),
            history.document().pattern_definition_id_for(ChannelId(2)),
        );
        let copied = history
            .document()
            .definition(
                history
                    .document()
                    .pattern_definition_id_for(ChannelId(1))
                    .expect("selected definition resolves"),
            )
            .expect("copied definition resolves");
        let shared = history
            .document()
            .definition(PatternDefinitionId(210))
            .expect("shared definition remains");
        let base_binding = guide_face_binding(&base);
        let copied_binding = guide_face_binding(copied);
        assert_ne!(copied_binding.0, base_binding.0, "output ID remaps");
        assert_ne!(copied_binding.1, base_binding.1, "guide ID remaps");
        assert_eq!(copied_binding.2.len(), base_binding.2.len());
        assert!(
            copied_binding
                .2
                .iter()
                .zip(&base_binding.2)
                .all(|(copied, original)| copied != original),
            "each owned guide dimension remaps",
        );
        assert_eq!(
            copied_binding.3,
            vec![copied_binding.2[0], copied_binding.2[2]],
            "edited dimensions use the exact old-to-new correspondence",
        );
        assert_eq!(
            guide_face_binding(shared),
            base_binding,
            "the shared base remains unchanged after selected copy-on-edit",
        );
        let after = history.document().clone();
        history
            .undo()
            .expect("undo applies")
            .expect("inverse exists");
        assert_eq!(history.document(), &before);
        history.redo().expect("redo applies").expect("redo exists");
        assert_eq!(history.document(), &after);
    }

    /// Proves shared Guide Faces edits retain IDs, affect linked channels, reject stale bases, and undo exactly.
    #[test]
    fn guide_face_dimension_edit_shared_keeps_ids_and_history_exact() {
        let mut history = guide_face_history();
        let before = history.document().clone();
        let base = before
            .definition(PatternDefinitionId(210))
            .expect("shared definition")
            .clone();
        let binding_before = guide_face_binding(&base);
        let result = history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(210),
                base_definition: base.clone(),
                edit: PatternDefinitionEdit::SetGuideFaceDimensions {
                    output_layer_id: PatternOutputLayerId(213),
                    dimensions: vec![GuideDimensionId(214), GuideDimensionId(216)],
                },
            })
            .expect("shared Guide Faces edit applies");
        assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
        assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
        assert_eq!(
            history.document().pattern_definition_id_for(ChannelId(1)),
            Some(PatternDefinitionId(210)),
        );
        assert_eq!(
            history.document().pattern_definition_id_for(ChannelId(2)),
            Some(PatternDefinitionId(210)),
        );
        let edited = history
            .document()
            .definition(PatternDefinitionId(210))
            .expect("shared definition remains");
        let binding_after = guide_face_binding(edited);
        assert_eq!(binding_after.0, binding_before.0, "output ID stays shared");
        assert_eq!(binding_after.1, binding_before.1, "guide ID stays shared");
        assert_eq!(
            binding_after.2, binding_before.2,
            "dimension IDs stay shared"
        );
        assert_eq!(
            binding_after.3,
            vec![GuideDimensionId(214), GuideDimensionId(216)],
        );
        assert!(matches!(
            history.apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(210),
                base_definition: base,
                edit: PatternDefinitionEdit::SetGuideFaceDimensions {
                    output_layer_id: PatternOutputLayerId(213),
                    dimensions: vec![GuideDimensionId(214), GuideDimensionId(215)],
                },
            }),
            Err(DocumentSessionError::Validation(error)) if error.path() == "pattern_definitions.base"
        ));
        let after = history.document().clone();
        history
            .undo()
            .expect("undo applies")
            .expect("inverse exists");
        assert_eq!(history.document(), &before);
        history.redo().expect("redo applies").expect("redo exists");
        assert_eq!(history.document(), &after);
    }

    /// Returns one valid connection program for each descriptor applicability witness.
    fn connection_program(kind: ConnectionProgramKind) -> ConnectionProgram {
        let adjacency = ConnectionAdjacencyIntent {
            maximum_degree: 3,
            maximum_distance: 12.0,
        };
        match kind {
            ConnectionProgramKind::NearestLinks => ConnectionProgram::NearestLinks { adjacency },
            ConnectionProgramKind::RandomLinks => ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree: 1,
                seed: 7,
            },
            ConnectionProgramKind::GridSpanningTree => ConnectionProgram::GridSpanningTree {
                adjacency,
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed: 9,
            },
        }
    }

    /// Proves parametric descriptors and every active edit retain family authority, stale bases,
    /// copy-on-edit, history, and private-draft squash semantics without UI involvement.
    #[test]
    fn parametric_curve_edits_are_descriptor_driven_reversible_and_draft_safe() {
        let mut history = parametric_history();
        let expected_fields = [
            PropertyFieldId::ParametricShape,
            PropertyFieldId::ParametricTurns,
            PropertyFieldId::ParametricRadialSpacing,
            PropertyFieldId::ParametricPhase,
            PropertyFieldId::ParametricWinding,
            PropertyFieldId::ParametricRepetition,
            PropertyFieldId::AlongParametricInterval,
            PropertyFieldId::AlongParametricPhase,
        ];
        let descriptors = history.document().property_descriptors();
        let values = history.document().property_values();
        assert!(expected_fields.iter().all(|field| {
            descriptors
                .iter()
                .any(|descriptor| descriptor.field == *field)
                && values.iter().any(|value| value.descriptor.field == *field)
        }));

        let original = history
            .document()
            .definition(PatternDefinitionId(40))
            .expect("linked definition")
            .clone();
        let edits = [
            PatternDefinitionEdit::SetParametricShape {
                mechanism_id: PatternMechanismId(41),
                shape: SpiralShape::Square,
            },
            PatternDefinitionEdit::SetParametricTurns {
                mechanism_id: PatternMechanismId(41),
                turns: 1.5,
            },
            PatternDefinitionEdit::SetParametricRadialSpacing {
                mechanism_id: PatternMechanismId(41),
                radial_spacing: 9.0,
            },
            PatternDefinitionEdit::SetParametricPhase {
                mechanism_id: PatternMechanismId(41),
                phase_degrees: 12.0,
            },
            PatternDefinitionEdit::SetParametricWinding {
                mechanism_id: PatternMechanismId(41),
                winding: CurveWinding::Clockwise,
            },
            PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(41),
                repetition: CurveRepetition::TransformStack {
                    direction_degrees: 0.0,
                    spacing_multiplier: 1.0,
                },
            },
            PatternDefinitionEdit::SetParametricStackDirection {
                mechanism_id: PatternMechanismId(41),
                value: 15.0,
            },
            PatternDefinitionEdit::SetParametricStackSpacingMultiplier {
                mechanism_id: PatternMechanismId(41),
                value: 1.5,
            },
            PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(41),
                repetition: CurveRepetition::NormalOffset {
                    spacing: 7.0,
                    sides: OffsetSides::Both,
                    cleanup: OffsetCleanup::DissolveCrossings,
                },
            },
            PatternDefinitionEdit::SetParametricOffsetSpacing {
                mechanism_id: PatternMechanismId(41),
                spacing: 8.0,
            },
            PatternDefinitionEdit::SetParametricOffsetSides {
                mechanism_id: PatternMechanismId(41),
                sides: OffsetSides::Left,
            },
            PatternDefinitionEdit::SetParametricOffsetCleanup {
                mechanism_id: PatternMechanismId(41),
                cleanup: OffsetCleanup::DissolveCrossings,
            },
            PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(41),
                repetition: CurveRepetition::Single,
            },
            PatternDefinitionEdit::SetAlongParametricInterval {
                mechanism_id: PatternMechanismId(42),
                interval: 7.0,
            },
            PatternDefinitionEdit::SetAlongParametricPhase {
                mechanism_id: PatternMechanismId(42),
                phase: 0.25,
            },
        ];
        for edit in edits {
            let active_payload_fields: &[PropertyFieldId] = match &edit {
                PatternDefinitionEdit::SetParametricRepetition {
                    repetition: CurveRepetition::TransformStack { .. },
                    ..
                } => &[
                    PropertyFieldId::ParametricStackDirection,
                    PropertyFieldId::ParametricStackSpacingMultiplier,
                ],
                PatternDefinitionEdit::SetParametricRepetition {
                    repetition: CurveRepetition::NormalOffset { .. },
                    ..
                } => &[
                    PropertyFieldId::ParametricOffsetSpacing,
                    PropertyFieldId::ParametricOffsetSides,
                    PropertyFieldId::ParametricOffsetCleanup,
                ],
                _ => &[],
            };
            let base = history
                .document()
                .definition(PatternDefinitionId(40))
                .expect("shared definition remains current")
                .clone();
            if matches!(
                edit,
                PatternDefinitionEdit::SetParametricOffsetCleanup { .. }
            ) {
                assert!(
                    history
                        .apply(&DocumentCommand::EditSharedPatternDefinition {
                            definition_id: PatternDefinitionId(40),
                            base_definition: base,
                            edit,
                        })
                        .is_err()
                );
                continue;
            }
            let result = history
                .apply(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: PatternDefinitionId(40),
                    base_definition: base,
                    edit,
                })
                .expect("active parametric edit applies");
            assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
            assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
            assert!(active_payload_fields.iter().all(|field| {
                history
                    .document()
                    .property_values()
                    .iter()
                    .any(|value| value.descriptor.field == *field)
            }));
        }
        assert!(matches!(
            history.apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(40),
                base_definition: original.clone(),
                edit: PatternDefinitionEdit::SetParametricTurns {
                    mechanism_id: PatternMechanismId(41),
                    turns: 3.0,
                },
            }),
            Err(DocumentSessionError::Validation(error)) if error.path() == "pattern_definitions.base"
        ));
        let current = history
            .document()
            .definition(PatternDefinitionId(40))
            .expect("edited definition")
            .clone();
        assert!(
            history
                .apply(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id: PatternDefinitionId(40),
                    base_definition: current.clone(),
                    edit: PatternDefinitionEdit::SetParametricTurns {
                        mechanism_id: PatternMechanismId(41),
                        turns: 1.5,
                    },
                })
                .is_err()
        );
        history.undo().expect("undo succeeds").expect("one inverse");
        history.redo().expect("redo succeeds").expect("one redo");

        let selected_base = history
            .document()
            .definition(PatternDefinitionId(40))
            .expect("selected base")
            .clone();
        let selected = history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: selected_base,
                edit: PatternDefinitionEdit::SetParametricTurns {
                    mechanism_id: PatternMechanismId(41),
                    turns: 2.0,
                },
            })
            .expect("selected copy-on-edit applies");
        assert_eq!(selected.affected_channels, vec![ChannelId(1)]);
        assert_ne!(
            history.document().pattern_definition_id_for(ChannelId(1)),
            history.document().pattern_definition_id_for(ChannelId(2))
        );
        history
            .apply(&DocumentCommand::DuplicatePatternDefinition {
                definition_id: PatternDefinitionId(40),
            })
            .expect("parametric duplicate applies");
        let shared = history
            .document()
            .definition(PatternDefinitionId(40))
            .expect("shared parametric definition");
        let duplicate = history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|definition| {
                definition.id != PatternDefinitionId(40)
                    && definition.id
                        != history
                            .document()
                            .pattern_definition_id_for(ChannelId(1))
                            .expect("selected clone")
            })
            .expect("unreferenced parametric duplicate");
        assert!(matches!(
            duplicate.family,
            PatternFamily::ParametricCurve { .. }
        ));
        let (shared_curve, shared_repetition, shared_interval, shared_phase) = match (
            shared.mechanisms.as_slice(),
            duplicate.mechanisms.as_slice(),
        ) {
            (
                [
                    PatternMechanism::ParametricCurveSource {
                        curve, repetition, ..
                    },
                    PatternMechanism::AlongParametricCurveSites {
                        interval, phase, ..
                    },
                ],
                [
                    PatternMechanism::ParametricCurveSource {
                        curve: duplicate_curve,
                        repetition: duplicate_repetition,
                        ..
                    },
                    PatternMechanism::AlongParametricCurveSites {
                        interval: duplicate_interval,
                        phase: duplicate_phase,
                        ..
                    },
                ],
            ) => {
                assert_eq!(curve, duplicate_curve);
                assert_eq!(repetition, duplicate_repetition);
                assert_eq!(interval.to_bits(), duplicate_interval.to_bits());
                assert_eq!(phase.to_bits(), duplicate_phase.to_bits());
                (curve, repetition, interval, phase)
            }
            _ => panic!("duplicate preserves parametric source and site chain"),
        };
        assert!(matches!(
            (shared.output_layers.as_slice(), duplicate.output_layers.as_slice()),
            (
                [PatternOutputLayer::MarkPrototype { prototype, orientation: MarkOrientation::Fixed, .. }],
                [PatternOutputLayer::MarkPrototype { prototype: duplicate_prototype, orientation: MarkOrientation::Fixed, .. }]
            ) if prototype == duplicate_prototype
        ));
        let _ = (
            shared_curve,
            shared_repetition,
            shared_interval,
            shared_phase,
        );

        let mut draft = DocumentHistory::new_draft(&history);
        let definition_id = draft
            .document()
            .pattern_definition_id_for(ChannelId(1))
            .expect("selected parametric override");
        let base = draft
            .document()
            .definition(definition_id)
            .expect("draft selected definition")
            .clone();
        let site_mechanism_id = base
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::AlongParametricCurveSites { id, .. } => Some(*id),
                _ => None,
            })
            .expect("selected definition retains parametric site mechanism");
        draft
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: base,
                edit: PatternDefinitionEdit::SetAlongParametricInterval {
                    mechanism_id: site_mechanism_id,
                    interval: 8.0,
                },
            })
            .expect("draft edit applies");
        let squash = history.squash_draft(&draft).expect("draft squash applies");
        assert!(!squash.unchanged);
        assert_eq!(squash.affected_channels, vec![ChannelId(1)]);
        assert_eq!(squash.invalidation, Some(InvalidationLevel::Family));
    }

    /// Proves invalid parametric numeric and inactive-payload edits fail before history or document mutation.
    #[test]
    fn parametric_curve_validation_rejects_invalid_values_atomically() {
        let mut history = parametric_history();
        let before = history.document().clone();
        let invalid_edits = vec![
            PatternDefinitionEdit::SetParametricTurns {
                mechanism_id: PatternMechanismId(41),
                turns: 0.0,
            },
            PatternDefinitionEdit::SetParametricTurns {
                mechanism_id: PatternMechanismId(41),
                turns: 1025.0,
            },
            PatternDefinitionEdit::SetParametricTurns {
                mechanism_id: PatternMechanismId(41),
                turns: f64::NAN,
            },
            PatternDefinitionEdit::SetParametricRadialSpacing {
                mechanism_id: PatternMechanismId(41),
                radial_spacing: 0.0,
            },
            PatternDefinitionEdit::SetParametricPhase {
                mechanism_id: PatternMechanismId(41),
                phase_degrees: f64::NAN,
            },
            PatternDefinitionEdit::SetAlongParametricInterval {
                mechanism_id: PatternMechanismId(42),
                interval: 0.0,
            },
            PatternDefinitionEdit::SetAlongParametricPhase {
                mechanism_id: PatternMechanismId(42),
                phase: f64::NAN,
            },
            PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(41),
                repetition: CurveRepetition::TransformStack {
                    direction_degrees: f64::NAN,
                    spacing_multiplier: 1.0,
                },
            },
            PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(41),
                repetition: CurveRepetition::NormalOffset {
                    spacing: 0.0,
                    sides: OffsetSides::Both,
                    cleanup: OffsetCleanup::DissolveCrossings,
                },
            },
            PatternDefinitionEdit::SetParametricStackDirection {
                mechanism_id: PatternMechanismId(41),
                value: 10.0,
            },
        ];
        for edit in invalid_edits {
            let base = history
                .document()
                .definition(PatternDefinitionId(40))
                .expect("unmodified base")
                .clone();
            assert!(
                history
                    .apply(&DocumentCommand::EditSharedPatternDefinition {
                        definition_id: PatternDefinitionId(40),
                        base_definition: base,
                        edit,
                    })
                    .is_err()
            );
            assert_eq!(history.document(), &before);
            assert!(!history.can_undo());
            assert!(!history.can_redo());
        }
    }

    #[test]
    fn history_revision_exhaustion_keeps_document_and_stacks_atomic() {
        let mut history = history();
        history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: false,
            })
            .unwrap();
        let before_document = history.document().clone();
        history.session.revision = Revision(u64::MAX);

        assert_eq!(history.undo(), Err(DocumentSessionError::RevisionExhausted));
        assert_eq!(history.document(), &before_document);
        assert_eq!(history.revision(), Revision(u64::MAX));
        assert!(history.can_undo());
        assert!(!history.can_redo());

        assert_eq!(
            history.apply(&DocumentCommand::SetVisibility {
                channel_id: ChannelId(1),
                visible: true,
            }),
            Err(DocumentSessionError::RevisionExhausted)
        );
        assert_eq!(history.document(), &before_document);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    /// Proves squash validates an invalid final private snapshot before advancing main history.
    #[test]
    fn squash_draft_rejects_an_invalid_final_snapshot_without_main_mutation() {
        let mut main = history();
        let mut draft = DocumentHistory::new_draft(&main);
        draft.session.document.canvas.width = f64::NAN;
        let before = main.document().clone();
        assert!(main.squash_draft(&draft).is_err());
        assert_eq!(main.document(), &before);
        assert!(!main.can_undo());
    }

    /// Proves connection descriptors expose exact current values and only the active program payloads.
    #[test]
    fn connection_program_descriptors_are_conditional_and_current() {
        for kind in [
            ConnectionProgramKind::NearestLinks,
            ConnectionProgramKind::RandomLinks,
            ConnectionProgramKind::GridSpanningTree,
        ] {
            let document = connection_history(connection_program(kind))
                .document()
                .clone();
            let values = document.property_values();
            let current = |field| {
                values
                    .iter()
                    .find(|value| value.descriptor.field == field)
                    .expect("active connection descriptor")
                    .value
                    .clone()
            };
            assert_eq!(
                current(PropertyFieldId::ConnectionProgram),
                PropertyCurrentValueKind::EnumChoice(PropertyEnumChoice::ConnectionProgram(kind))
            );
            assert_eq!(
                current(PropertyFieldId::ConnectionMaximumDegree),
                PropertyCurrentValueKind::U32(3)
            );
            assert_eq!(
                current(PropertyFieldId::ConnectionMaximumDistance),
                PropertyCurrentValueKind::FiniteF64(12.0)
            );
            assert_eq!(
                values
                    .iter()
                    .any(|value| value.descriptor.field == PropertyFieldId::ConnectionMinimumDegree),
                kind == ConnectionProgramKind::RandomLinks
            );
            assert_eq!(
                values
                    .iter()
                    .any(|value| value.descriptor.field == PropertyFieldId::ConnectionSeed),
                kind != ConnectionProgramKind::NearestLinks
            );
        }
    }

    /// Proves the public capability projection exposes only each active connection or maze intent.
    #[test]
    fn pattern_capabilities_project_connection_program_and_maze_wall_controls() {
        for (kind, expected_minimum, expected_seed) in [
            (ConnectionProgramKind::NearestLinks, None, None),
            (ConnectionProgramKind::RandomLinks, Some(1), Some(7)),
            (ConnectionProgramKind::GridSpanningTree, None, Some(9)),
        ] {
            let document = connection_history(connection_program(kind))
                .document()
                .clone();
            let projection = document
                .pattern_capabilities(PatternCapabilityScope::DocumentBase)
                .expect("validated connection definition projects through the public authority");
            let [
                PatternOutputCapabilityRecord {
                    structural: PatternOutputCapabilityProjection::ConnectionPaths(connection),
                    ..
                },
            ] = projection.outputs.as_slice()
            else {
                panic!("connection definition projects exactly one connection output")
            };
            assert_eq!(connection.program_kind, kind);
            assert_eq!(connection.maximum_degree, 3);
            assert_eq!(connection.maximum_distance, 12.0);
            assert_eq!(connection.minimum_degree, expected_minimum);
            assert_eq!(connection.seed, expected_seed);
            assert!(connection.round_join);
            assert!(connection.round_cap);
            assert!(connection.thickness_range);
        }

        let mut maze_document =
            connection_history(connection_program(ConnectionProgramKind::NearestLinks))
                .document()
                .clone();
        maze_document
            .pattern_definition_bundles
            .iter_mut()
            .find(|definition| definition.id == PatternDefinitionId(60))
            .expect("connection fixture definition exists")
            .output_layers = vec![PatternOutputLayer::MazeWalls {
            id: PatternOutputLayerId(63),
            site_mechanism_id: PatternMechanismId(62),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed: 17,
            },
            style: PathStrokeStyle::default(),
        }];
        let projection = maze_document
            .pattern_capabilities(PatternCapabilityScope::DocumentBase)
            .expect("validated maze definition projects through the public authority");
        let [
            PatternOutputCapabilityRecord {
                structural: PatternOutputCapabilityProjection::MazeWalls(maze),
                ..
            },
        ] = projection.outputs.as_slice()
        else {
            panic!("maze definition projects exactly one maze-wall output")
        };
        assert!(maze.seeded_recursive_backtracker);
        assert_eq!(maze.seed, 17);
        assert!(maze.round_join);
        assert!(maze.round_cap);
        assert!(maze.thickness_range);
    }

    /// Proves typed program replacement and scalar edits retain output identity, family invalidation, and history truth.
    #[test]
    fn connection_program_edits_are_reversible_and_family_invalidating() {
        let mut history =
            connection_history(connection_program(ConnectionProgramKind::NearestLinks));
        let base = history
            .document()
            .definition(PatternDefinitionId(60))
            .expect("connection definition")
            .clone();
        let replacement = connection_program(ConnectionProgramKind::RandomLinks);
        let result = history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(60),
                base_definition: base,
                edit: PatternDefinitionEdit::SetConnectionProgram {
                    output_layer_id: PatternOutputLayerId(63),
                    program: replacement.clone(),
                },
            })
            .expect("typed replacement applies");
        assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
        assert_eq!(result.affected_channels, vec![ChannelId(1), ChannelId(2)]);
        let after_replacement = history.document().clone();
        history.undo().expect("replacement undo applies");
        assert!(matches!(
            history
                .document()
                .definition(PatternDefinitionId(60))
                .expect("definition")
                .output_layers
                .as_slice(),
            [PatternOutputLayer::ConnectionPaths {
                program: ConnectionProgram::NearestLinks { .. },
                ..
            }]
        ));
        history.redo().expect("replacement redo applies");
        assert_eq!(history.document(), &after_replacement);

        let base = history
            .document()
            .definition(PatternDefinitionId(60))
            .expect("replaced definition")
            .clone();
        history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(60),
                base_definition: base,
                edit: PatternDefinitionEdit::SetConnectionMinimumDegree {
                    output_layer_id: PatternOutputLayerId(63),
                    minimum_degree: 2,
                },
            })
            .expect("random scalar edit applies");
        assert!(matches!(
            history.document().definition(PatternDefinitionId(60)).expect("definition").output_layers.as_slice(),
            [PatternOutputLayer::ConnectionPaths { program: ConnectionProgram::RandomLinks { adjacency, minimum_degree: 2, seed: 7 }, .. }]
                if adjacency.maximum_degree == 3 && adjacency.maximum_distance == 12.0
        ));
    }

    /// Proves invalid bounds and inactive connection variants leave document and history state untouched.
    #[test]
    fn connection_program_edits_reject_invalid_or_inactive_payloads_atomically() {
        let mut history =
            connection_history(connection_program(ConnectionProgramKind::NearestLinks));
        let before = history.document().clone();
        for edit in [
            PatternDefinitionEdit::SetConnectionMaximumDegree {
                output_layer_id: PatternOutputLayerId(63),
                maximum_degree: 0,
            },
            PatternDefinitionEdit::SetConnectionMaximumDistance {
                output_layer_id: PatternOutputLayerId(63),
                maximum_distance: 0.0,
            },
            PatternDefinitionEdit::SetConnectionMinimumDegree {
                output_layer_id: PatternOutputLayerId(63),
                minimum_degree: 1,
            },
            PatternDefinitionEdit::SetConnectionSeed {
                output_layer_id: PatternOutputLayerId(63),
                seed: 1,
            },
        ] {
            let base = history
                .document()
                .definition(PatternDefinitionId(60))
                .expect("unchanged definition")
                .clone();
            assert!(
                history
                    .apply(&DocumentCommand::EditSharedPatternDefinition {
                        definition_id: PatternDefinitionId(60),
                        base_definition: base,
                        edit,
                    })
                    .is_err()
            );
            assert_eq!(history.document(), &before);
            assert!(!history.can_undo());
        }
    }

    /// Proves selected connection edits clone and retarget only one channel while shared edits retain both links.
    #[test]
    fn connection_program_selected_copy_and_shared_edit_preserve_authority() {
        let mut history =
            connection_history(connection_program(ConnectionProgramKind::RandomLinks));
        let original = history
            .document()
            .definition(PatternDefinitionId(60))
            .expect("shared connection definition")
            .clone();
        let selected = history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: original,
                edit: PatternDefinitionEdit::SetConnectionSeed {
                    output_layer_id: PatternOutputLayerId(63),
                    seed: 99,
                },
            })
            .expect("selected copy applies");
        assert_eq!(selected.invalidation, Some(InvalidationLevel::Family));
        let copied_id = history
            .document()
            .pattern_definition_id_for(ChannelId(1))
            .expect("selected definition");
        assert_ne!(copied_id, PatternDefinitionId(60));
        assert_eq!(
            history.document().pattern_definition_id_for(ChannelId(2)),
            Some(PatternDefinitionId(60))
        );
        let shared_base = history
            .document()
            .definition(PatternDefinitionId(60))
            .expect("shared definition remains")
            .clone();
        let shared = history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(60),
                base_definition: shared_base,
                edit: PatternDefinitionEdit::SetConnectionMaximumDegree {
                    output_layer_id: PatternOutputLayerId(63),
                    maximum_degree: 4,
                },
            })
            .expect("shared edit applies");
        assert_eq!(shared.invalidation, Some(InvalidationLevel::Family));
        assert_eq!(shared.affected_channels, vec![ChannelId(2)]);
    }

    /// Keeps maze-wall seed edits typed, reversible, family-invalidating, and copy-on-edit safe.
    #[test]
    fn maze_seed_edit_is_targeted_reversible_and_copy_on_edit_safe() {
        let mut history =
            connection_history(connection_program(ConnectionProgramKind::NearestLinks));
        history
            .session
            .document
            .pattern_definition_bundles
            .iter_mut()
            .find(|definition| definition.id == PatternDefinitionId(60))
            .expect("connection definition exists")
            .output_layers = vec![PatternOutputLayer::MazeWalls {
            id: PatternOutputLayerId(63),
            site_mechanism_id: PatternMechanismId(62),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed: 7,
            },
            style: PathStrokeStyle::default(),
        }];
        let base = history
            .document()
            .definition(PatternDefinitionId(60))
            .expect("maze definition")
            .clone();
        let result = history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: ChannelId(1),
                base_definition: base,
                edit: PatternDefinitionEdit::SetMazeSeed {
                    output_layer_id: PatternOutputLayerId(63),
                    seed: 99,
                },
            })
            .expect("selected maze seed applies");
        assert_eq!(result.invalidation, Some(InvalidationLevel::Family));
        assert_eq!(result.affected_channels, vec![ChannelId(1)]);
        let edited = history.document().clone();
        assert!(matches!(
            history
                .document()
                .pattern_definition_for(ChannelId(1))
                .expect("selected copied maze definition")
                .output_layers
                .as_slice(),
            [PatternOutputLayer::MazeWalls {
                program: MazeProgram { seed: 99, .. },
                ..
            }]
        ));
        history.undo().expect("maze seed undo applies");
        history.redo().expect("maze seed redo applies");
        assert_eq!(history.document(), &edited);
    }

    /// Materializes one connection preset wrapper atomically while retaining the fresh output/site IDs.
    #[test]
    fn connection_recipe_wrapper_materializes_only_eligible_families() {
        let mut history = connection_history(ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 12.0,
            },
        });
        let program = ConnectionProgram::GridSpanningTree {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 12.0,
            },
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 7,
        };
        let recipe = PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
            definition: Box::new(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "maze grid".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                },
            )),
            program: program.clone(),
            style: PathStrokeStyle::default(),
        });
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .definition(base.definition_id)
            .expect("base definition")
            .clone();
        history
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe,
            })
            .expect("eligible connection recipe materializes");
        let definition = history
            .document()
            .definition(history.document().pattern_settings().definition_id)
            .expect("materialized definition");
        let [
            PatternOutputLayer::ConnectionPaths {
                id,
                site_mechanism_id,
                program: materialized_program,
                style,
            },
        ] = definition.output_layers.as_slice()
        else {
            panic!("connection recipe must replace its one mark output")
        };
        assert_eq!(materialized_program, &program);
        assert_eq!(*style, PathStrokeStyle::default());
        assert!(
            definition
                .mechanisms
                .iter()
                .any(|mechanism| mechanism.id() == *site_mechanism_id)
        );
        assert_eq!(*id, PatternOutputLayerId(64));

        let before_invalid = history.document().clone();
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .definition(base.definition_id)
            .expect("materialized base definition")
            .clone();
        let invalid = PatternDefinitionRecipe::connected(PatternStructureRecipe::ConnectionPaths {
            definition: Box::new(PatternStructureRecipe::RandomSites {
                name: "invalid maze dispersion".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
                character: RandomSiteCharacter::RawUniform,
                seed: 0,
                density_modulation: SiteDensityModulation::Uniform,
                exclusion: SiteExclusionPolicy::None,
                maximum_attempts: 1,
                maximum_neighbor_checks: 1,
            }),
            program,
            style: PathStrokeStyle::default(),
        });
        assert!(
            history
                .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                    base,
                    base_definition,
                    recipe: invalid,
                })
                .is_err()
        );
        assert_eq!(history.document(), &before_invalid);
    }

    /// Fixes the persisted connection-program numeric contract before patterns can allocate topology.
    #[test]
    fn connection_program_validation_bounds_are_explicit() {
        assert!(
            ConnectionProgram::RandomLinks {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 2,
                    maximum_distance: 10.0
                },
                minimum_degree: 2,
                seed: 7,
            }
            .validate()
            .is_ok()
        );
        assert_eq!(
            ConnectionProgram::NearestLinks {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 0,
                    maximum_distance: 1.0
                },
            }
            .validate()
            .expect_err("zero degree rejects")
            .path(),
            "connection.adjacency.maximum_degree"
        );
        assert_eq!(
            ConnectionProgram::RandomLinks {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 1,
                    maximum_distance: 1.0
                },
                minimum_degree: 2,
                seed: 0,
            }
            .validate()
            .expect_err("minimum over maximum rejects")
            .path(),
            "connection.minimum_degree"
        );
    }

    /// Proves keyed delta insertion and replacement follow structural output order without lifting the public gate.
    #[test]
    fn keyed_delta_helpers_order_insertions_and_prune_only_incompatible_entries() {
        let mut definition = PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "two output helper",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(10),
            CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        );
        definition
            .output_layers
            .push(PatternOutputLayer::CircularMarks {
                id: PatternOutputLayerId(11),
                site_mechanism_id: PatternMechanismId(2),
            });
        let bundle = PatternDefinitionBundle {
            definition,
            output_settings: vec![
                PatternOutputSettings {
                    output_layer_id: PatternOutputLayerId(10),
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                },
                PatternOutputSettings {
                    output_layer_id: PatternOutputLayerId(11),
                    response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                        minimum_fill: 0.0,
                        maximum_fill: 1.0,
                    }),
                },
            ],
        };
        let delta = |output_layer_id, minimum_fill_delta| PatternOutputResponseDelta {
            output_layer_id,
            delta: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                minimum_fill_delta: Some(minimum_fill_delta),
                maximum_fill_delta: None,
            }),
        };
        let mut deltas = Vec::new();
        upsert_pattern_output_delta_in_order(
            &bundle,
            &mut deltas,
            delta(PatternOutputLayerId(11), 0.2),
        )
        .expect("later output inserts");
        upsert_pattern_output_delta_in_order(
            &bundle,
            &mut deltas,
            delta(PatternOutputLayerId(10), 0.1),
        )
        .expect("earlier output inserts before later output");
        assert_eq!(
            deltas
                .iter()
                .map(|entry| entry.output_layer_id)
                .collect::<Vec<_>>(),
            vec![PatternOutputLayerId(10), PatternOutputLayerId(11)]
        );
        upsert_pattern_output_delta_in_order(
            &bundle,
            &mut deltas,
            delta(PatternOutputLayerId(10), 0.3),
        )
        .expect("existing output replaces in place");
        assert_eq!(
            deltas[0].delta,
            ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                minimum_fill_delta: Some(0.3),
                maximum_fill_delta: None,
            })
        );
        assert_eq!(
            upsert_pattern_output_delta_in_order(
                &bundle,
                &mut deltas,
                delta(PatternOutputLayerId(12), 0.4),
            )
            .expect_err("foreign output rejects")
            .path(),
            "channel.pattern.output_layer_id"
        );
        assert_eq!(
            upsert_pattern_output_delta_in_order(
                &bundle,
                &mut deltas,
                PatternOutputResponseDelta {
                    output_layer_id: PatternOutputLayerId(10),
                    delta: ChannelGeometryResponseDelta::Connected(
                        ConnectedGeometryResponseDelta {
                            minimum_thickness_delta: Some(0.1),
                            maximum_thickness_delta: None,
                        },
                    ),
                },
            )
            .expect_err("kind mismatch rejects")
            .path(),
            "channel.pattern.output_deltas.kind"
        );
        let retained = prune_output_response_deltas_for_bundle(
            &bundle,
            &[
                delta(PatternOutputLayerId(11), 0.2),
                PatternOutputResponseDelta {
                    output_layer_id: PatternOutputLayerId(12),
                    delta: ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                        minimum_fill_delta: Some(0.4),
                        maximum_fill_delta: None,
                    }),
                },
            ],
        );
        assert_eq!(retained, vec![delta(PatternOutputLayerId(11), 0.2)]);
    }
}
