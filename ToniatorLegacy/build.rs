use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_tool(tool: &str, args: &[&str]) {
    let output = Command::new(tool)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("could not run {tool}: {error}"));
    if !output.status.success() {
        panic!(
            "{tool} {:?} failed:\n{}{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/toniator.gresource.xml");

    let sources = [
        ("resources/toniator-window.blp", "toniator-window.ui"),
        (
            "resources/toniator-channel-controls.blp",
            "toniator-channel-controls.ui",
        ),
        (
            "resources/toniator-aggregate-channel-controls.blp",
            "toniator-aggregate-channel-controls.ui",
        ),
    ];
    for (source, output) in sources {
        println!("cargo:rerun-if-changed={source}");
        let output_path =
            PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join(output);
        let output_string = output_path.to_string_lossy().into_owned();
        run_tool(
            "blueprint-compiler",
            &["compile", source, "--output", &output_string],
        );
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let resource_manifest = Path::new("resources/toniator.gresource.xml");
    let target = out_dir.join("toniator.gresource");
    let target_string = target.to_string_lossy().into_owned();
    let out_dir_string = out_dir.to_string_lossy().into_owned();
    run_tool(
        "glib-compile-resources",
        &[
            resource_manifest
                .to_str()
                .expect("resource manifest is UTF-8"),
            "--sourcedir",
            &out_dir_string,
            "--target",
            &target_string,
        ],
    );
}
