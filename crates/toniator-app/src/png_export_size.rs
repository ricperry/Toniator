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
