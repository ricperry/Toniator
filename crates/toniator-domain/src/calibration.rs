//! Validated, source-independent coverage measurements carried by current documents.

use crate::{
    ChannelId, ConnectionProgram, CurveRepetition, Document, MarkOrientationDraft,
    ParametricCurve, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternOutputRealizationRecipe, PatternRecipeFamilyKind, PatternStructureRecipe,
    RegionSamplingStrategy, SiteUseFilterRecipe, SpiralCoveragePolicy, SpiralShape, ValidationError,
};
#[path = "default_calibration.rs"]
mod default_calibration;

/// Fixed square canvas side used to materialize and measure one structural reference.
pub const CALIBRATION_REFERENCE_CANVAS_SIDE: f64 = 1024.0;
/// Bounds retained raw knots while admitting measured steep saturation under the strict criterion.
pub const MAX_SAVED_CALIBRATION_KNOTS: usize = 144;
/// Current geometry and 32-spacing reference-window contract for a saved measurement.
pub const CALIBRATION_PROFILE: &str = "ton018-reference-fixed-canvas1024-spacings32-v4";
/// Current geometry and 64-spacing reference-window contract after independent population retry.
pub const CALIBRATION_PROFILE_64: &str = "ton018-reference-fixed-canvas1024-spacings64-v4";
/// Guide Faces producer v2 on the strict fixed 1024-square, 32-spacing reference.
pub const CALIBRATION_PROFILE_GUIDE_FACES: &str =
    "ton018-reference-fixed-canvas1024-spacings32-guide-faces-v5";
/// Guide Faces producer v2 after the independent 64/128-spacing strict retry.
pub const CALIBRATION_PROFILE_GUIDE_FACES_64: &str =
    "ton018-reference-fixed-canvas1024-spacings64-guide-faces-v5";
/// Bounded empirical mean of two frozen random populations, checked against a third.
pub const CALIBRATION_PROFILE_MEAN: &str =
    "ton018-reference-fixed-canvas1024-spacings32-64-mean-seed-neutral-v5";
/// Seed-neutral 32-spacing reference for recipes with authored variation seeds.
pub const CALIBRATION_PROFILE_SEEDED: &str =
    "ton018-reference-fixed-canvas1024-spacings32-seed-neutral-v5";
/// Seed-neutral 64-spacing retry for recipes with authored variation seeds.
pub const CALIBRATION_PROFILE_SEEDED_64: &str =
    "ton018-reference-fixed-canvas1024-spacings64-seed-neutral-v5";
/// Covered-interior 768-square 64/128 reference for one finite isotropic Round Spiral.
pub const CALIBRATION_PROFILE_ROUND_INTERIOR: &str =
    "ton018-reference-covered-interior-canvas768-spacings64-v4";
/// One fixed statistical reference realization; authored rendering seeds remain untouched.
pub const CALIBRATION_REFERENCE_SEED: u32 = 43;

/// One measured input and occupied fraction, in increasing input order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibrationKnot {
    pub input: f64,
    pub coverage: f64,
}

/// Typed structural verification metadata; it never materializes a Pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationBinding {
    recipe: PatternDefinitionRecipe,
    density_aspect: f64,
    shape_rotation_degrees: f64,
}

impl CalibrationBinding {
    /// Adds only current typed geometry-producer revisions to one recognized reference profile.
    ///
    /// Source, presentation, scalar Density, Feature size, and authored response remain outside
    /// measured identity. Numeric callers still validate the selected reference-window contract.
    pub fn profile_for_reference(&self, reference_profile: &str) -> String {
        let suffix = self.geometry_profile_suffix();
        format!("{reference_profile}{suffix}")
    }

    /// Reports persistent canvas-covering intent through structural wrappers without deriving geometry.
    /// This participates in producer provenance and exact donor eligibility, never labels or IDs.
    pub fn requires_canvas_coverage(&self) -> bool {
        /// Reads the single structural source through every accepted recipe wrapper.
        fn covers(structure: &PatternStructureRecipe) -> bool {
            match structure {
                PatternStructureRecipe::ParametricCurve {
                    spiral_coverage, ..
                } => *spiral_coverage == SpiralCoveragePolicy::CoverCanvas,
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::ConnectionPaths { definition, .. }
                | PatternStructureRecipe::MazeWalls { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. }
                | PatternStructureRecipe::OrderedOutputs { definition, .. } => covers(definition),
                _ => false,
            }
        }
        covers(&self.recipe.structure)
    }

    /// Identifies actual Maze drawing outputs through typed wrappers, independent of product labels.
    pub fn uses_maze_walls(&self) -> bool {
        /// Visits accepted structural/output wrappers without deriving geometry or changing recipe intent.
        fn maze(structure: &PatternStructureRecipe) -> bool {
            match structure {
                PatternStructureRecipe::MazeWalls { .. } => true,
                PatternStructureRecipe::OrderedOutputs {
                    definition,
                    outputs,
                } => {
                    maze(definition)
                        || outputs.iter().any(|output| {
                            matches!(output, PatternOutputRealizationRecipe::MazeWalls { .. })
                        })
                }
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::ConnectionPaths { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. } => maze(definition),
                _ => false,
            }
        }
        maze(&self.recipe.structure)
    }

    /// Identifies authored open guide cores whose derived exterior uses the current ray coverage producer.
    /// Fixed parametric sources and authored marks/motifs do not acquire this revision.
    pub fn uses_authored_guide_coverage(&self) -> bool {
        /// Reads only structural guide prototypes through accepted recipe wrappers.
        fn authored(structure: &PatternStructureRecipe) -> bool {
            match structure {
                PatternStructureRecipe::GenericGuides { dimensions, .. } => {
                    dimensions.iter().any(|dimension| {
                        matches!(
                            dimension.prototype,
                            crate::GenericGuidePrototypeDraft::AuthoredOpenPathReference { .. }
                        )
                    })
                }
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::ConnectionPaths { definition, .. }
                | PatternStructureRecipe::MazeWalls { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. }
                | PatternStructureRecipe::OrderedOutputs { definition, .. } => authored(definition),
                _ => false,
            }
        }
        authored(&self.recipe.structure)
    }

    /// Identifies only ordinary Voronoi outputs backed by a complete typed straight intersection family.
    /// Curved, finite parametric, Along Guides and stochastic populations keep their own contracts.
    pub fn uses_straight_intersection_voronoi(&self) -> bool {
        /// Reads the actual structural source without inferring topology from output names.
        fn straight(structure: &PatternStructureRecipe) -> bool {
            match structure {
                PatternStructureRecipe::StraightGrid(_) => true,
                PatternStructureRecipe::GeneralizedStraightGuides {
                    product: crate::GeneralizedSiteProductDraft::Intersections { .. },
                    ..
                } => true,
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::ConnectionPaths { definition, .. }
                | PatternStructureRecipe::MazeWalls { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. }
                | PatternStructureRecipe::OrderedOutputs { definition, .. } => straight(definition),
                _ => false,
            }
        }
        /// Reads active drawing output authority through resource/output wrappers.
        fn voronoi(structure: &PatternStructureRecipe) -> bool {
            match structure {
                PatternStructureRecipe::VoronoiRegions { .. } => true,
                PatternStructureRecipe::OrderedOutputs { outputs, .. } => outputs
                    .iter()
                    .any(|output| matches!(output, PatternOutputRealizationRecipe::VoronoiRegions)),
                PatternStructureRecipe::AuthoredResources { definition, .. } => voronoi(definition),
                _ => false,
            }
        }
        straight(&self.recipe.structure) && voronoi(&self.recipe.structure)
    }

    /// Recognizes former numeric companions affected by a corrected typed geometry producer.
    ///
    /// Only recognized reference tiers and exact prior guide/inset suffixes qualify;
    /// malformed bindings, partial artistic markers and unknown future strings never authorize removal.
    pub fn is_obsolete_geometry_profile(&self, profile: &str) -> bool {
        let current = self.geometry_profile_suffix();
        if current.is_empty() {
            return false;
        }
        let former_suffix = self.base_geometry_profile_suffix();
        let previous_suffix = self.geometry_profile_suffix_before_coverage();
        let recognized = [
            CALIBRATION_PROFILE,
            CALIBRATION_PROFILE_64,
            CALIBRATION_PROFILE_GUIDE_FACES,
            CALIBRATION_PROFILE_GUIDE_FACES_64,
            CALIBRATION_PROFILE_SEEDED,
            CALIBRATION_PROFILE_SEEDED_64,
            CALIBRATION_PROFILE_ROUND_INTERIOR,
            CALIBRATION_PROFILE_MEAN,
        ];
        recognized.into_iter().any(|base| {
            let tier = self.supports_strict_reference_profile(base)
                || (base == CALIBRATION_PROFILE_MEAN
                    && self.has_variation_seed()
                    && self.supports_stochastic_mean());
            tier && (profile == base
                || (current != former_suffix && profile == format!("{base}{former_suffix}"))
                || (current != previous_suffix && profile == format!("{base}{previous_suffix}")))
        })
    }

    /// Classifies strict reference tiers by their existing typed producer and window authority.
    fn supports_strict_reference_profile(&self, profile: &str) -> bool {
        if self.uses_guide_faces() {
            [
                CALIBRATION_PROFILE_GUIDE_FACES,
                CALIBRATION_PROFILE_GUIDE_FACES_64,
            ]
            .contains(&profile)
        } else if self.has_variation_seed() {
            [CALIBRATION_PROFILE_SEEDED, CALIBRATION_PROFILE_SEEDED_64].contains(&profile)
        } else {
            [CALIBRATION_PROFILE, CALIBRATION_PROFILE_64].contains(&profile)
                || (profile == CALIBRATION_PROFILE_ROUND_INTERIOR && self.supports_round_interior())
        }
    }

    /// Derives current output-producer provenance from the normalized typed recipe.
    /// Existing guide/inset markers remain in order before the artistic producer revision.
    fn geometry_profile_suffix_before_coverage(&self) -> String {
        /// Reads actual drawing outputs without treating the structural family as a drawing mode.
        fn artistic(structure: &PatternStructureRecipe) -> (bool, bool) {
            match structure {
                PatternStructureRecipe::AuthoredResources { definition, .. } => artistic(definition),
                PatternStructureRecipe::ConnectionPaths { .. } => (false, true),
                PatternStructureRecipe::CurveMotifPaths { .. }
                | PatternStructureRecipe::ParametricCurve { sites: None, .. } => (true, false),
                PatternStructureRecipe::OrderedOutputs { outputs, .. } => (
                    outputs.iter().any(|output| {
                        matches!(
                            output,
                            PatternOutputRealizationRecipe::StructuralPaths { .. }
                                | PatternOutputRealizationRecipe::CurveMotifPaths { .. }
                        )
                    }),
                    outputs.iter().any(|output| {
                        matches!(
                            output,
                            PatternOutputRealizationRecipe::ConnectionPaths { .. }
                        )
                    }),
                ),
                _ => (false, false),
            }
        }
        let (tone, connections) = artistic(&self.recipe.structure);
        let mut suffix = self.base_geometry_profile_suffix().to_owned();
        if tone {
            suffix.push_str("+anchored-tone-v1");
        }
        if connections {
            suffix.push_str("+connection-selection-v2");
        }
        suffix
    }

    /// Adds all current typed coverage producers to the single existing artistic/base suffix authority.
    /// Exact prior suffix recognition excludes these coverage revisions together, including mixed outputs.
    fn geometry_profile_suffix(&self) -> String {
        let mut suffix = self.geometry_profile_suffix_before_coverage();
        if self.requires_canvas_coverage() {
            suffix.push_str("+cover-envelope-v1");
        }
        if self.uses_maze_walls() {
            suffix.push_str("+maze-guard-topology-v2");
        }
        if self.uses_guide_faces() {
            suffix.push_str("+cell-reach-v1");
        }
        if self.uses_authored_guide_coverage() {
            suffix.push_str("+guide-ray-coverage-v1");
        }
        if self.uses_straight_intersection_voronoi() {
            suffix.push_str("+voronoi-cell-reach-v1");
        }
        suffix
    }

    /// Derives previously current guide/inset provenance for exact source-free cache recognition.
    fn base_geometry_profile_suffix(&self) -> &'static str {
        let guide = self.uses_authored_guide_coverage();
        let inset = self.recipe.output_settings.iter().any(|setting| matches!(&setting.response,
            PatternGeometryResponse::Regions(response) if response.algorithm == crate::RegionResizeAlgorithm::UniformOffset));
        match (guide, inset) {
            (false, false) => "",
            (false, true) => "+convex-inset-v3",
            (true, false) => "+guide-span-v2",
            (true, true) => "+guide-span-v2+convex-inset-v3",
        }
    }

    /// Identifies the single active Guide Faces drawing output through the typed recipe.
    ///
    /// The normalized binding, not a Preset ID or stored label, determines which producer
    /// contract measured a curve. An optional resource-table wrapper does not change output kind.
    pub fn uses_guide_faces(&self) -> bool {
        let mut structure = &self.recipe.structure;
        while let PatternStructureRecipe::AuthoredResources { definition, .. } = structure {
            structure = definition;
        }
        match structure {
            PatternStructureRecipe::OrderedOutputs { outputs, .. } => outputs
                .iter()
                .any(|output| matches!(output, PatternOutputRealizationRecipe::GuideFaceRegions { .. })),
            PatternStructureRecipe::GuideFaceRegions { .. } => true,
            _ => false,
        }
    }

    /// Identifies a single finite Round Spiral whose final complete revolution encloses the
    /// fixed 768-square reference, without changing the authored curve or output semantics.
    pub fn supports_round_interior(&self) -> bool {
        const HALF_SIDE: f64 = 384.0;
        if self.density_aspect != 1.0 || self.recipe.output_settings.len() != 1 {
            return false;
        }
        let PatternStructureRecipe::OrderedOutputs { definition, outputs } = &self.recipe.structure else {
            return false;
        };
        let PatternStructureRecipe::ParametricCurve {
            curve: ParametricCurve::Spiral(spiral),
            spiral_coverage: SpiralCoveragePolicy::Fixed,
            repetition: CurveRepetition::Single,
            sites,
            ..
        } = definition.as_ref() else {
            return false;
        };
        let outer_revolution_radius = (spiral.turns - 1.0) * spiral.radial_spacing;
        if spiral.shape != SpiralShape::Round
            || !spiral.turns.is_finite()
            || spiral.turns <= 1.0
            || !spiral.radial_spacing.is_finite()
            || spiral.radial_spacing <= 0.0
            || !outer_revolution_radius.is_finite()
            || outer_revolution_radius < HALF_SIDE.hypot(HALF_SIDE)
        {
            return false;
        }
        matches!(self.recipe.output_settings[0].source_filter, SiteUseFilterRecipe::All)
            && matches!(
                (outputs.as_slice(), &self.recipe.output_settings[0].response, sites),
                ([PatternOutputRealizationRecipe::Marks], PatternGeometryResponse::Marks(_), Some(_))
                    | ([PatternOutputRealizationRecipe::StructuralPaths { .. }], PatternGeometryResponse::Connected(_), None)
            )
    }

    /// Reports whether this binding contains any site, connection, or maze variation seed.
    pub fn has_variation_seed(&self) -> bool {
        /// Finds seed-bearing geometry through every structural wrapper and ordered output.
        fn structure(value: &PatternStructureRecipe) -> bool {
            match value {
                PatternStructureRecipe::RandomSites { .. } => true,
                PatternStructureRecipe::ConnectionPaths { definition, program, .. } =>
                    program.seed().is_some() || structure(definition),
                PatternStructureRecipe::MazeWalls { .. } => true,
                PatternStructureRecipe::OrderedOutputs { definition, outputs } =>
                    structure(definition) || outputs.iter().any(|output| match output {
                        PatternOutputRealizationRecipe::ConnectionPaths { program, .. } =>
                            program.seed().is_some(),
                        PatternOutputRealizationRecipe::MazeWalls { .. } => true,
                        _ => false,
                    }),
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. } => structure(definition),
                PatternStructureRecipe::StraightGrid(_)
                | PatternStructureRecipe::GeneralizedStraightGuides { .. }
                | PatternStructureRecipe::GenericGuides { .. }
                | PatternStructureRecipe::ParametricCurve { .. } => false,
            }
        }
        structure(&self.recipe.structure)
    }

    /// Identifies eligible stochastic reference populations from typed geometry only.
    ///
    /// Pattern IDs, labels, and response mode do not grant eligibility; a random-site family,
    /// seeded connection program, or maze must occur in the active structure or output.
    pub fn supports_stochastic_mean(&self) -> bool {
        /// Finds a seeded connection or maze recursively within complete typed structure.
        fn structure(value: &PatternStructureRecipe) -> bool {
            match value {
                PatternStructureRecipe::ConnectionPaths {
                    definition,
                    program,
                    ..
                } => program.seed().is_some() || structure(definition),
                PatternStructureRecipe::MazeWalls { .. } => true,
                PatternStructureRecipe::OrderedOutputs {
                    definition,
                    outputs,
                } => {
                    structure(definition)
                        || outputs.iter().any(|output| match output {
                            PatternOutputRealizationRecipe::ConnectionPaths { program, .. } => {
                                program.seed().is_some()
                            }
                            PatternOutputRealizationRecipe::MazeWalls { .. } => true,
                            _ => false,
                        })
                }
                PatternStructureRecipe::AuthoredResources { definition, .. }
                | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
                | PatternStructureRecipe::CurveMotifPaths { definition, .. }
                | PatternStructureRecipe::VoronoiRegions { definition }
                | PatternStructureRecipe::GuideFaceRegions { definition, .. } => {
                    structure(definition)
                }
                PatternStructureRecipe::StraightGrid(_)
                | PatternStructureRecipe::GeneralizedStraightGuides { .. }
                | PatternStructureRecipe::GenericGuides { .. }
                | PatternStructureRecipe::RandomSites { .. }
                | PatternStructureRecipe::ParametricCurve { .. } => false,
            }
        }
        self.recipe.family_kind() == PatternRecipeFamilyKind::Dispersion
            || structure(&self.recipe.structure)
    }

    /// Captures the hidden shared Pattern and last-ALL aspect used by a fresh model topology.
    ///
    /// Named channel overrides, scalar density, and whole-Pattern placement cannot change this
    /// binding. Shape rotation comes from the document base because the ALL Appearance baseline
    /// stores only whole-Pattern rotation, which is excluded from calibration.
    ///
    /// # Errors
    /// Rejects a base definition that cannot reconstruct its complete typed recipe.
    pub fn for_document_baseline(document: &Document) -> Result<Self, ValidationError> {
        let recipe = document
            .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)?;
        let aspect = if recipe.family_kind() == PatternRecipeFamilyKind::Dispersion {
            1.0 // Scatter spacing ignores guide Density aspect for ALL and named channels alike.
        } else {
            document
                .appearance_baseline()
                .density_aspect
                .unwrap_or(document.pattern_settings().density.aspect)
        };
        Self::new(
            recipe,
            aspect,
            document.pattern_settings().shape_rotation_degrees,
        )
    }

    /// Captures current ID-free geometry intent while ignoring labels, artwork, scalar Feature
    /// size, whole-Pattern placement, and Advanced response bounds.
    ///
    /// # Errors
    /// Rejects a channel whose current definition cannot be reconstructed as a typed recipe.
    pub fn for_channel(
        document: &Document,
        channel_id: ChannelId,
    ) -> Result<Self, ValidationError> {
        let effective = document.effective_channel_pattern(channel_id)?;
        let mut recipe = document.reconstruct_pattern_definition_recipe(effective.definition_id)?;
        // Only connected bias is effective structural response; other response controls are free.
        for (setting, current) in recipe
            .output_settings
            .iter_mut()
            .zip(&effective.output_settings)
        {
            match &mut setting.response {
                PatternGeometryResponse::Marks(_) => {}
                PatternGeometryResponse::Connected(value) => {
                    if let PatternGeometryResponse::Connected(current) = &current.response {
                        value.bias = current.bias;
                    }
                }
                PatternGeometryResponse::Regions(_) => {}
            }
        }
        let density_aspect = if recipe.family_kind() == PatternRecipeFamilyKind::Dispersion {
            1.0 // Scatter's authored spacing ignores guide Density aspect.
        } else {
            effective.density.aspect
        };
        Self::new(recipe, density_aspect, effective.shape_rotation_degrees)
    }

    /// Returns the existing typed recipe solely as verification metadata.
    pub fn recipe(&self) -> &PatternDefinitionRecipe {
        &self.recipe
    }

    /// Returns the guide-aspect binding; scalar density remains free to change.
    pub fn density_aspect(&self) -> f64 {
        self.density_aspect
    }

    /// Compares reusable structural intent while retaining measured rotation as provenance.
    ///
    /// Ordinary aspect and connected bias remain structural. Individual Shape rotation changes
    /// appearance intentionally and never rewrites the saved numeric artifact or measured angle.
    pub fn is_compatible_with(&self, current: &Self) -> bool {
        self.recipe == current.recipe && self.density_aspect == current.density_aspect
    }

    /// Returns immutable measured shape rotation, independent of the current artistic angle.
    pub fn shape_rotation_degrees(&self) -> f64 {
        self.shape_rotation_degrees
    }

    /// Restores ID-free measured identity, replacing only variation seeds with fixed seed 43.
    ///
    /// The caller's authored recipe is cloned; rendering still consumes its original seeds.
    ///
    /// # Errors
    /// Rejects nonfinite orientation or invalid guide aspect.
    pub fn new(
        recipe: PatternDefinitionRecipe,
        density_aspect: f64,
        shape_rotation_degrees: f64,
    ) -> Result<Self, ValidationError> {
        if !density_aspect.is_finite()
            || density_aspect <= 0.0
            || !shape_rotation_degrees.is_finite()
        {
            return Err(ValidationError::new(
                "calibration.binding",
                "aspect and shape rotation must be finite, with positive aspect",
            ));
        }
        // Reconstructed definitions use ordered outputs, while public preset constructors
        // retain equivalent one-output shorthand wrappers. The existing recipe editor is the
        // sole typed authority for converting those two representations.
        let mut recipe = crate::assemble_recipe_outputs(crate::decompose_recipe_outputs(&recipe)?);
        normalize_names(&mut recipe.structure);
        normalize_inactive_family_orientation(&mut recipe.structure);
        normalize_variation_seeds(&mut recipe.structure);
        normalize_response_shells(&mut recipe);
        Ok(Self {
            recipe,
            density_aspect,
            shape_rotation_degrees,
        })
    }
}

/// Uniform full-tone footprint and stable survival inverse, independent of artwork.
#[derive(Clone, Debug, PartialEq)]
pub struct UniformCalibration {
    radius_per_nominal_spacing: f64,
    inverse: Vec<CalibrationKnot>,
}

/// Observed transfer differences for a bounded stochastic mean, not a per-render guarantee.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibrationVariation {
    raw_absolute: f64,
    raw_fraction_of_span: f64,
    normalized_tone_absolute: f64,
    low_tone_relative: f64,
    uniform_tone_absolute: Option<f64>,
    uniform_low_tone_relative: Option<f64>,
}

impl CalibrationVariation {
    /// Validates empirical 128-population transfer maxima against the approved bounds.
    ///
    /// # Errors
    /// Rejects nonfinite, negative, out-of-bound, or incomplete Uniform measurements.
    pub fn new(
        raw_absolute: f64,
        raw_fraction_of_span: f64,
        normalized_tone_absolute: f64,
        low_tone_relative: f64,
        uniform_tone_absolute: Option<f64>,
        uniform_low_tone_relative: Option<f64>,
    ) -> Result<Self, ValidationError> {
        let values = [
            raw_absolute,
            raw_fraction_of_span,
            normalized_tone_absolute,
            low_tone_relative,
        ];
        if values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || raw_fraction_of_span > 0.02
            || normalized_tone_absolute > 0.01
            || low_tone_relative > 0.1
            || uniform_tone_absolute.is_some() != uniform_low_tone_relative.is_some()
            || uniform_tone_absolute
                .is_some_and(|value| !value.is_finite() || !(0.0..=0.01).contains(&value))
            || uniform_low_tone_relative
                .is_some_and(|value| !value.is_finite() || !(0.0..=0.1).contains(&value))
        {
            return Err(ValidationError::new(
                "calibration.variation",
                "observed stochastic transfer exceeds its bounded contract",
            ));
        }
        Ok(Self {
            raw_absolute,
            raw_fraction_of_span,
            normalized_tone_absolute,
            low_tone_relative,
            uniform_tone_absolute,
            uniform_low_tone_relative,
        })
    }

    /// Returns the largest absolute raw occupied-fraction difference.
    pub fn raw_absolute(&self) -> f64 {
        self.raw_absolute
    }
    /// Returns the largest raw difference divided by the measured reachable span.
    pub fn raw_fraction_of_span(&self) -> f64 {
        self.raw_fraction_of_span
    }
    /// Returns the largest independent endpoint-normalized Variable tone difference.
    pub fn normalized_tone_absolute(&self) -> f64 {
        self.normalized_tone_absolute
    }
    /// Returns the largest low-tone difference relative to the requested tone.
    pub fn low_tone_relative(&self) -> f64 {
        self.low_tone_relative
    }
    /// Returns the optional largest endpoint-normalized Uniform transfer difference.
    pub fn uniform_tone_absolute(&self) -> Option<f64> {
        self.uniform_tone_absolute
    }
    /// Returns the optional largest low-tone Uniform relative transfer difference.
    pub fn uniform_low_tone_relative(&self) -> Option<f64> {
        self.uniform_low_tone_relative
    }
}

impl UniformCalibration {
    /// Validates a dimensionless radius and ordered probability-to-coverage observations.
    ///
    /// # Errors
    /// Rejects unsupported radius, missing endpoints, nonfinite, or nonmonotone knots.
    pub fn new(
        radius_per_nominal_spacing: f64,
        inverse: Vec<CalibrationKnot>,
    ) -> Result<Self, ValidationError> {
        if !radius_per_nominal_spacing.is_finite() || radius_per_nominal_spacing <= 0.0 {
            return Err(ValidationError::new(
                "calibration.uniform.radius",
                "radius must be finite and positive",
            ));
        }
        validate_knots(&inverse, 1.0, 512)?;
        if inverse.first().is_none_or(|first| first.coverage != 0.0)
            || inverse.last().is_none_or(|last| last.coverage < 0.999)
        {
            return Err(ValidationError::new(
                "calibration.uniform.full_tone",
                "Uniform coverage must begin at zero and reach 99.9% at full tone",
            ));
        }
        Ok(Self {
            radius_per_nominal_spacing,
            inverse,
        })
    }

    /// Returns the dimensionless full-tone footprint.
    pub fn radius_per_nominal_spacing(&self) -> f64 {
        self.radius_per_nominal_spacing
    }
    /// Returns immutable measured survival knots.
    pub fn inverse(&self) -> &[CalibrationKnot] {
        &self.inverse
    }
}

/// One validated, immutable measurement for an exact structural binding.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedCalibration {
    profile: String,
    binding: CalibrationBinding,
    raw: Vec<CalibrationKnot>,
    uniform: Option<UniformCalibration>,
    variation: Option<CalibrationVariation>,
}

impl SavedCalibration {
    /// Validates raw zero-to-two coverage and optional Uniform data under the producer's
    /// default strict profile, including current typed geometry-producer revisions.
    ///
    /// # Errors
    /// Rejects missing endpoints, nonfinite, unordered, or out-of-range observations.
    pub fn new(
        binding: CalibrationBinding,
        raw: Vec<CalibrationKnot>,
        uniform: Option<UniformCalibration>,
    ) -> Result<Self, ValidationError> {
        let profile = if binding.uses_guide_faces() {
            CALIBRATION_PROFILE_GUIDE_FACES
        } else if binding.has_variation_seed() {
            CALIBRATION_PROFILE_SEEDED
        } else {
            CALIBRATION_PROFILE
        };
        Self::with_profile(binding.profile_for_reference(profile), binding, raw, uniform)
    }

    /// Restores a persisted measurement only under its typed producer's strict or eligible
    /// covered-interior profile, rejecting superseded typed producer observations.
    ///
    /// # Errors
    /// Rejects obsolete profiles and invalid measured payloads.
    pub fn with_profile(
        profile: String,
        binding: CalibrationBinding,
        raw: Vec<CalibrationKnot>,
        uniform: Option<UniformCalibration>,
    ) -> Result<Self, ValidationError> {
        let supported = [
            CALIBRATION_PROFILE,
            CALIBRATION_PROFILE_64,
            CALIBRATION_PROFILE_GUIDE_FACES,
            CALIBRATION_PROFILE_GUIDE_FACES_64,
            CALIBRATION_PROFILE_SEEDED,
            CALIBRATION_PROFILE_SEEDED_64,
            CALIBRATION_PROFILE_ROUND_INTERIOR,
        ]
        .into_iter()
        .any(|base| {
            binding.supports_strict_reference_profile(base)
                && profile == binding.profile_for_reference(base)
        });
        if !supported {
            return Err(ValidationError::new(
                "calibration.profile",
                "saved calibration profile is unsupported; Recalibrate",
            ));
        }
        validate_knots(&raw, 2.0, MAX_SAVED_CALIBRATION_KNOTS)?;
        if raw.first().unwrap().coverage == raw.last().unwrap().coverage {
            return Err(ValidationError::new(
                "calibration.raw.span",
                "raw coverage has no measurable response; Recalibrate",
            ));
        }
        Ok(Self {
            profile,
            binding,
            raw,
            uniform,
            variation: None,
        })
    }

    /// Restores a seed-neutral stochastic mean with validated population-transfer observations.
    ///
    /// # Errors
    /// Rejects invalid knots, a raw-span inconsistency, or missing Uniform transfer evidence.
    pub fn with_mean_profile(
        binding: CalibrationBinding,
        raw: Vec<CalibrationKnot>,
        uniform: Option<UniformCalibration>,
        variation: CalibrationVariation,
    ) -> Result<Self, ValidationError> {
        validate_knots(&raw, 2.0, MAX_SAVED_CALIBRATION_KNOTS)?;
        let span = raw.last().unwrap().coverage - raw.first().unwrap().coverage;
        if !binding.supports_stochastic_mean()
            || !binding.has_variation_seed()
            || span <= 0.0
            || variation.raw_absolute > 0.02 * span
            || (variation.raw_absolute / span - variation.raw_fraction_of_span).abs() > 1e-9
            || uniform.is_some() != variation.uniform_tone_absolute.is_some()
        {
            return Err(ValidationError::new(
                "calibration.variation",
                "stochastic variation does not match its measured curve or Uniform payload",
            ));
        }
        Ok(Self {
            profile: binding.profile_for_reference(CALIBRATION_PROFILE_MEAN),
            binding,
            raw,
            uniform,
            variation: Some(variation),
        })
    }

    /// Returns the persisted geometry and reference contract identifier.
    pub fn profile(&self) -> &str {
        &self.profile
    }
    /// Returns exact structural verification metadata.
    pub fn binding(&self) -> &CalibrationBinding {
        &self.binding
    }
    /// Returns raw coverage at authored size zero through two.
    pub fn raw(&self) -> &[CalibrationKnot] {
        &self.raw
    }
    /// Returns Uniform data when this geometry supports that mode.
    pub fn uniform(&self) -> Option<&UniformCalibration> {
        self.uniform.as_ref()
    }
    /// Returns empirical population-transfer maxima only for the stochastic mean profile.
    pub fn variation(&self) -> Option<&CalibrationVariation> {
        self.variation.as_ref()
    }
}

/// One channel's independently owned measurement, even when another measured binding is equal.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedCalibration {
    pub channel_id: ChannelId,
    pub artifact: SavedCalibration,
}

/// Returns checked-in numeric reference data for one built-in straight circular channel.
///
/// # Errors
/// Rejects a malformed built-in binding or numeric payload; startup never runs a geometry fit.
pub(crate) fn default_straight_artifact(
    document: &Document,
    channel_id: ChannelId,
) -> Result<SavedCalibration, ValidationError> {
    SavedCalibration::built_in_straight(document, channel_id)
}

impl SavedCalibration {
    /// Rebinds checked-in straight circular reference numbers to equivalent default geometry.
    ///
    /// This is a finite built-in numeric artifact, never a render-time fit. Other structures and
    /// stretched guide aspects require an explicit calibration job.
    ///
    /// # Errors
    /// Rejects an unsupported structure or a malformed current binding.
    pub fn built_in_straight(
        document: &Document,
        channel_id: ChannelId,
    ) -> Result<Self, ValidationError> {
        let binding = CalibrationBinding::for_channel(document, channel_id)?;
        Self::built_in_straight_binding(binding)
    }

    /// Installs checked-in numeric data into a standalone default straight Pattern recipe.
    ///
    /// # Errors
    /// Rejects a recipe outside the fixed straight circular reference contract.
    pub fn built_in_straight_recipe(
        recipe: &PatternDefinitionRecipe,
    ) -> Result<Self, ValidationError> {
        let binding = CalibrationBinding::new(recipe.clone(), 1.0, 0.0)?;
        Self::built_in_straight_binding(binding)
    }

    /// Verifies the fixed built-in contract before sharing its checked-in observations.
    ///
    /// # Errors
    /// Rejects any structural deviation from the measured straight circular recipe.
    fn built_in_straight_binding(binding: CalibrationBinding) -> Result<Self, ValidationError> {
        let supported = matches!(&binding.recipe.structure,
            PatternStructureRecipe::OrderedOutputs { definition, outputs }
                if matches!(definition.as_ref(), PatternStructureRecipe::StraightGrid(draft)
                    if draft.coverage.guard_steps == 2 && draft.coverage.additional_margin == 0.0)
                && outputs.as_slice() == [PatternOutputRealizationRecipe::Marks])
            && binding.recipe.output_settings.len() == 1
            && matches!(
                binding.recipe.output_settings[0].response,
                PatternGeometryResponse::Marks(_)
            )
            && binding.density_aspect == 1.0
            && binding.shape_rotation_degrees == 0.0;
        if !supported {
            return Err(ValidationError::new(
                "calibration.builtin",
                "built-in numeric profile applies only to the default straight circular geometry",
            ));
        }
        SavedCalibration::new(
            binding,
            default_calibration::DEFAULT_STRAIGHT_RAW.to_vec(),
            None,
        )
    }
}

/// Validates exact endpoints and bounded monotone observations without modifying measurements.
///
/// # Errors
/// Rejects absent endpoints, nonfinite values, reversed samples, or excessive payload size.
fn validate_knots(
    knots: &[CalibrationKnot],
    maximum: f64,
    limit: usize,
) -> Result<(), ValidationError> {
    if knots.len() < 2
        || knots.len() > limit
        || knots.first().is_none_or(|k| k.input != 0.0)
        || knots.last().is_none_or(|k| k.input != maximum)
    {
        return Err(ValidationError::new(
            "calibration.knots",
            "calibration requires bounded exact endpoints",
        ));
    }
    for (index, knot) in knots.iter().enumerate() {
        if !knot.input.is_finite()
            || !knot.coverage.is_finite()
            || !(0.0..=maximum).contains(&knot.input)
            || !(0.0..=1.0).contains(&knot.coverage)
            || (index > 0
                && (knot.input <= knots[index - 1].input
                    || knot.coverage < knots[index - 1].coverage))
        {
            return Err(ValidationError::new(
                "calibration.knots",
                "measurements must be finite and monotone",
            ));
        }
    }
    Ok(())
}

/// Removes presentation-only family labels while preserving typed structural choices.
fn normalize_names(structure: &mut PatternStructureRecipe) {
    match structure {
        PatternStructureRecipe::StraightGrid(draft) => draft.name.clear(),
        PatternStructureRecipe::GeneralizedStraightGuides { name, .. }
        | PatternStructureRecipe::GenericGuides { name, .. }
        | PatternStructureRecipe::RandomSites { name, .. }
        | PatternStructureRecipe::ParametricCurve { name, .. } => name.clear(),
        PatternStructureRecipe::AuthoredResources { definition, .. }
        | PatternStructureRecipe::ConnectionPaths { definition, .. }
        | PatternStructureRecipe::MazeWalls { definition, .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
        | PatternStructureRecipe::CurveMotifPaths { definition, .. }
        | PatternStructureRecipe::VoronoiRegions { definition }
        | PatternStructureRecipe::GuideFaceRegions { definition, .. }
        | PatternStructureRecipe::OrderedOutputs { definition, .. } => normalize_names(definition),
    }
}

/// Replaces only authored variation seed values in measured identity, never in the document.
///
/// Distribution, spacing, refinement, connection algorithm, and all other structural choices
/// remain exact. The fixed seed supplies one reproducible reference realization for fitting.
fn normalize_variation_seeds(structure: &mut PatternStructureRecipe) {
    /// Normalizes a randomized connection choice while retaining its complete graph policy.
    fn connection(program: &mut ConnectionProgram) {
        match program {
            ConnectionProgram::RandomLinks { seed, .. }
            | ConnectionProgram::GridSpanningTree { seed, .. } => {
                *seed = CALIBRATION_REFERENCE_SEED;
            }
            _ => {}
        }
    }
    match structure {
        PatternStructureRecipe::RandomSites { seed, .. } => *seed = CALIBRATION_REFERENCE_SEED,
        PatternStructureRecipe::ConnectionPaths { definition, program, .. } => {
            normalize_variation_seeds(definition);
            connection(program);
        }
        PatternStructureRecipe::MazeWalls { definition, program, .. } => {
            normalize_variation_seeds(definition);
            program.seed = CALIBRATION_REFERENCE_SEED;
        }
        PatternStructureRecipe::OrderedOutputs { definition, outputs } => {
            normalize_variation_seeds(definition);
            for output in outputs {
                match output {
                    PatternOutputRealizationRecipe::ConnectionPaths { program, .. } => connection(program),
                    PatternOutputRealizationRecipe::MazeWalls { program, .. } => {
                        program.seed = CALIBRATION_REFERENCE_SEED;
                    }
                    _ => {}
                }
            }
        }
        PatternStructureRecipe::AuthoredResources { definition, .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
        | PatternStructureRecipe::CurveMotifPaths { definition, .. }
        | PatternStructureRecipe::VoronoiRegions { definition }
        | PatternStructureRecipe::GuideFaceRegions { definition, .. } =>
            normalize_variation_seeds(definition),
        PatternStructureRecipe::StraightGrid(_)
        | PatternStructureRecipe::GeneralizedStraightGuides { .. }
        | PatternStructureRecipe::GenericGuides { .. }
        | PatternStructureRecipe::ParametricCurve { .. } => {}
    }
}

/// Removes family mark orientation only when no output consumes inherited mark orientation.
///
/// Structural paths, motifs, connections, mazes, and regions do not consume this family field;
/// document materialization cannot retain it without a mark output. Explicit authored-shape
/// orientation remains on its own output and is never altered here.
fn normalize_inactive_family_orientation(structure: &mut PatternStructureRecipe) {
    match structure {
        PatternStructureRecipe::AuthoredResources { definition, .. } => {
            normalize_inactive_family_orientation(definition);
        }
        PatternStructureRecipe::OrderedOutputs {
            definition,
            outputs,
        } if !outputs
            .iter()
            .any(|output| matches!(output, PatternOutputRealizationRecipe::Marks)) =>
        {
            match definition.as_mut() {
                PatternStructureRecipe::GeneralizedStraightGuides { orientation, .. }
                | PatternStructureRecipe::GenericGuides { orientation, .. } => {
                    *orientation = MarkOrientationDraft::Fixed;
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// Removes Advanced response intervals and sampling from structural verification metadata.
fn normalize_response_shells(recipe: &mut PatternDefinitionRecipe) {
    for setting in &mut recipe.output_settings {
        match &mut setting.response {
            PatternGeometryResponse::Marks(value) => {
                value.minimum_fill = 0.0;
                value.maximum_fill = 1.0;
            }
            PatternGeometryResponse::Connected(value) => {
                value.minimum_thickness = 0.0;
                value.maximum_thickness = 1.0;
            }
            PatternGeometryResponse::Regions(value) => {
                value.minimum_fill = 0.0;
                value.maximum_fill = 1.0;
                value.sampling = RegionSamplingStrategy::AreaAverage;
            }
        }
    }
}
