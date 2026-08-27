use std::{fs, path::Path, process::Command};

use sha2::{Digest, Sha256};
use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, DensityMetric2D, Document,
    GeneralizedSiteProduct, GuideDimensionId, MarkOrientation, MarkPrototype, PatternDefinition,
    PatternDefinitionId, PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId,
    PatternOutputRealization, SourceReference, SourceReferenceId, StraightGuideDimension,
    StraightGuideRepetition,
};
use toniator_engine::{SourceFormatHint, resolve_source_identity};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, save};

/// Returns the lowercase SHA256 of one exact artifact without normalizing its bytes.
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Builds a current shape-bearing modeled document at the supplied intrinsic source dimensions.
fn shape_document(source_id: SourceReferenceId, width: u32, height: u32) -> Document {
    let canvas = CanvasSpec {
        width: f64::from(width),
        height: f64::from(height),
    };
    let base = Document::new_default_document(
        canvas.clone(),
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default modeled document validates");
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "CLI authored shape",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.25,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(2),
        },
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let PatternOutputRealization::MarkPrototype { prototype, .. } =
        &mut definition.output_layers[0].realization
    else {
        panic!("generalized guides own a typed mark output")
    };
    *prototype = MarkPrototype::AuthoredClosedShape {
        structure_id: AuthoredStructureId(7),
    };
    let first = AuthoredPoint2 { x: -3.0, y: -2.0 };
    let second = AuthoredPoint2 { x: 3.0, y: 2.0 };
    let third = AuthoredPoint2 { x: -3.0, y: 2.0 };
    let fourth = AuthoredPoint2 { x: 3.0, y: -2.0 };
    let shape = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::ClosedShape,
        vec![
            AuthoredCurveSegment::Line {
                start: first,
                end: second,
            },
            AuthoredCurveSegment::Line {
                start: second,
                end: third,
            },
            AuthoredCurveSegment::CubicBezier {
                start: third,
                control_1: AuthoredPoint2 { x: 0.0, y: 5.0 },
                control_2: AuthoredPoint2 { x: 0.0, y: -5.0 },
                end: fourth,
            },
            AuthoredCurveSegment::Line {
                start: fourth,
                end: first,
            },
        ],
    )
    .unwrap();
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    bundle.definition = definition;
    let PatternGeometryResponse::Marks(response) = &mut bundle.output_settings[0].response else {
        panic!("default bundle owns a mark response")
    };
    response.minimum_fill = 0.25;
    response.maximum_fill = 1.0;
    let mut settings = base.pattern_settings().clone();
    settings.density = DensityMetric2D {
        across_x: 12.0,
        across_y: 12.0,
        aspect_locked: true,
    };
    settings.pattern_rotation_degrees = 7.0;
    settings.shape_rotation_degrees = 15.0;
    Document::with_source_topology_and_authored_structures(
        base.id(),
        canvas,
        SourceReference::Assigned(source_id),
        vec![bundle],
        settings,
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![shape],
    )
    .unwrap()
}

/// Runs one CLI command and returns stdout, failing with the exact stderr diagnostic.
fn run(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Exercises document validation, descriptor inspection, native PNG, and structural SVG through
/// the CLI for both immutable sources at intrinsic dimensions and writes bounded E2 evidence.
#[test]
fn shape_documents_validate_inspect_and_render_both_immutable_sources_intrinsically() {
    let validation_root = Path::new("../../target/validation/stage-20e2");
    fs::create_dir_all(validation_root).unwrap();
    for (label, source_path, embedded_format, source_hint) in [
        (
            "raster",
            "../../assets/raster-sample.png",
            EmbeddedSourceFormat::Png,
            SourceFormatHint::Png,
        ),
        (
            "vector",
            "../../assets/vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            SourceFormatHint::Svg,
        ),
    ] {
        let source_bytes = fs::read(source_path).unwrap();
        let source_identity = resolve_source_identity(&source_bytes, source_hint).unwrap();
        let case_root = validation_root.join(label);
        fs::create_dir_all(&case_root).unwrap();
        let source_id = SourceReferenceId::new(format!("stage20e2-{label}-source")).unwrap();
        let document = shape_document(
            source_id.clone(),
            source_identity.width,
            source_identity.height,
        );
        let sources = SourceBundle::new([EmbeddedSource::new(
            source_id,
            embedded_format,
            source_bytes.clone(),
            Path::new(source_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned),
        )
        .unwrap()])
        .unwrap();
        let container = case_root.join("shape.toniator");
        let png = case_root.join("native.png");
        let svg = case_root.join("editable.svg");
        save(&container, &document, &sources).unwrap();
        let validate = run(&["validate", "--input", container.to_str().unwrap()]);
        assert!(validate.contains("document v5"));
        fs::write(case_root.join("validate.txt"), &validate).unwrap();
        let capabilities = run(&["capabilities", "--input", container.to_str().unwrap()]);
        assert!(capabilities.contains("OutputAuthoredClosedShape"));
        assert!(capabilities.contains("AuthoredClosedShape"));
        assert!(capabilities.contains("reference=Singular"));
        fs::write(case_root.join("inspect-capabilities.txt"), &capabilities).unwrap();
        run(&[
            "render",
            "--input",
            container.to_str().unwrap(),
            "--output",
            png.to_str().unwrap(),
        ]);
        run(&[
            "render",
            "--input",
            container.to_str().unwrap(),
            "--output",
            svg.to_str().unwrap(),
        ]);
        let png_bytes = fs::read(&png).unwrap();
        assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(png_bytes[24], 8, "native PNG uses eight-bit channels");
        assert_eq!(png_bytes[25], 6, "native PNG preserves straight RGBA");
        let rendered_identity = resolve_source_identity(&png_bytes, SourceFormatHint::Png).unwrap();
        assert_eq!(rendered_identity.width, source_identity.width);
        assert_eq!(rendered_identity.height, source_identity.height);
        let svg_text = fs::read_to_string(&svg).unwrap();
        assert!(svg_text.contains("<path "));
        assert!(svg_text.contains(" C "));
        assert!(svg_text.contains("fill-rule=\"evenodd\""));
        assert!(svg_text.contains("clip-path=\"url(#canvas-clip)\""));
        assert!(!svg_text.contains("<circle "));
        let manifest = format!(
            "stage=20e2\nsource={}\nsource_sha256={}\nsource_intrinsic={}x{}\ncontainer_sha256={}\nnative_png_sha256={}\nnative_png_color_type=rgba\neditable_svg_sha256={}\nsvg_font_caveat={}\n",
            source_path,
            sha256(&source_bytes),
            source_identity.width,
            source_identity.height,
            sha256(&fs::read(&container).unwrap()),
            sha256(&png_bytes),
            sha256(svg_text.as_bytes()),
            if label == "vector" {
                "decoded SVG text pixels are font-dependent; structural path assertions are authoritative"
            } else {
                "not-applicable"
            },
        );
        fs::write(case_root.join("manifest.txt"), manifest).unwrap();
    }
}
