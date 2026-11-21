//! Main-window-only presentation state and system theme inheritance.
//!
//! This module cannot see documents, history, evaluation, or persistence. It
//! owns only viewport choices and a live projection of the desktop color
//! preference into GTK's application theme hint.

pub(crate) use creatiator::{SystemThemeBridge, inherit_system_color_scheme};

/// Selects which existing image is shown in the shared main viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MainViewMode {
    /// Shows the newest accepted canonical pattern preview.
    #[default]
    Preview,
    /// Shows the immutable embedded source artwork.
    Source,
}

/// Retains bounded zoom and view choices without participating in document authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainViewState {
    mode: MainViewMode,
    zoom_index: usize,
    fit: bool,
    fitted_zoom: Option<f64>,
    quality: toniator_engine::preview_cache::Quality,
}

impl Default for MainViewState {
    /// Starts in the canonical Preview view fitted to the available viewport.
    fn default() -> Self {
        Self {
            mode: MainViewMode::Preview,
            zoom_index: Self::DEFAULT_ZOOM_INDEX,
            fit: true,
            fitted_zoom: None,
            quality: toniator_engine::preview_cache::Quality::Accurate,
        }
    }
}

impl MainViewState {
    const ZOOM_LEVELS: [f64; 9] = [0.25, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 3.0, 4.0];
    const DEFAULT_ZOOM_INDEX: usize = 4;

    /// Returns the current presentation mode without changing document or preview state.
    pub(crate) const fn mode(self) -> MainViewMode {
        self.mode
    }

    /// Selects an already available viewport image and preserves the current pan/zoom choice.
    pub(crate) fn set_mode(&mut self, mode: MainViewMode) {
        self.mode = mode;
    }

    /// Returns the window's transient requested quality, independently of accepted pixel provenance.
    pub(crate) const fn quality(self) -> toniator_engine::preview_cache::Quality {
        self.quality
    }
    /// Selects only requested precision; document, camera, endpoint and accepted pixels remain external.
    pub(crate) fn set_quality(&mut self, quality: toniator_engine::preview_cache::Quality) -> bool {
        if self.quality == quality {
            return false;
        }
        self.quality = quality;
        true
    }

    /// Returns whether the current image is fitted to the shared viewport.
    pub(crate) const fn is_fit(self) -> bool {
        self.fit
    }

    /// Returns the current bounded zoom multiplier.
    pub(crate) fn zoom(self) -> f64 {
        Self::ZOOM_LEVELS[self.zoom_index]
    }

    /// Returns fitted zoom to a tenth of a percent, whole manual presets, or an em dash before allocation.
    /// Whole fitted percentages omit the decimal; retained manual levels remain independent of Fit.
    pub(crate) fn zoom_label(self) -> String {
        self.displayed_zoom()
            .map(|zoom| {
                if self.fit {
                    let percentage = format!("{:.1}", zoom * 100.0);
                    format!("{}%", percentage.strip_suffix(".0").unwrap_or(&percentage))
                } else {
                    format!("{:.0}%", zoom * 100.0)
                }
            })
            .unwrap_or_else(|| "—".to_owned())
    }

    /// Selects Fit or restores the retained manual level without changing document authority.
    /// Entering Fit invalidates its old allocation observation before the caller projects the live page.
    pub(crate) fn set_fit(&mut self, fit: bool) {
        if fit && !self.fit {
            self.fitted_zoom = None;
        }
        self.fit = fit;
    }

    /// Observes the fitted page's contain scale in logical canvas units, independently of device/proxy pixels.
    /// Invalid or unallocated dimensions clear the observation; manual mode retains its own level.
    pub(crate) fn observe_fit(&mut self, canvas: (f64, f64), page: (f64, f64)) {
        self.fitted_zoom = [canvas.0, canvas.1, page.0, page.1]
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
            .then(|| (page.0 / canvas.0).min(page.1 / canvas.1))
            .filter(|zoom| zoom.is_finite() && *zoom > 0.0);
    }

    /// Returns the live fitted scale or retained manual ladder value in logical canvas units.
    fn displayed_zoom(self) -> Option<f64> {
        if self.fit {
            self.fitted_zoom
        } else {
            Some(self.zoom())
        }
    }

    /// Finds the nearest strictly directional manual level from the actually displayed zoom.
    /// No candidate leaves Fit and the retained manual camera unchanged, including outside bounds.
    fn adjacent_zoom_index(self, zoom_in: bool) -> Option<usize> {
        let current = self.displayed_zoom()?;
        if zoom_in {
            Self::ZOOM_LEVELS.iter().position(|zoom| *zoom > current)
        } else {
            Self::ZOOM_LEVELS.iter().rposition(|zoom| *zoom < current)
        }
    }

    /// Selects the nearest strictly larger manual level and leaves Fit only when one exists.
    pub(crate) fn zoom_in(&mut self) {
        if let Some(index) = self.adjacent_zoom_index(true) {
            self.zoom_index = index;
            self.fit = false;
        }
    }

    /// Selects the nearest strictly smaller manual level and leaves Fit only when one exists.
    pub(crate) fn zoom_out(&mut self) {
        if let Some(index) = self.adjacent_zoom_index(false) {
            self.zoom_index = index;
            self.fit = false;
        }
    }

    /// Reports whether a larger bounded zoom level remains available.
    pub(crate) fn can_zoom_in(self) -> bool {
        self.adjacent_zoom_index(true).is_some()
    }

    /// Reports whether a smaller bounded zoom level remains available.
    pub(crate) fn can_zoom_out(self) -> bool {
        self.adjacent_zoom_index(false).is_some()
    }
}

/// Names the actual accepted backend; requesting Fast alone never creates a Fast claim.
pub(crate) const fn quality_label(
    provenance: toniator_engine::preview_cache::Provenance,
) -> &'static str {
    use toniator_engine::preview_cache::Quality;
    match (provenance.requested, provenance.effective) {
        (Quality::Fast, Quality::Accurate) => "Accurate · CPU fallback",
        (_, Quality::Fast) => "Fast",
        _ => "Accurate",
    }
}

/// Projects a pointer drag into one native adjustment value when that axis can actually scroll.
///
/// The GTK adjustment remains the viewport authority; this helper subtracts drag movement from
/// the captured origin and clamps it to the adjustment's current legal range. It returns `None`
/// for non-finite or invalid ranges and for an axis with no overflow.
pub(crate) fn pan_adjustment_value(
    lower: f64,
    upper: f64,
    page_size: f64,
    origin: f64,
    pointer_delta: f64,
) -> Option<f64> {
    if !lower.is_finite()
        || !upper.is_finite()
        || !page_size.is_finite()
        || !origin.is_finite()
        || !pointer_delta.is_finite()
        || upper < lower
        || page_size < 0.0
    {
        return None;
    }
    let maximum = upper - page_size;
    if !maximum.is_finite() {
        return None;
    }
    let maximum = maximum.max(lower);
    if maximum <= lower {
        return None;
    }
    let requested = origin - pointer_delta;
    requested
        .is_finite()
        .then(|| requested.clamp(lower, maximum))
}

#[cfg(test)]
mod tests {
    /// Keeps each new window Accurate while quality changes preserve camera and Source preference.
    /// # Panics
    /// Panics if a presentation-only choice resets zoom/Fit or silently resets when viewing Source.
    #[test]
    fn stage3_requested_quality_is_transient_and_camera_independent() {
        use toniator_engine::preview_cache::Quality;
        let mut state = MainViewState::default();
        assert_eq!(state.quality(), Quality::Accurate);
        state.set_fit(false);
        state.zoom_in();
        let zoom = state.zoom();
        let fit = state.is_fit();
        assert!(state.set_quality(Quality::Fast));
        assert_eq!(state.zoom(), zoom);
        assert_eq!(state.is_fit(), fit);
        state.set_mode(MainViewMode::Source);
        assert_eq!(state.quality(), Quality::Fast);
        state.set_mode(MainViewMode::Preview);
        assert_eq!(state.quality(), Quality::Fast);
        assert!(!state.set_quality(Quality::Fast));
        state.set_fit(true);
        assert_eq!(state.quality(), Quality::Fast);
        assert_eq!(MainViewState::default().quality(), Quality::Accurate);
    }

    use super::*;

    /// Proves viewport choices stay bounded and mode changes preserve zoom state.
    /// # Panics
    /// Panics if manual ladder bounds or presentation-only mode independence changes.
    #[test]
    fn main_view_state_bounds_zoom_and_keeps_mode_independent() {
        let mut state = MainViewState::default();
        state.set_fit(false);
        state.zoom_in();
        let zoom = state.zoom();
        state.set_mode(MainViewMode::Source);
        assert_eq!(state.mode(), MainViewMode::Source);
        assert_eq!(state.zoom(), zoom);
        for _ in 0..20 {
            state.zoom_in();
        }
        assert_eq!(state.zoom(), 4.0);
        assert!(!state.can_zoom_in());
        for _ in 0..20 {
            state.zoom_out();
        }
        assert_eq!(state.zoom(), 0.25);
        assert!(!state.can_zoom_out());
        state.set_fit(true);
        assert!(state.is_fit());
    }

    /// Checks strict nearest Fit steps, exact presets, unavailable directions and both ladder bounds.
    /// # Panics
    /// Panics if Fit steps use the retained manual level or move opposite the requested direction.
    #[test]
    fn fit_zoom_steps_from_displayed_scale_in_requested_direction() {
        for (fit, smaller, larger) in [
            (0.10, None, Some(0.25)),
            (0.25, None, Some(0.33)),
            (0.33, Some(0.25), Some(0.5)),
            (0.42, Some(0.33), Some(0.5)),
            (1.0, Some(0.67), Some(1.5)),
            (1.8, Some(1.5), Some(2.0)),
            (4.0, Some(3.0), None),
            (6.0, Some(4.0), None),
        ] {
            let mut fitted = MainViewState::default();
            fitted.observe_fit((100.0, 100.0), (fit * 100.0, fit * 100.0));
            assert_eq!(fitted.can_zoom_out(), smaller.is_some());
            assert_eq!(fitted.can_zoom_in(), larger.is_some());
            for (zoom_in, expected) in [(false, smaller), (true, larger)] {
                let mut state = fitted;
                if zoom_in {
                    state.zoom_in();
                } else {
                    state.zoom_out();
                }
                match expected {
                    Some(zoom) => {
                        assert_eq!(state.zoom(), zoom);
                        assert!(!state.is_fit());
                    }
                    None => assert_eq!(state, fitted),
                }
            }
        }
    }

    /// Checks fractional fitted labels distinguish nearby manual presets without unnecessary decimal zeros.
    /// # Panics
    /// Panics if fitted tenths disappear, whole fitted labels gain decimals, or manual labels change.
    #[test]
    fn fit_zoom_label_keeps_fractional_steps_visible() {
        let mut state = MainViewState::default();
        for (page_width, expected) in [
            (1136.0, "83.5%"),
            (916.0, "67.4%"),
            (1360.0, "100%"),
            (680.0, "50%"),
        ] {
            state.observe_fit((1360.0, 768.0), (page_width, 1000.0));
            assert_eq!(state.zoom_label(), expected);
        }
        state.observe_fit((1360.0, 768.0), (916.0, 1000.0));
        state.zoom_out();
        assert_eq!(state.zoom_label(), "67%");
        state.zoom_out();
        assert_eq!(state.zoom_label(), "50%");
    }

    /// Checks Fit observations track contain allocation and preserve manual zoom across toggle/mode changes.
    /// # Panics
    /// Panics if resize, absent allocation or a Source switch changes the retained manual camera.
    #[test]
    fn fit_toggle_retains_manual_zoom_while_allocation_changes() {
        let mut state = MainViewState::default();
        assert_eq!(state.zoom_label(), "—");
        assert!(!state.can_zoom_in() && !state.can_zoom_out());
        state.set_fit(false);
        state.zoom_in();
        assert_eq!(state.zoom(), 1.5);
        state.set_fit(true);
        assert_eq!(state.zoom_label(), "—");
        state.zoom_in();
        assert!(state.is_fit());
        assert_eq!(state.zoom(), 1.5);
        state.observe_fit((900.0, 620.0), (1000.0, 310.0));
        assert_eq!(state.zoom_label(), "50%");
        state.observe_fit((900.0, 620.0), (450.0, 1000.0));
        assert_eq!(state.zoom_label(), "50%");
        state.set_mode(MainViewMode::Source);
        state.observe_fit((900.0, 620.0), (1800.0, 1240.0));
        assert_eq!(state.zoom_label(), "200%");
        state.observe_fit((900.0, 620.0), (0.0, 0.0));
        assert_eq!(state.zoom_label(), "—");
        state.observe_fit((f64::NAN, 620.0), (1800.0, 1240.0));
        assert!(!state.can_zoom_in());
        state.set_fit(false);
        assert_eq!(state.zoom(), 1.5);
        assert_eq!(state.zoom_label(), "150%");
    }

    /// Checks pointer movement follows the drag and clamps to the live adjustment range.
    ///
    /// # Panics
    ///
    /// Panics if any result differs from the expected bounded adjustment projection.
    #[test]
    fn pan_adjustment_value_clamps_and_requires_overflow() {
        assert_eq!(
            pan_adjustment_value(0.0, 1000.0, 200.0, 300.0, 75.0),
            Some(225.0)
        );
        assert_eq!(
            pan_adjustment_value(0.0, 1000.0, 200.0, 20.0, 90.0),
            Some(0.0)
        );
        assert_eq!(
            pan_adjustment_value(0.0, 1000.0, 200.0, 780.0, -90.0),
            Some(800.0)
        );
        assert_eq!(pan_adjustment_value(0.0, 200.0, 200.0, 0.0, 40.0), None);
        assert_eq!(pan_adjustment_value(10.0, 9.0, 0.0, 10.0, 0.0), None);
        assert_eq!(
            pan_adjustment_value(0.0, f64::INFINITY, 100.0, 0.0, 0.0),
            None
        );
    }
}
