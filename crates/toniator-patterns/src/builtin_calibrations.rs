//! Checked-in typed reference measurements for immutable bundled Pattern recipes.
//! Numeric values are reconstructed from current validated artifacts; runtime never loads JSON.

use toniator_domain::{
    CalibrationBinding, CalibrationKnot, CalibrationVariation, CanvasSpec, ChannelId, Document,
    DocumentCommand, PatternDefinitionRecipe, SavedCalibration, SourceReference,
    UniformCalibration, ValidationError,
};

type VariationData = (f64, f64, f64, f64, Option<f64>, Option<f64>);
/// Frozen complete normalized identity and measured payload; no runtime parser or lossy key is used.
type MeasuredData<'a> = (
    &'a str,
    &'a str,
    &'a [CalibrationKnot],
    Option<VariationData>,
    Option<(f64, &'a [CalibrationKnot])>,
);

#[path = "builtin_calibrations_data.rs"]
mod measured;

/// Materializes one recipe on the canonical reference document before binding it.
///
/// # Errors
/// Rejects invalid default-document construction or a recipe the domain cannot install.
fn canonical_reference_binding(
    recipe: &PatternDefinitionRecipe,
) -> Result<CalibrationBinding, ValidationError> {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )?;
    let base = document.pattern_settings().clone();
    let base_definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.id == base.definition_id)
        .ok_or_else(|| {
            ValidationError::new(
                "calibration.reference.definition",
                "default Pattern definition is missing",
            )
        })?
        .definition
        .clone();
    let (document, _) =
        document.apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })?;
    CalibrationBinding::for_channel(&document, ChannelId(1))
}

/// Restores observations only for their frozen complete normalized binding and stored producer.
///
/// The derived Debug representation is intentionally conservative: formatting drift rejects reuse
/// and requires regeneration. It is never parsed or hashed into a second identity authority.
/// Current domain constructors still own profile eligibility, monotonicity and payload validation.
/// # Errors
/// Returns invalid numeric payload diagnostics; changed binding or producer returns no companion.
pub(super) fn restore_measurement(
    binding: CalibrationBinding,
    measured: MeasuredData<'_>,
) -> Result<Option<SavedCalibration>, ValidationError> {
    let (frozen_binding, profile, raw, variation, uniform) = measured;
    if format!("{binding:?}") != frozen_binding {
        return Ok(None);
    }
    let current_profile = [
        toniator_domain::CALIBRATION_PROFILE,
        toniator_domain::CALIBRATION_PROFILE_64,
        toniator_domain::CALIBRATION_PROFILE_GUIDE_FACES,
        toniator_domain::CALIBRATION_PROFILE_GUIDE_FACES_64,
        toniator_domain::CALIBRATION_PROFILE_SEEDED,
        toniator_domain::CALIBRATION_PROFILE_SEEDED_64,
        toniator_domain::CALIBRATION_PROFILE_ROUND_INTERIOR,
        toniator_domain::CALIBRATION_PROFILE_MEAN,
    ]
    .into_iter()
    .any(|reference| binding.profile_for_reference(reference) == profile);
    if !current_profile {
        return Ok(None);
    }
    let uniform = uniform
        .map(|(radius, inverse)| UniformCalibration::new(radius, inverse.to_vec()))
        .transpose()?;
    let calibration = if let Some((
        raw_absolute,
        raw_fraction,
        tone_absolute,
        low_tone_relative,
        uniform_tone_absolute,
        uniform_low_tone_relative,
    )) = variation
    {
        if profile != binding.profile_for_reference(toniator_domain::CALIBRATION_PROFILE_MEAN) {
            return Ok(None);
        }
        let variation = CalibrationVariation::new(
            raw_absolute,
            raw_fraction,
            tone_absolute,
            low_tone_relative,
            uniform_tone_absolute,
            uniform_low_tone_relative,
        )?;
        SavedCalibration::with_mean_profile(binding, raw.to_vec(), uniform, variation)?
    } else {
        SavedCalibration::with_profile(profile.to_owned(), binding, raw.to_vec(), uniform)?
    };
    Ok(Some(calibration))
}

/// Builds a companion only when the current canonical recipe matches its frozen measurement.
/// Runtime lookup never measures, appends producer tags, or changes the immutable catalog.
/// # Errors
/// Rejects invalid canonical materialization or a malformed checked-in numeric payload.
pub(super) fn calibration_for_recipe(
    id: &str,
    recipe: &PatternDefinitionRecipe,
) -> Result<Option<SavedCalibration>, ValidationError> {
    let Some(measured) = measured::data(id) else {
        return Ok(None);
    };
    restore_measurement(canonical_reference_binding(recipe)?, measured)
}
