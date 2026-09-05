#[path = "../examples/stage21b_prerequisite_curve_motif_validation.rs"]
#[allow(dead_code)]
mod validation;

use std::{fs, path::PathBuf, sync::Arc};
use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, AuthoredStructureKind,
    CanvasSpec, PatternDefinitionRecipe, PatternStructureRecipe, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, encode_png, evaluate, write_svg,
};
use toniator_render::GeometryOutput;

/// Bends only the guide appended by the domain conversion, preserving its endpoints and motif payload.
fn bend_converted_guide(
    mut recipe: PatternDefinitionRecipe,
    frame: CanvasSpec,
) -> PatternDefinitionRecipe {
    let PatternStructureRecipe::AuthoredResources { resources, .. } = &mut recipe.structure else {
        panic!("editable conversion retains the root authored resources");
    };
    let guide = resources.last_mut().expect("conversion appends its guide");
    let start = guide.segments().first().expect("guide has a start").start();
    let end = guide.segments().last().expect("guide has an end").end();
    *guide = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::CubicBezier {
            start,
            end,
            control_1: AuthoredPoint2 {
                x: start.x + (end.x - start.x) / 3.0,
                y: start.y - frame.height * 0.35,
            },
            control_2: AuthoredPoint2 {
                x: start.x + (end.x - start.x) * 2.0 / 3.0,
                y: end.y + frame.height * 0.35,
            },
        }],
    )
    .expect("finite open cubic guide validates");
    recipe
}

/// Exercises the exact nested Guide Editor conversion through ordinary canonical evaluation.
/// Both immutable sources retain intrinsic canvases; only current-gate derived artifacts are written.
#[test]
fn editable_curve_motif_guides_evaluate_both_intrinsic_sources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/validation/stage21b4/authored-guide-motif");
    fs::create_dir_all(&output).expect("current-gate artifact directory creates");
    for (label, input, width, height, hint) in [
        (
            "raster",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/raster-sample.png"
            ),
            1024.0,
            1024.0,
            SourceFormatHint::Png,
        ),
        (
            "vector",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/vector-sample.svg"
            ),
            900.0,
            620.0,
            SourceFormatHint::Svg,
        ),
    ] {
        let case = validation::SourceCase {
            label,
            input,
            width,
            height,
            hint,
        };
        let recipe = validation::curve_recipe(true, Some(0.25))
            .with_editable_guide_paths(CanvasSpec { width, height })
            .expect("existing domain capability admits authored motif guides");
        for (variant, recipe) in [
            (label.to_owned(), recipe.clone()),
            (
                format!("{label}-curved"),
                bend_converted_guide(recipe, CanvasSpec { width, height }),
            ),
        ] {
            let session = validation::materialized_session(&case, recipe);
            let result = evaluate(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new(format!("stage21b-{label}"))
                        .expect("source identity validates"),
                    Arc::<[u8]>::from(fs::read(input).expect("immutable source reads")),
                    hint,
                )
                .expect("source resolves"),
            ))
            .expect("authored Curve Motif evaluates through the shared pipeline");
            assert_eq!(result.raster().width(), width as u32);
            assert_eq!(result.raster().height(), height as u32);
            let strokes = result
                .scene()
                .layers()
                .iter()
                .flat_map(|layer| layer.outputs())
                .filter_map(|output| match output.geometry() {
                    GeometryOutput::CanonicalStrokes(strokes) => Some(strokes),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>();
            assert!(
                !strokes.is_empty(),
                "authored guides realize visible motif strokes"
            );
            for stroke in strokes {
                assert!(
                    stroke
                        .path
                        .segments()
                        .windows(2)
                        .all(|pair| pair[0].end() == pair[1].start())
                );
            }
            fs::write(
                output.join(format!("{variant}.png")),
                encode_png(result.raster()).expect("PNG encodes"),
            )
            .expect("native PNG artifact writes");
            fs::write(
                output.join(format!("{variant}.svg")),
                write_svg(result.scene()),
            )
            .expect("native SVG artifact writes");
        }
    }
}
