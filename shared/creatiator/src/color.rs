// SPDX-License-Identifier: GPL-3.0-or-later
//! Double-precision sRGB transfer functions without gamut clipping or alpha changes.

/// Converts one encoded sRGB component to linear light without clipping.
///
/// Values outside the nominal 0–1 interval retain the consumer's extended-range
/// policy. The caller owns any required clipping, finite-value validation, or
/// alpha association. The threshold belongs to the linear branch.
pub fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts one linear-light component to encoded sRGB without clipping.
///
/// Values outside the nominal 0–1 interval retain the consumer's extended-range
/// policy. The caller owns any required clipping, finite-value validation, or
/// alpha association. The threshold belongs to the linear branch.
pub fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks exact branch selection at and immediately around both transfer thresholds.
    ///
    /// # Panics
    /// Panics if either threshold uses the wrong branch or changes arithmetic precision.
    #[test]
    fn transfer_thresholds_preserve_branch_arithmetic() {
        let encoded_threshold = 0.040_45_f64;
        for encoded in [encoded_threshold.next_down(), encoded_threshold] {
            assert_eq!(srgb_to_linear(encoded), encoded / 12.92);
        }
        let encoded_above = encoded_threshold.next_up();
        assert_eq!(
            srgb_to_linear(encoded_above),
            ((encoded_above + 0.055) / 1.055).powf(2.4)
        );

        let linear_threshold = 0.003_130_8_f64;
        for linear in [linear_threshold.next_down(), linear_threshold] {
            assert_eq!(linear_to_srgb(linear), 12.92 * linear);
        }
        let linear_above = linear_threshold.next_up();
        assert_eq!(
            linear_to_srgb(linear_above),
            1.055 * linear_above.powf(1.0 / 2.4) - 0.055
        );
    }

    /// Keeps extended-range components unclipped and preserves known neutral reference values.
    ///
    /// # Panics
    /// Panics if a transfer clips negative/HDR values or changes the established neutral mapping.
    #[test]
    fn transfers_preserve_extended_range_and_neutral_reference() {
        assert_eq!(srgb_to_linear(-0.25), -0.25 / 12.92);
        assert_eq!(linear_to_srgb(-0.25), 12.92 * -0.25);
        assert!(srgb_to_linear(1.25) > 1.0);
        assert!(linear_to_srgb(1.25) > 1.0);
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert_eq!(linear_to_srgb(0.0), 0.0);
        assert!((srgb_to_linear(0.5) - 0.214_041_140_482_232_55).abs() < 1e-15);
        for value in [-0.25, 0.0, 0.25, 0.5, 1.0, 1.25] {
            assert!((linear_to_srgb(srgb_to_linear(value)) - value).abs() < 1e-14);
        }
    }
}
