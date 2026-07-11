pub mod curve_render;
pub mod model;
pub mod persistence;
pub mod png_export;
pub mod preset;
pub mod render;
pub mod svg_export;

pub use model::{
    AlternateTileTransform, CubicCurveSegment, CurveLayout, CurvePath, CurvePoint, Document,
    DocumentEditor, Ink, MotifCoverage, RenderVariant, Settings, SourceArtwork, Treatment,
    ValueMode, WebCurveChannel, WebCurveChannels, WebCurveSettings, WebShape, WebShapeChannel,
    WebShapeChannels, WebShapeDeltas, WebShapeSettings,
};
pub use persistence::{load_document, save_document_atomic};
pub use png_export::{PngExportOptions, document_artboard, export_png, png_bytes};
pub use render::{
    RenderGate, RenderResult, generate_document_marks, render_document_preview, render_preview,
};
pub use svg_export::export_svg;
