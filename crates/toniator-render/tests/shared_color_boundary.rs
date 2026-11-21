//! Keeps the public rendering boundary clipped while shared transfers remain extended-range.

/// Preserves output clipping and known sRGB reference values after the library extraction.
///
/// # Panics
/// Panics if rendering leaks out-of-range color or changes the half-intensity mapping.
#[test]
fn render_transfers_keep_output_boundary_clipping() {
    assert_eq!(toniator_render::srgb_to_linear(-0.25), 0.0);
    assert_eq!(toniator_render::srgb_to_linear(1.25), 1.0);
    assert_eq!(toniator_render::linear_to_srgb(-0.25), 0.0);
    assert!((toniator_render::linear_to_srgb(1.25) - 1.0).abs() < 1e-15);
    assert!((toniator_render::srgb_to_linear(0.5) - 0.214_041_140_482_232_55).abs() < 1e-15);
    assert!((toniator_render::linear_to_srgb(0.214_041_140_482_232_55) - 0.5).abs() < 1e-15);
}
