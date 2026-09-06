//! Recoverable software video encoding from the shared canonical PNG sequence runner.

#[path = "export_video_process.rs"]
mod process;

#[cfg(test)]
#[path = "export_video_tests.rs"]
mod tests;

use super::*;
use toniator_domain::{FrameRate, ProjectTiming};
use toniator_io::video_output::{RenderWorkspace, VideoOutput};

/// Captures the exact encoded stream expected from a job without redefining project timing.
#[derive(Clone, Copy)]
struct VideoStreamSpec {
    width: u32,
    height: u32,
    frame_count: u64,
    frame_rate: FrameRate,
}

/// Selects lossless preservation output or explicitly lossy sharing output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoCodec {
    #[default]
    Ffv1Matroska,
    Av1Webm,
}

/// Keeps video destination, codec and consumer settings outside authored document authority.
#[derive(Clone, Debug)]
pub struct VideoExportOptions {
    pub destination: PathBuf,
    pub codec: VideoCodec,
    /// Defaults to `/tmp`; a caller may explicitly choose another temporary filesystem.
    pub temporary_directory: Option<PathBuf>,
    pub background: Option<RasterBackground>,
    pub target: Option<OutputRasterTarget>,
    pub antialiasing: RasterAntialiasing,
    pub limits: EvaluationLimits,
}

/// Separates rendering and encoding work instead of assigning encoding the last few percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoPhase {
    Preflight,
    Rendering,
    Encoding,
    Validating,
    SavingPngs,
    Complete,
}

/// Reports phase-local completed frames/time and optional actual current-render work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProgress {
    pub phase: VideoPhase,
    pub completed_frames: u64,
    pub total_frames: u64,
    pub encoded_time_micros: u64,
    pub render: Option<ExportProgress>,
}

/// Captures the immutable document and consumer choices for a complete video operation.
#[derive(Clone, Debug)]
pub struct VideoExportJob {
    sequence: SequenceExportJob,
    options: VideoExportOptions,
}

/// Identifies a successfully finalized video and any actual temporary-cleanup limitation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoExportResult {
    pub file: PathBuf,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub retained_intermediates: Option<PathBuf>,
    pub cleanup_warning: Option<String>,
}

/// Returns complete rendered PNGs to the caller when encoding fails and recovery is possible.
#[derive(Debug)]
pub struct VideoExportFailure {
    pub error: ExportError,
    pub recovery: Option<Box<VideoRecovery>>,
}
impl std::fmt::Display for VideoExportFailure {
    /// Preserves the underlying phase diagnostic and the recoverable PNG location.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(path) = self
            .recovery
            .as_ref()
            .and_then(|recovery| recovery.frames_path())
        {
            write!(f, "; rendered PNGs retained at {}", path.display())?;
        }
        Ok(())
    }
}
impl std::error::Error for VideoExportFailure {}

/// Retains completed private PNGs for Retry encoding, Save PNG sequence, or Discard.
///
/// Dropping this value deliberately preserves its path. Only explicit discard or successful
/// encoding/copy cleanup removes job-owned files; no failed retry destroys the rendered frames.
#[derive(Debug)]
pub struct VideoRecovery {
    workspace: Option<RenderWorkspace>,
    options: VideoExportOptions,
    timing: ProjectTiming,
    width: u32,
    height: u32,
    frame_count: u64,
    estimated_bytes: u64,
}

impl VideoExportJob {
    /// Validates immutable video choices before any tool invocation or filesystem mutation.
    ///
    /// AV1 requires an explicit opaque matte. Neither codec includes audio in this stage.
    ///
    /// # Errors
    /// Returns document/range/size errors, mismatched suffixes, or missing AV1 matte diagnostics.
    pub fn new(
        document: Document,
        sources: SourceBundle,
        options: VideoExportOptions,
    ) -> Result<Self, ExportError> {
        validate_destination(&options.destination, options.codec)?;
        if options.codec == VideoCodec::Av1Webm
            && !matches!(
                options.background,
                Some(RasterBackground::OpaqueBlack | RasterBackground::OpaqueWhite)
            )
        {
            return Err(ExportError::new(
                "export.video.alpha",
                "AV1 sharing output requires an explicit black or white matte",
            ));
        }
        let sequence = SequenceExportJob::new(
            document,
            sources,
            SequenceExportOptions {
                destination: PathBuf::new(),
                format: SequenceFormat::Png,
                background: options.background,
                target: options.target,
                antialiasing: options.antialiasing,
                limits: options.limits,
            },
        )?;
        Ok(Self { sequence, options })
    }

    /// Renders and encodes one silent video, preserving PNG recovery only on encoding failure.
    ///
    /// # Errors
    /// Returns preflight/render failures without recovery or encoding failures with completed PNGs.
    /// Cancellation removes the temporary video and discards only the owned render workspace.
    pub fn run(
        &self,
        tools: MediaTools,
        cancelled: &AtomicBool,
        report: &(dyn Fn(VideoProgress) + Sync),
    ) -> Result<VideoExportResult, VideoExportFailure> {
        let mut recovery = self
            .render_frames(tools.clone(), cancelled, report)
            .map_err(|error| VideoExportFailure {
                error,
                recovery: None,
            })?;
        match recovery.retry(tools, &self.options.destination, cancelled, report) {
            Ok(result) => Ok(result),
            Err(mut error) => {
                if error.is_cancelled() {
                    if let Err(cleanup) = recovery.discard() {
                        error.detail.push_str(&format!("; {cleanup}"));
                    }
                    let retained = recovery.frames_path().is_some();
                    Err(VideoExportFailure {
                        error,
                        recovery: retained.then(|| Box::new(recovery)),
                    })
                } else {
                    Err(VideoExportFailure {
                        error,
                        recovery: Some(Box::new(recovery)),
                    })
                }
            }
        }
    }

    /// Proves the actual encoder/pixel format/muxer at output size before rendering private PNGs.
    ///
    /// A one-frame capability encode and decode-count validation use an owned temporary sibling.
    /// Storage checks account for simultaneous PNG and video bytes on shared filesystems. The
    /// shared sequence job then validates every authored frame before generating PNG output.
    ///
    /// # Errors
    /// Returns tool, capability, space, source/frame, rendering, or cleanup diagnostics. Failed
    /// rendering never presents a partial sequence as recoverable complete video input.
    pub fn render_frames(
        &self,
        tools: MediaTools,
        cancelled: &AtomicBool,
        report: &(dyn Fn(VideoProgress) + Sync),
    ) -> Result<VideoRecovery, ExportError> {
        check_cancelled(cancelled)?;
        let total = self
            .sequence
            .document
            .project_timing()
            .frame_range()
            .frame_count();
        report(progress(VideoPhase::Preflight, 0, total, 0));
        let (width, height) = self.sequence.dimensions()?;
        let estimate = self.sequence.estimated_bytes()?;
        let output =
            VideoOutput::reserve(&self.options.destination, estimate).map_err(storage_error)?;
        process::capability_probe(
            &tools,
            self.options.codec,
            VideoStreamSpec {
                width,
                height,
                frame_count: 1,
                frame_rate: self.sequence.document.project_timing().frame_rate(),
            },
            &output,
            cancelled,
        )?;
        let mut workspace = RenderWorkspace::create(
            self.options
                .temporary_directory
                .as_deref()
                .unwrap_or_else(|| Path::new("/tmp")),
        )
        .map_err(storage_error)?;
        let storage = (|| {
            workspace.require_space(estimate).map_err(storage_error)?;
            let simultaneous = if output
                .shares_filesystem(&workspace)
                .map_err(storage_error)?
            {
                estimate.checked_mul(2).ok_or_else(|| {
                    ExportError::new("export.storage", "combined staging estimate overflowed")
                })?
            } else {
                estimate
            };
            output.require_space(simultaneous).map_err(storage_error)
        })();
        if let Err(mut error) = storage {
            if let Err(cleanup) = workspace.discard() {
                error.detail.push_str(&format!("; cleanup: {cleanup}"));
            }
            return Err(error);
        }
        drop(output);
        let mut sequence = self.sequence.clone();
        sequence.options.destination = workspace.frames_path();
        let rendered = sequence.run(tools, cancelled, &|event| {
            if event.phase == ExportPhase::Complete {
                return;
            }
            report(VideoProgress {
                phase: if event.phase == ExportPhase::Preflight {
                    VideoPhase::Preflight
                } else {
                    VideoPhase::Rendering
                },
                completed_frames: event.completed_frames,
                total_frames: event.total_frames,
                encoded_time_micros: 0,
                render: Some(event),
            });
        });
        if let Err(mut error) = rendered {
            error.output = None;
            if let Err(cleanup) = workspace.discard() {
                error.detail.push_str(&format!("; cleanup: {cleanup}"));
                error.output = Some(workspace.path().to_owned());
            }
            return Err(error);
        }
        Ok(VideoRecovery {
            workspace: Some(workspace),
            options: self.options.clone(),
            timing: self.sequence.document.project_timing().clone(),
            width,
            height,
            frame_count: total,
            estimated_bytes: estimate,
        })
    }
}

impl VideoRecovery {
    /// Returns the complete retained PNG sequence directory while recovery remains available.
    pub fn frames_path(&self) -> Option<PathBuf> {
        self.workspace.as_ref().map(RenderWorkspace::frames_path)
    }
    /// Returns the initially selected destination for a retry dialog's default value.
    pub fn destination(&self) -> &Path {
        &self.options.destination
    }

    /// Encodes retained PNGs again, optionally to a newly selected destination, without rendering.
    ///
    /// # Errors
    /// Returns cancellation, tool, file validation, storage or publication failures while retaining
    /// PNGs. Successful publication reports any actual cleanup failure separately from file success.
    pub fn retry(
        &mut self,
        tools: MediaTools,
        destination: &Path,
        cancelled: &AtomicBool,
        report: &(dyn Fn(VideoProgress) + Sync),
    ) -> Result<VideoExportResult, ExportError> {
        check_cancelled(cancelled)?;
        validate_destination(destination, self.options.codec)?;
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            ExportError::new("export.recovery", "rendered frames were already released")
        })?;
        let output =
            VideoOutput::reserve(destination, self.estimated_bytes).map_err(storage_error)?;
        report(progress(VideoPhase::Encoding, 0, self.frame_count, 0));
        process::encode(
            &tools,
            self.options.codec,
            &workspace.frames_path(),
            VideoStreamSpec {
                width: self.width,
                height: self.height,
                frame_count: self.frame_count,
                frame_rate: self.timing.frame_rate(),
            },
            &output,
            cancelled,
            &|frames, time| {
                report(progress(
                    VideoPhase::Encoding,
                    frames.min(self.frame_count),
                    self.frame_count,
                    time,
                ));
            },
        )?;
        check_cancelled(cancelled)?;
        report(progress(
            VideoPhase::Validating,
            self.frame_count,
            self.frame_count,
            0,
        ));
        process::validate(
            &tools,
            self.options.codec,
            VideoStreamSpec {
                width: self.width,
                height: self.height,
                frame_count: self.frame_count,
                frame_rate: self.timing.frame_rate(),
            },
            &output,
            cancelled,
        )?;
        check_cancelled(cancelled)?;
        let file = output.publish().map_err(storage_error)?;
        let cleanup_warning = self.discard().err().map(|error| error.to_string());
        let retained_intermediates = self.frames_path();
        report(progress(
            VideoPhase::Complete,
            self.frame_count,
            self.frame_count,
            0,
        ));
        Ok(VideoExportResult {
            file,
            frame_count: self.frame_count,
            width: self.width,
            height: self.height,
            retained_intermediates,
            cleanup_warning,
        })
    }

    /// Copies retained PNGs into a new ordinary sequence directory before releasing temporary files.
    ///
    /// # Errors
    /// Retains originals on cancellation/copy/publication failure. A cleanup diagnostic identifies
    /// the finalized destination when copying succeeds but original temporary files cannot be removed.
    pub fn save_png_sequence(
        &mut self,
        destination: &Path,
        cancelled: &AtomicBool,
        report: &(dyn Fn(VideoProgress) + Sync),
    ) -> Result<PathBuf, ExportError> {
        check_cancelled(cancelled)?;
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            ExportError::new("export.recovery", "rendered frames were already released")
        })?;
        let manifest =
            SequenceManifest::new(SequenceFormat::Png, self.width, self.height, &self.timing);
        let mut writer = SequenceWriter::create(destination, manifest, self.estimated_bytes)
            .map_err(|error| ExportError::new("export.write", error))?;
        let per_frame = u64::from(self.width) * u64::from(self.height) * 8 + 65_536;
        for index in 0..self.frame_count {
            check_cancelled(cancelled).map_err(|error| error.at(Some(index), Some(destination)))?;
            let bytes = workspace
                .read_frame(index, per_frame)
                .map_err(storage_error)?;
            writer.write_frame(&bytes).map_err(|error| {
                ExportError::new("export.write", error).at(Some(index), Some(destination))
            })?;
            report(progress(
                VideoPhase::SavingPngs,
                index + 1,
                self.frame_count,
                0,
            ));
        }
        let path = writer.finish().map_err(|error| {
            ExportError::new("export.finalize", error).at(None, Some(destination))
        })?;
        self.discard()
            .map_err(|error| error.at(None, Some(destination)))?;
        Ok(path)
    }

    /// Explicitly removes only this job's known intermediate files and releases recovery state.
    ///
    /// # Errors
    /// Returns ownership/cleanup failures while keeping the recovery location available.
    pub fn discard(&mut self) -> Result<(), ExportError> {
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.discard().map_err(storage_error)?;
        }
        self.workspace = None;
        Ok(())
    }
}

/// Creates a phase-local video update without conflating rendering with encoding effort.
fn progress(
    phase: VideoPhase,
    completed_frames: u64,
    total_frames: u64,
    encoded_time_micros: u64,
) -> VideoProgress {
    VideoProgress {
        phase,
        completed_frames,
        total_frames,
        encoded_time_micros,
        render: None,
    }
}

/// Projects concrete storage ownership failures into the shared export diagnostic boundary.
fn storage_error(error: impl std::fmt::Display) -> ExportError {
    ExportError::new("export.storage", error)
}

/// Keeps initial and retried video destinations consistent with the chosen container.
///
/// # Errors
/// Rejects suffixes that would misidentify the encoded format in ordinary desktop applications.
fn validate_destination(path: &Path, codec: VideoCodec) -> Result<(), ExportError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let expected = match codec {
        VideoCodec::Ffv1Matroska => "mkv",
        VideoCodec::Av1Webm => "webm",
    };
    if !extension.eq_ignore_ascii_case(expected) {
        return Err(ExportError::new(
            "export.video",
            format!("selected video format requires .{expected}"),
        ));
    }
    Ok(())
}
