use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Compiles every tracked Blueprint resource and registers the resulting GTK bundle for the app.
///
/// Cargo reruns this build script when a listed resource changes. Missing `OUT_DIR`, a missing
/// Blueprint compiler, a missing MSVC resource compiler, failed resource compilation, or staging
/// failure aborts the build so runtime templates and native Windows identity remain intact.
///
/// # Panics
///
/// Panics when Cargo does not supply `OUT_DIR`, a required Blueprint or Windows MSVC resource
/// compiler cannot start or fails, CSS staging fails, or GResource compilation cannot read a
/// listed resource such as an adopted icon.
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
    println!("cargo:rerun-if-changed=../../assets/appicon.png");
    println!("cargo:rerun-if-changed=resources/toniator.rc");
    println!("cargo:rerun-if-changed=../../assets/appicon.ico");
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
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        embed_windows_application_icon(&out_dir);
    }

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

/// Compiles the product icon into the Windows executable through the active MSVC resource compiler.
///
/// The ICO is derived from the existing package artwork. This resource gives Windows shells a
/// native executable icon; the matching PNG remains in GTK's resource bundle for window surfaces.
/// The function adds no GTK or Windows API dependency to the app crate.
///
/// # Panics
///
/// Panics when Cargo does not supply the app manifest directory, `rc.exe` cannot start, or the
/// resource compiler cannot embed the checked-in icon.
fn embed_windows_application_icon(out_dir: &Path) {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let resource_dir = manifest_dir.join("resources");
    let output = out_dir.join("toniator-app-icon.res");
    let status = Command::new("rc.exe")
        .args(["/nologo", "/fo"])
        .arg(&output)
        .arg("toniator.rc")
        .current_dir(resource_dir)
        .status()
        .expect("rc.exe is required to embed the Toniator Windows application icon");
    assert!(
        status.success(),
        "rc.exe failed to compile resources/toniator.rc"
    );

    println!("cargo:rustc-link-arg-bin=toniator-app={}", output.display());
}
