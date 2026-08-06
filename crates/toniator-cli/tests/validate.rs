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
    assert!(json["sites"].as_array().expect("sites array").len() > 10_000);
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

#[test]
fn render_infers_png_and_svg_and_rejects_invalid_output_extensions() {
    let directory = std::env::temp_dir().join(format!(
        "toniator-stage-5-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for extension in ["png", "svg"] {
        let output_path = directory.join(format!("output.{extension}"));
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "render",
                "--source",
                "../../assets/raster-sample.png",
                "--output",
                output_path.to_str().unwrap(),
                "--mode",
                "rgb",
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
                "--transparent",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(&output_path).unwrap();
        assert!(!bytes.is_empty());
        if extension == "svg" {
            assert!(String::from_utf8(bytes).unwrap().contains("<circle "));
        }
    }
    let invalid = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "--source",
            "../../assets/raster-sample.png",
            "--output",
            directory.join("output.txt").to_str().unwrap(),
            "--mode",
            "rgb",
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
            "--support-radius",
            "4.5",
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
        ])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("output extension must be .png or .svg")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn render_rejects_missing_extension_and_invalid_presentation_options() {
    let directory = std::env::temp_dir().join(format!(
        "toniator-stage-5-errors-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for (output_name, color, opacity, expected) in [
        (
            "missing-extension",
            "#00b7ff",
            "0.72",
            "output extension must be .png or .svg",
        ),
        (
            "invalid-opacity.png",
            "#00b7ff",
            "1.1",
            "opacity must be within 0.0..=1.0",
        ),
        ("invalid-color.png", "00b7ff", "0.72", "expected #RRGGBB"),
    ] {
        let output_path = directory.join(output_name);
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "render",
                "--source",
                "../../assets/raster-sample.png",
                "--output",
                output_path.to_str().unwrap(),
                "--mode",
                "rgb",
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
                "--support-radius",
                "4.5",
                "--source-component",
                "luminance",
                "--size-min",
                "2",
                "--size-max",
                "9",
                "--color",
                color,
                "--opacity",
                opacity,
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
    fs::remove_dir_all(directory).unwrap();
}
