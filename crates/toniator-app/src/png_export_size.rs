//! Runtime-only output-size choices resolved from authoritative document dimensions.

use toniator_domain::CanvasSpec;
use toniator_engine::OutputRasterTarget;

#[derive(Clone, Copy)]
enum SizeMenu {
    Png,
    Temporal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SizeChoice {
    Half,
    Native,
    Double,
    Quadruple,
    Eightfold,
    Custom,
}

const PNG_CHOICES: [SizeChoice; 5] = [
    SizeChoice::Native,
    SizeChoice::Double,
    SizeChoice::Quadruple,
    SizeChoice::Eightfold,
    SizeChoice::Custom,
];
const VIDEO_CHOICES: [SizeChoice; 6] = [
    SizeChoice::Half,
    SizeChoice::Native,
    SizeChoice::Double,
    SizeChoice::Quadruple,
    SizeChoice::Eightfold,
    SizeChoice::Custom,
];

/// Carries checked video dimensions and preserves the existing native raster path at 1x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VideoOutputSize {
    dimensions: OutputRasterTarget,
    target: Option<OutputRasterTarget>,
}

impl VideoOutputSize {
    /// Returns checked final pixel dimensions for the live size readout.
    pub(crate) fn dimensions(self) -> (u32, u32) {
        (self.dimensions.width(), self.dimensions.height())
    }

    /// Returns an explicit raster target for scaled/custom choices and native intent for 1x.
    pub(crate) fn target(self) -> Option<OutputRasterTarget> {
        self.target
    }
}

/// Carries a checked print-fit raster and its requested-DPI physical readout.
///
/// The source canvas remains the authority for artwork aspect. The secondary axis uses nearest
/// whole-pixel rounding after the limiting axis is floored to its maximum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PrintFit {
    target: OutputRasterTarget,
    target_dpi: f64,
    pixels_per_metre: u32,
    physical_width_mm: f64,
    physical_height_mm: f64,
}

impl PrintFit {
    /// Returns final pixel dimensions admitted by the shared renderer budget.
    pub(crate) fn target(self) -> OutputRasterTarget {
        self.target
    }

    /// Returns the requested density before conversion to PNG's integer pixels-per-metre value.
    pub(crate) fn target_dpi(self) -> f64 {
        self.target_dpi
    }

    /// Returns the equal horizontal and vertical density encoded in PNG pixels per metre.
    pub(crate) fn pixels_per_metre(self) -> u32 {
        self.pixels_per_metre
    }

    /// Returns the fitted physical width and height derived from final pixels and requested DPI.
    pub(crate) fn physical_size_mm(self) -> (f64, f64) {
        (self.physical_width_mm, self.physical_height_mm)
    }
}

/// Fits an authoritative canvas aspect inside a maximum physical box at one requested DPI.
///
/// Width and height are millimetres. Pixel bounds are floored from the requested DPI, with a small
/// floating-point tolerance for values that represent an exact whole-pixel bound. The limiting
/// axis uses its maximum whole-pixel bound, while the other axis rounds to the nearest pixel and
/// clamps to its own bound. This retains the source aspect to normal pixel-rounding accuracy
/// without stretching the artwork. The returned physical size follows the requested DPI; the
/// integer pixels-per-metre value is rounded separately for PNG metadata and may differ slightly.
///
/// # Errors
/// Rejects nonintegral or invalid source dimensions, nonpositive or unrepresentable DPI/metadata,
/// nonpositive or subpixel physical bounds, unrepresentable output dimensions, and renderer-budget
/// violations.
pub(crate) fn fit_print_box(
    canvas: &CanvasSpec,
    width_mm: f64,
    height_mm: f64,
    target_dpi: f64,
) -> Result<PrintFit, String> {
    let valid_source_dimension = |value: f64| {
        value.is_finite() && value > 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX)
    };
    if !valid_source_dimension(canvas.width) || !valid_source_dimension(canvas.height) {
        return Err("The document must have positive whole pixel dimensions.".to_owned());
    }
    if !width_mm.is_finite() || width_mm <= 0.0 || !height_mm.is_finite() || height_mm <= 0.0 {
        return Err("Print width and height must be positive finite millimetres.".to_owned());
    }
    if !target_dpi.is_finite() || target_dpi <= 0.0 {
        return Err("Print resolution must be a positive finite DPI value.".to_owned());
    }

    // PNG pHYs stores integer pixels per metre. The pixel dimensions follow the requested DPI;
    // pHYs is rounded separately because the chunk cannot store every possible DPI exactly.
    let encoded_ppm = (target_dpi * (5000.0 / 127.0)).round();
    if !encoded_ppm.is_finite() || encoded_ppm < 1.0 || encoded_ppm > f64::from(u32::MAX) {
        return Err("Print resolution is outside the supported PNG metadata range.".to_owned());
    }
    let pixels_per_metre = encoded_ppm as u32;
    let maximum_width = print_pixel_limit(width_mm, target_dpi)?;
    let maximum_height = print_pixel_limit(height_mm, target_dpi)?;

    let source_width = canvas.width as u32;
    let source_height = canvas.height as u32;
    let width_scale = maximum_width / f64::from(source_width);
    let height_scale = maximum_height / f64::from(source_height);
    let (width, height) = if width_scale <= height_scale {
        (
            maximum_width,
            (maximum_width / f64::from(source_width) * f64::from(source_height))
                .round()
                .min(maximum_height),
        )
    } else {
        (
            (maximum_height / f64::from(source_height) * f64::from(source_width))
                .round()
                .min(maximum_width),
            maximum_height,
        )
    };
    let checked_dimension = |value: f64| {
        if value.is_finite() && value >= 1.0 && value <= f64::from(u32::MAX) {
            Ok(value as u32)
        } else {
            Err("The fitted print dimensions are outside the supported pixel range.".to_owned())
        }
    };
    let width = checked_dimension(width)?;
    let height = checked_dimension(height)?;
    let target = OutputRasterTarget::new(width, height)
        .map_err(|_| "This print image exceeds the renderer pixel budget.".to_owned())?;

    Ok(PrintFit {
        target,
        target_dpi,
        pixels_per_metre,
        physical_width_mm: f64::from(width) * 25.4 / target_dpi,
        physical_height_mm: f64::from(height) * 25.4 / target_dpi,
    })
}

/// Floors one millimetre bound into pixels at the requested DPI.
///
/// A scale-relative few-ULP tolerance restores exact whole-pixel cases such as 50.8 mm at 300 DPI
/// when binary floating-point arithmetic lands just below the mathematical integer.
///
/// # Errors
/// Rejects nonfinite or subpixel pixel bounds.
fn print_pixel_limit(size_mm: f64, target_dpi: f64) -> Result<f64, String> {
    let pixels = size_mm * target_dpi / 25.4;
    if !pixels.is_finite() {
        return Err(
            "The fitted print dimensions are outside the supported pixel range.".to_owned(),
        );
    }
    let nearest = pixels.round();
    let tolerance = 8.0 * f64::EPSILON * pixels.abs().max(1.0);
    let limit = if (pixels - nearest).abs() <= tolerance {
        nearest
    } else {
        pixels.floor()
    };
    if limit < 1.0 {
        return Err("The print box must allow at least one pixel on each axis.".to_owned());
    }
    Ok(limit)
}

/// Returns the selected temporal-size position for the normal 1x default.
///
/// # Panics
/// Panics only if the video size menu loses its native choice.
pub(crate) fn video_native_position() -> u32 {
    choice_position(SizeMenu::Temporal, SizeChoice::Native)
}

/// Reports whether one temporal selector position enables the Custom dimensions field.
pub(crate) fn video_custom_selected(position: u32) -> bool {
    selected_choice(SizeMenu::Temporal, position) == Ok(SizeChoice::Custom)
}

/// Resolves the existing PNG menu without changing its visible presets or custom-index behavior.
///
/// The base is the document canvas, never the current viewport zoom. Custom dimensions retain
/// anisotropic output semantics and use the shared renderer pixel budget.
///
/// # Errors
/// Returns malformed custom dimensions, invalid native canvas dimensions, or output-budget errors.
pub(crate) fn target(
    canvas: &CanvasSpec,
    position: u32,
    custom: &str,
) -> Result<OutputRasterTarget, String> {
    let choice = selected_choice(SizeMenu::Png, position)?;
    resolve_target(canvas, choice, custom)
}

/// Resolves a temporal size choice and keeps 1x on the existing native raster route.
///
/// The resolved dimensions are always checked for the live pixel readout; scaled and custom
/// selections also produce an explicit shared-renderer target. No choice changes document state.
///
/// # Errors
/// Returns an invalid selector, native canvas, custom-dimension, or output-budget diagnostic.
pub(crate) fn video_output_size(
    canvas: &CanvasSpec,
    position: u32,
    custom: &str,
) -> Result<VideoOutputSize, String> {
    let choice = selected_choice(SizeMenu::Temporal, position)?;
    let dimensions = resolve_target(canvas, choice, custom)?;
    Ok(VideoOutputSize {
        dimensions,
        target: (choice != SizeChoice::Native).then_some(dimensions),
    })
}

/// Maps each visible dropdown position in one place so GTK callers carry no index policy.
///
/// # Errors
/// Rejects positions beyond the displayed size choices.
fn selected_choice(menu: SizeMenu, position: u32) -> Result<SizeChoice, String> {
    menu_choices(menu)
        .get(usize::try_from(position).map_err(|_| "Choose an output size.")?)
        .copied()
        .ok_or_else(|| "Choose an output size.".to_owned())
}

/// Finds one visible choice position using the same choice arrays as selector resolution.
///
/// # Panics
/// Panics only if an internal size menu omits a required native choice.
fn choice_position(menu: SizeMenu, choice: SizeChoice) -> u32 {
    menu_choices(menu)
        .iter()
        .position(|candidate| *candidate == choice)
        .expect("size menu includes the requested choice") as u32
}

/// Returns the authoritative ordered choices for one visible menu.
fn menu_choices(menu: SizeMenu) -> &'static [SizeChoice] {
    match menu {
        SizeMenu::Png => &PNG_CHOICES,
        SizeMenu::Temporal => &VIDEO_CHOICES,
    }
}

/// Resolves one scale or custom choice through the final renderer target authority.
///
/// # Errors
/// Rejects malformed custom dimensions, invalid native dimensions, scaling overflow, and targets
/// outside the shared output pixel budget.
fn resolve_target(
    canvas: &CanvasSpec,
    choice: SizeChoice,
    custom: &str,
) -> Result<OutputRasterTarget, String> {
    if choice == SizeChoice::Custom {
        return crate::parse_output_target(custom)
            .map_err(|error| {
                if error.contains("pixel count exceeds") {
                    "This image is too large to export. Choose a smaller size.".to_owned()
                } else {
                    "Enter dimensions as WIDTHxHEIGHT using positive whole numbers.".to_owned()
                }
            })?
            .ok_or_else(|| "Enter custom dimensions as WIDTHxHEIGHT.".to_owned());
    }

    let (width, height) = native_dimensions(canvas, choice)?;
    let width = u32::try_from(width).map_err(|_| "Output width is too large.")?;
    let height = u32::try_from(height).map_err(|_| "Output height is too large.")?;
    OutputRasterTarget::new(width, height)
        .map_err(|_| "This image is too large to export. Choose a smaller size.".to_owned())
}

/// Validates positive integral document dimensions, then applies one exact positive scale.
///
/// Half-size uses integer ceiling so positive half pixels round up and thin canvases remain at
/// least one pixel. The shared pixel budget applies only after the final dimensions are resolved.
///
/// # Errors
/// Rejects nonintegral, zero, nonfinite, unrepresentable, or overflowing scaled dimensions.
fn native_dimensions(canvas: &CanvasSpec, choice: SizeChoice) -> Result<(u64, u64), String> {
    let valid = |value: f64| {
        value.is_finite() && value > 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX)
    };
    if !valid(canvas.width) || !valid(canvas.height) {
        return Err("The document must have positive whole pixel dimensions.".to_owned());
    }
    let width = canvas.width as u64;
    let height = canvas.height as u64;
    let scale = |value: u64| -> Option<u64> {
        match choice {
            SizeChoice::Half => Some(value.div_ceil(2)),
            SizeChoice::Native => Some(value),
            SizeChoice::Double => value.checked_mul(2),
            SizeChoice::Quadruple => value.checked_mul(4),
            SizeChoice::Eightfold => value.checked_mul(8),
            SizeChoice::Custom => None,
        }
    };
    let width = scale(width).ok_or("Output width is too large.")?;
    let height = scale(height).ok_or("Output height is too large.")?;
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a valid document canvas with the requested test aspect.
    ///
    /// # Panics
    /// This helper does not panic.
    fn canvas(width: f64, height: f64) -> CanvasSpec {
        CanvasSpec { width, height }
    }

    /// Proves common print boxes use requested-DPI pixels and preserve source aspect.
    ///
    /// # Panics
    /// Panics if expected physical limits, output dimensions, or encoded metadata density change.
    #[test]
    fn print_fit_preserves_aspect_inside_square_landscape_and_portrait_boxes() {
        let square = fit_print_box(&canvas(1000.0, 1000.0), 50.8, 50.8, 300.0).unwrap();
        assert_eq!(square.pixels_per_metre(), 11_811);
        assert_eq!(
            (square.target().width(), square.target().height()),
            (600, 600)
        );
        assert!(square.physical_size_mm().0 <= 50.8);
        assert!(square.physical_size_mm().1 <= 50.8);

        let landscape = fit_print_box(&canvas(1600.0, 900.0), 304.8, 152.4, 300.0).unwrap();
        assert_eq!(
            (landscape.target().width(), landscape.target().height()),
            (3200, 1800)
        );
        assert!(landscape.physical_size_mm().0 <= 304.8);
        assert!(landscape.physical_size_mm().1 <= 152.4);
        assert!(
            (f64::from(landscape.target().width()) / f64::from(landscape.target().height())
                - 1600.0 / 900.0)
                .abs()
                < 0.001
        );

        let portrait = fit_print_box(&canvas(900.0, 1600.0), 152.4, 304.8, 300.0).unwrap();
        assert_eq!(
            (portrait.target().width(), portrait.target().height()),
            (1800, 3200)
        );
        assert!(portrait.physical_size_mm().0 <= 152.4);
        assert!(portrait.physical_size_mm().1 <= 304.8);
    }

    /// Proves larger boxes stay within bounds and inch equivalents use the millimetre API.
    ///
    /// # Panics
    /// Panics if aspect stretches, requested-DPI dimensions change, or physical readouts disagree.
    #[test]
    fn print_fit_treats_physical_dimensions_as_maximum_bounds() {
        let fit = fit_print_box(&canvas(100.0, 50.0), 50.8, 50.8, 300.0).unwrap();
        assert_eq!((fit.target().width(), fit.target().height()), (600, 300));
        assert!(fit.physical_size_mm().0 <= 50.8);
        assert!(fit.physical_size_mm().1 <= 50.8);
        assert!(
            (f64::from(fit.target().width()) / f64::from(fit.target().height()) - 2.0).abs() < 0.01
        );

        // Two by one inches expressed as millimetres, then bounded at 300 DPI.
        let inch_equivalent = fit_print_box(&canvas(2.0, 1.0), 50.8, 25.4, 300.0).unwrap();
        assert_eq!(inch_equivalent.target_dpi(), 300.0);
        assert_eq!(inch_equivalent.pixels_per_metre(), 11_811);
        assert_eq!(
            (
                inch_equivalent.target().width(),
                inch_equivalent.target().height()
            ),
            (600, 300)
        );
        let actual_mm = inch_equivalent.physical_size_mm();
        assert_eq!(
            actual_mm.0,
            f64::from(inch_equivalent.target().width()) * 25.4 / 300.0
        );
        assert_eq!(
            actual_mm.1,
            f64::from(inch_equivalent.target().height()) * 25.4 / 300.0
        );
        assert!(actual_mm.0 <= 50.8);
        assert!(actual_mm.1 <= 25.4);

        let fractional = fit_print_box(&canvas(100.0, 50.0), 25.4, 12.7, 72.5).unwrap();
        assert_eq!(fractional.target_dpi(), 72.5);
        assert_eq!(
            (fractional.target().width(), fractional.target().height()),
            (72, 36)
        );
    }

    /// Proves invalid, subpixel, overflowing, and over-budget print fits are rejected.
    ///
    /// # Panics
    /// Panics if invalid input reaches a renderer target or if a bounded valid fit is rejected.
    #[test]
    fn print_fit_rejects_invalid_tiny_overflow_and_renderer_budget_inputs() {
        for invalid_canvas in [
            canvas(0.0, 100.0),
            canvas(100.5, 100.0),
            canvas(f64::INFINITY, 1.0),
        ] {
            assert!(fit_print_box(&invalid_canvas, 50.8, 25.4, 300.0).is_err());
        }
        for (width_mm, height_mm, dpi) in [
            (0.0, 25.4, 300.0),
            (f64::NAN, 25.4, 300.0),
            (50.8, f64::INFINITY, 300.0),
            (50.8, 25.4, 0.0),
            (50.8, 25.4, f64::INFINITY),
            (50.8, 25.4, f64::MAX),
            (0.01, 0.01, 300.0),
            (f64::MAX, f64::MAX, 300.0),
            (1.0e12, 1.0e12, 300.0),
        ] {
            assert!(fit_print_box(&canvas(100.0, 50.0), width_mm, height_mm, dpi).is_err());
        }
        assert!(fit_print_box(&canvas(1_000_000.0, 1.0), 100.0, 0.1, 300.0).is_err());
        assert!(fit_print_box(&canvas(1000.0, 1000.0), 3000.0, 3000.0, 300.0).is_err());
    }

    /// Proves the PNG selector keeps its native, scaled, custom, and shared-budget mappings.
    ///
    /// # Panics
    /// Panics if an existing PNG choice changes dimensions or an invalid size enters the renderer.
    #[test]
    fn png_export_scales_and_custom_bounds() {
        for (width, height) in [(1024.0, 1024.0), (900.0, 620.0)] {
            let canvas = CanvasSpec { width, height };
            for (position, multiplier) in [1, 2, 4, 8].into_iter().enumerate() {
                let output = target(&canvas, position as u32, "ignored").unwrap();
                assert_eq!(
                    (output.width(), output.height()),
                    (width as u32 * multiplier, height as u32 * multiplier)
                );
            }
            let custom = target(&canvas, 4, "1200x800").unwrap();
            assert_eq!((custom.width(), custom.height()), (1200, 800));
            for text in ["", "96", "0x10", "8193x8193", "4294967296x1"] {
                assert!(target(&canvas, 4, text).is_err());
            }
        }
        assert!(
            target(
                &CanvasSpec {
                    width: 2048.0,
                    height: 2048.0
                },
                3,
                ""
            )
            .is_err()
        );
    }

    /// Proves temporal presets, live dimensions, and native-size routing use document dimensions.
    ///
    /// # Panics
    /// Panics if scale positions drift, dimensions use viewport state, or 1x stops using native
    /// rendering.
    #[test]
    fn video_output_scales_resolve_exact_dimensions_and_keep_native_target() {
        assert_eq!(video_native_position(), 1);
        assert!(!video_custom_selected(video_native_position()));
        assert!(video_custom_selected(5));
        for (width, height, half) in [
            (1024.0, 1024.0, (512, 512)),
            (900.0, 620.0, (450, 310)),
            (901.0, 621.0, (451, 311)),
            (1.0, 100.0, (1, 50)),
            (100.0, 1.0, (50, 1)),
            (1.0, 1.0, (1, 1)),
        ] {
            let canvas = CanvasSpec { width, height };
            let output = video_output_size(&canvas, 0, "ignored").unwrap();
            assert_eq!(output.dimensions(), half);
            assert_eq!(output.target().unwrap().width(), half.0);
            assert_eq!(output.target().unwrap().height(), half.1);
            assert_eq!(
                video_output_size(&canvas, 1, "ignored")
                    .unwrap()
                    .dimensions(),
                (width as u32, height as u32)
            );
            assert!(
                video_output_size(&canvas, 1, "ignored")
                    .unwrap()
                    .target()
                    .is_none()
            );
            for (position, multiplier) in [(2, 2), (3, 4), (4, 8)] {
                assert_eq!(
                    video_output_size(&canvas, position, "ignored")
                        .unwrap()
                        .dimensions(),
                    (width as u32 * multiplier, height as u32 * multiplier)
                );
            }
        }
    }

    /// Proves half-size is validated after scaling and Custom does not require valid native canvas.
    ///
    /// # Panics
    /// Panics if a safe half-size target inherits the full canvas budget or Custom depends on the
    /// document's integral dimensions.
    #[test]
    fn video_half_custom_and_invalid_canvas_bounds_are_authoritative() {
        let large = CanvasSpec {
            width: 130_000.0,
            height: 1_000.0,
        };
        assert!(video_output_size(&large, 1, "").is_err());
        assert_eq!(
            video_output_size(&large, 0, "").unwrap().dimensions(),
            (65_000, 500)
        );
        assert_eq!(
            video_output_size(
                &CanvasSpec {
                    width: f64::NAN,
                    height: 0.0
                },
                5,
                "16x8"
            )
            .unwrap()
            .dimensions(),
            (16, 8)
        );
        for custom in ["", "96", "0x10", "8193x8193", "4294967296x1"] {
            assert!(video_output_size(&large, 5, custom).is_err());
        }
        for position in [6, u32::MAX] {
            assert!(video_output_size(&large, position, "").is_err());
        }
        assert!(
            video_output_size(
                &CanvasSpec {
                    width: 10.5,
                    height: 20.0
                },
                0,
                ""
            )
            .is_err()
        );
        assert!(
            video_output_size(
                &CanvasSpec {
                    width: f64::INFINITY,
                    height: 20.0
                },
                2,
                ""
            )
            .is_err()
        );
    }
}
