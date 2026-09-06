//! Thin headless export projection with cooperative terminal cancellation.

use crate::CliError;
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
};
use toniator_domain::Document;
use toniator_engine::export::video::{VideoExportJob, VideoExportOptions, VideoPhase};
use toniator_engine::{
    MediaTools,
    export::{ExportPhase, SequenceExportJob, SequenceExportOptions},
};
use toniator_io::{SourceBundle, sequence::SequenceFormat};

/// Projects exact command-line timing through the shared media timing authority.
#[derive(Debug, Default, clap::Args)]
pub(super) struct TimingArgs {
    /// Output frames per second as an integer, decimal, or rational (for example 30000/1001).
    #[arg(long, value_parser = parse_rate)]
    fps: Option<toniator_domain::FrameRate>,
    /// First included source frame at the selected output rate (zero-based).
    #[arg(long, conflicts_with_all = ["start_time", "end_time"])]
    start_frame: Option<u64>,
    /// Last included source frame at the selected output rate (zero-based).
    #[arg(long, conflicts_with_all = ["start_time", "end_time"])]
    end_frame: Option<u64>,
    /// Included source start in exact seconds, as a decimal or rational.
    #[arg(long, value_parser = parse_time)]
    start_time: Option<toniator_domain::RationalTime>,
    /// Excluded source end in exact seconds, as a decimal or rational.
    #[arg(long, value_parser = parse_time)]
    end_time: Option<toniator_domain::RationalTime>,
}

impl TimingArgs {
    /// Returns the explicitly assigned sequence rate; no filesystem-order inference is permitted.
    pub(super) fn sequence_rate(&self) -> Option<toniator_domain::FrameRate> {
        self.fps
    }

    /// Resolves shared defaults and explicit range selection without modifying the input document.
    ///
    /// # Errors
    /// Returns exact timing and source-bound diagnostics from engine/domain authority.
    pub(super) fn select(
        &self,
        metadata: &toniator_engine::SourceMediaMetadata,
        current: Option<&toniator_domain::ProjectTiming>,
    ) -> Result<toniator_domain::ProjectTiming, CliError> {
        let timing = toniator_engine::select_media_timing(
            metadata,
            current,
            &toniator_engine::MediaTimingSelection {
                frame_rate: self.fps,
                start_frame: self.start_frame,
                end_frame: self.end_frame,
                start_time: self.start_time,
                end_time: self.end_time,
            },
        )
        .map_err(|error| CliError::new(error.path(), error.message()))?;
        if metadata.variable_frame_rate {
            eprintln!(
                "Variable-rate source normalized to {}/{} fps",
                timing.frame_rate().numerator(),
                timing.frame_rate().denominator()
            );
        }
        if metadata.has_audio {
            eprintln!("Source audio is omitted; exports are silent.");
        }
        Ok(timing)
    }
}

/// Parses nonnegative exact seconds without passing through floating point.
///
/// # Errors
/// Rejects signs, exponents, malformed decimals/fractions, and integer overflow.
fn parse_time(value: &str) -> Result<toniator_domain::RationalTime, String> {
    toniator_engine::parse_media_time(value)
}

/// Parses a positive reduced exact rate within the domain's u32 component bounds.
///
/// # Errors
/// Rejects malformed/nonpositive values and rates outside the domain representation.
fn parse_rate(value: &str) -> Result<toniator_domain::FrameRate, String> {
    toniator_engine::parse_media_rate(value)
}

/// Runs the shared silent video job with terminal cancellation and phase-specific progress.
///
/// Encoding failure keeps rendered PNGs at the reported recovery path. Interactive recovery
/// actions are supplied by the engine's retained-state API for the desktop workflow.
///
/// # Errors
/// Returns static/capability/render/encode/validation/publication diagnostics and retained paths.
pub(super) fn render_project_video(
    document: Document,
    sources: SourceBundle,
    options: VideoExportOptions,
    cancelled: &AtomicBool,
) -> Result<(), CliError> {
    let job = VideoExportJob::new(document, sources, options)
        .map_err(|error| CliError::new(error.stage, error.to_string()))?;
    let last = Mutex::new(None);
    let result = job
        .run(MediaTools::default(), cancelled, &|event| {
            let mut last = last.lock().unwrap_or_else(|error| error.into_inner());
            let state = (event.phase, event.completed_frames);
            if *last == Some(state) {
                return;
            }
            *last = Some(state);
            match event.phase {
                VideoPhase::Preflight => eprintln!("Preparing silent video export..."),
                VideoPhase::Rendering => eprintln!(
                    "Rendering: {}/{} frames",
                    event.completed_frames, event.total_frames
                ),
                VideoPhase::Encoding => eprintln!(
                    "Encoding: {}/{} frames ({:.3}s)",
                    event.completed_frames,
                    event.total_frames,
                    event.encoded_time_micros as f64 / 1_000_000.0
                ),
                VideoPhase::Validating => eprintln!("Validating encoded video..."),
                _ => {}
            }
        })
        .map_err(|failure| CliError::new(failure.error.stage, failure.to_string()))?;
    println!(
        "Exported {} silent video frames to {}",
        result.frame_count,
        result.file.display()
    );
    if let Some(warning) = result.cleanup_warning {
        eprintln!("Video saved; temporary cleanup: {warning}");
    }
    Ok(())
}

/// Recognizes only the current fixed numbered-frame pattern, never arbitrary path templates.
pub(super) fn sequence_format(path: &Path) -> Option<SequenceFormat> {
    match path.file_name()?.to_str()? {
        "frame-%06d.png" => Some(SequenceFormat::Png),
        "frame-%06d.svg" => Some(SequenceFormat::Svg),
        _ => None,
    }
}

/// Runs the shared immutable sequence job and reports its finalized or retained incomplete path.
///
/// SIGINT/SIGTERM set the same flag polled by decode, evaluation and publication. Signal callbacks
/// perform no I/O; the ordinary job worker cleans up processes and leaves a truthful manifest.
///
/// # Errors
/// Returns signal-registration or shared export diagnostics without modifying the source project.
pub(super) fn render_project_sequence(
    document: Document,
    sources: SourceBundle,
    options: SequenceExportOptions,
    cancelled: &AtomicBool,
) -> Result<(), CliError> {
    let job = SequenceExportJob::new(document, sources, options)
        .map_err(|error| CliError::new(error.stage, error.to_string()))?;
    let result = job
        .run(
            MediaTools::default(),
            cancelled,
            &|event| match event.phase {
                ExportPhase::Preflight => eprintln!("Preparing frame export..."),
                ExportPhase::Rendering if event.frame.is_none() => eprintln!(
                    "Rendered {}/{} frames",
                    event.completed_frames, event.total_frames
                ),
                ExportPhase::Finalizing => eprintln!("Finalizing sequence..."),
                _ => {}
            },
        )
        .map_err(|error| {
            let retained = error
                .output
                .as_ref()
                .map(|path| format!("; incomplete sequence retained at {}", path.display()))
                .unwrap_or_default();
            CliError::new(error.stage, format!("{error}{retained}"))
        })?;
    println!(
        "Exported {} frames to {}",
        result.frame_count,
        result.directory.display()
    );
    Ok(())
}

/// Owns only signal callbacks installed for one CLI export invocation.
pub(super) struct ExportSignals(Vec<signal_hook::SigId>);
impl ExportSignals {
    /// Installs supported termination flags, rolling back earlier registrations if setup fails.
    ///
    /// # Errors
    /// Returns the operating system's signal-registration diagnostic.
    pub(super) fn new(cancelled: Arc<AtomicBool>) -> Result<Self, CliError> {
        let mut guard = Self(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            guard.0.push(
                signal_hook::flag::register(signal, Arc::clone(&cancelled))
                    .map_err(|error| CliError::new("export.signal", error.to_string()))?,
            );
        }
        Ok(guard)
    }
}
impl Drop for ExportSignals {
    /// Removes this invocation's callbacks after its worker and child processes have stopped.
    fn drop(&mut self) {
        for id in self.0.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}
