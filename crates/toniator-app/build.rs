use std::{env, fs, path::PathBuf, process::Command};

/// Compiles every tracked Blueprint resource and registers the resulting GTK bundle for the app.
///
/// Cargo reruns this build script when a listed resource changes. Missing `OUT_DIR`, a missing
/// Blueprint compiler, failed resource compilation, or staging failure aborts the build so runtime
/// template loading cannot silently fall back to incomplete presentation assets.
///
/// # Panics
///
/// Panics when Cargo does not supply `OUT_DIR`, Blueprint compilation cannot start or fails, CSS
/// staging fails, or GResource compilation cannot read a listed resource such as an adopted icon.
fn main() {
    for blueprint in [
        "resources/window.blp",
        "resources/channel-editor.blp",
        "resources/pattern-editor.blp",
        "resources/advanced-settings.blp",
        "resources/pattern-wizard.blp",
        "resources/pattern-wizard-card.blp",
        "resources/preset-row.blp",
        "resources/confirmation-dialog.blp",
        "resources/png-export-options.blp",
    ] {
        println!("cargo:rerun-if-changed={blueprint}");
    }
    println!("cargo:rerun-if-changed=resources/toniator.css");
    println!("cargo:rerun-if-changed=../../assets/Stage21D_Mockup/SplashMockup.png");
    println!("cargo:rerun-if-changed=resources/toniator.gresource.xml");
    println!("cargo:rerun-if-changed=../../assets/stage20s-preset-icon-source.svg");
    for icon in [
        "clustered-dispersion-random-links",
        "curve-motif-rows",
        "even-random-circles",
        "grid-voronoi-scale",
        "one-guide-lines",
        "residual-sites-along-guide",
        "round-spiral-line",
        "round-spiral-marks",
        "source-weighted-dispersion-voronoi",
        "square-spiral-marks",
        "straight-grid-circles",
        "three-guide-cells-scale",
        "three-guide-maze",
        "triagrid-custom-shape-marks",
        "triagrid-spanning-tree",
        "two-guide-cells-uniform-offset",
        "two-guide-maze",
    ] {
        println!("cargo:rerun-if-changed=../../assets/stage20s-preset-icons/{icon}.svg");
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    for blueprint in [
        "window.blp",
        "channel-editor.blp",
        "pattern-editor.blp",
        "advanced-settings.blp",
        "pattern-wizard.blp",
        "pattern-wizard-card.blp",
        "preset-row.blp",
        "confirmation-dialog.blp",
        "png-export-options.blp",
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
