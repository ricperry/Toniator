//! Current CLI still-PNG consumer defaults, explicit policies, and prepublication preservation.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Returns fresh stage-owned CLI policy artifacts without changing immutable source inputs.
/// # Panics
/// Panics on a stage-owned filesystem failure.
fn stage() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-png-export-integration-20261013/cli-policy-tests");
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Runs the unchanged explicit direct-source construction interface with a tiny native canvas.
/// # Panics
/// Panics only if the current CLI binary cannot start.
fn render(source: &Path, output: &Path, model: &str, canvas: &str, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toniator"))
        .arg("render")
        .arg("--input")
        .arg(source)
        .arg("--output")
        .arg(output)
        .args([
            "--canvas",
            canvas,
            "--channel-model",
            model,
            "--density",
            "32",
            "--density-aspect",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
        ])
        .args(extra)
        .output()
        .unwrap()
}

/// Proves default/explicit backing and AA survive streaming for both immutable sources.
/// # Panics
/// Panics on failed CLI execution, unexpected dimensions/alpha policy, or lost AA coverage.
#[test]
fn cli_streaming_preserves_model_backing_and_explicit_antialiasing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source in ["raster-sample.png", "vector-sample.svg"] {
        for model in ["rgb", "cmyk", "source-color-alpha"] {
            let path = stage().join(format!("{source}-{model}-default.png"));
            let result = render(
                &root.join("assets").join(source),
                &path,
                model,
                "96x64",
                &["--antialiasing", "off"],
            );
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let pixels = image::open(&path).unwrap().to_rgba8();
            assert_eq!(pixels.dimensions(), (96, 64));
            if model != "source-color-alpha" {
                assert!(pixels.pixels().all(|pixel| pixel[3] == 255));
            }
            let transparent = stage().join(format!("{source}-{model}-transparent.png"));
            let result = render(
                &root.join("assets").join(source),
                &transparent,
                model,
                "96x64",
                &["--background", "transparent", "--antialiasing", "off"],
            );
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let off = image::open(transparent).unwrap().to_rgba8();
            if model == "source-color-alpha" {
                assert_eq!(
                    pixels, off,
                    "Source Color Alpha defaults to transparent backing"
                );
            } else {
                assert!(off.pixels().any(|pixel| pixel[3] == 0));
            }
            assert!(off.pixels().all(|pixel| matches!(pixel[3], 0 | 255)));
            if model == "rgb" {
                assert!(pixels.pixels().any(|pixel| pixel.0 == [0, 0, 0, 255]));
                let on_path = stage().join(format!("{source}-rgb-AA-on.png"));
                let result = render(
                    &root.join("assets").join(source),
                    &on_path,
                    model,
                    "96x64",
                    &["--background", "transparent", "--antialiasing", "on"],
                );
                assert!(result.status.success());
                let on = image::open(on_path).unwrap().to_rgba8();
                assert!(on.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
                assert!(on != off);
            }
            if model == "cmyk" {
                assert!(pixels.pixels().any(|pixel| pixel.0 == [255, 255, 255, 255]));
            }
        }
    }
}

/// Proves invalid current dimensions preserve an existing approved output before any PNG work.
/// # Panics
/// Panics if failed argument admission truncates the prior destination or reports success.
#[test]
fn cli_invalid_dimensions_preserve_existing_output() {
    let output = stage().join("approved-sentinel.png");
    fs::write(&output, b"approved prior output").unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png");
    let result = render(&source, &output, "rgb", "0x64", &[]);
    assert!(!result.status.success());
    assert_eq!(fs::read(output).unwrap(), b"approved prior output");
}
