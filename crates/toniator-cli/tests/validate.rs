use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ChannelTopologyTemplate, ColorValue, CoveragePolicy, DensityMetric2D, Document,
    DocumentId, DocumentSession, HalftoneChannelModel, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionId, PatternMechanismId, PatternOutputLayerId, SourceComponent,
    SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationLimits, EvaluationRequest, ResolvedSource, SourceFormatHint, evaluate_with_limits,
    write_svg,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

#[test]
fn valid_document_exits_zero_and_reports_success() {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "--canvas",
            "900x600",
            "--density-x",
            "90.0",
            "--density-y",
            "60.0",
            "--opacity",
            "0.75",
        ])
        .output()
        .expect("CLI runs");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stderr).expect("utf8 stderr"), "");
    assert!(
        String::from_utf8(output.stdout)
            .expect("utf8 stdout")
            .contains("valid document")
    );
}

#[test]
fn zero_density_exits_two_with_schema_path_and_no_success_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "--canvas",
            "900x600",
            "--density-x",
            "0",
            "--density-y",
            "60.0",
            "--opacity",
            "0.75",
        ])
        .output()
        .expect("CLI runs");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .expect("utf8 stderr")
            .contains("channel.pattern.layout.density.across_x")
    );
    assert_eq!(String::from_utf8(output.stdout).expect("utf8 stdout"), "");
}

#[test]
fn inspect_grid_accepts_negative_offsets_and_emits_deterministic_json() {
    let output_path = std::env::temp_dir().join(format!(
        "toniator-stage-3-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "grid",
            "--output",
            output_path.to_str().expect("utf8 temp path"),
            "--canvas",
            "900x600",
            "--density-x",
            "90.0",
            "--density-y",
            "60.0",
            "--rotation",
            "17.0",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--format",
            "json",
        ])
        .output()
        .expect("CLI runs");

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&output_path).expect("output")).expect("valid JSON");
    assert_eq!(json["coverage"][0]["first_index"], -11);
    assert_eq!(json["coverage"][1]["last_index"], 76);
    assert_eq!(json["sites"].as_array().expect("sites array").len(), 6_185);
    let fixture: serde_json::Value = serde_json::from_slice(
        &fs::read("../../fixtures/canonical/stage-3-sites.sorted.json").expect("fixture"),
    )
    .expect("fixture JSON");
    assert_eq!(
        json, fixture,
        "CLI output must match the canonical Stage 3 sites"
    );
    fs::remove_file(output_path).expect("remove temporary artifact");
}

#[test]
fn every_evaluation_command_accepts_the_candidate_limit_and_rejects_an_oversized_grid() {
    let grid = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "grid",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--max-family-candidates",
            "1",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(grid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&grid.stderr).contains("coverage.candidate_limit"));

    let marks = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "marks",
            "--source",
            "../../assets/raster-sample.png",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--max-family-candidates",
            "1",
            "--source-component",
            "luminance",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--color",
            "#00b7ff",
            "--opacity",
            "0.72",
            "--summary",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(marks.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&marks.stderr).contains("coverage.candidate_limit"));

    let output = std::env::temp_dir().join("toniator-stage-8-limit.png");
    let render = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            "../../assets/raster-sample.png",
            "-o",
            output.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--max-family-candidates",
            "1",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--opacity",
            "0.72",
        ])
        .output()
        .unwrap();
    assert_eq!(render.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&render.stderr).contains("coverage.candidate_limit"));
}

#[test]
fn inspect_marks_compact_summaries_match_both_canonical_fixtures() {
    for (source, fixture) in [
        (
            "../../assets/raster-sample.png",
            "../../fixtures/canonical/stage-4-raster-summary.json",
        ),
        (
            "../../assets/vector-sample.svg",
            "../../fixtures/canonical/stage-4-svg-summary.json",
        ),
    ] {
        let output_path = std::env::temp_dir().join(format!(
            "toniator-stage-4-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "inspect",
                "marks",
                "--source",
                source,
                "--output",
                output_path.to_str().unwrap(),
                "--canvas",
                "900x600",
                "--density-x",
                "90.0",
                "--density-y",
                "60.0",
                "--rotation",
                "17.0",
                "--offset-x",
                "3.25",
                "--offset-y",
                "-4.5",
                "--guard-steps",
                "2",
                "--support-radius",
                "4.5",
                "--source-component",
                "luminance",
                "--size-min",
                "2.0",
                "--size-max",
                "9.0",
                "--color",
                "#00b7ff",
                "--opacity",
                "0.72",
                "--summary",
                "--format",
                "json",
            ])
            .output()
            .expect("CLI runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: serde_json::Value =
            serde_json::from_slice(&fs::read(&output_path).unwrap()).unwrap();
        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        assert_eq!(actual, expected);
        fs::remove_file(output_path).unwrap();
    }
}

#[test]
fn inspect_marks_presentation_changes_leave_geometry_summary_identical() {
    let mut summaries = Vec::new();
    for (color, opacity) in [("#00b7ff", "0.72"), ("#ff5500", "0.19")] {
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "inspect",
                "marks",
                "--source",
                "../../assets/raster-sample.png",
                "--canvas",
                "900x600",
                "--density-x",
                "90.0",
                "--density-y",
                "60.0",
                "--rotation",
                "17.0",
                "--offset-x",
                "3.25",
                "--offset-y",
                "-4.5",
                "--guard-steps",
                "2",
                "--support-radius",
                "4.5",
                "--source-component",
                "luminance",
                "--size-min",
                "2.0",
                "--size-max",
                "9.0",
                "--color",
                color,
                "--opacity",
                opacity,
                "--summary",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        summaries.push(serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap());
    }
    let [mut first, mut second] = summaries.try_into().unwrap();
    let presentation_a = first
        .as_object_mut()
        .unwrap()
        .remove("presentation")
        .unwrap();
    let presentation_b = second
        .as_object_mut()
        .unwrap()
        .remove("presentation")
        .unwrap();
    assert_ne!(presentation_a, presentation_b);
    assert_eq!(first, second);
}

fn render_command(source: &str, output: &std::path::Path, model: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command.args([
        "render",
        "--input",
        source,
        "--output",
        output.to_str().unwrap(),
        "--channel-model",
        model,
        "--canvas",
        "900x600",
        "--density-x",
        "90.0",
        "--density-y",
        "60.0",
        "--rotation",
        "17.0",
        "--offset-x",
        "3.25",
        "--offset-y",
        "-4.5",
        "--guard-steps",
        "2",
        "--size-min",
        "2.0",
        "--size-max",
        "9.0",
        "--opacity",
        "0.72",
    ]);
    command
}

fn direct_render_without_canvas(source: &str, output: &Path, model: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command.args([
        "render",
        "--input",
        source,
        "--output",
        output.to_str().unwrap(),
        "--channel-model",
        model,
        "--density-x",
        "90",
        "--density-y",
        "60",
        "--rotation",
        "17",
        "--offset-x",
        "3.25",
        "--offset-y",
        "-4.5",
        "--guard-steps",
        "2",
        "--size-min",
        "2",
        "--size-max",
        "9",
    ]);
    command
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

#[test]
fn direct_source_intrinsic_canvas_and_png_antialiasing_are_consumer_only() {
    let directory = temporary_directory("stage-13b-cli");
    let raster_default = directory.join("raster-default.png");
    let vector_default = directory.join("vector-default.png");
    let vector_svg = directory.join("vector-default.svg");
    for (source, output, dimensions) in [
        (
            "../../assets/raster-sample.png",
            &raster_default,
            (1024, 1024),
        ),
        (
            "../../assets/vector-sample.svg",
            &vector_default,
            (900, 620),
        ),
    ] {
        let output = direct_render_without_canvas(source, output, "rgb")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            png_dimensions(
                &fs::read(if source.ends_with("png") {
                    &raster_default
                } else {
                    &vector_default
                })
                .unwrap()
            ),
            dimensions
        );
    }
    let output = direct_render_without_canvas("../../assets/vector-sample.svg", &vector_svg, "rgb")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(&vector_svg).unwrap();
    assert!(svg.contains("width=\"900\" height=\"620\" viewBox=\"0 0 900 620\""));

    let overridden = directory.join("raster-overridden.png");
    let mut command =
        direct_render_without_canvas("../../assets/raster-sample.png", &overridden, "rgb");
    command.args(["--canvas", "320x180"]);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(png_dimensions(&fs::read(&overridden).unwrap()), (320, 180));

    let default_aa = directory.join("aa-default.png");
    let explicit_aa = directory.join("aa-on.png");
    let hard_edges = directory.join("aa-off.png");
    for (path, mode) in [
        (&default_aa, None),
        (&explicit_aa, Some("on")),
        (&hard_edges, Some("off")),
    ] {
        let mut command = direct_render_without_canvas(
            "../../assets/raster-sample.png",
            path,
            "source-color-alpha",
        );
        if let Some(mode) = mode {
            command.args(["--antialiasing", mode]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read(&default_aa).unwrap(),
        fs::read(&explicit_aa).unwrap()
    );
    assert_ne!(
        fs::read(&default_aa).unwrap(),
        fs::read(&hard_edges).unwrap()
    );
    let invalid = direct_render_without_canvas(
        "../../assets/raster-sample.png",
        &directory.join("invalid.png"),
        "rgb",
    )
    .args(["--antialiasing", "soft"])
    .output()
    .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("antialiasing"));
    let unsafe_output = directory.join("unsafe-output.png");
    let unsafe_canvas = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "--input",
            "../../assets/raster-sample.png",
            "--output",
            unsafe_output.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "67108865x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "0",
            "--size-min",
            "2",
            "--size-max",
            "2",
        ])
        .output()
        .unwrap();
    assert_eq!(unsafe_canvas.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unsafe_canvas.stderr).contains("output.target"),
        "{}",
        String::from_utf8_lossy(&unsafe_canvas.stderr)
    );
    assert!(!unsafe_output.exists(), "unsafe output must not be created");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn container_render_uses_its_stored_canvas_not_direct_source_defaulting() {
    let directory = temporary_directory("stage-13b-container");
    for fixture in ["raster-sample-v1.toniator", "vector-sample-v1.toniator"] {
        let output_path = directory.join(format!("{fixture}.png"));
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "render",
                "--input",
                &format!("../../assets/{fixture}"),
                "--output",
                output_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let loaded = load(&baseline_path(fixture)).unwrap();
        assert_eq!(
            png_dimensions(&fs::read(output_path).unwrap()),
            (
                loaded.document().canvas().width as u32,
                loaded.document().canvas().height as u32,
            )
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "toniator-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    directory
}

fn baseline_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(name)
}

fn parity_document(
    bytes: Vec<u8>,
    format: EmbeddedSourceFormat,
    width: f64,
    height: f64,
    model: HalftoneChannelModel,
) -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("source-1").unwrap();
    let layout = ChannelPatternLayout {
        density: DensityMetric2D {
            across_x: width / 10.0,
            across_y: height / 10.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
    };
    let legacy = Document::with_source(
        DocumentId(1),
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id.clone()),
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: layout.clone(),
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
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let topology = legacy
        .canonical_channel_topology(
            model,
            ChannelTopologyTemplate {
                pattern_definition_id: PatternDefinitionId(1),
                layout,
                mark_geometry_response: MarkGeometryResponse {
                    minimum_size: 2.0,
                    maximum_size: 9.0,
                },
            },
        )
        .unwrap();
    let document = Document::with_source_and_topology(
        legacy.id(),
        legacy.canvas().clone(),
        legacy.source().clone(),
        legacy.pattern_definitions().to_vec(),
        model,
        topology,
    )
    .unwrap();
    let bundle =
        SourceBundle::new([EmbeddedSource::new(source_id, format, bytes, None).unwrap()]).unwrap();
    (document, bundle)
}

fn evaluate_parity(
    document: &Document,
    source: &EmbeddedSource,
) -> toniator_engine::EvaluationResult {
    let session = DocumentSession::new(document.clone()).unwrap();
    let format = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
    };
    evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source.id().clone(), source.bytes().to_vec(), format).unwrap(),
        ),
        EvaluationLimits::default(),
    )
    .unwrap()
}

#[test]
fn direct_and_container_evaluation_are_identical_for_both_baselines_and_all_models() {
    for (baseline, format, width, height) in [
        (
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            1024.0,
            1024.0,
        ),
        ("vector-sample.svg", EmbeddedSourceFormat::Svg, 900.0, 620.0),
    ] {
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let (document, sources) = parity_document(
                fs::read(baseline_path(baseline)).unwrap(),
                format,
                width,
                height,
                model,
            );
            let direct = evaluate_parity(&document, sources.entries().next().unwrap());
            let directory = temporary_directory("stage-12-parity");
            let path = directory.join("document.toniator");
            save(&path, &document, &sources).unwrap();
            let loaded = load(&path).unwrap();
            let container = evaluate_parity(
                loaded.document(),
                loaded.sources().entries().next().unwrap(),
            );
            assert_eq!(container.source_identity(), direct.source_identity());
            assert_eq!(container.channels(), direct.channels());
            assert_eq!(container.scene().identity(), direct.scene().identity());
            assert_eq!(container.raster().pixels(), direct.raster().pixels());
            assert_eq!(write_svg(container.scene()), write_svg(direct.scene()));
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[test]
fn render_uses_authoritative_document_models_for_both_immutable_sources() {
    let directory = temporary_directory("stage-9e-models");
    for (source, source_label) in [
        ("../../assets/raster-sample.png", "raster"),
        ("../../assets/vector-sample.svg", "vector"),
    ] {
        for (model, title, roles) in [
            ("rgb", "Toniator RGB halftone", &[1, 2, 3][..]),
            ("cmyk", "Toniator CMYK halftone", &[4, 5, 6, 7][..]),
            (
                "source-color-alpha",
                "Toniator source-colored halftone",
                &[8][..],
            ),
        ] {
            let output_path = directory.join(format!("{model}-{source_label}.svg"));
            let output = render_command(source, &output_path, model)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{model}/{source_label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let svg = String::from_utf8(fs::read(&output_path).unwrap()).unwrap();
            assert!(svg.contains(title));
            for channel_id in roles {
                assert!(svg.contains(&format!("id=\"channel-{channel_id}\"")));
            }
            assert!(svg.contains("id=\"canvas\""));
            assert!(svg.contains("<circle "));
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn render_background_is_consumer_only_and_png_is_straight_srgba() {
    let directory = temporary_directory("stage-9e-background");
    let transparent = directory.join("transparent-cmyk.png");
    let black = directory.join("black.png");
    let white = directory.join("white.png");

    for model in ["rgb", "cmyk", "source-color-alpha"] {
        let output_path = directory.join(format!("transparent-{model}.png"));
        let output = render_command("../../assets/raster-sample.png", &output_path, model)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(output_path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(bytes[25], 6, "PNG must use RGBA color type");
    }

    for (path, background) in [
        (&transparent, None),
        (&black, Some("black")),
        (&white, Some("white")),
    ] {
        let mut command = render_command("../../assets/raster-sample.png", path, "cmyk");
        if let Some(background) = background {
            command.args(["--background", background]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(bytes[25], 6, "PNG must use RGBA color type");
    }
    assert_ne!(fs::read(&transparent).unwrap(), fs::read(&black).unwrap());
    assert_ne!(fs::read(&black).unwrap(), fs::read(&white).unwrap());

    let svg = directory.join("opaque.svg");
    let output = render_command("../../assets/raster-sample.png", &svg, "rgb")
        .args(["--background", "black"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("error at render.background"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn render_requires_channel_model_and_rejects_obsolete_render_options() {
    let directory = temporary_directory("stage-9e-arguments");
    let output_path = directory.join("output.png");
    let required = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "--input",
            "../../assets/raster-sample.png",
            "--output",
            output_path.to_str().unwrap(),
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "2",
            "--size-max",
            "9",
        ])
        .output()
        .unwrap();
    assert_eq!(required.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&required.stderr).contains("--channel-model"));

    for (option, value) in [
        ("--mode", "rgb"),
        ("--source-component", "luminance"),
        ("--color", "#00b7ff"),
    ] {
        let output = render_command("../../assets/raster-sample.png", &output_path, "rgb")
            .args([option, value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(option));
    }

    let invalid_extension = render_command(
        "../../assets/raster-sample.png",
        &directory.join("output.txt"),
        "rgb",
    )
    .output()
    .unwrap();
    assert_eq!(invalid_extension.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&invalid_extension.stderr)
            .contains("output extension must be .png or .svg")
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn portable_document_create_validate_render_and_argument_matrix() {
    let directory = temporary_directory("stage-12-document-cli");
    let source = directory.join("temporary-source.png");
    fs::copy("../../assets/raster-sample.png", &source).unwrap();
    let document = directory.join("created.toniator");
    let create = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            source.to_str().unwrap(),
            "-o",
            document.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1024x1024",
            "--density-x",
            "102.4",
            "--density-y",
            "102.4",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--opacity",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    assert!(document.exists());
    assert_eq!(
        fs::read_dir(&directory).unwrap().count(),
        2,
        "create must not render as a side effect"
    );
    let validated = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["validate", "-i", document.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(validated.status.success());
    assert!(
        String::from_utf8_lossy(&validated.stdout)
            .contains("container v1, document v2, migrations: empty")
    );
    let mutual = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "-i",
            document.to_str().unwrap(),
            "--canvas",
            "1x1",
        ])
        .output()
        .unwrap();
    assert_eq!(mutual.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&mutual.stdout), "");
    let moved = directory.join("moved.toniator");
    fs::rename(&document, &moved).unwrap();
    fs::remove_file(&source).unwrap();
    let rendered = directory.join("rendered.png");
    let render = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            moved.to_str().unwrap(),
            "-o",
            rendered.to_str().unwrap(),
            "--background",
            "black",
            "--max-family-candidates",
            "1048576",
        ])
        .output()
        .unwrap();
    assert!(
        render.status.success(),
        "{}",
        String::from_utf8_lossy(&render.stderr)
    );
    assert!(rendered.exists());
    let override_failure = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            moved.to_str().unwrap(),
            "-o",
            rendered.to_str().unwrap(),
            "--canvas",
            "1x1",
        ])
        .output()
        .unwrap();
    assert_eq!(override_failure.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&override_failure.stderr)
            .contains("container rendering does not accept document override flags")
    );
    let bad_create_input = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            "../../assets/README.md",
            "-o",
            directory.join("bad.toniator").to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "0",
            "--size-max",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(bad_create_input.status.code(), Some(2));
    let bad_create_output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            "../../assets/raster-sample.png",
            "-o",
            directory.join("bad.txt").to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "0",
            "--size-max",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(bad_create_output.status.code(), Some(2));
    fs::remove_dir_all(directory).unwrap();
}
