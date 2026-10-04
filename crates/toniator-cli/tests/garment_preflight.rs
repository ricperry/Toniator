//! Current CLI witnesses for bounded garment print-preflight JSON and its authority boundary.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const PROJECT_BYTES: &[u8] =
    include_bytes!("../../toniator-io/tests/fixtures/garment-schema10-clean-fade.toniator");

/// Creates a private project path without touching tracked or user-owned input bytes.
struct Scratch(PathBuf);

impl Scratch {
    /// Allocates one unique temporary directory for a project and its CLI evidence.
    ///
    /// # Panics
    /// Panics when the system clock or temporary filesystem cannot create the directory.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "toniator-cli-garment-preflight-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock follows the Unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("private test directory creates");
        Self(path)
    }

    /// Writes the immutable schema-10 clean-fade witness under a non-required extension.
    ///
    /// # Panics
    /// Panics when the test fixture cannot be copied into its private directory.
    fn project(&self) -> PathBuf {
        let path = self.0.join("current-project.input");
        fs::write(&path, PROJECT_BYTES).expect("fixture project writes");
        path
    }
}

impl Drop for Scratch {
    /// Removes only files created inside this test's private temporary directory.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs one project-only preflight process with additional exact CLI arguments.
fn run(input: &Path, extra: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command
        .arg("preflight")
        .arg("--input")
        .arg(input)
        .args(extra);
    command.output().expect("preflight CLI process starts")
}

/// Parses one successful or explicit-unavailable JSON document from standard output.
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("CLI writes one JSON value to stdout")
}

/// Asserts a JSON floating-point value within the conversion's numerical roundoff.
fn assert_close(value: &serde_json::Value, expected: f64) {
    let actual = value.as_f64().expect("measurement value is numeric");
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "{actual} differs from {expected}"
    );
}

/// Proves deterministic complete inventory, stable metadata, and read-only project authority.
#[test]
fn default_inventory_is_deterministic_and_keeps_project_bytes_unchanged() {
    let scratch = Scratch::new();
    let project = scratch.project();
    let before = fs::read(&project).expect("project bytes read before preflight");
    let first = run(&project, &[]);
    let second = run(&project, &[]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let value = json(&first);
    assert_eq!(value["report_format_version"], 1);
    assert_eq!(value["algorithm_id"], "toniator-garment-preflight-g1b-v1");
    assert_eq!(value["status"], "completed_advisory");
    assert_eq!(
        value["analysis_scope"]["name"],
        "final-transparent-composition-alpha"
    );
    assert_eq!(
        value["analysis_scope"]["channel_separations_analyzed"],
        false
    );
    assert_eq!(value["identity"]["antialiasing"], "on");
    assert_eq!(
        value["measurement"]["placement_mm"],
        serde_json::Value::Null
    );
    assert_eq!(value["measurement"]["positive_feature_threshold_mm"], 0.0);
    assert_eq!(value["measurement"]["negative_gap_threshold_mm"], 0.0);
    assert_eq!(
        value["alpha"]["positive_feature_check"]["status"],
        "disabled"
    );
    assert_eq!(value["alpha"]["negative_gap_check"]["status"], "disabled");
    let histogram = value["alpha"]["histogram_by_alpha"]
        .as_array()
        .expect("full histogram is an array");
    assert_eq!(histogram.len(), 256);
    let histogram_pixels = histogram
        .iter()
        .map(|count| count.as_u64().expect("histogram count is unsigned"))
        .sum::<u64>();
    let target_pixels = value["identity"]["target"]["width"].as_u64().unwrap()
        * value["identity"]["target"]["height"].as_u64().unwrap();
    assert_eq!(histogram_pixels, target_pixels);
    assert!(
        !value["alpha"]["support_components"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read(&project).unwrap(), before);
}

/// Proves inch overrides convert to canonical millimetres and remain session-local.
#[test]
fn runtime_measurement_overrides_convert_and_do_not_persist() {
    let scratch = Scratch::new();
    let project = scratch.project();
    let before = fs::read(&project).expect("project bytes read before preflight");
    let overridden = run(
        &project,
        &[
            "--unit",
            "in",
            "--print-width",
            "4",
            "--print-height",
            "1.5",
            "--positive-threshold",
            "0.05",
            "--gap-threshold",
            "0",
        ],
    );
    assert!(
        overridden.status.success(),
        "{}",
        String::from_utf8_lossy(&overridden.stderr)
    );
    let report = json(&overridden);
    assert_eq!(report["measurement"]["entry_unit"], "in");
    assert_close(&report["measurement"]["placement_mm"]["width"], 101.6);
    assert_close(&report["measurement"]["placement_mm"]["height"], 38.1);
    assert_close(
        &report["measurement"]["positive_feature_threshold_mm"],
        1.27,
    );
    assert_eq!(report["measurement"]["negative_gap_threshold_mm"], 0.0);
    assert_eq!(report["identity"]["document"]["revision"], 1);
    assert!(report["measurement"]["ppi_axes"]["horizontal"].is_number());

    let default_again = run(&project, &[]);
    assert!(default_again.status.success());
    let default_report = json(&default_again);
    assert_eq!(
        default_report["measurement"]["placement_mm"],
        serde_json::Value::Null
    );
    assert_eq!(
        default_report["measurement"]["positive_feature_threshold_mm"],
        0.0
    );
    assert_eq!(default_report["identity"]["document"]["revision"], 0);
    assert_eq!(fs::read(&project).unwrap(), before);
}

/// Proves invalid target and physical values fail before publishing any report bytes.
#[test]
fn invalid_zero_nan_and_incomplete_measurements_are_rejected() {
    let scratch = Scratch::new();
    let project = scratch.project();
    for arguments in [
        vec!["--output-size", "0x96"],
        vec!["--print-width", "0", "--print-height", "1"],
        vec!["--print-width", "NaN", "--print-height", "1"],
        vec!["--print-width", "4"],
        vec!["--positive-threshold", "NaN"],
    ] {
        let result = run(&project, &arguments);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}

/// Proves backing metadata changes without affecting the transparent-alpha inventory.
#[test]
fn selected_background_is_metadata_only_for_alpha_findings() {
    let scratch = Scratch::new();
    let project = scratch.project();
    let black = run(&project, &["--background", "black"]);
    let white = run(&project, &["--background", "white"]);
    let transparent = run(&project, &["--background", "transparent"]);
    assert!(black.status.success());
    assert!(white.status.success());
    assert!(transparent.status.success());
    let black_json = json(&black);
    let white_json = json(&white);
    let transparent_json = json(&transparent);
    assert_eq!(black_json["alpha"], white_json["alpha"]);
    assert_eq!(black_json["alpha"], transparent_json["alpha"]);
    assert_eq!(
        black_json["identity"]["alpha_sha256"],
        white_json["identity"]["alpha_sha256"]
    );
    assert_eq!(black_json["export"]["selected_background"], "black");
    assert_eq!(white_json["export"]["selected_background"], "white");
    assert_eq!(transparent_json["export"]["flattens_transparency"], false);
    assert_eq!(black_json["export"]["flattens_transparency"], true);
}

/// Proves explicit bounded failure is JSON-visible and contains no partial findings.
#[test]
fn pixel_limit_reports_unavailable_without_partial_findings() {
    let scratch = Scratch::new();
    let project = scratch.project();
    let result = run(&project, &["--max-pixels", "1"]);
    assert_eq!(result.status.code(), Some(2));
    let value = json(&result);
    assert_eq!(value["report_format_version"], 1);
    assert_eq!(value["status"], "unavailable");
    assert_eq!(value["limits"]["max_pixels"], 1);
    assert_eq!(value["report"], serde_json::Value::Null);
    assert!(value.get("alpha").is_none());
    assert!(!value["reason"].as_str().unwrap().is_empty());
}

/// Proves frame, target, antialiasing, background, and default model policy enter identity.
#[test]
fn resolved_output_selection_is_reported_exactly() {
    let scratch = Scratch::new();
    let project = scratch.project();
    let result = run(
        &project,
        &[
            "--frame",
            "0",
            "--output-size",
            "512x192",
            "--antialiasing",
            "off",
            "--background",
            "white",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value = json(&result);
    assert_eq!(value["identity"]["frame"], 0);
    assert_eq!(value["identity"]["target"]["width"], 512);
    assert_eq!(value["identity"]["target"]["height"], 192);
    assert_eq!(value["identity"]["antialiasing"], "off");
    assert_eq!(value["identity"]["selected_background"], "white");
    assert_eq!(
        value["identity"]["alpha_sha256"].as_str().unwrap().len(),
        64
    );
}
