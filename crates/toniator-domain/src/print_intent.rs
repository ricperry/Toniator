//! Project-owned physical print intent, independent of source and render geometry.

use crate::ValidationError;

const MILLIMETRES_PER_INCH: f64 = 25.4;

/// Explicit physical placement of the complete exported canvas in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalPrintSizeMm {
    width_mm: f64,
    height_mm: f64,
}

impl PhysicalPrintSizeMm {
    /// Creates canonical millimetre placement without inferring a size from canvas pixels.
    ///
    /// # Errors
    /// Rejects zero, negative, or non-finite dimensions.
    pub fn new(width_mm: f64, height_mm: f64) -> Result<Self, ValidationError> {
        validate_positive(width_mm, "print.size.width_mm")?;
        validate_positive(height_mm, "print.size.height_mm")?;
        Ok(Self {
            width_mm,
            height_mm,
        })
    }

    /// Converts explicit inch entry to canonical millimetres without persisting a display unit.
    ///
    /// # Errors
    /// Rejects zero, negative, non-finite, overflowing, or underflowing dimensions.
    pub fn from_inches(width_in: f64, height_in: f64) -> Result<Self, ValidationError> {
        validate_positive(width_in, "print.size.width_in")?;
        validate_positive(height_in, "print.size.height_in")?;
        let width_mm =
            checked_positive_result(width_in * MILLIMETRES_PER_INCH, "print.size.width_mm")?;
        let height_mm =
            checked_positive_result(height_in * MILLIMETRES_PER_INCH, "print.size.height_mm")?;
        Self::new(width_mm, height_mm)
    }

    /// Returns physical millimetre dimensions in horizontal and vertical order.
    pub const fn millimetres(self) -> (f64, f64) {
        (self.width_mm, self.height_mm)
    }

    /// Converts the canonical millimetre placement for inch display only.
    ///
    /// # Errors
    /// Rejects a dimension that underflows to zero in inch representation.
    pub fn inches(self) -> Result<(f64, f64), ValidationError> {
        Ok((
            checked_positive_result(self.width_mm / MILLIMETRES_PER_INCH, "print.size.width_in")?,
            checked_positive_result(
                self.height_mm / MILLIMETRES_PER_INCH,
                "print.size.height_in",
            )?,
        ))
    }

    /// Computes effective horizontal and vertical PPI from the selected final output target.
    ///
    /// The placement stays fixed when output pixels change; unequal axes remain separate.
    ///
    /// # Errors
    /// Rejects zero pixel dimensions and non-finite or underflowing results.
    pub fn ppi_for_pixels(
        self,
        width_px: u32,
        height_px: u32,
    ) -> Result<PrintResolutionPpi, ValidationError> {
        validate_pixels(width_px, height_px)?;
        let horizontal = checked_positive_result(
            f64::from(width_px) * MILLIMETRES_PER_INCH / self.width_mm,
            "print.ppi.horizontal",
        )?;
        let vertical = checked_positive_result(
            f64::from(height_px) * MILLIMETRES_PER_INCH / self.height_mm,
            "print.ppi.vertical",
        )?;
        Ok(PrintResolutionPpi {
            horizontal,
            vertical,
        })
    }
}

/// Final-output pixel density along each physical placement axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintResolutionPpi {
    horizontal: f64,
    vertical: f64,
}

impl PrintResolutionPpi {
    /// Returns effective horizontal and vertical pixels per inch in axis order.
    pub const fn axes(self) -> (f64, f64) {
        (self.horizontal, self.vertical)
    }
}

/// Project-owned placement and independent artist-chosen advisory width limits.
///
/// Zero disables the corresponding future width warning. Unknown placement leaves
/// physical analysis unavailable while retaining separately entered thresholds.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintPreparationSettings {
    size_mm: Option<PhysicalPrintSizeMm>,
    minimum_positive_feature_width_mm: f64,
    minimum_negative_gap_width_mm: f64,
}

impl Default for PrintPreparationSettings {
    /// Starts with unknown placement and both independent checks disabled.
    fn default() -> Self {
        Self {
            size_mm: None,
            minimum_positive_feature_width_mm: 0.0,
            minimum_negative_gap_width_mm: 0.0,
        }
    }
}

impl PrintPreparationSettings {
    /// Converts a nonnegative inch threshold to canonical millimetres, preserving disabled zero.
    ///
    /// # Errors
    /// Rejects negative, non-finite, overflowing, or positive-underflowing values.
    pub fn threshold_mm_from_inches(inches: f64) -> Result<f64, ValidationError> {
        validate_nonnegative(inches, "print.threshold.inches")?;
        let value = inches * MILLIMETRES_PER_INCH;
        if inches == 0.0 {
            Ok(0.0)
        } else {
            checked_positive_result(value, "print.threshold.mm")
        }
    }

    /// Converts a nonnegative millimetre threshold for inch display, preserving disabled zero.
    ///
    /// # Errors
    /// Rejects negative, non-finite, or positive-underflowing values.
    pub fn threshold_inches_from_mm(millimetres: f64) -> Result<f64, ValidationError> {
        validate_nonnegative(millimetres, "print.threshold.mm")?;
        let value = millimetres / MILLIMETRES_PER_INCH;
        if millimetres == 0.0 {
            Ok(0.0)
        } else {
            checked_positive_result(value, "print.threshold.inches")
        }
    }

    /// Validates one complete project setting, canonicalizing negative zero thresholds.
    ///
    /// # Errors
    /// Rejects negative or non-finite thresholds and invalid physical dimensions.
    pub fn new(
        size_mm: Option<PhysicalPrintSizeMm>,
        minimum_positive_feature_width_mm: f64,
        minimum_negative_gap_width_mm: f64,
    ) -> Result<Self, ValidationError> {
        validate_nonnegative(
            minimum_positive_feature_width_mm,
            "print.threshold.positive_width_mm",
        )?;
        validate_nonnegative(
            minimum_negative_gap_width_mm,
            "print.threshold.negative_gap_mm",
        )?;
        let result = Self {
            size_mm,
            minimum_positive_feature_width_mm: normalize_zero(minimum_positive_feature_width_mm),
            minimum_negative_gap_width_mm: normalize_zero(minimum_negative_gap_width_mm),
        };
        result.validate()?;
        Ok(result)
    }

    /// Returns the explicit physical placement or None when the artist has not set one.
    pub const fn size_mm(&self) -> Option<PhysicalPrintSizeMm> {
        self.size_mm
    }

    /// Returns the positive feature width warning threshold; zero means disabled.
    pub const fn minimum_positive_feature_width_mm(&self) -> f64 {
        self.minimum_positive_feature_width_mm
    }

    /// Returns the negative gap width warning threshold; zero means disabled.
    pub const fn minimum_negative_gap_width_mm(&self) -> f64 {
        self.minimum_negative_gap_width_mm
    }

    /// Reports PPI for final output dimensions, or unavailable when placement is unknown.
    ///
    /// # Errors
    /// Rejects zero pixel dimensions or a non-representable physical result.
    pub fn ppi_for_pixels(
        &self,
        width_px: u32,
        height_px: u32,
    ) -> Result<Option<PrintResolutionPpi>, ValidationError> {
        validate_pixels(width_px, height_px)?;
        self.size_mm
            .map(|size| size.ppi_for_pixels(width_px, height_px))
            .transpose()
    }

    /// Rechecks persisted intent at the document boundary without inventing defaults.
    ///
    /// # Errors
    /// Rejects non-finite, negative, or noncanonical threshold values.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(size) = self.size_mm {
            PhysicalPrintSizeMm::new(size.width_mm, size.height_mm)?;
        }
        validate_nonnegative(
            self.minimum_positive_feature_width_mm,
            "print.threshold.positive_width_mm",
        )?;
        validate_nonnegative(
            self.minimum_negative_gap_width_mm,
            "print.threshold.negative_gap_mm",
        )?;
        if self.minimum_positive_feature_width_mm.is_sign_negative()
            && self.minimum_positive_feature_width_mm == 0.0
            || self.minimum_negative_gap_width_mm.is_sign_negative()
                && self.minimum_negative_gap_width_mm == 0.0
        {
            return Err(ValidationError::new(
                "print.threshold",
                "negative zero must be normalized",
            ));
        }
        Ok(())
    }
}

/// Checks that both axes have a selected nonzero final pixel count.
///
/// # Errors
/// Rejects a zero width or height, including when physical placement is unknown.
fn validate_pixels(width_px: u32, height_px: u32) -> Result<(), ValidationError> {
    if width_px == 0 || height_px == 0 {
        Err(ValidationError::new(
            "print.ppi.pixels",
            "final output dimensions must be positive",
        ))
    } else {
        Ok(())
    }
}

/// Checks one authored positive finite physical scalar.
///
/// # Errors
/// Rejects zero, negative, NaN, and infinity.
fn validate_positive(value: f64, path: &'static str) -> Result<(), ValidationError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be positive and finite",
        ))
    }
}

/// Checks one independently enabled nonnegative finite width limit.
///
/// # Errors
/// Rejects negatives, NaN, and infinity.
fn validate_nonnegative(value: f64, path: &'static str) -> Result<(), ValidationError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new(
            path,
            "value must be nonnegative and finite",
        ))
    }
}

/// Accepts a positive representable conversion result without silent overflow or underflow.
///
/// # Errors
/// Rejects infinity, NaN, negative, and a positive source rounded to zero.
fn checked_positive_result(value: f64, path: &'static str) -> Result<f64, ValidationError> {
    validate_positive(value, path)?;
    Ok(value)
}

/// Canonicalizes both signs of zero to the disabled-threshold representation.
fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}
