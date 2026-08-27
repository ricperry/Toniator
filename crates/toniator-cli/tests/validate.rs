//! Current headless CLI integration witnesses.
//!
//! Superseded schema migration and historical low-resolution evidence generation are intentionally
//! excluded. These tests exercise the current document, descriptor, inspect, rendering, limit,
//! and argument boundaries against both immutable project sources.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

/// Allocates one collision-resistant disposable directory for CLI artifacts.
fn temporary_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "toniator-current-cli-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("temporary directory creates");
    path
}

/// Runs the Toniator binary with exact arguments and returns its complete process result.
fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(arguments)
        .output()
        .expect("CLI process starts")
}

/// Builds one direct-source render command with the retained current headless controls.
fn direct_render(source: &str, output: &Path, model: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command.args([
        "render",
        "--input",
        source,
        "--output",
        output.to_str().expect("temporary path is UTF-8"),
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
        "--fill-min",
        "0.2",
        "--fill-max",
        "0.9",
    ]);
    command
}

/// Reads intrinsic dimensions from the fixed PNG signature and IHDR fields.
fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

/// Proves current ad-hoc validation succeeds and invalid density rejects atomically.
#[test]
fn validate_reports_current_success_and_stable_density_failure() {
    let valid = run(&[
        "validate",
        "--canvas",
        "900x600",
        "--density-x",
        "90",
        "--density-y",
        "60",
        "--opacity",
        "0.75",
    ]);
    assert!(valid.status.success());
    assert!(String::from_utf8_lossy(&valid.stdout).contains("valid document"));
    assert!(valid.stderr.is_empty());

    let invalid = run(&[
        "validate",
        "--canvas",
        "900x600",
        "--density-x",
        "0",
        "--density-y",
        "60",
        "--opacity",
        "0.75",
    ]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&invalid.stderr)
            .contains("channel.pattern.layout.density.across_x")
    );
    assert!(invalid.stdout.is_empty());
}

/// Proves the schema-derived capability surface is complete, deterministic, and presentation-free.
#[test]
fn capabilities_are_deterministic_current_descriptor_output() {
    let first = run(&["capabilities", "--canvas", "900x600"]);
    let second = run(&["capabilities", "--canvas", "900x600"]);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(text.starts_with("capabilities-v1\tcount="));
    assert!(text.lines().skip(1).all(|line| {
        line.contains("field=")
            && line.contains("target=")
            && line.contains("command=")
            && line.contains("invalidation=")
    }));
    assert!(text.contains("ColorRed"));
    assert!(text.contains("ModeledMappingGain"));
}

/// Proves grid inspection accepts negative placement and emits stable ordered JSON.
#[test]
fn inspect_grid_is_deterministic_with_negative_offsets() {
    let args = [
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
        "--format",
        "json",
    ];
    let first = run(&args);
    let second = run(&args);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert!(json["coverage"][0]["first_index"].as_i64().unwrap() < 0);
    assert!(json["coverage"][1]["last_index"].as_i64().unwrap() > 0);
    assert_eq!(json["sites"].as_array().unwrap().len(), 6_185);
}

/// Proves mark summaries remain deterministic for both immutable source decoders.
#[test]
fn inspect_marks_is_deterministic_for_png_and_svg_sources() {
    for source in [
        "../../assets/raster-sample.png",
        "../../assets/vector-sample.svg",
    ] {
        let args = [
            "inspect",
            "marks",
            "--source",
            source,
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
            "7.0",
            "--source-component",
            "luminance",
            "--fill-min",
            "0.2",
            "--fill-max",
            "0.9",
            "--color",
            "#00b7ff",
            "--opacity",
            "0.72",
            "--summary",
            "--format",
            "json",
        ];
        let first = run(&args);
        let second = run(&args);
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(first.stdout, second.stdout);
        let json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
        assert!(json["marks"]["total"].as_u64().unwrap_or_default() > 0);
    }
}

/// Proves every evaluation command exposes the same candidate-limit rejection boundary.
#[test]
fn evaluation_commands_reject_tiny_candidate_limits() {
    let grid = run(&[
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
    ]);
    assert_eq!(grid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&grid.stderr).contains("coverage.candidate_limit"));

    let directory = temporary_directory("limits");
    let output = directory.join("limited.png");
    let rendered = direct_render("../../assets/raster-sample.png", &output, "rgb")
        .args(["--canvas", "900x600", "--max-family-candidates", "1"])
        .output()
        .unwrap();
    assert_eq!(rendered.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&rendered.stderr).contains("coverage.candidate_limit"));
    assert!(!output.exists());
    fs::remove_dir_all(directory).unwrap();
}

/// Proves direct PNG and SVG rendering defaults to each immutable source's intrinsic canvas.
#[test]
fn direct_render_preserves_intrinsic_dimensions_and_native_formats() {
    let directory = temporary_directory("intrinsic");
    let raster = directory.join("raster.png");
    let vector_png = directory.join("vector.png");
    let vector_svg = directory.join("vector.svg");
    for (source, output, dimensions) in [
        ("../../assets/raster-sample.png", &raster, (1024, 1024)),
        ("../../assets/vector-sample.svg", &vector_png, (900, 620)),
    ] {
        let result = direct_render(source, output, "rgb").output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = fs::read(output).unwrap();
        assert_eq!(png_dimensions(&bytes), dimensions);
        assert_eq!(bytes[25], 6, "native PNG preserves RGBA");
    }
    let result = direct_render("../../assets/vector-sample.svg", &vector_svg, "rgb")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let svg = fs::read_to_string(&vector_svg).unwrap();
    assert!(svg.contains("width=\"900\" height=\"620\" viewBox=\"0 0 900 620\""));
    fs::remove_dir_all(directory).unwrap();
}

/// Proves antialiasing and background remain output-only PNG controls with strict SVG rejection.
#[test]
fn raster_consumer_options_do_not_change_authoritative_geometry_interface() {
    let directory = temporary_directory("consumer-options");
    let default = directory.join("default.png");
    let explicit = directory.join("explicit.png");
    let hard = directory.join("hard.png");
    for (path, antialiasing) in [
        (&default, None),
        (&explicit, Some("on")),
        (&hard, Some("off")),
    ] {
        let mut command =
            direct_render("../../assets/raster-sample.png", path, "source-color-alpha");
        if let Some(value) = antialiasing {
            command.args(["--antialiasing", value]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read(&default).unwrap(), fs::read(&explicit).unwrap());
    assert_ne!(fs::read(&default).unwrap(), fs::read(&hard).unwrap());

    let svg = directory.join("invalid.svg");
    let invalid = direct_render("../../assets/raster-sample.png", &svg, "rgb")
        .args(["--background", "black"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("render.background"));
    assert!(!svg.exists());
    fs::remove_dir_all(directory).unwrap();
}

/// Proves current document creation is side-effect-free, portable, valid, and renderable.
#[test]
fn document_create_validate_and_render_current_container() {
    let directory = temporary_directory("document");
    let source = directory.join("source.png");
    fs::copy("../../assets/raster-sample.png", &source).unwrap();
    let document = directory.join("created.toniator");
    let create = run(&[
        "document",
        "create",
        "--input",
        source.to_str().unwrap(),
        "--output",
        document.to_str().unwrap(),
        "--channel-model",
        "rgb",
        "--canvas",
        "320x180",
        "--density-x",
        "32",
        "--density-y",
        "18",
        "--rotation",
        "0",
        "--offset-x",
        "0",
        "--offset-y",
        "0",
        "--guard-steps",
        "2",
        "--fill-min",
        "0.2",
        "--fill-max",
        "0.9",
        "--opacity",
        "1",
    ]);
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    assert!(document.exists());
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);

    let validated = run(&["validate", "--input", document.to_str().unwrap()]);
    assert!(validated.status.success());
    assert!(String::from_utf8_lossy(&validated.stdout).contains("document v5"));

    let capabilities = run(&["capabilities", "--input", document.to_str().unwrap()]);
    assert!(capabilities.status.success());
    assert!(String::from_utf8_lossy(&capabilities.stdout).contains("ModeledMappingComponent"));

    let rendered = directory.join("rendered.png");
    let output = run(&[
        "render",
        "--input",
        document.to_str().unwrap(),
        "--output",
        rendered.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(png_dimensions(&fs::read(rendered).unwrap()), (320, 180));
    fs::remove_dir_all(directory).unwrap();
}

/// Proves direct rendering requires a channel model and rejects retired option aliases.
#[test]
fn render_argument_surface_rejects_obsolete_options() {
    let directory = temporary_directory("arguments");
    let output = directory.join("output.png");
    let missing_model = run(&[
        "render",
        "--input",
        "../../assets/raster-sample.png",
        "--output",
        output.to_str().unwrap(),
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
        "--fill-min",
        "0.2",
        "--fill-max",
        "0.9",
    ]);
    assert_eq!(missing_model.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_model.stderr).contains("--channel-model"));

    for (option, value) in [
        ("--mode", "rgb"),
        ("--source-component", "luminance"),
        ("--color", "#00b7ff"),
    ] {
        let result = direct_render("../../assets/raster-sample.png", &output, "rgb")
            .args([option, value])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains(option));
    }
    fs::remove_dir_all(directory).unwrap();
}
