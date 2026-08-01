pub mod artwork_pipeline;
pub mod curve_render;
pub mod model;
pub mod pattern;
pub mod persistence;
pub mod png_export;
pub mod preset;
pub mod render;
pub mod site_distribution;
pub mod svg_export;
pub mod voronoi_geometry;
pub mod weighted_voronoi;

pub use artwork_pipeline::{
    ArtworkPipelineSettings, ArtworkSource, AutomaticSeparationStrategy, ChannelAssignment,
    LegacyBrightnessKind, LegacyCompatibilityAssignment, LegacyProjectionError,
    LegacyValueModeProjection, OutputChannelId, OutputModel, PipelineStateError, SourceAlphaPolicy,
    UnknownStableIdError, project_legacy_value_mode,
};
pub use cancel::{CancellationToken, OperationCancelled};
pub use model::{
    AlternateTileTransform, CubicCurveSegment, CurveLayout, CurvePath, CurvePoint, Document,
    DocumentAppearance, DocumentEditor, ExportBackground, Ink, MotifCoverage, OutputMode,
    PatternDocumentState, PatternSelection, PreviewSurface, RenderVariant, RgbaColor, Settings,
    SourceArtwork, Treatment, ValueMode, WebCurveChannel, WebCurveChannels, WebCurveSettings,
    WebShape, WebShapeChannel, WebShapeChannels, WebShapeDeltas, WebShapeSettings,
    WeightedVoronoiArrangementPolicy, WeightedVoronoiChannelSettings,
    WeightedVoronoiDensityPolarity, WeightedVoronoiPlacementMode, WeightedVoronoiSettings,
};
pub use pattern::{
    AffineTransform, ArtboardSpace, CanonicalBlendMode, CanonicalColor, CanonicalLayer,
    CanonicalLayerId, CanonicalOutputError, CanonicalOutputLimits, CanonicalPatternOutput,
    CanonicalPoint, CompositePatternOutput, FillRule, FilledRegion, GeometryPolarity,
    LegacyPatternCompatibility, MarkPatternOutput, NetworkEdgeId, NetworkNode, NetworkNodeId,
    NetworkPatternOutput, PATTERN_REGISTRY, PathPatternOutput, PatternCompatibility, PatternFamily,
    PatternId, PatternIdError, PatternInspectorPanel, PatternMetadata, PatternOutputKind,
    PatternParameterDescriptor, PatternParameterError, PatternParameterScope,
    PatternParameterVisibility, PatternRegistry, PatternRegistryError, PatternSelectorMetadata,
    PolygonRing, RegionId, RegionPatternOutput, RingWinding, SharedBoundaryEdge,
    VersionedPatternParameters, is_valid_dotted_id,
};
pub use persistence::{
    atomic_write_cancellable, load_document, save_document_atomic, save_document_atomic_cancellable,
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
    generate_web_shape_marks_cancellable, render_canonical_pattern_output_cancellable,
    render_document_export_cancellable, render_document_output_cancellable,
    render_document_preview, render_document_preview_cancellable, render_preview,
};
pub use site_distribution::{
    ArrangementPolicy, DistributionField, DistributionFingerprint, DistributionIdentity,
    DistributionLimits, DistributionMode, DistributionPolarity, DistributionRequest,
    DistributionRequestMetadata, DomainBounds, OrderedPoint, SiteDistribution,
    generate_site_distribution, generate_site_distribution_cancellable,
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
    WEIGHTED_VORONOI_MAX_FIELD_EDGE, WeightedVoronoiCacheMetadata, WeightedVoronoiCellRegion,
    WeightedVoronoiGeneratedOutput, generate_weighted_voronoi_cancellable,
    weighted_voronoi_field_dimensions,
};
pub mod cancel;
