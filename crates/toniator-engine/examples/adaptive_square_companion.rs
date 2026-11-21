//! Reproduces the genuine discrete calibration companion from current canonical geometry.
use std::{error::Error, fmt::Write as _, fs, path::PathBuf};
use toniator_engine::{
    EvaluationLimits,
    reference_calibration::{calibrate_saved_pattern, reference_document},
};

/// Measures one current binding and writes exact typed observations to a caller-owned directory.
/// # Errors
/// Propagates argument, filesystem, domain, canonical measurement and formatting diagnostics.
fn main() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: adaptive_square_companion OUTPUT_DIRECTORY")?,
    );
    fs::create_dir_all(&out)?;
    let mut recipe = toniator_patterns::adaptive_square_marks_recipe();
    if std::env::args().nth(2).as_deref() == Some("blue-noise") {
        let toniator_domain::PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut recipe.structure else { unreachable!("adaptive recipe owns ordered outputs") };
        outputs[0] = toniator_domain::PatternOutputRealizationRecipe::AdaptiveSquareBlueNoiseMarks;
    }
    let (document, _, channel) = reference_document(&recipe, 32)?;
    let saved = calibrate_saved_pattern(
        &document,
        channel,
        EvaluationLimits::new(2_000_000)?,
        &|| false,
    )?;
    let mut data = format!(
        "//! Genuine canonical discrete response; regenerate with adaptive_square_companion.\nuse super::MeasuredData;\nuse toniator_domain::CalibrationKnot;\n/// Returns the frozen complete binding and unchanged reference observations.\npub(super) fn data(id: &str) -> Option<MeasuredData<'static>> {{\n    if id != \"adaptive-shape-marks\" {{ return None; }}\n    Some((\n        {:?},\n        {:?},\n        &[\n",
        format!("{:?}", saved.binding()),
        saved.profile()
    );
    for knot in saved.raw() {
        writeln!(
            data,
            "            CalibrationKnot {{ input: {:?}, coverage: {:?} }},",
            knot.input, knot.coverage
        )?;
    }
    data.push_str("        ],\n        None,\n        None,\n    ))\n}\n");
    fs::write(out.join("adaptive_square_calibrations_data.rs"), data)?;
    fs::write(out.join("measurement.txt"), format!("{saved:#?}\n"))?;
    println!("{}", out.display());
    Ok(())
}
