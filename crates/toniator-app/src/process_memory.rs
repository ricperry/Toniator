//! Linux process-memory sampling for the compact, read-only status projection.

/// Parses `VmRSS` and `VmHWM` from Linux `/proc/self/status` in KiB.
///
/// Missing, malformed, or partial kernel data stays unavailable instead of displaying zero as a
/// measured value. The counters describe this Toniator process, including its caches and undo
/// history; they do not include child processes or GPU allocations.
///
/// # Errors
/// Returns `None` when either required counter is absent or malformed.
pub(crate) fn parse_status(text: &str) -> Option<(u64, u64)> {
    let mut resident_kib = None;
    let mut peak_kib = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let number = value
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok());
        match key {
            "VmRSS" => resident_kib = number,
            "VmHWM" => peak_kib = number,
            _ => {}
        }
    }
    Some((resident_kib?, peak_kib?))
}

/// Reads current resident and process-lifetime peak memory from Linux procfs.
///
/// Values are estimates reported by the kernel in KiB. Unsupported systems, permissions, or
/// missing counters produce `None` and the UI exposes that unavailable state explicitly.
pub(crate) fn read() -> Option<(u64, u64)> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_status(&status)
}

/// Formats kernel KiB counters compactly for a narrow sidebar status row.
///
/// # Errors
/// Returns an unavailable message when current RSS or peak data is absent.
pub(crate) fn format(reading: Option<(u64, u64)>) -> String {
    let Some((resident_kib, peak_kib)) = reading else {
        return "Memory usage unavailable".to_owned();
    };
    format!(
        "Memory · {} current · {} peak",
        human_size(resident_kib),
        human_size(peak_kib)
    )
}

/// Uses MiB for small readings and GiB once process memory reaches that scale.
///
/// This presentation is approximate and does not transform or persist the kernel counters.
fn human_size(kib: u64) -> String {
    if kib >= 1024 * 1024 {
        format!("{:.1} GiB", kib as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} MiB", kib.div_ceil(1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the two required Linux counters and ignores unrelated status fields.
    ///
    /// # Panics
    /// Panics if a valid status excerpt is rejected or changed units.
    #[test]
    fn parses_rss_and_peak_in_kib() {
        let status = "Name:\ttoniator\nVmPeak:\t500000 kB\nVmRSS:\t12345 kB\nVmHWM:\t23456 kB\n";
        assert_eq!(parse_status(status), Some((12345, 23456)));
    }

    /// Keeps incomplete procfs data unavailable instead of presenting a false zero.
    ///
    /// # Panics
    /// Panics if a missing required counter is interpreted as a measurement.
    #[test]
    fn rejects_missing_or_malformed_counters() {
        assert_eq!(parse_status("VmRSS:\t12 kB\n"), None);
        assert_eq!(parse_status("VmRSS:\t? kB\nVmHWM:\t1 kB\n"), None);
        assert_eq!(format(None), "Memory usage unavailable");
    }

    /// Presents small and large measurements in compact human-readable units.
    ///
    /// # Panics
    /// Panics if byte-scale formatting ceases to use MiB and GiB correctly.
    #[test]
    fn formats_current_and_peak() {
        assert_eq!(
            format(Some((2048, 2 * 1024 * 1024))),
            "Memory · 2 MiB current · 2.0 GiB peak"
        );
    }
}
