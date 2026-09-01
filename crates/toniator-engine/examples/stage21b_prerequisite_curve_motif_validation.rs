//! Generates ordinary-pipeline Curve Motif validation artifacts for both immutable sources.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft, PathStrokeStyle,
    PatternDefinitionRecipe, PatternStructureRecipe, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, encode_png, evaluate, write_svg,
};

/// Describes one immutable source and its intrinsic ordinary document canvas.
pub struct SourceCase {
    pub label: &'static str,
    pub input: &'static str,
    pub width: f64,
    pub height: f64,
    pub hint: SourceFormatHint,
}

/// Builds the asymmetric authored open path embedded by every evidence recipe.
fn asymmetric_motif() -> AuthoredStructureDraft {
    AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.32, y: 0.27 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.32, y: 0.27 },
                end: AuthoredPoint2 { x: 0.7, y: -0.18 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.7, y: -0.18 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("fixed asymmetric motif validates")
}

/// Builds one current single-guide Along Guides Curve Motif recipe without a catalog entry.
pub fn curve_recipe(
    mirror_alternate_rows: bool,
    alternate_row_phase: Option<f64>,
) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::connected(PatternStructureRecipe::AuthoredResources {
        resources: vec![asymmetric_motif()],
        definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
            definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                name: "Stage 21B prerequisite Curve Motif evidence".into(),
                coverage: CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.0,
                },
                dimensions: vec![GuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.125,
                    spacing_multiplier: 1.0,
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.25,
                },
                orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
            }),
            resource_index: 0,
            style: PathStrokeStyle::default(),
            mirror_alternate_rows,
            alternate_row_phase,
        }),
    })
}

/// Materializes one ordinary current document through the history-backed recipe command.
pub fn materialized_session(case: &SourceCase, recipe: PatternDefinitionRecipe) -> DocumentSession {
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let document = Document::new_default_document(
        CanvasSpec {
            width: case.width,
            height: case.height,
        },
        SourceReference::Assigned(source_id),
    )
    .expect("ordinary evidence document validates");
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session starts"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .map(|bundle| bundle.definition.clone())
        .expect("default definition exists");
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .expect("Curve Motif recipe materializes");
    let base = history.document().pattern_settings().clone();
    let mut settings = base.clone();
    // The existing density authority deliberately governs motif cadence; this is high enough to
    // make asymmetric repetitions legible at both intrinsic evidence canvases without adding a
    // motif-size setting.
    settings.density.density = 10.0;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .expect("evidence density applies");
    history.session().clone()
}

/// Writes separate native RGBA coverage and visible-color statistics without flattening output.
fn write_rgba_statistics(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let mut opaque = 0_usize;
    let mut transparent = 0_usize;
    let mut partial = 0_usize;
    let mut hidden_rgb = 0_usize;
    for pixel in pixels.chunks_exact(4) {
        match pixel[3] {
            0 => {
                transparent += 1;
                hidden_rgb += usize::from(pixel[..3].iter().any(|component| *component != 0));
            }
            255 => opaque += 1,
            _ => partial += 1,
        }
    }
    fs::write(
        path,
        format!(
            "dimensions={width}x{height}\nrgba_pixels={}\nopaque_alpha={opaque}\npartial_alpha={partial}\ntransparent_alpha={transparent}\nhidden_rgb_under_zero_alpha={hidden_rgb}\n",
            width as usize * height as usize
        ),
    )
    .expect("native RGBA statistics write");
}

/// Evaluates one ordinary source/recipe pair and writes native PNG, raw SVG, and SVG raster evidence.
fn write_variant(case: &SourceCase, label: &str, mirror: bool, phase: Option<f64>, output: &Path) {
    let stem = format!("{}-{label}", case.label);
    let png_path = output.join(format!("{stem}.png"));
    let svg_path = output.join(format!("{stem}.svg"));
    let svg_raster_path = output.join(format!("{stem}-svg-rasterized.png"));
    let session = materialized_session(case, curve_recipe(mirror, phase));
    let bytes = Arc::<[u8]>::from(fs::read(case.input).expect("immutable source reads"));
    let source_id = SourceReferenceId::new(format!("stage21b-{}", case.label))
        .expect("fixed source identifier validates");
    let evaluated = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source_id, bytes, case.hint).expect("source resolves"),
    ))
    .expect("ordinary Curve Motif evaluation succeeds");
    fs::write(
        &png_path,
        encode_png(evaluated.raster()).expect("native PNG encodes"),
    )
    .expect("native PNG writes");
    fs::write(&svg_path, write_svg(evaluated.scene())).expect("raw SVG writes");
    write_rgba_statistics(
        &output.join(format!("{stem}-native-rgba-stats.txt")),
        evaluated.raster().width(),
        evaluated.raster().height(),
        evaluated.raster().pixels(),
    );
    let status = Command::new("inkscape")
        .arg(&svg_path)
        .arg("--export-type=png")
        .arg(format!("--export-filename={}", svg_raster_path.display()))
        .status()
        .expect("Inkscape is available for SVG evidence");
    assert!(status.success(), "raw SVG rasterizes with Inkscape");
}

/// Generates composed odd-row artifacts from both immutable sources at intrinsic dimensions.
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage21b-prerequisite-curve-motif");
    fs::create_dir_all(&output).expect("validation output directory creates");
    let cases = [
        SourceCase {
            label: "raster-1024x1024",
            input: concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ),
            width: 1024.0,
            height: 1024.0,
            hint: SourceFormatHint::Png,
        },
        SourceCase {
            label: "vector-900x620",
            input: concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/vector-sample.svg"
            ),
            width: 900.0,
            height: 620.0,
            hint: SourceFormatHint::Svg,
        },
    ];
    for case in &cases {
        write_variant(case, "mirror-phase", true, Some(0.25), &output);
    }
}
