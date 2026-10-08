//! Bounded, deterministic pixel-mask correction after final transparent composition.
//!
//! Widths use horizontal and vertical pixel runs at square-distance ridges.
//! Diagonal widths are therefore grid approximations, not physical measurements.

use std::sync::atomic::{AtomicBool, Ordering};

use toniator_engine::{RasterSurface, linear_to_srgb, srgb_to_linear};

/// Maximum raster subject to optional correction; larger output fails without partial export.
pub(crate) const MAX_CORRECTION_PIXELS: usize = 32_000_000;
/// Maximum artist-entered width. It bounds local expansion work and avoids unbounded paint loops.
const MAX_WIDTH: u32 = 256;
/// Limits repeated ridge expansion independently of the output pixel budget.
const MAX_EXPANSION_WRITES: u64 = 192_000_000;

/// The four user-selected treatments for a locally narrow transparent gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GapTreatment {
    FillAverage,
    FillBackground,
    FillCustom,
    GrowGap,
}

/// Export-local mask correction intent; zero thresholds disable their passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub(crate) remove_below_px: u32,
    pub(crate) minimum_feature_px: u32,
    pub(crate) minimum_gap_px: u32,
    pub(crate) gap_treatment: GapTreatment,
    pub(crate) background_fill_rgb: [u8; 3],
    pub(crate) custom_fill_rgb: [u8; 3],
}

impl Default for Settings {
    /// Starts without any shape correction; the separate alpha switch remains explicit.
    fn default() -> Self {
        Self {
            remove_below_px: 0,
            minimum_feature_px: 0,
            minimum_gap_px: 0,
            gap_treatment: GapTreatment::FillAverage,
            background_fill_rgb: [255, 255, 255],
            custom_fill_rgb: [255, 255, 255],
        }
    }
}

impl Settings {
    /// Validates bounded integer widths and the independent removal cutoff.
    ///
    /// # Errors
    /// Rejects a cutoff exceeding the feature minimum or any excessive width.
    pub(crate) fn validate(self) -> Result<(), String> {
        if self.remove_below_px > self.minimum_feature_px {
            return Err("Remove below must not exceed minimum feature width.".into());
        }
        if [
            self.remove_below_px,
            self.minimum_feature_px,
            self.minimum_gap_px,
        ]
        .into_iter()
        .any(|width| width > MAX_WIDTH)
        {
            return Err(format!(
                "Print correction widths must be at most {MAX_WIDTH} pixels."
            ));
        }
        Ok(())
    }
}

/// Corrects isolated undersized features, then surviving thin ridges, then narrow gaps.
///
/// The input must have binary transparent alpha. Whole connected components below
/// the removal cutoff disappear; an attached spur belongs to its larger component.
/// Thin ridges expand across their short horizontal/vertical run. Gap treatment
/// runs last and wins where carving conflicts with feature thickening.
///
/// # Errors
/// Rejects nonbinary input, invalid settings, over-budget work, cancellation, and allocation.
pub(crate) fn apply(
    surface: &RasterSurface,
    settings: Settings,
    cancelled: &AtomicBool,
) -> Result<RasterSurface, String> {
    settings.validate()?;
    if settings.remove_below_px == 0
        && settings.minimum_feature_px == 0
        && settings.minimum_gap_px == 0
    {
        return Ok(surface.clone());
    }
    let width = usize::try_from(surface.width())
        .map_err(|_| "Print width exceeds the platform.".to_owned())?;
    let height = usize::try_from(surface.height())
        .map_err(|_| "Print height exceeds the platform.".to_owned())?;
    let count = width
        .checked_mul(height)
        .ok_or("Print pixel count overflows.")?;
    if count > MAX_CORRECTION_PIXELS {
        return Err("Selected PNG exceeds the bounded print-correction raster budget.".into());
    }
    let mut mask = allocated(count, false)?;
    for (index, pixel) in surface.pixels().chunks_exact(4).enumerate() {
        check_cancel(index, cancelled)?;
        match pixel[3] {
            0 => mask[index] = false,
            255 => mask[index] = true,
            _ => return Err("Print correction requires binary alpha before mask work.".into()),
        }
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(surface.pixels().len())
        .map_err(|error| format!("Print correction pixel allocation failed: {error}"))?;
    pixels.extend_from_slice(surface.pixels());
    if settings.remove_below_px > 0 {
        remove_small_components(
            &mut mask,
            &mut pixels,
            width,
            height,
            settings.remove_below_px,
            cancelled,
        )?;
    }
    let (horizontal, vertical) = run_lengths(&mask, width, height, cancelled)?;
    let mut gap_source = allocated(0, false)?;
    if settings.minimum_gap_px > 0 {
        gap_source = allocated(mask.len(), false)?;
        gap_source.copy_from_slice(&mask);
    }
    if settings.minimum_feature_px > 0 {
        thicken_ridges(
            &mut mask,
            &mut pixels,
            &horizontal,
            &vertical,
            width,
            height,
            settings.minimum_feature_px,
            cancelled,
        )?;
    }
    if settings.minimum_gap_px > 0 {
        treat_gaps(
            &gap_source,
            surface.pixels(),
            &mut mask,
            &mut pixels,
            &horizontal,
            &vertical,
            width,
            height,
            settings,
            cancelled,
        )?;
    }
    RasterSurface::new(surface.width(), surface.height(), pixels).map_err(|error| error.to_string())
}

/// Allocates a checked, initialized scratch vector without exposing partial state.
///
/// # Errors
/// Returns a capacity error when the requested scratch space cannot be reserved.
fn allocated<T: Clone>(count: usize, value: T) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| format!("Print correction allocation failed: {error}"))?;
    values.resize(count, value);
    Ok(values)
}

/// Polls cancellation at a bounded pixel interval without making loop order observable.
///
/// # Errors
/// Returns cancellation before publishing a partial corrected surface.
fn check_cancel(index: usize, cancelled: &AtomicBool) -> Result<(), String> {
    if index.is_multiple_of(4096) && cancelled.load(Ordering::Acquire) {
        Err("Print correction cancelled.".into())
    } else {
        Ok(())
    }
}

/// Computes exact horizontal and vertical same-mask run lengths for every pixel.
///
/// # Errors
/// Returns bounded allocation failure; dimensions are checked by `apply`.
fn run_lengths(
    mask: &[bool],
    width: usize,
    height: usize,
    cancelled: &AtomicBool,
) -> Result<(Vec<u16>, Vec<u16>), String> {
    let mut horizontal = allocated(mask.len(), 0_u16)?;
    let mut vertical = allocated(mask.len(), 0_u16)?;
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let start = x;
            let value = mask[y * width + x];
            while x < width && mask[y * width + x] == value {
                check_cancel(y * width + x, cancelled)?;
                x += 1;
            }
            let length = u16::try_from(x - start).unwrap_or(u16::MAX);
            for run_x in start..x {
                horizontal[y * width + run_x] = length;
            }
        }
    }
    for x in 0..width {
        let mut y = 0;
        while y < height {
            let start = y;
            let value = mask[y * width + x];
            while y < height && mask[y * width + x] == value {
                check_cancel(y * width + x, cancelled)?;
                y += 1;
            }
            let length = u16::try_from(y - start).unwrap_or(u16::MAX);
            for run_y in start..y {
                vertical[run_y * width + x] = length;
            }
        }
    }
    Ok((horizontal, vertical))
}

/// Removes complete eight-connected foreground components below the explicit width cutoff.
///
/// # Errors
/// Returns cancellation or scratch allocation failure without publishing output.
fn remove_small_components(
    mask: &mut [bool],
    pixels: &mut [u8],
    width: usize,
    height: usize,
    cutoff: u32,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let mut core = allocated(mask.len(), 0_u16)?;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            check_cancel(index, cancelled)?;
            if !mask[index] {
                continue;
            }
            core[index] = if x > 0 && y > 0 {
                1 + core[index - 1]
                    .min(core[index - width])
                    .min(core[index - width - 1])
            } else {
                1
            };
        }
    }
    let mut seen = allocated(mask.len(), false)?;
    let mut component = Vec::new();
    component
        .try_reserve(1024)
        .map_err(|error| format!("Print correction component allocation failed: {error}"))?;
    for start in 0..mask.len() {
        check_cancel(start, cancelled)?;
        if !mask[start] || seen[start] {
            continue;
        }
        component.clear();
        component.push(start);
        seen[start] = true;
        let mut maximum_width = 0;
        let mut cursor = 0;
        while cursor < component.len() {
            let index = component[cursor];
            cursor += 1;
            maximum_width = maximum_width.max(core[index]);
            let x = index % width;
            let y = index / width;
            for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = ny * width + nx;
                    if mask[neighbor] && !seen[neighbor] {
                        seen[neighbor] = true;
                        if component.len() == component.capacity() {
                            component.try_reserve(1024).map_err(|error| {
                                format!("Print correction component allocation failed: {error}")
                            })?;
                        }
                        component.push(neighbor);
                    }
                }
            }
            check_cancel(cursor, cancelled)?;
        }
        if u32::from(maximum_width) < cutoff {
            for &index in &component {
                mask[index] = false;
                pixels[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Ok(())
}

/// Computes a two-pass square-grid distance to the opposite mask value.
///
/// Background canvas exterior is treated as background for positive features;
/// negative-gap selection separately requires bounded opaque edges.
///
/// # Errors
/// Returns an allocation failure before any caller-visible pixels change.
fn distance_map(
    mask: &[bool],
    width: usize,
    height: usize,
    foreground: bool,
    cancelled: &AtomicBool,
) -> Result<Vec<u16>, String> {
    let mut distance = allocated(mask.len(), 0_u16)?;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            check_cancel(index, cancelled)?;
            if mask[index] != foreground {
                continue;
            }
            let mut value = if foreground {
                (x + 1)
                    .min(y + 1)
                    .min(width - x)
                    .min(height - y)
                    .min(u16::MAX as usize) as u16
            } else {
                u16::MAX / 2
            };
            if x > 0 {
                value = value.min(distance[index - 1].saturating_add(1));
            }
            if y > 0 {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    value = value.min(distance[(y - 1) * width + nx].saturating_add(1));
                }
            }
            distance[index] = value;
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let index = y * width + x;
            check_cancel(index, cancelled)?;
            if mask[index] != foreground {
                continue;
            }
            let mut value = distance[index];
            if x + 1 < width {
                value = value.min(distance[index + 1].saturating_add(1));
            }
            if y + 1 < height {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    value = value.min(distance[(y + 1) * width + nx].saturating_add(1));
                }
            }
            distance[index] = value;
        }
    }
    Ok(distance)
}

/// Selects a local square-distance ridge, including even-width center plateaus.
fn is_ridge(distance: &[u16], x: usize, y: usize, width: usize, height: usize) -> bool {
    let value = distance[y * width + x];
    if value == 0 {
        return false;
    }
    for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
        for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
            if distance[ny * width + nx] > value {
                return false;
            }
        }
    }
    true
}

/// Expands only narrow-axis foreground runs selected by an original-mask ridge.
///
/// # Errors
/// Returns cancellation or allocation failure; caller discards partial work.
#[allow(clippy::too_many_arguments)] // Separate borrowed raster buffers avoid extra peak allocations.
fn thicken_ridges(
    mask: &mut [bool],
    pixels: &mut [u8],
    horizontal: &[u16],
    vertical: &[u16],
    width: usize,
    height: usize,
    minimum: u32,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let distance = distance_map(mask, width, height, true, cancelled)?;
    let mut original = allocated(mask.len(), false)?;
    original.copy_from_slice(mask);
    let mut expansion_writes = 0_u64;
    let mut visited_horizontal = allocated(mask.len(), false)?;
    let mut visited_vertical = allocated(mask.len(), false)?;
    for index in 0..mask.len() {
        check_cancel(index, cancelled)?;
        if !original[index] || u32::from(horizontal[index].min(vertical[index])) >= minimum {
            continue;
        }
        let x = index % width;
        let y = index / width;
        if !is_ridge(&distance, x, y, width, height) {
            continue;
        }
        // A disk's one-pixel outer cap is not an independent thin stroke: its
        // neighboring cross-section immediately opens into the broad body.
        // Real narrow bars retain their short cross-section along the axis.
        let short_horizontal = horizontal[index] < vertical[index];
        let short_vertical = vertical[index] < horizontal[index];
        if short_horizontal
            && [y.checked_sub(1), (y + 1 < height).then_some(y + 1)]
                .into_iter()
                .flatten()
                .any(|neighbor_y| {
                    let neighbor = neighbor_y * width + x;
                    original[neighbor] && u32::from(horizontal[neighbor]) >= minimum
                })
        {
            continue;
        }
        if short_vertical
            && [x.checked_sub(1), (x + 1 < width).then_some(x + 1)]
                .into_iter()
                .flatten()
                .any(|neighbor_x| {
                    let neighbor = y * width + neighbor_x;
                    original[neighbor] && u32::from(vertical[neighbor]) >= minimum
                })
        {
            continue;
        }
        if horizontal[index] > vertical[index] && !visited_horizontal[index] {
            let qualifies = |px: usize| {
                let at = y * width + px;
                original[at] && vertical[at] < minimum as u16 && horizontal[at] > vertical[at]
            };
            let mut left = x;
            let mut right = x;
            while left > 0 && qualifies(left - 1) {
                left -= 1;
                check_cancel(y * width + left, cancelled)?;
            }
            while right + 1 < width && qualifies(right + 1) {
                right += 1;
                check_cancel(y * width + right, cancelled)?;
            }
            for px in left..=right {
                let at = y * width + px;
                visited_horizontal[at] = true;
                expand_feature_at(
                    mask,
                    pixels,
                    &original,
                    horizontal,
                    vertical,
                    width,
                    height,
                    at,
                    minimum,
                    &mut expansion_writes,
                    cancelled,
                )?;
            }
        } else if vertical[index] > horizontal[index] && !visited_vertical[index] {
            let qualifies = |py: usize| {
                let at = py * width + x;
                original[at] && horizontal[at] < minimum as u16 && vertical[at] > horizontal[at]
            };
            let mut top = y;
            let mut bottom = y;
            while top > 0 && qualifies(top - 1) {
                top -= 1;
                check_cancel(top * width + x, cancelled)?;
            }
            while bottom + 1 < height && qualifies(bottom + 1) {
                bottom += 1;
                check_cancel(bottom * width + x, cancelled)?;
            }
            for py in top..=bottom {
                let at = py * width + x;
                visited_vertical[at] = true;
                expand_feature_at(
                    mask,
                    pixels,
                    &original,
                    horizontal,
                    vertical,
                    width,
                    height,
                    at,
                    minimum,
                    &mut expansion_writes,
                    cancelled,
                )?;
            }
        } else if horizontal[index] == vertical[index] {
            expand_feature_at(
                mask,
                pixels,
                &original,
                horizontal,
                vertical,
                width,
                height,
                index,
                minimum,
                &mut expansion_writes,
                cancelled,
            )?;
        }
    }
    Ok(())
}

/// Paints one narrow-axis run using an original opaque sample and a checked work budget.
///
/// # Errors
/// Returns cancellation or work overflow without publishing the caller's partial image.
#[allow(clippy::too_many_arguments)] // The bounded inner loop borrows its original mask and output in place.
fn expand_feature_at(
    mask: &mut [bool],
    pixels: &mut [u8],
    original: &[bool],
    horizontal: &[u16],
    vertical: &[u16],
    width: usize,
    height: usize,
    index: usize,
    minimum: u32,
    writes: &mut u64,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let x = index % width;
    let y = index / width;
    let (left, right) = if horizontal[index] <= vertical[index] {
        expanded_run(original, width, height, x, y, true, minimum)
    } else {
        (x, x)
    };
    let (top, bottom) = if vertical[index] <= horizontal[index] {
        expanded_run(original, width, height, x, y, false, minimum)
    } else {
        (y, y)
    };
    let area = (right - left + 1)
        .checked_mul(bottom - top + 1)
        .ok_or("Print expansion work overflows.")?;
    *writes = writes
        .checked_add(area as u64)
        .ok_or("Print expansion work overflows.")?;
    if *writes > MAX_EXPANSION_WRITES {
        return Err("Print expansion exceeds the bounded work budget.".into());
    }
    let rgb = [
        pixels[index * 4],
        pixels[index * 4 + 1],
        pixels[index * 4 + 2],
    ];
    for py in top..=bottom {
        check_cancel(py * width + left, cancelled)?;
        for px in left..=right {
            let target = py * width + px;
            if !mask[target] {
                mask[target] = true;
                pixels[target * 4..target * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    Ok(())
}

/// Finds a same-mask run and extends only its short axis to a centered target width.
fn expanded_run(
    mask: &[bool],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    horizontal: bool,
    minimum: u32,
) -> (usize, usize) {
    let coordinate = if horizontal { x } else { y };
    let maximum = if horizontal { width } else { height };
    let at = |value: usize| {
        mask[if horizontal {
            y * width + value
        } else {
            value * width + x
        }]
    };
    let mut start = coordinate;
    let mut end = coordinate;
    while start > 0 && at(start - 1) {
        start -= 1;
    }
    while end + 1 < maximum && at(end + 1) {
        end += 1;
    }
    centered_interval(
        start,
        end,
        maximum,
        usize::try_from(minimum).unwrap_or(usize::MAX),
    )
}

/// Expands a run to its requested span and shifts any clipped side to available canvas space.
fn centered_interval(start: usize, end: usize, maximum: usize, requested: usize) -> (usize, usize) {
    let desired = requested.max(end - start + 1).min(maximum);
    let extra = desired - (end - start + 1);
    let from = start.saturating_sub(extra / 2);
    let to = from.saturating_add(desired - 1).min(maximum - 1);
    (to + 1 - desired, to)
}

/// Applies one final gap policy to short, opaque-bounded runs near gap ridges.
///
/// # Errors
/// Returns cancellation or scratch allocation failure before caller publication.
#[allow(clippy::too_many_arguments)] // The final pass requires both saved source and mutable output buffers.
fn treat_gaps(
    source: &[bool],
    source_pixels: &[u8],
    mask: &mut [bool],
    pixels: &mut [u8],
    horizontal: &[u16],
    vertical: &[u16],
    width: usize,
    height: usize,
    settings: Settings,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let distance = distance_map(source, width, height, false, cancelled)?;
    let mut candidate = allocated(mask.len(), false)?;
    // Full bounded runs include finite corridor caps; ridge seeds prevent isolated
    // broad-void corners from becoming a global fill target.
    for axis in [true, false] {
        let lines = if axis { height } else { width };
        let maximum = if axis { width } else { height };
        for line in 0..lines {
            let at = |position: usize| {
                if axis {
                    line * width + position
                } else {
                    position * width + line
                }
            };
            let mut position = 0;
            while position < maximum {
                check_cancel(at(position), cancelled)?;
                if source[at(position)] {
                    position += 1;
                    continue;
                }
                let start = position;
                while position < maximum && !source[at(position)] {
                    check_cancel(at(position), cancelled)?;
                    position += 1;
                }
                if start > 0
                    && position < maximum
                    && position - start < settings.minimum_gap_px as usize
                {
                    for member in start..position {
                        candidate[at(member)] = true;
                    }
                }
            }
        }
    }
    let mut seen = allocated(mask.len(), false)?;
    let mut selected = allocated(mask.len(), false)?;
    let mut component = Vec::new();
    component
        .try_reserve(1024)
        .map_err(|error| format!("Print gap allocation failed: {error}"))?;
    for start in 0..source.len() {
        check_cancel(start, cancelled)?;
        if !candidate[start] || seen[start] {
            continue;
        }
        component.clear();
        component.push(start);
        seen[start] = true;
        let mut ridge_found = false;
        let mut cursor = 0;
        while cursor < component.len() {
            let index = component[cursor];
            cursor += 1;
            check_cancel(cursor, cancelled)?;
            let x = index % width;
            let y = index / width;
            ridge_found |= is_ridge(&distance, x, y, width, height);
            for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = ny * width + nx;
                    if candidate[neighbor] && !seen[neighbor] {
                        seen[neighbor] = true;
                        if component.len() == component.capacity() {
                            component
                                .try_reserve(1024)
                                .map_err(|error| format!("Print gap allocation failed: {error}"))?;
                        }
                        component.push(neighbor);
                    }
                }
            }
        }
        if ridge_found {
            for &member in &component {
                selected[member] = true;
            }
        }
    }
    let targets = if settings.gap_treatment == GapTreatment::GrowGap {
        let mut carved = allocated(mask.len(), false)?;
        let mut writes = 0_u64;
        for axis in [true, false] {
            let lines = if axis { height } else { width };
            let maximum = if axis { width } else { height };
            for line in 0..lines {
                let at = |position: usize| {
                    if axis {
                        line * width + position
                    } else {
                        position * width + line
                    }
                };
                let mut position = 0;
                while position < maximum {
                    check_cancel(at(position), cancelled)?;
                    if source[at(position)] {
                        position += 1;
                        continue;
                    }
                    let start = position;
                    while position < maximum && !source[at(position)] {
                        position += 1;
                    }
                    if start > 0
                        && position < maximum
                        && position - start < settings.minimum_gap_px as usize
                        && (start..position).any(|member| selected[at(member)])
                    {
                        let (from, to) = centered_interval(
                            start,
                            position - 1,
                            maximum,
                            settings.minimum_gap_px as usize,
                        );
                        writes = writes
                            .checked_add((to - from + 1) as u64)
                            .ok_or("Gap growth work overflows.")?;
                        if writes > MAX_EXPANSION_WRITES {
                            return Err("Gap growth exceeds the bounded work budget.".into());
                        }
                        for member in from..=to {
                            carved[at(member)] = true;
                        }
                    }
                }
            }
        }
        carved
    } else {
        selected
    };
    let mut average_work = 0_u64;
    for index in 0..mask.len() {
        check_cancel(index, cancelled)?;
        if !targets[index] {
            continue;
        }
        match settings.gap_treatment {
            GapTreatment::GrowGap => {
                mask[index] = false;
                pixels[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
            _ => {
                mask[index] = true;
                let rgb = match settings.gap_treatment {
                    GapTreatment::FillAverage => {
                        average_work = average_work
                            .checked_add(u64::from(settings.minimum_gap_px) * 2)
                            .ok_or("Average fill work overflows.")?;
                        if average_work > MAX_EXPANSION_WRITES {
                            return Err("Average gap fill exceeds the bounded work budget.".into());
                        }
                        average_surrounding_edges(
                            source,
                            source_pixels,
                            horizontal,
                            vertical,
                            width,
                            height,
                            index,
                            settings.minimum_gap_px,
                        )?
                    }
                    GapTreatment::FillBackground => settings.background_fill_rgb,
                    GapTreatment::FillCustom => settings.custom_fill_rgb,
                    GapTreatment::GrowGap => unreachable!(),
                };
                pixels[index * 4..index * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    Ok(())
}

/// Averages each unique original opaque endpoint of a selected narrow gap in linear sRGB.
///
/// # Errors
/// Rejects a target that no longer has an original opaque bounded edge.
#[allow(clippy::too_many_arguments)] // Endpoint lookup uses the saved mask, pixels, and both run maps.
fn average_surrounding_edges(
    mask: &[bool],
    pixels: &[u8],
    horizontal: &[u16],
    vertical: &[u16],
    width: usize,
    height: usize,
    index: usize,
    minimum: u32,
) -> Result<[u8; 3], String> {
    let x = index % width;
    let y = index / width;
    let mut edges = [0_usize; 4];
    let mut count = 0;
    for horizontal_axis in [true, false] {
        let length = if horizontal_axis {
            horizontal[index]
        } else {
            vertical[index]
        };
        if u32::from(length) >= minimum {
            continue;
        }
        let coordinate = if horizontal_axis { x } else { y };
        let maximum = if horizontal_axis { width } else { height };
        let at = |value: usize| {
            if horizontal_axis {
                y * width + value
            } else {
                value * width + x
            }
        };
        let mut start = coordinate;
        let mut end = coordinate;
        while start > 0 && !mask[at(start - 1)] {
            start -= 1;
        }
        while end + 1 < maximum && !mask[at(end + 1)] {
            end += 1;
        }
        if start == 0 || end + 1 >= maximum {
            continue;
        }
        for edge in [at(start - 1), at(end + 1)] {
            if mask[edge] && !edges[..count].contains(&edge) {
                edges[count] = edge;
                count += 1;
            }
        }
    }
    if count == 0 {
        return Err("Average gap fill lost its original opaque edges.".into());
    }
    let mut rgb = [0; 3];
    for channel in 0..3 {
        let linear = edges[..count]
            .iter()
            .map(|&edge| srgb_to_linear(f64::from(pixels[edge * 4 + channel]) / 255.0))
            .sum::<f64>()
            / count as f64;
        rgb[channel] = (linear_to_srgb(linear) * 255.0).round() as u8;
    }
    Ok(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Converts ASCII mask rows to a native binary RGBA witness with stable color.
    fn surface(rows: &[&str]) -> RasterSurface {
        let width = rows[0].len();
        let mut pixels = Vec::new();
        for row in rows {
            assert_eq!(row.len(), width);
            for pixel in row.bytes() {
                pixels.extend_from_slice(if pixel == b'#' {
                    &[20, 80, 140, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        RasterSurface::new(width as u32, rows.len() as u32, pixels).unwrap()
    }

    /// Reads one output alpha without treating RGB of transparent pixels as artwork.
    fn alpha(surface: &RasterSurface, x: usize, y: usize) -> u8 {
        surface.pixels()[(y * surface.width() as usize + x) * 4 + 3]
    }

    /// Distinguishes one-, two-, and three-pixel strokes and shifts edge expansion inward.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn feature_widths_and_canvas_edge_are_explicit() {
        let cancelled = AtomicBool::new(false);
        let mut settings = Settings {
            minimum_feature_px: 3,
            ..Settings::default()
        };
        for (top, thickness) in [(4, 1), (3, 2), (3, 3), (0, 1)] {
            let mut rows = vec![".........".to_owned(); 9];
            for row in &mut rows[top..top + thickness] {
                *row = "..#####..".to_owned();
            }
            let input = surface(&rows.iter().map(String::as_str).collect::<Vec<_>>());
            let output = apply(&input, settings, &cancelled).unwrap();
            let expected_top = if top == 0 { 0 } else { 3 };
            for y in expected_top..expected_top + 3 {
                assert_eq!(alpha(&output, 4, y), 255);
            }
            assert_eq!(
                output
                    .pixels()
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .filter(|&&a| a != 0 && a != 255)
                    .count(),
                0
            );
        }
        settings.minimum_feature_px = 0;
        assert_eq!(
            apply(&surface(&["#"]), settings, &cancelled)
                .unwrap()
                .pixels(),
            &[20, 80, 140, 255]
        );
    }

    /// Removes isolated tiny components including a clipped edge island, retaining an attached spur.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn removal_uses_whole_component_width() {
        let input = surface(&[
            "#...........",
            "............",
            "............",
            "...###......",
            "...####.....",
            "...###......",
            "............",
            "..........#.",
        ]);
        let settings = Settings {
            remove_below_px: 2,
            minimum_feature_px: 2,
            ..Settings::default()
        };
        let output = apply(&input, settings, &AtomicBool::new(false)).unwrap();
        assert_eq!(alpha(&output, 0, 0), 0);
        assert_eq!(alpha(&output, 10, 7), 0);
        assert_eq!(alpha(&output, 6, 4), 255);
        assert_eq!(alpha(&output, 4, 4), 255);
    }

    /// Fills an open one-pixel channel, leaves a two-pixel channel at threshold two, and preserves a wide void.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn bounded_gap_widths_and_wide_void() {
        let one = surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]);
        let two = surface(&["##..###", "##..###", "##..###", "##..###", "##..###"]);
        let settings = Settings {
            minimum_gap_px: 2,
            gap_treatment: GapTreatment::FillBackground,
            background_fill_rgb: [9, 19, 29],
            ..Settings::default()
        };
        let a = apply(&one, settings, &AtomicBool::new(false)).unwrap();
        let b = apply(&two, settings, &AtomicBool::new(false)).unwrap();
        assert_eq!(alpha(&a, 3, 0), 255);
        assert_eq!(
            &a.pixels()[(2 * 7 + 3) * 4..(2 * 7 + 3) * 4 + 4],
            &[9, 19, 29, 255]
        );
        assert_eq!(alpha(&b, 2, 2), 0);
        let wide = surface(&[
            "#########",
            "#.......#",
            "#.......#",
            "#.......#",
            "#########",
        ]);
        let unchanged = apply(&wide, settings, &AtomicBool::new(false)).unwrap();
        assert_eq!(unchanged.pixels(), wide.pixels());
    }

    /// Combines all four distinct opaque hole edges in linear sRGB for average fill.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn average_fill_uses_four_unique_edges() {
        let mut input = surface(&[".#.", "#.#", ".#."]);
        let mut pixels = input.pixels().to_vec();
        for (x, y, rgb) in [
            (1, 0, [0, 255, 0]),
            (0, 1, [255, 0, 0]),
            (2, 1, [0, 0, 255]),
            (1, 2, [0, 0, 0]),
        ] {
            let index = (y * 3 + x) * 4;
            pixels[index..index + 3].copy_from_slice(&rgb);
        }
        input = RasterSurface::new(3, 3, pixels).unwrap();
        let output = apply(
            &input,
            Settings {
                minimum_gap_px: 2,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(&output.pixels()[16..20], &[137, 137, 137, 255]);
    }

    /// Expands a narrow gap to its target span, including when one side clips at the canvas.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn grow_gap_carves_only_a_local_bounded_channel() {
        let input = surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]);
        let output = apply(
            &input,
            Settings {
                minimum_gap_px: 3,
                gap_treatment: GapTreatment::GrowGap,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            (
                alpha(&output, 2, 2),
                alpha(&output, 3, 2),
                alpha(&output, 4, 2)
            ),
            (0, 0, 0)
        );
        assert_eq!((alpha(&output, 0, 2), alpha(&output, 6, 2)), (255, 255));
    }

    /// Confirms a common 4500×5400 target fits the explicit correction pixel budget.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn common_print_target_is_within_budget() {
        assert!(4_500_usize * 5_400 <= MAX_CORRECTION_PIXELS);
    }

    /// Keeps the complete finite three-pixel bar at four-pixel width, including end columns.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn finite_bar_caps_receive_feature_width() {
        let input = surface(&[
            "...........",
            "...........",
            "..#######..",
            "..#######..",
            "..#######..",
            "...........",
            "...........",
        ]);
        let output = apply(
            &input,
            Settings {
                minimum_feature_px: 4,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        for x in 2..=8 {
            assert_eq!(
                (0..7).filter(|&y| alpha(&output, x, y) == 255).count(),
                4,
                "column {x}"
            );
        }
    }

    /// Fills all finite slit caps after a center ridge qualifies the connected narrow run.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn finite_slit_caps_receive_gap_treatment() {
        let input = surface(&[
            "###########",
            "###########",
            "##...######",
            "##...######",
            "##...######",
            "###########",
            "###########",
        ]);
        let output = apply(
            &input,
            Settings {
                minimum_gap_px: 4,
                gap_treatment: GapTreatment::FillCustom,
                custom_fill_rgb: [4, 8, 12],
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        for y in 2..=4 {
            for x in 2..=4 {
                assert_eq!(alpha(&output, x, y), 255);
            }
        }
    }

    /// Applies saved narrow-gap intent after feature growth has covered the original gap.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn gap_last_wins_against_feature_thickening() {
        let input = surface(&[
            ".......", "..#.#..", "..#.#..", "..#.#..", "..#.#..", "..#.#..", ".......",
        ]);
        let output = apply(
            &input,
            Settings {
                minimum_feature_px: 3,
                minimum_gap_px: 3,
                gap_treatment: GapTreatment::GrowGap,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        for y in 2..=4 {
            assert_eq!(alpha(&output, 3, y), 0);
        }
    }

    /// A one-pixel plus junction has no two-by-two opaque core and is removable as one island.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn plus_junction_does_not_fake_broad_component_width() {
        let input = surface(&["..#..", "..#..", "#####", "..#..", "..#.."]);
        let output = apply(
            &input,
            Settings {
                remove_below_px: 2,
                minimum_feature_px: 2,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(output.pixels().chunks_exact(4).all(|pixel| pixel[3] == 0));
    }

    /// A broad disk remains mostly stable while the local ridge pass avoids a blanket dilation.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn broad_disk_is_not_blanket_dilated() {
        let mut pixels = Vec::new();
        for y in 0_i32..31 {
            for x in 0_i32..31 {
                pixels.extend_from_slice(if (x - 15).pow(2) + (y - 15).pow(2) <= 10_i32.pow(2) {
                    &[50, 100, 150, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        let input = RasterSurface::new(31, 31, pixels).unwrap();
        let output = apply(
            &input,
            Settings {
                minimum_feature_px: 4,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        let added = input
            .pixels()
            .chunks_exact(4)
            .zip(output.pixels().chunks_exact(4))
            .filter(|(before, after)| before[3] == 0 && after[3] == 255)
            .count();
        assert!(added < 20, "broad disk gained {added} boundary pixels");
    }

    /// A broad round halftone dot has no local thin feature at its extreme caps.
    ///
    /// # Panics
    /// Panics if the analytic witness, correction, or assertion fails.
    #[test]
    fn broad_round_dot_keeps_its_exact_binary_shape() {
        let mut pixels = Vec::new();
        for y in 0_i32..49 {
            for x in 0_i32..49 {
                pixels.extend_from_slice(if (x - 24).pow(2) + (y - 24).pow(2) <= 16_i32.pow(2) {
                    &[50, 100, 150, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        let input = RasterSurface::new(49, 49, pixels).unwrap();
        for minimum in [3, 4] {
            let output = apply(
                &input,
                Settings {
                    minimum_feature_px: minimum,
                    ..Settings::default()
                },
                &AtomicBool::new(false),
            )
            .unwrap();
            let added = input
                .pixels()
                .chunks_exact(4)
                .zip(output.pixels().chunks_exact(4))
                .filter(|(before, after)| before != after)
                .count();
            assert_eq!(added, 0, "minimum {minimum} changed {added} pixels");
        }
    }

    /// Keeps a broad dot stable while thickening an attached, genuinely narrow spur.
    ///
    /// # Panics
    /// Panics if an original broad-body pixel changes or the spur remains one pixel wide.
    #[test]
    fn attached_spur_thickens_without_round_dot_bumps() {
        let mut pixels = Vec::new();
        for y in 0_i32..49 {
            for x in 0_i32..49 {
                let dot = (x - 24).pow(2) + (y - 24).pow(2) <= 16_i32.pow(2);
                let spur = x == 24 && (2..=8).contains(&y);
                pixels.extend_from_slice(if dot || spur {
                    &[50, 100, 150, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        let input = RasterSurface::new(49, 49, pixels).unwrap();
        let output = apply(
            &input,
            Settings {
                minimum_feature_px: 3,
                ..Settings::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            (20..=28).filter(|&x| alpha(&output, x, 3) == 255).count(),
            3
        );
        for y in 9_i32..49 {
            for x in 0_i32..49 {
                let index = (y as usize * 49 + x as usize) * 4;
                assert_eq!(
                    &output.pixels()[index..index + 4],
                    &input.pixels()[index..index + 4],
                    "dot at {x},{y}"
                );
            }
        }
    }

    /// Writes raw before/after PNG witnesses for visual review of shape correction.
    ///
    /// # Panics
    /// Panics if correction, native PNG encoding, or stage-local evidence writing fails.
    #[test]
    fn native_cleanup_png_witnesses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/validation/garment-binary-alpha-20261007/analytic");
        std::fs::create_dir_all(&root).unwrap();
        let cases = [
            (
                "remove",
                surface(&[
                    "#...........",
                    "............",
                    "...###......",
                    "...###......",
                    "...###......",
                ]),
                Settings {
                    remove_below_px: 2,
                    minimum_feature_px: 2,
                    ..Settings::default()
                },
            ),
            (
                "feature",
                surface(&[
                    "...........",
                    "..#######..",
                    "..#######..",
                    "..#######..",
                    "...........",
                ]),
                Settings {
                    minimum_feature_px: 4,
                    ..Settings::default()
                },
            ),
            (
                "gap_average",
                surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]),
                Settings {
                    minimum_gap_px: 2,
                    ..Settings::default()
                },
            ),
            (
                "gap_background",
                surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]),
                Settings {
                    minimum_gap_px: 2,
                    gap_treatment: GapTreatment::FillBackground,
                    background_fill_rgb: [112, 64, 160],
                    ..Settings::default()
                },
            ),
            (
                "gap_custom",
                surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]),
                Settings {
                    minimum_gap_px: 2,
                    gap_treatment: GapTreatment::FillCustom,
                    custom_fill_rgb: [10, 170, 60],
                    ..Settings::default()
                },
            ),
            (
                "gap_grow",
                surface(&["###.###", "###.###", "###.###", "###.###", "###.###"]),
                Settings {
                    minimum_gap_px: 3,
                    gap_treatment: GapTreatment::GrowGap,
                    ..Settings::default()
                },
            ),
        ];
        for (name, before, settings) in cases {
            let after = apply(&before, settings, &AtomicBool::new(false)).unwrap();
            std::fs::write(
                root.join(format!("{name}-before.png")),
                toniator_engine::encode_png(&before).unwrap(),
            )
            .unwrap();
            std::fs::write(
                root.join(format!("{name}-after.png")),
                toniator_engine::encode_png(&after).unwrap(),
            )
            .unwrap();
        }
    }
}
