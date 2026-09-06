//! Immutable shared sequence jobs using domain timing, canonical scenes, and exclusive I/O.

#[path = "export_video.rs"]
pub mod video;

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;

use crate::{
    CancellationProbe, DocumentDerivedCache, EvaluationLimits, EvaluationProgressStage,
    EvaluationRunError, FrameSource, MediaTools, OutputRasterTarget, PreviewRasterTarget,
    RasterAntialiasing, RasterBackground, RasterRequest, SourceMediaMetadata, encode_png,
    evaluate_cached_document, frame_evaluation_request, open_source_media, write_svg,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use toniator_domain::{Document, DocumentSession, SourceReference};
use toniator_io::{
    SourceBundle,
    sequence::{SequenceFormat, SequenceManifest, SequenceWriter},
};

/// Describes job phases independently of preview tickets or authored document state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportPhase {
    Preflight,
    Rendering,
    Finalizing,
    Complete,
}

/// Reports completed frames and real current-frame work without a fabricated ETA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportProgress {
    pub phase: ExportPhase,
    pub completed_frames: u64,
    pub total_frames: u64,
    pub frame: Option<u64>,
    pub frame_fraction: f64,
}

/// Defines consumer-only sequence choices; temporal settings remain in the document.
#[derive(Clone, Debug)]
pub struct SequenceExportOptions {
    pub destination: PathBuf,
    pub format: SequenceFormat,
    pub background: Option<RasterBackground>,
    pub target: Option<OutputRasterTarget>,
    pub antialiasing: RasterAntialiasing,
    pub limits: EvaluationLimits,
}

/// Captures an immutable export snapshot isolated from subsequent editor/history changes.
#[derive(Clone, Debug)]
pub struct SequenceExportJob {
    document: Document,
    sources: SourceBundle,
    options: SequenceExportOptions,
}

/// Describes a fully finalized sequence; incomplete jobs return an error carrying their path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceExportResult {
    pub directory: PathBuf,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
}

/// Carries a stable export phase/frame diagnostic and any retained incomplete sequence location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportError {
    pub stage: &'static str,
    pub frame: Option<u64>,
    pub output: Option<PathBuf>,
    detail: String,
}

impl ExportError {
    /// Records one failure without creating output or suppressing the underlying diagnostic.
    fn new(stage: &'static str, detail: impl ToString) -> Self {
        Self {
            stage,
            frame: None,
            output: None,
            detail: detail.to_string(),
        }
    }
    /// Adds the failed absolute frame and any owned incomplete output location.
    fn at(mut self, frame: Option<u64>, output: Option<&Path>) -> Self {
        self.frame = frame;
        self.output = output.map(Path::to_path_buf);
        self
    }
    /// Identifies cooperative cancellation for frontend lifecycle handling.
    pub fn is_cancelled(&self) -> bool {
        self.stage == "export.cancelled"
    }
}
impl std::fmt::Display for ExportError {
    /// Formats the stable phase and optional absolute frame before the specific failure.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.stage)?;
        if let Some(frame) = self.frame {
            write!(f, " at frame {frame}")?;
        }
        write!(f, ": {}", self.detail)
    }
}
impl std::error::Error for ExportError {}

impl SequenceExportJob {
    /// Captures current source-backed document authority and validates static output choices.
    ///
    /// # Errors
    /// Rejects invalid documents, absent/mismatched media, excessive frame counts, invalid native
    /// raster dimensions, or SVG consumer choices that would imply a matte or resized geometry.
    pub fn new(
        document: Document,
        sources: SourceBundle,
        options: SequenceExportOptions,
    ) -> Result<Self, ExportError> {
        document
            .validate()
            .map_err(|error| ExportError::new("export.document", error))?;
        let primary = sources.media().and_then(|media| media.primary_source_id());
        if !matches!(document.source(), SourceReference::Assigned(id) if Some(id) == primary) {
            return Err(ExportError::new(
                "export.source",
                "document must reference the primary bundled media source",
            ));
        }
        if document.project_timing().frame_range().frame_count() > 1_000_000 {
            return Err(ExportError::new(
                "export.frames",
                "export is limited to one million output frames",
            ));
        }
        if options.format == SequenceFormat::Svg
            && (options.target.is_some()
                || options
                    .background
                    .is_some_and(|background| background != RasterBackground::Transparent))
        {
            return Err(ExportError::new(
                "export.svg",
                "SVG sequences retain native geometry and transparent backgrounds",
            ));
        }
        let job = Self {
            document,
            sources,
            options,
        };
        job.dimensions()?;
        Ok(job)
    }

    /// Returns a conservative raw-RGBA-plus-overhead storage estimate, not compressed-input size.
    ///
    /// # Errors
    /// Rejects invalid output dimensions or exact byte-count overflow. Actual writes recheck space;
    /// scene-dependent SVG size can exceed a raster-based estimate and is never an allocation bound.
    pub fn estimated_bytes(&self) -> Result<u64, ExportError> {
        let (width, height) = self.dimensions()?;
        (u64::from(width) * u64::from(height))
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(65_536))
            .and_then(|bytes| {
                bytes.checked_mul(self.document.project_timing().frame_range().frame_count())
            })
            .and_then(|bytes| bytes.checked_add(65_536))
            .ok_or_else(|| {
                ExportError::new("export.storage", "sequence storage estimate overflowed")
            })
    }

    /// Preflights every requested frame, then renders and publishes through one private cache.
    ///
    /// The caller owns its worker and permits only one active job. This synchronous method never
    /// edits history; cancellation retains completed sequence frames under an incomplete manifest.
    /// Constructing the media provider inside this call keeps native decoders on their owner thread.
    ///
    /// # Errors
    /// Returns document/frame/source/storage/render/publication failures with the absolute frame
    /// and retained output path when applicable. No output directory is created before preflight.
    pub fn run(
        &self,
        tools: MediaTools,
        cancelled: &AtomicBool,
        report: &(dyn Fn(ExportProgress) + Sync),
    ) -> Result<SequenceExportResult, ExportError> {
        let observer = ExportObserver {
            cancelled,
            report,
            last_update: Mutex::new(None),
        };
        let timing = self.document.project_timing();
        let range = timing.frame_range();
        let total = range.frame_count();
        observer.emit(
            ExportProgress {
                phase: ExportPhase::Preflight,
                completed_frames: 0,
                total_frames: total,
                frame: None,
                frame_fraction: 0.0,
            },
            true,
        );
        check_cancelled(cancelled)?;
        let mut media =
            open_source_media(&self.sources, tools, &|| cancelled.load(Ordering::Acquire))
                .map_err(|error| source_failure(cancelled, error))?;
        self.preflight_frames(media.metadata(), cancelled)?;
        let (width, height) = self.dimensions()?;
        let manifest = SequenceManifest::new(self.options.format, width, height, timing);
        check_cancelled(cancelled)?;
        let mut writer =
            SequenceWriter::create(&self.options.destination, manifest, self.estimated_bytes()?)
                .map_err(|error| ExportError::new("export.storage", error))?;
        let output = writer.path().to_owned();
        let session = DocumentSession::new(self.document.clone())
            .map_err(|error| ExportError::new("export.document", error).at(None, Some(&output)))?;
        let mut cache = DocumentDerivedCache::default();
        for (completed, frame) in (range.start()..range.end_exclusive()).enumerate() {
            let completed = completed as u64;
            let base = ExportProgress {
                phase: ExportPhase::Rendering,
                completed_frames: completed,
                total_frames: total,
                frame: Some(frame),
                frame_fraction: 0.0,
            };
            observer.emit(base, true);
            let result = (|| {
                check_cancelled(cancelled)?;
                let mut request = frame_evaluation_request(&session, &mut media, frame, &|| {
                    cancelled.load(Ordering::Acquire)
                })
                .map_err(|error| source_failure(cancelled, error))?;
                match self.options.format {
                    SequenceFormat::Png => {
                        let background = self.options.background.unwrap_or_else(|| {
                            RasterBackground::default_for_model(self.document.channel_model())
                        });
                        request = request.for_output(
                            background,
                            self.options.target,
                            self.options.antialiasing,
                        );
                    }
                    SequenceFormat::Svg => {
                        // The evaluator retains a raster cache slot; a minimal derived preview
                        // avoids full raster work while SVG consumes the unchanged native scene.
                        request.raster_request = RasterRequest::Preview(
                            PreviewRasterTarget::new(1, 1).expect("one pixel is valid"),
                        );
                    }
                }
                let probe = ExportFrameProbe {
                    observer: &observer,
                    progress: base,
                };
                let evaluated =
                    evaluate_cached_document(request, self.options.limits, &cache, &probe)
                        .map_err(|error| match error {
                            EvaluationRunError::Cancelled => {
                                ExportError::new("export.cancelled", "rendering cancelled")
                            }
                            EvaluationRunError::Evaluation(error) => {
                                ExportError::new("export.render", error)
                            }
                        })?;
                let bytes = match self.options.format {
                    SequenceFormat::Png => encode_png(evaluated.result.raster())
                        .map_err(|error| ExportError::new("export.png", error))?,
                    SequenceFormat::Svg => write_svg(evaluated.result.scene()).into_bytes(),
                };
                check_cancelled(cancelled)?;
                writer
                    .write_frame(&bytes)
                    .map_err(|error| ExportError::new("export.write", error))?;
                cache.commit(evaluated.transaction);
                Ok::<(), ExportError>(())
            })();
            result.map_err(|error| error.at(Some(frame), Some(&output)))?;
            observer.emit(
                ExportProgress {
                    completed_frames: completed + 1,
                    frame: None,
                    frame_fraction: 0.0,
                    ..base
                },
                true,
            );
        }
        check_cancelled(cancelled).map_err(|error| error.at(None, Some(&output)))?;
        observer.emit(
            ExportProgress {
                phase: ExportPhase::Finalizing,
                completed_frames: total,
                total_frames: total,
                frame: None,
                frame_fraction: 0.0,
            },
            true,
        );
        let directory = writer
            .finish()
            .map_err(|error| ExportError::new("export.finalize", error).at(None, Some(&output)))?;
        observer.emit(
            ExportProgress {
                phase: ExportPhase::Complete,
                completed_frames: total,
                total_frames: total,
                frame: None,
                frame_fraction: 0.0,
            },
            true,
        );
        Ok(SequenceExportResult {
            directory,
            frame_count: total,
            width,
            height,
        })
    }

    /// Validates exact source bounds and all interpolated coupled settings before publication.
    ///
    /// # Errors
    /// Returns frame-specific domain/range diagnostics or cancellation without creating output.
    fn preflight_frames(
        &self,
        metadata: &SourceMediaMetadata,
        cancelled: &AtomicBool,
    ) -> Result<(), ExportError> {
        let timing = self.document.project_timing();
        if let Some(interval) = timing.source_time_range() {
            if metadata
                .duration
                .is_some_and(|duration| interval.end().checked_cmp(duration).is_gt())
            {
                return Err(ExportError::new(
                    "export.range",
                    "selected interval extends beyond the source duration",
                ));
            }
            let expected = timing
                .frames_for_duration(
                    interval
                        .duration()
                        .map_err(|error| ExportError::new("export.range", error))?,
                )
                .map_err(|error| ExportError::new("export.range", error))?;
            if expected != timing.frame_range().frame_count() {
                return Err(ExportError::new(
                    "export.range",
                    "frame count must equal ceil(selected duration * frame rate)",
                ));
            }
        }
        for frame in timing.frame_range().start()..timing.frame_range().end_exclusive() {
            check_cancelled(cancelled).map_err(|error| error.at(Some(frame), None))?;
            self.document
                .materialize_frame(frame)
                .map_err(|error| ExportError::new("export.frame", error).at(Some(frame), None))?;
            let time = timing
                .source_time_for_frame(frame)
                .map_err(|error| ExportError::new("export.range", error).at(Some(frame), None))?;
            if metadata
                .duration
                .is_some_and(|duration| time.checked_cmp(duration).is_ge())
            {
                return Err(ExportError::new(
                    "export.range",
                    "requested frame is outside the finite source duration",
                )
                .at(Some(frame), None));
            }
        }
        Ok(())
    }

    /// Projects native integral canvas dimensions or the renderer-validated explicit PNG target.
    ///
    /// # Errors
    /// Rejects nonintegral/unrepresentable native dimensions or the renderer's pixel safety bound.
    fn dimensions(&self) -> Result<(u32, u32), ExportError> {
        if let Some(target) = self.options.target {
            return Ok((target.width(), target.height()));
        }
        let canvas = self.document.canvas();
        if [canvas.width, canvas.height].iter().any(|value| {
            !value.is_finite()
                || *value <= 0.0
                || value.fract() != 0.0
                || *value > f64::from(u32::MAX)
        }) {
            return Err(ExportError::new(
                "export.dimensions",
                "native output requires positive integral canvas dimensions",
            ));
        }
        let target = OutputRasterTarget::new(canvas.width as u32, canvas.height as u32)
            .map_err(|error| ExportError::new("export.dimensions", error))?;
        Ok((target.width(), target.height()))
    }
}

/// Shares cancellation and throttles current-frame progress to roughly ten updates per second.
struct ExportObserver<'a> {
    cancelled: &'a AtomicBool,
    report: &'a (dyn Fn(ExportProgress) + Sync),
    last_update: Mutex<Option<Instant>>,
}
impl ExportObserver<'_> {
    /// Emits explicit lifecycle boundaries immediately and coalesces ordinary work increments.
    fn emit(&self, progress: ExportProgress, force: bool) {
        let mut last = self
            .last_update
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if force || last.is_none_or(|time| time.elapsed() >= Duration::from_millis(100)) {
            *last = Some(Instant::now());
            drop(last);
            (self.report)(progress);
        }
    }
}

/// Projects evaluator effort into the current export frame without changing evaluator authority.
struct ExportFrameProbe<'a, 'b> {
    observer: &'a ExportObserver<'b>,
    progress: ExportProgress,
}
impl CancellationProbe for ExportFrameProbe<'_, '_> {
    /// Polls the caller's job cancellation flag across canonical evaluation workers.
    fn is_cancelled(&self) -> bool {
        self.observer.cancelled.load(Ordering::Acquire)
    }
    /// Reserves frame completion for successful encoding and publication after actual render work.
    fn report_progress(
        &self,
        _stage: EvaluationProgressStage,
        completed: u16,
        _stage_completed: u16,
    ) {
        self.observer.emit(
            ExportProgress {
                frame_fraction: f64::from(completed.min(1000)) / 1000.0 * 0.95,
                ..self.progress
            },
            false,
        );
    }
}

/// Stops before further output mutation when the caller cancels the job.
///
/// # Errors
/// Returns the stable cancellation diagnostic when the atomic flag is set.
fn check_cancelled(cancelled: &AtomicBool) -> Result<(), ExportError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(ExportError::new("export.cancelled", "export cancelled"));
    }
    Ok(())
}

/// Preserves cancellation identity when a source provider stops during probe or decode.
fn source_failure(cancelled: &AtomicBool, error: crate::SourceError) -> ExportError {
    if cancelled.load(Ordering::Acquire) {
        ExportError::new("export.cancelled", error)
    } else {
        ExportError::new("export.source", error)
    }
}
