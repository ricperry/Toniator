use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=resources/window.blp");
    println!("cargo:rerun-if-changed=resources/toniator.css");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    let window_ui = out_dir.join("window.ui");
    let blueprint_status = Command::new("blueprint-compiler")
        .args(["compile", "resources/window.blp", "--output"])
        .arg(&window_ui)
        .status()
        .expect("blueprint-compiler is required to build toniator-app; install blueprint-compiler");
    assert!(
        blueprint_status.success(),
        "blueprint-compiler failed while compiling resources/window.blp"
    );

    fs::copy("resources/toniator.css", out_dir.join("toniator.css"))
        .expect("failed to stage Toniator CSS resource");
    let manifest = out_dir.join("toniator.gresource.xml");
    fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/com/silentbutdigital/Toniator">
    <file preprocess="xml-stripblanks">window.ui</file>
    <file>toniator.css</file>
  </gresource>
</gresources>
"#,
    )
    .expect("failed to write generated GResource manifest");
    glib_build_tools::compile_resources(
        &[out_dir],
        manifest
            .to_str()
            .expect("generated GResource manifest path must be UTF-8"),
        "toniator.gresource",
    );
}
