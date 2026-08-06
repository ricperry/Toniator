pub mod artwork_pipeline;
pub mod bundled_pattern_definitions;
pub mod curve_render;
pub mod curves_native;
pub mod curves_recipe;
pub mod definition_runtime;
pub mod guided_editor;
pub mod model;
pub mod parametric_paths;
pub mod pattern;
pub mod pattern_definition;
pub mod pattern_definition_lifecycle;
pub mod pattern_definition_registry;
pub mod persistence;
pub mod png_export;
pub mod preset;
pub mod render;
pub mod shapes_native;
pub mod shapes_recipe;
pub mod site_distribution;
pub mod structured_fields;
pub mod svg_export;
pub mod voronoi_geometry;
pub mod weighted_voronoi;

pub use artwork_pipeline::{
    ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
    LegacyBrightnessKind, LegacyCompatibilityAssignment, LegacyProjectionError,
    LegacyValueModeProjection, OutputChannelId, OutputModel, PipelineStateError, SourceAlphaPolicy,
    UnknownStableIdError, project_legacy_value_mode,
};
pub use bundled_pattern_definitions::{
    BundledPatternDefinitionError, CURVES_BUNDLED_BYTES, QUADRATIC_RADIAL_SPIRAL_BUNDLED_BYTES,
    SHAPES_BUNDLED_BYTES, WAVE_LINE_FIELD_BUNDLED_BYTES, WEIGHTED_VORONOI_BUNDLED_BYTES,
    load_bundled_curves_definition, load_bundled_pattern_definition_registry,
    load_bundled_quadratic_radial_spiral_definition, load_bundled_shapes_definition,
    load_bundled_wave_line_field_definition, load_bundled_weighted_voronoi_definition,
};
pub use cancel::{CancellationToken, OperationCancelled};
pub use curves_native::{
    CURVES_NATIVE_OPERATION_REGISTRY, CURVES_NATIVE_OPERATIONS, CurvesDeformedPaths,
    CurvesModulatedPaths, CurvesMotif, CurvesPlacement, CurvesSamples,
};
pub use curves_recipe::{
    CurvesRecipeAdaptation, adapt_curves_settings_to_recipe, adapt_document_curves_to_recipe,
};
pub use definition_runtime::{
    RESOLVED_DEFINITION_NATIVE_OPERATION_REGISTRY, RESOLVED_DEFINITION_NATIVE_OPERATIONS,
    execute_resolved_definition_cancellable, execute_resolved_definition_with_source_cancellable,
};
pub use guided_editor::{
    GuidedControlDescriptor, GuidedDefinitionCatalog, GuidedDefinitionCatalogEntry,
    GuidedDefinitionDraft, GuidedEditorError, GuidedNumericPresentation, GuidedSection,
    GuidedWidgetKind, SharedRecipeEditorDraft,
};
pub use model::{
    AlternateTileTransform, CubicCurveSegment, CurveLayout, CurvePath, CurvePoint, Document,
    DocumentAppearance, DocumentEditor, ExportBackground, Ink, MotifCoverage, OutputMode,
    PatternDocumentState, PatternSelection, PreviewSurface, RenderVariant,
    ResolvedSelectedPatternDefinition, RgbaColor, Settings, SourceArtwork, Treatment, ValueMode,
    WebCurveChannel, WebCurveChannels, WebCurveSettings, WebShape, WebShapeChannel,
    WebShapeChannels, WebShapeDeltas, WebShapeSettings, WeightedVoronoiArrangementPolicy,
    WeightedVoronoiChannelSettings, WeightedVoronoiDensityPolarity, WeightedVoronoiPlacementMode,
    WeightedVoronoiSettings,
};
pub use parametric_paths::{
    PARAMETRIC_PATH_EMIT_PATHS_OPERATION_ID, PARAMETRIC_PATH_EMIT_PATHS_OPERATION_VERSION,
    PARAMETRIC_PATHS_MAX_SAMPLES, PARAMETRIC_PATHS_NATIVE_OPERATION_REGISTRY,
    PARAMETRIC_PATHS_NATIVE_OPERATIONS, ParametricPath, ParametricPathPoint,
    QUADRATIC_RADIAL_SPIRAL_OPERATION_ID, QUADRATIC_RADIAL_SPIRAL_OPERATION_VERSION,
    QuadraticRadialSpiralDirection, QuadraticRadialSpiralParameters,
    execute_parametric_paths_definition_cancellable, generate_quadratic_radial_spiral,
    quadratic_radial_spiral_authoring_layout, quadratic_radial_spiral_parameter_definitions,
};
pub use pattern::{
    AffineTransform, ArtboardSpace, CanonicalBlendMode, CanonicalColor, CanonicalLayer,
    CanonicalLayerId, CanonicalOutputError, CanonicalOutputLimits, CanonicalPatternOutput,
    CanonicalPoint, CompositePatternOutput, FillRule, FilledRegion, GeometryPolarity,
    LegacyPatternCompatibility, MarkPatternOutput, NetworkEdgeId, NetworkNode, NetworkNodeId,
    NetworkPatternOutput, NetworkStroke, NetworkStrokeId, PATTERN_REGISTRY, PathPatternOutput,
    PatternCompatibility, PatternFamily, PatternId, PatternIdError, PatternInspectorPanel,
    PatternMetadata, PatternOutputKind, PatternParameterDescriptor, PatternParameterError,
    PatternParameterScope, PatternParameterVisibility, PatternRegistry, PatternRegistryError,
    PatternSelectorMetadata, PolygonRing, RegionId, RegionPatternOutput, RingWinding,
    SharedBoundaryEdge, VersionedPatternParameters, is_valid_dotted_id,
};
pub use pattern_definition::{
    AuthoringLayout, AuthoringSection, CreatorParameterCategory, CreatorParameterIncrement,
    CreatorParameterMetadata, CreatorParameterUnit, DefinitionParameterScope, EmbeddedSvgAsset,
    GraphPosition, LiteralValue, MAX_EMBEDDED_SVG_BYTES, MAX_PATTERN_ASSETS, MAX_PATTERN_EDGES,
    MAX_PATTERN_INSTANCE_OUTPUT_CHANNELS, MAX_PATTERN_INSTANCE_VALUES, MAX_PATTERN_NODES,
    MAX_PATTERN_PARAMETERS, MAX_TEXT_PARAMETER_BYTES, MAX_TOTAL_EMBEDDED_SVG_BYTES,
    NativeRecipeOperation, NativeRecipeOperationError, NativeRecipeOperationRegistry,
    NativeRecipePreflight, OperationParameterDescriptor, OperationPortDescriptor,
    OperationReference, OperationRegistry, OutputChannelParameterValues, ParameterApplicability,
    ParameterAuthoring, ParameterInvalidationScope, ParameterOwnership,
    ParameterSerializationBehavior, ParameterValidationBehavior, PatternDefinition,
    PatternDefinitionError, PatternDisplayMetadata, PatternInstanceParameters,
    PatternInstanceParametersError, PatternInstanceValue, PatternParameterConstraints,
    PatternParameterDefinition, PortReference, QuickControlDefinition, QuickControlKind,
    REGISTERED_OPERATIONS, RecipeArgument, RecipeEdge, RecipeExecutionContext,
    RecipeExecutionError, RecipeGraph, RecipeNode, RecipeOperationInputs,
    RecipeOperationParameters, RecipePortType, RecipeRuntimeValue, RecipeSourceFieldProvider,
    RecipeValueType, RecipeVoronoiDiagram, RegisteredNativeRecipeOperation,
    RegisteredOperationDescriptor, TNPATTERN_FORMAT_VERSION, TNPATTERN_INSTANCE_FORMAT_VERSION,
    TNPATTERN_RECIPE_VERSION, TwoDimensionalAxis, TwoDimensionalRelation, parse_tnpattern,
    parse_tnpattern_instance_parameters, serialize_tnpattern,
    serialize_tnpattern_instance_parameters,
};
pub use pattern_definition_lifecycle::{
    DefinitionLifecycleInputs, ExternalPatternDefinitionImport,
    ExternalPatternDefinitionImportChoice, ExternalPatternDefinitionImportCommit,
    ExternalPatternDefinitionImportError, ExternalPatternDefinitionImportPlan,
    MissingPatternDefinitionDiagnostic, PatternDefinitionLibrarySaveError,
    PatternDefinitionLifecycleError, PatternDefinitionLifecycleResolver,
    USER_PATTERN_LIBRARY_RELATIVE_PATH, UserLibrarySelectionError, UserPatternLibraryDiagnostic,
    UserPatternLibraryEntry, UserPatternLibrarySnapshot, commit_external_pattern_definition_import,
    inspect_external_pattern_definition_import, native_user_pattern_library_dir,
    project_definition_for_library_save, save_user_pattern_definition, user_pattern_library_dir,
};
pub use pattern_definition_registry::{
    PatternDefinitionFingerprint, PatternDefinitionRegistry, PatternDefinitionRegistryError,
    PatternDefinitionResolutionDiagnostic, PatternDefinitionSource, ResolvedPatternDefinition,
};
pub use persistence::{
    DocumentOpenCandidate, LoadAdjustments, LoadedDocument, MissingPatternDefinitionOpenCandidate,
    MissingPatternDefinitionReplacementCandidate, atomic_write_cancellable, load_document,
    load_document_open_candidate, load_document_open_candidate_with_library, save_document_atomic,
    save_document_atomic_cancellable,
};
pub use png_export::{
    PngBackground, PngExportOptions, canonical_pattern_png_bytes,
    canonical_pattern_png_bytes_cancellable, document_artboard, export_png, export_png_cancellable,
    png_bytes, png_bytes_cancellable,
};
pub use render::{
    RenderGate, RenderResult, composite_export_background, composite_preview,
    generate_document_marks, generate_document_marks_cancellable, generate_document_pattern_output,
    generate_document_pattern_output_cancellable, generate_marks_cancellable,
    render_canonical_pattern_output_cancellable, render_document_export_cancellable,
    render_document_output_cancellable, render_document_preview,
    render_document_preview_cancellable, render_preview,
};
pub use shapes_native::{
    SHAPES_NATIVE_OPERATION_REGISTRY, SHAPES_NATIVE_OPERATIONS, ShapesLattice, ShapesMappedValues,
    ShapesSamples, ShapesSelectedPrimitive, ShapesTransformedMarks,
    execute_bundled_shapes_recipe_cancellable, execute_shapes_definition_cancellable,
    shapes_instance_artboard, validate_shapes_definition_instance,
};
pub use shapes_recipe::{
    ShapesRecipeAdaptation, adapt_document_shapes_to_recipe, adapt_shapes_settings_to_recipe,
};
pub use site_distribution::{
    ArrangementPolicy, DistributionField, DistributionFingerprint, DistributionIdentity,
    DistributionLimits, DistributionMode, DistributionPolarity, DistributionRequest,
    DistributionRequestMetadata, DomainBounds, OrderedPoint, SiteDistribution,
    generate_site_distribution, generate_site_distribution_cancellable,
};
pub use structured_fields::{
    STRUCTURED_FIELD_EMIT_PATHS_OPERATION_ID, STRUCTURED_FIELD_SOURCE_WIDTH_OPERATION_ID,
    STRUCTURED_FIELDS_NATIVE_OPERATIONS, StructuredFieldPaths, WAVE_LINE_FIELD_OPERATION_ID,
    WaveLineFieldParameters, execute_structured_fields_definition_cancellable,
    generate_wave_line_field,
};
pub use svg_export::{
    canonical_pattern_svg_bytes, canonical_pattern_svg_bytes_cancellable, export_svg,
    export_svg_cancellable,
};
pub use voronoi_geometry::{
    ClippedVoronoiCell, GeometryLimits, VoronoiBoundary, VoronoiBoundaryKind, VoronoiDiagram,
    build_voronoi_diagram, build_voronoi_diagram_cancellable, inset_clipped_cell,
    inset_clipped_cell_for_response,
};
pub use weighted_voronoi::{
    WEIGHTED_VORONOI_MAX_FIELD_EDGE, WEIGHTED_VORONOI_NATIVE_OPERATION_REGISTRY,
    WEIGHTED_VORONOI_NATIVE_OPERATIONS, execute_bundled_weighted_voronoi_recipe_cancellable,
    weighted_voronoi_field_dimensions, weighted_voronoi_recipe_instance_from_document,
    weighted_voronoi_recipe_instance_from_settings,
};
pub mod cancel;
