use std::process::Command;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

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
