use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    for blueprint in [
        "resources/window.blp",
        "resources/channel-editor.blp",
        "resources/pattern-editor.blp",
        "resources/preset-row.blp",
        "resources/confirmation-dialog.blp",
    ] {
        println!("cargo:rerun-if-changed={blueprint}");
    }
    println!("cargo:rerun-if-changed=resources/toniator.css");
    println!("cargo:rerun-if-changed=resources/toniator.gresource.xml");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    for blueprint in [
        "window.blp",
        "channel-editor.blp",
        "pattern-editor.blp",
        "preset-row.blp",
        "confirmation-dialog.blp",
    ] {
        let output = out_dir.join(blueprint.replace(".blp", ".ui"));
        let source = format!("resources/{blueprint}");
        let blueprint_status = Command::new("blueprint-compiler")
            .args(["compile", &source, "--output"])
            .arg(&output)
            .status()
            .expect(
                "blueprint-compiler is required to build toniator-app; install blueprint-compiler",
            );
        assert!(
            blueprint_status.success(),
            "blueprint-compiler failed while compiling {source}"
        );
    }

    fs::copy("resources/toniator.css", out_dir.join("toniator.css"))
        .expect("failed to stage Toniator CSS resource");
    let manifest = "resources/toniator.gresource.xml";
    glib_build_tools::compile_resources(
        &[out_dir, PathBuf::from("resources")],
        manifest,
        "toniator.gresource",
    );
}
