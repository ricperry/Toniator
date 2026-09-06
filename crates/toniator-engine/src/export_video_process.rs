//! Bounded diagnostics and cancellable FFmpeg encoding/validation with owned file descriptors.

use super::*;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, atomic::AtomicU64},
    thread,
    time::{Duration, Instant},
};

const STDOUT_LIMIT: usize = 1024 * 1024;
const DIAGNOSTIC_LIMIT: usize = 64 * 1024;
const LINE_LIMIT: usize = 8192;

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancels a confirmed running realtime encoder and waits for its pipe readers and process.
    ///
    /// # Panics
    /// Panics if FFmpeg cannot start, never reports a frame, or cancellation fails to reap promptly.
    #[test]
    fn running_encoder_cancels_and_reaps_without_waiting_for_media_end() {
        let cancelled = AtomicBool::new(false);
        let mut command = Command::new("ffmpeg");
        command
            .args([
                "-nostdin",
                "-v",
                "error",
                "-progress",
                "pipe:2",
                "-stats_period",
                "0.1",
                "-re",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x64:rate=6",
                "-t",
                "3600",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        let started = Instant::now();
        let failure = run_process(
            &mut command,
            &cancelled,
            Some(Duration::from_secs(10)),
            None,
            None,
            &|frames, _| {
                if frames > 0 {
                    cancelled.store(true, Ordering::Release);
                }
            },
        )
        .unwrap_err();
        assert!(failure.is_cancelled(), "{failure}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}

/// Exercises the selected encoder, pixel format, muxer, dimensions and validator before rendering.
///
/// # Errors
/// Returns capability, bounded probe-time, cancellation, process or encoded-stream diagnostics.
pub(super) fn capability_probe(
    tools: &MediaTools,
    codec: VideoCodec,
    spec: VideoStreamSpec,
    output: &VideoOutput,
    cancelled: &AtomicBool,
) -> Result<(), ExportError> {
    let mut command = encoder_command(tools, output)?;
    command.args(["-f", "lavfi", "-i"]).arg(format!(
        "color=c=red:s={}x{}:r={}/{}",
        spec.width,
        spec.height,
        spec.frame_rate.numerator(),
        spec.frame_rate.denominator()
    ));
    append_output(&mut command, codec, 1);
    run_process(
        &mut command,
        cancelled,
        Some(Duration::from_secs(30)),
        None,
        Some(output),
        &|_, _| {},
    )?;
    validate(
        tools,
        codec,
        VideoStreamSpec {
            frame_count: 1,
            ..spec
        },
        output,
        cancelled,
    )?;
    output.clear().map_err(storage_error)
}

/// Encodes the complete numbered PNG sequence into the owned temporary video file.
///
/// # Errors
/// Returns cancellation, storage exhaustion, stalled-encoder, bounded-diagnostic or process errors.
pub(super) fn encode(
    tools: &MediaTools,
    codec: VideoCodec,
    frames: &Path,
    spec: VideoStreamSpec,
    output: &VideoOutput,
    cancelled: &AtomicBool,
    report: &(dyn Fn(u64, u64) + Sync),
) -> Result<(), ExportError> {
    let mut command = encoder_command(tools, output)?;
    command
        .args([
            "-protocol_whitelist",
            "file,pipe",
            "-f",
            "image2",
            "-framerate",
        ])
        .arg(format!(
            "{}/{}",
            spec.frame_rate.numerator(),
            spec.frame_rate.denominator()
        ))
        .args(["-start_number", "0", "-i"])
        .arg(frames.join("frame-%06d.png"));
    append_output(&mut command, codec, spec.frame_count);
    run_process(
        &mut command,
        cancelled,
        None,
        Some(Duration::from_secs(180)),
        Some(output),
        report,
    )?;
    Ok(())
}

/// Decodes/counts the completed stream and verifies codec, alpha format, dimensions, rate and silence.
///
/// The validator reads the owned file through standard input's descriptor path, never a staging
/// pathname. Large local validation remains cancellable without an arbitrary whole-video deadline.
///
/// # Errors
/// Rejects malformed/truncated files, extra streams, wrong codecs/formats/sizes/counts or frame rates.
pub(super) fn validate(
    tools: &MediaTools,
    codec: VideoCodec,
    spec: VideoStreamSpec,
    output: &VideoOutput,
    cancelled: &AtomicBool,
) -> Result<(), ExportError> {
    let mut command = Command::new(&tools.ffprobe);
    command.args(["-v", "error", "-protocol_whitelist", "file,pipe", "-count_frames", "-show_entries", "stream=codec_type,codec_name,width,height,pix_fmt,nb_read_frames,avg_frame_rate,r_frame_rate", "-of", "json", "/proc/self/fd/0"])
        .stdin(Stdio::from(output.file_handle().map_err(storage_error)?)).stdout(Stdio::piped());
    let bytes = run_process(&mut command, cancelled, None, None, None, &|_, _| {})?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| ExportError::new("export.validate", error))?;
    let streams = json["streams"]
        .as_array()
        .ok_or_else(|| ExportError::new("export.validate", "validator did not return streams"))?;
    if streams.len() != 1 {
        return Err(ExportError::new(
            "export.validate",
            "video must contain exactly one silent video stream",
        ));
    }
    let stream = &streams[0];
    let (expected_codec, expected_pixels) = match codec {
        VideoCodec::Ffv1Matroska => ("ffv1", "bgra"),
        VideoCodec::Av1Webm => ("av1", "yuv420p10le"),
    };
    let count = stream["nb_read_frames"]
        .as_str()
        .and_then(|value| value.parse::<u64>().ok());
    let rate_matches = ["avg_frame_rate", "r_frame_rate"]
        .iter()
        .any(|key| stream[key].as_str().and_then(parse_rate) == Some(spec.frame_rate));
    if stream["codec_type"] != "video"
        || stream["codec_name"] != expected_codec
        || stream["pix_fmt"] != expected_pixels
        || stream["width"].as_u64() != Some(u64::from(spec.width))
        || stream["height"].as_u64() != Some(u64::from(spec.height))
        || count != Some(spec.frame_count)
        || !rate_matches
    {
        return Err(ExportError::new(
            "export.validate",
            format!(
                "encoded stream does not match the requested codec, pixels, dimensions, frame count or exact rate: {stream}"
            ),
        ));
    }
    Ok(())
}

/// Constructs a software encoder with progress on stderr and output bound to an owned regular file.
///
/// # Errors
/// Returns the owned output descriptor's duplication diagnostic before starting a process.
fn encoder_command(tools: &MediaTools, output: &VideoOutput) -> Result<Command, ExportError> {
    let mut command = Command::new(&tools.ffmpeg);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-progress",
            "pipe:2",
            "-stats_period",
            "0.1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.file_handle().map_err(storage_error)?));
    Ok(command)
}

/// Appends explicit software codec, color, silence and owned-output arguments without a shell.
fn append_output(command: &mut Command, codec: VideoCodec, frames: u64) {
    command
        .args(["-map", "0:v:0", "-an", "-sn", "-dn", "-frames:v"])
        .arg(frames.to_string());
    match codec {
        VideoCodec::Ffv1Matroska => {
            command.args([
                "-c:v",
                "ffv1",
                "-level",
                "3",
                "-coder",
                "1",
                "-context",
                "1",
                "-g",
                "1",
                "-slicecrc",
                "1",
                "-pix_fmt",
                "bgra",
                "-colorspace",
                "rgb",
                "-color_trc",
                "iec61966-2-1",
                "-color_primaries",
                "bt709",
                "-color_range",
                "pc",
                "-f",
                "matroska",
            ]);
        }
        VideoCodec::Av1Webm => {
            // Full-range sRGB PNGs first enter explicitly BT.709-matrix YUV, then the mature
            // colorspace filter converts transfer/range and subsamples for 10-bit sharing output.
            command.args(["-vf", "scale=in_range=full:out_range=full:out_color_matrix=bt709,format=yuv444p12le,colorspace=iall=bt709:itrc=srgb:irange=pc:all=bt709:range=tv:format=yuv420p10", "-c:v", "libsvtav1", "-crf", "18", "-preset", "6", "-pix_fmt", "yuv420p10le", "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-color_range", "tv", "-f", "webm"]);
        }
    }
    // The inherited regular file descriptor remains seekable for finalized container metadata.
    command.args(["-y", "/proc/self/fd/1"]);
}

/// Parses an exact positive frame-rate fraction through domain validation.
fn parse_rate(value: &str) -> Option<FrameRate> {
    let (numerator, denominator) = value.split_once('/')?;
    FrameRate::new(numerator.parse().ok()?, denominator.parse().ok()?).ok()
}

/// Shares bounded process-reader facts with the cancellable job worker.
struct ReaderState {
    frames: AtomicU64,
    time: AtomicU64,
    exceeded: AtomicBool,
    activity: Mutex<Instant>,
    diagnostic: Mutex<Vec<u8>>,
}
impl ReaderState {
    /// Initializes empty progress and a live-process activity timestamp.
    fn new() -> Self {
        Self {
            frames: AtomicU64::new(0),
            time: AtomicU64::new(0),
            exceeded: AtomicBool::new(false),
            activity: Mutex::new(Instant::now()),
            diagnostic: Mutex::new(Vec::new()),
        }
    }

    /// Parses one bounded progress line or retains bounded non-progress diagnostics.
    fn line(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let mut known = false;
        if let Some((key, value)) = text.trim().split_once('=') {
            match key {
                "frame" => {
                    if let Ok(value) = value.trim().parse() {
                        self.frames.fetch_max(value, Ordering::Relaxed);
                    }
                    known = true;
                }
                "out_time_us" => {
                    if let Ok(value) = value.trim().parse::<i64>() {
                        self.time.fetch_max(value.max(0) as u64, Ordering::Relaxed);
                    }
                    known = true;
                }
                "fps" | "bitrate" | "total_size" | "out_time_ms" | "out_time" | "dup_frames"
                | "drop_frames" | "speed" | "progress" => known = true,
                key if key.starts_with("stream_") => known = true,
                _ => {}
            }
        }
        if known {
            *self
                .activity
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Instant::now();
        } else {
            let mut diagnostic = self
                .diagnostic
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if diagnostic.len().saturating_add(bytes.len()) > DIAGNOSTIC_LIMIT {
                self.exceeded.store(true, Ordering::Release);
            } else {
                diagnostic.extend_from_slice(bytes);
            }
        }
    }
}

/// Consumes bounded stderr lines concurrently with encoding and progress polling.
///
/// # Errors
/// Returns pipe-read failures; oversized lines set the shared stop flag without growing memory.
fn read_diagnostic(mut input: impl Read, state: &ReaderState) -> std::io::Result<()> {
    let mut buffer = [0_u8; 4096];
    let mut line = Vec::new();
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        for byte in &buffer[..count] {
            if line.len() >= LINE_LIMIT {
                state.exceeded.store(true, Ordering::Release);
                return Ok(());
            }
            line.push(*byte);
            if *byte == b'\n' {
                state.line(&line);
                line.clear();
            }
        }
    }
    if !line.is_empty() {
        state.line(&line);
    }
    Ok(())
}

/// Runs a local tool while draining pipes, observing space/progress and reaping on cancellation.
///
/// Probe deadlines and encoder inactivity bounds are explicit; complete-file validation remains
/// cancellable without a deadline tied to video duration. Reader memory is independently capped.
///
/// # Errors
/// Returns spawn/read/wait, timeout, cancellation, output-bound, storage or nonzero-exit diagnostics.
fn run_process(
    command: &mut Command,
    cancelled: &AtomicBool,
    deadline: Option<Duration>,
    idle_timeout: Option<Duration>,
    output: Option<&VideoOutput>,
    report: &(dyn Fn(u64, u64) + Sync),
) -> Result<Vec<u8>, ExportError> {
    check_cancelled(cancelled)?;
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| ExportError::new("export.process", error))?;
    let captures_validation = child.stdout.is_some();
    let state = Arc::new(ReaderState::new());
    let reader_state = Arc::clone(&state);
    let stderr = child.stderr.take().expect("requested stderr pipe");
    let stderr_reader = thread::spawn(move || read_diagnostic(stderr, &reader_state));
    let stdout_reader = child.stdout.take().map(|stdout| {
        let state = Arc::clone(&state);
        thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take(STDOUT_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > STDOUT_LIMIT {
                state.exceeded.store(true, Ordering::Release);
            }
            Ok::<_, std::io::Error>(bytes)
        })
    });
    let started = Instant::now();
    let mut last_report = Instant::now();
    let mut last_storage_check = Instant::now();
    let mut failure = None;
    let status = loop {
        if cancelled.load(Ordering::Acquire) {
            failure = Some(ExportError::new(
                "export.cancelled",
                "video operation cancelled",
            ));
        } else if state.exceeded.load(Ordering::Acquire) {
            failure = Some(ExportError::new(
                "export.process.bounds",
                "media tool output exceeded its diagnostic bound",
            ));
        } else if deadline.is_some_and(|duration| started.elapsed() >= duration) {
            failure = Some(ExportError::new(
                "export.process.timeout",
                "encoder capability probe exceeded 30 seconds",
            ));
        } else if idle_timeout.is_some_and(|duration| {
            state
                .activity
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .elapsed()
                >= duration
        }) {
            failure = Some(ExportError::new(
                "export.process.timeout",
                "encoder stopped reporting progress for 180 seconds",
            ));
        }
        if failure.is_none() && last_storage_check.elapsed() >= Duration::from_millis(500) {
            if let Some(output) = output
                && let Err(error) = output.require_space(65_536)
            {
                failure = Some(storage_error(error));
            }
            last_storage_check = Instant::now();
        }
        if failure.is_some() {
            let _ = child.kill();
        }
        if last_report.elapsed() >= Duration::from_millis(100) {
            report(
                state.frames.load(Ordering::Relaxed),
                state.time.load(Ordering::Relaxed),
            );
            last_report = Instant::now();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(ExportError::new("export.process", error));
            }
        }
    };
    let stderr_result = stderr_reader.join();
    let stdout_result = stdout_reader.map(|reader| reader.join());
    let status = status?;
    stderr_result
        .map_err(|_| ExportError::new("export.process", "diagnostic reader panicked"))?
        .map_err(|error| ExportError::new("export.process", error))?;
    let stdout = match stdout_result {
        Some(result) => result
            .map_err(|_| ExportError::new("export.process", "validator reader panicked"))?
            .map_err(|error| ExportError::new("export.process", error))?,
        None => Vec::new(),
    };
    if let Some(error) = failure {
        return Err(error);
    }
    check_cancelled(cancelled)?;
    if state.exceeded.load(Ordering::Acquire) {
        return Err(ExportError::new(
            "export.process.bounds",
            "media tool output exceeded its diagnostic bound",
        ));
    }
    if !status.success() {
        return Err(ExportError::new(
            "export.process",
            format!(
                "media tool exited with {status}: {}",
                String::from_utf8_lossy(
                    &state
                        .diagnostic
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                )
            ),
        ));
    }
    if captures_validation {
        let diagnostic = state
            .diagnostic
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !diagnostic.is_empty() {
            return Err(ExportError::new(
                "export.validate",
                format!(
                    "decoder reported an error: {}",
                    String::from_utf8_lossy(&diagnostic)
                ),
            ));
        }
    }
    report(
        state.frames.load(Ordering::Relaxed),
        state.time.load(Ordering::Relaxed),
    );
    Ok(stdout)
}
