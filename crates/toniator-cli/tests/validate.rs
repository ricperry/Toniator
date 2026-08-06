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
