use std::{
    fmt,
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use toniator_domain::{FrameRate, RationalTime};

use crate::{DECODER_CONTRACT_ID, SamplingError, SourceField};

pub(crate) const MAX_COMPRESSED_BYTES: usize = 128 * 1024 * 1024;
const MAX_PROBE_STDOUT: usize = 16 * 1024 * 1024;
const MAX_PROCESS_DIAGNOSTIC: usize = 64 * 1024;
const MAX_FRAME_COUNT: usize = 1_000_000;
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
const DECODE_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const DEMUXERS: &str = "mov,matroska,webm,avi,ogg,mpeg,mpegts,gif,apng,webp_pipe";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Identifies which supported source container owns a frame provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaKind {
    StillImage,
    AnimatedImage,
    ImageSequence,
    Video,
}

/// Bounded decoded-source metadata shared by every frame provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMediaMetadata {
    pub kind: SourceMediaKind,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    /// Finite half-open source duration, or `None` for a repeating still.
    pub duration: Option<RationalTime>,
    pub nominal_frame_rate: Option<FrameRate>,
    pub variable_frame_rate: bool,
    pub has_audio: bool,
    pub stream_index: u32,
}

/// Cache-relevant identity for one decoded source frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameIdentity {
    pub source_fingerprint: String,
    pub stream_index: u32,
    pub original_pts: i64,
    pub time_base: RationalTime,
    pub decoder_contract: String,
    pub color_policy: String,
    pub decoded_pixel_hash: String,
}

/// One immutable decoded field paired with exact source timing and identity.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceFrame {
    pub field: Arc<SourceField>,
    pub identity: FrameIdentity,
    pub index: u64,
    pub normalized_time: RationalTime,
}

/// Canonical headless frame-provider boundary for still and moving media.
pub trait FrameSource {
    /// Returns immutable validated source metadata.
    fn metadata(&self) -> &SourceMediaMetadata;

    /// Selects the latest source frame whose normalized PTS is at or before `position`.
    ///
    /// Finite moving sources use a half-open duration; still sources repeat. Implementations poll
    /// `is_cancelled` during bounded decode/probe work and retain at most a small frame cache.
    ///
    /// # Errors
    ///
    /// Returns stable cancellation, range, probe, decode, color, process, or bounds diagnostics.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError>;
}

/// Concrete unified source container used by the engine-facing media boundary.
#[derive(Debug)]
pub enum SourceMedia {
    StillImage(super::StillImageSource),
    AnimatedImage(super::AnimatedImageSource),
    ImageSequence(super::ImageSequenceSource),
    Video(Box<super::VideoSource>),
}

impl FrameSource for SourceMedia {
    /// Delegates metadata to the concrete provider without container-specific engine behavior.
    fn metadata(&self) -> &SourceMediaMetadata {
        match self {
            Self::StillImage(source) => source.metadata(),
            Self::AnimatedImage(source) => source.metadata(),
            Self::ImageSequence(source) => source.metadata(),
            Self::Video(source) => source.metadata(),
        }
    }

    /// Delegates exact frame selection and cooperative cancellation to the concrete provider.
    ///
    /// # Errors
    ///
    /// Returns the selected provider's stable source diagnostic.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        match self {
            Self::StillImage(source) => source.frame_at(position, is_cancelled),
            Self::AnimatedImage(source) => source.frame_at(position, is_cancelled),
            Self::ImageSequence(source) => source.frame_at(position, is_cancelled),
            Self::Video(source) => source.frame_at(position, is_cancelled),
        }
    }
}

/// Explicit FFmpeg tool paths supplied by packaging or the local development environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Default for MediaTools {
    /// Resolves package-private tools relative to the executable in SDK-packaged builds.
    /// Development builds use PATH. Missing packaged tools fail at invocation without host fallback.
    fn default() -> Self {
        if option_env!("TONIATOR_PACKAGED_MEDIA") == Some("1") {
            let prefix = std::env::current_exe()
                .ok()
                .and_then(|executable| executable.parent()?.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("/toniator-unavailable-media-tools"));
            return Self::in_directory(prefix.join("libexec/toniator-media"));
        }
        Self {
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
        }
    }
}

impl MediaTools {
    /// Selects both tools in one explicit directory without testing existence or falling back to PATH.
    pub fn in_directory(directory: impl AsRef<Path>) -> Self {
        Self {
            ffmpeg: directory.as_ref().join("ffmpeg"),
            ffprobe: directory.as_ref().join("ffprobe"),
        }
    }
}

/// Stable failure at the media probe, frame-selection, or decode boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError {
    path: &'static str,
    message: String,
}

impl SourceError {
    /// Constructs a stable diagnostic path with bounded caller-readable detail.
    pub fn new(path: &'static str, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic path.
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the bounded diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SourceError {
    /// Formats the stable diagnostic path and bounded message.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SourceError {}

impl From<SamplingError> for SourceError {
    /// Preserves sampling diagnostic paths while adapting to media-owned dynamic errors.
    fn from(value: SamplingError) -> Self {
        Self::new(value.path(), value.message())
    }
}

impl From<toniator_domain::ValidationError> for SourceError {
    /// Preserves exact temporal validation diagnostics at the source boundary.
    fn from(value: toniator_domain::ValidationError) -> Self {
        Self::new(value.path(), value.message())
    }
}

/// Rejects a cooperatively cancelled media operation.
///
/// # Errors
///
/// Returns `source.cancelled` whenever the callback reports cancellation.
pub(crate) fn cancelled(is_cancelled: &dyn Fn() -> bool) -> Result<(), SourceError> {
    if is_cancelled() {
        Err(SourceError::new(
            "source.cancelled",
            "media operation cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Enforces the aggregate 128 MiB compressed-source bound before process or decode work.
///
/// # Errors
///
/// Returns `source.size` when the byte count is zero or exceeds the shared bound.
pub(crate) fn validate_compressed_size(size: usize) -> Result<(), SourceError> {
    if size == 0 || size > MAX_COMPRESSED_BYTES {
        Err(SourceError::new(
            "source.size",
            "compressed source must contain 1 byte through 128 MiB",
        ))
    } else {
        Ok(())
    }
}

/// Enforces the bounded probe/frame-table count.
///
/// # Errors
///
/// Returns `source.frames` for empty or excessive frame tables.
pub(crate) fn validate_frame_count(count: usize) -> Result<(), SourceError> {
    if count == 0 || count > MAX_FRAME_COUNT {
        Err(SourceError::new(
            "source.frames",
            "source frame count must be between 1 and 1000000",
        ))
    } else {
        Ok(())
    }
}

/// Computes the stable compressed-source fingerprint used by frame identity.
pub(crate) fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Debug)]
pub(crate) struct OwnedMediaFile {
    path: PathBuf,
}

impl OwnedMediaFile {
    /// Creates a private, process-owned temporary source file with no shell or shared filename.
    ///
    /// # Errors
    ///
    /// Returns `source.temp` if a private file cannot be created or completely written.
    pub(crate) fn create(bytes: &[u8], suffix: &str) -> Result<Self, SourceError> {
        let suffix = suffix.trim_start_matches('.');
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(SourceError::new(
                "source.temp",
                "temporary suffix is unsafe",
            ));
        }
        for _ in 0..64 {
            let nonce = TEMP_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "toniator-media-{}-{nonce}.{suffix}",
                std::process::id()
            ));
            use std::os::unix::fs::OpenOptionsExt;
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(bytes).map_err(|error| {
                        let _ = std::fs::remove_file(&path);
                        SourceError::new("source.temp", format!("could not write source: {error}"))
                    })?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(SourceError::new(
                        "source.temp",
                        format!("could not create private source: {error}"),
                    ));
                }
            }
        }
        Err(SourceError::new(
            "source.temp",
            "could not allocate a unique private source path",
        ))
    }

    /// Returns the exact private path passed as one subprocess argument.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for OwnedMediaFile {
    /// Removes only the exact private file owned by this provider.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FrameTiming {
    pub original_pts: i64,
    pub duration_ticks: u64,
    pub normalized_time: RationalTime,
}

#[derive(Debug)]
pub(crate) struct MovingProbe {
    pub metadata: SourceMediaMetadata,
    pub frames: Vec<FrameTiming>,
    pub time_base: RationalTime,
    pub color_policy: String,
    pub filter: DecodeFilter,
    /// Selects the software decoder required to expose container-carried alpha.
    decoder: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub(crate) enum DecodeFilter {
    Simple(String),
    PreserveAlpha { spatial: String, color: String },
}

#[derive(Deserialize)]
struct ProbeRoot {
    #[serde(default)]
    frames: Vec<ProbeFrame>,
    #[serde(default)]
    streams: Vec<ProbeStream>,
}

#[derive(Deserialize)]
struct ProbeFrame {
    best_effort_timestamp: Option<i64>,
    pkt_duration: Option<u64>,
    duration: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    color_range: Option<String>,
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
}

#[derive(Clone, Deserialize)]
struct ProbeStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    sample_aspect_ratio: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    time_base: Option<String>,
    start_pts: Option<i64>,
    duration_ts: Option<u64>,
    color_range: Option<String>,
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    #[serde(default)]
    side_data_list: Vec<ProbeSideData>,
    #[serde(default)]
    tags: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Deserialize)]
struct ProbeSideData {
    rotation: Option<i32>,
}

/// Probes one moving source with bounded JSON, diagnostics, time, and cancellation.
///
/// Video uses the first video stream. Animated images prefer the first finite timed video track
/// over an AVIF poster item. RGB animations can use format-defined sRGB; YUV media must declare
/// a supported SDR range, matrix, transfer, and primaries tuple. Tagged VP8/VP9 alpha uses libvpx
/// for both metadata and frame decoding; missing decoder support fails instead of losing alpha.
///
/// # Errors
///
/// Returns process, JSON, stream, timing, color, orientation, pixel-bound, or cancellation errors.
pub(crate) fn probe_moving(
    tools: &MediaTools,
    file: &OwnedMediaFile,
    kind: SourceMediaKind,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<MovingProbe, SourceError> {
    cancelled(is_cancelled)?;
    let streams = probe_streams(tools, file, None, is_cancelled)?;
    let has_audio = streams
        .iter()
        .any(|stream| stream.codec_type.as_deref() == Some("audio"));
    let first_video = || {
        streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("video"))
    };
    let mut stream = if kind == SourceMediaKind::AnimatedImage {
        streams
            .iter()
            .find(|stream| {
                stream.codec_type.as_deref() == Some("video")
                    && stream.duration_ts.is_some_and(|duration| duration > 0)
            })
            .or_else(first_video)
    } else {
        first_video()
    }
    .cloned()
    .ok_or_else(|| {
        SourceError::new(
            "source.probe.stream",
            "source contains no first video stream",
        )
    })?;
    let decoder = alpha_decoder(&stream);
    if let Some(decoder) = decoder {
        stream = probe_streams(tools, file, Some(decoder), is_cancelled)?
            .into_iter()
            .find(|candidate| candidate.index == stream.index)
            .ok_or_else(|| SourceError::new("source.probe.stream", "alpha stream is missing"))?;
        if !stream
            .pix_fmt
            .as_deref()
            .is_some_and(|format| format.starts_with("yuva"))
        {
            return Err(SourceError::new(
                "source.alpha",
                "the selected decoder did not expose the declared alpha channel",
            ));
        }
    }
    let width = stream
        .width
        .ok_or_else(|| SourceError::new("source.probe.stream", "video width is missing"))?;
    let height = stream
        .height
        .ok_or_else(|| SourceError::new("source.probe.stream", "video height is missing"))?;
    crate::validate_dimensions(width, height)?;
    let (output_width, output_height, spatial_filter) = spatial_filter(
        width,
        height,
        stream.sample_aspect_ratio.as_deref(),
        &stream.side_data_list,
        stream.pix_fmt.as_deref(),
    )?;
    let (color_policy, color_filter, preserve_alpha) = color_filter(&stream, kind)?;
    let mut command = Command::new(&tools.ffprobe);
    command.args([
        "-v",
        "error",
        "-show_frames",
        "-show_entries",
        "stream=index,codec_type,codec_name,width,height,pix_fmt,sample_aspect_ratio,avg_frame_rate,r_frame_rate,time_base,start_pts,duration_ts,color_range,color_space,color_transfer,color_primaries:stream_side_data=rotation:frame=best_effort_timestamp,pkt_duration,duration,width,height,pix_fmt,color_range,color_space,color_transfer,color_primaries",
        "-select_streams",
        &stream.index.to_string(),
        "-of",
        "json",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        DEMUXERS,
    ]);
    if let Some(decoder) = decoder {
        command.args(["-c:v:0", decoder]);
    }
    command.arg(file.path());
    let output = run_bounded(
        &mut command,
        MAX_PROBE_STDOUT,
        MAX_PROCESS_DIAGNOSTIC,
        PROBE_TIMEOUT,
        is_cancelled,
    )?;
    let root: ProbeRoot = serde_json::from_slice(&output).map_err(|error| {
        SourceError::new(
            "source.probe.json",
            format!("invalid bounded probe JSON: {error}"),
        )
    })?;
    let time_base = parse_positive_time(stream.time_base.as_deref(), "source.probe.time_base")?;
    let start_pts = root
        .frames
        .first()
        .and_then(|frame| frame.best_effort_timestamp)
        .ok_or_else(|| {
            SourceError::new(
                "source.probe.pts",
                "first presentation timestamp is missing",
            )
        })?;
    validate_frame_count(root.frames.len())?;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(root.frames.len())
        .map_err(|_| SourceError::new("source.frames", "frame timing allocation failed"))?;
    let mut prior = None;
    for frame in root.frames {
        validate_frame_metadata(&frame, &stream, width, height, kind)?;
        let pts = frame.best_effort_timestamp.ok_or_else(|| {
            SourceError::new(
                "source.probe.pts",
                "frame presentation timestamp is missing",
            )
        })?;
        if prior.is_some_and(|prior| pts < prior) {
            return Err(SourceError::new(
                "source.probe.pts",
                "frame presentation timestamps must not decrease",
            ));
        }
        let offset = u64::try_from(pts.checked_sub(start_pts).ok_or_else(|| {
            SourceError::new(
                "source.probe.pts",
                "presentation timestamp subtraction overflowed",
            )
        })?)
        .map_err(|_| {
            SourceError::new("source.probe.pts", "frame precedes normalized source start")
        })?;
        let normalized_time = multiply_time(time_base, offset)?;
        frames.push(FrameTiming {
            original_pts: pts,
            duration_ticks: frame.pkt_duration.or(frame.duration).unwrap_or(0),
            normalized_time,
        });
        prior = Some(pts);
    }
    let duration_ticks = frames
        .last()
        .and_then(|frame| {
            (frame.duration_ticks > 0).then_some(())?;
            u64::try_from(frame.original_pts.checked_sub(start_pts)?)
                .ok()?
                .checked_add(frame.duration_ticks)
        })
        .or_else(|| {
            let offset =
                u64::try_from(start_pts.checked_sub(stream.start_pts.unwrap_or(start_pts))?)
                    .ok()?;
            stream.duration_ts?.checked_sub(offset)
        })
        .ok_or_else(|| {
            SourceError::new("source.probe.duration", "finite source duration is missing")
        })?;
    let duration = multiply_time(time_base, duration_ticks)?;
    if duration.numerator() == 0
        || frames
            .last()
            .is_some_and(|frame| frame.normalized_time.checked_cmp(duration).is_ge())
    {
        return Err(SourceError::new(
            "source.probe.duration",
            "source duration must be after every frame PTS",
        ));
    }
    let filter = if preserve_alpha {
        DecodeFilter::PreserveAlpha {
            spatial: spatial_filter,
            color: color_filter,
        }
    } else {
        DecodeFilter::Simple(
            [spatial_filter, color_filter]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(","),
        )
    };
    let nominal_frame_rate = parse_frame_rate(stream.avg_frame_rate.as_deref())
        .or_else(|| parse_frame_rate(stream.r_frame_rate.as_deref()));
    let variable_frame_rate = frames
        .windows(2)
        .map(|pair| pair[1].original_pts - pair[0].original_pts)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        > 1;
    Ok(MovingProbe {
        metadata: SourceMediaMetadata {
            kind,
            width: output_width,
            height: output_height,
            frame_count: frames.len() as u64,
            duration: Some(duration),
            nominal_frame_rate,
            variable_frame_rate,
            has_audio,
            stream_index: stream.index,
        },
        frames,
        time_base,
        color_policy,
        filter,
        decoder,
    })
}

/// Selects the latest presentation timestamp at or before an in-range source time.
///
/// # Errors
///
/// Returns `source.range` at or beyond the finite half-open duration.
pub(crate) fn select_frame(
    probe: &MovingProbe,
    position: RationalTime,
) -> Result<usize, SourceError> {
    let duration = probe
        .metadata
        .duration
        .expect("moving source duration exists");
    if position.checked_cmp(duration).is_ge() {
        return Err(SourceError::new(
            "source.range",
            "requested time lies outside the half-open source duration",
        ));
    }
    Ok(probe
        .frames
        .partition_point(|frame| frame.normalized_time.checked_cmp(position).is_le())
        .saturating_sub(1))
}

#[derive(Debug)]
pub(crate) struct DecodeSession {
    child: Child,
    receiver: Option<Receiver<Result<Vec<u8>, String>>>,
    reader: Option<JoinHandle<()>>,
    stderr_reader: Option<JoinHandle<()>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    next_index: usize,
}

impl DecodeSession {
    /// Starts one persistent presentation-order raw-RGBA FFmpeg stream with one queued frame.
    ///
    /// # Errors
    ///
    /// Returns `source.process` when the bounded decoder cannot be spawned or piped. The same
    /// explicit software decoder used for probing handles tagged VP8/VP9 alpha during decoding.
    pub(crate) fn start(
        tools: &MediaTools,
        file: &OwnedMediaFile,
        probe: &MovingProbe,
    ) -> Result<Self, SourceError> {
        let mut command = Command::new(&tools.ffmpeg);
        command.args([
            "-v",
            "error",
            "-nostdin",
            "-threads",
            "1",
            "-filter_threads",
            "1",
            "-filter_complex_threads",
            "1",
            "-noautorotate",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            DEMUXERS,
        ]);
        if let Some(decoder) = probe.decoder {
            command.args(["-c:v:0", decoder]);
        }
        command.arg("-i");
        command.arg(file.path());
        match &probe.filter {
            DecodeFilter::Simple(filter) => {
                command.args(["-map", &format!("0:{}", probe.metadata.stream_index)]);
                if !filter.is_empty() {
                    command.args(["-vf", filter]);
                }
            }
            DecodeFilter::PreserveAlpha { spatial, color } => {
                let chain = format!(
                    "[0:{index}]{prefix}split=2[color][alpha];[alpha]alphaextract[mask];[color]format=yuv444p12,{color}[converted];[converted][mask]alphamerge,format=rgba[out]",
                    index = probe.metadata.stream_index,
                    prefix = if spatial.is_empty() {
                        "".to_owned()
                    } else {
                        format!("{spatial},")
                    },
                );
                command.args(["-filter_complex", &chain, "-map", "[out]"]);
            }
        }
        command.args([
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ]);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut child = command.spawn().map_err(|error| {
            SourceError::new(
                "source.process",
                format!("could not start FFmpeg decoder: {error}"),
            )
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError::new("source.process", "decoder stdout pipe is missing"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| SourceError::new("source.process", "decoder stderr pipe is missing"))?;
        let frame_bytes =
            usize::try_from(u64::from(probe.metadata.width) * u64::from(probe.metadata.height) * 4)
                .map_err(|_| {
                    SourceError::new("source.dimensions", "decoded frame byte count is unsafe")
                })?;
        let (sender, receiver) = sync_channel(1);
        let reader = thread::spawn(move || read_raw_frames(stdout, frame_bytes, sender));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let stderr_target = Arc::clone(&stderr);
        let stderr_reader = thread::spawn(move || read_diagnostic(stderr_pipe, stderr_target));
        Ok(Self {
            child,
            receiver: Some(receiver),
            reader: Some(reader),
            stderr_reader: Some(stderr_reader),
            stderr,
            next_index: 0,
        })
    }

    /// Receives exactly one next decoded frame while polling cancellation and child status.
    ///
    /// # Errors
    ///
    /// Returns cancellation, truncated-stream, process, or channel diagnostics.
    pub(crate) fn next(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<Vec<u8>, SourceError> {
        let started = Instant::now();
        loop {
            cancelled(is_cancelled)?;
            if started.elapsed() >= PROBE_TIMEOUT {
                return Err(SourceError::new(
                    "source.process.timeout",
                    "frame decoding exceeded its time bound",
                ));
            }
            match self
                .receiver
                .as_ref()
                .expect("live decoder receiver")
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(Ok(frame)) => {
                    self.next_index += 1;
                    return Ok(frame);
                }
                Ok(Err(error)) => return Err(SourceError::new("source.decode", error)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // A reaped child can still have complete frames in the reader's pipe.
                    // Receiver disconnection proves that the reader drained them.
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(SourceError::new(
                        "source.decode",
                        format!(
                            "decoded stream ended early: {}",
                            bounded_diagnostic(&self.stderr)
                        ),
                    ));
                }
            }
        }
    }

    /// Validates exact end-of-stream cardinality and successful decoder completion.
    ///
    /// # Errors
    /// Returns cancellation, timeout, extra-frame, pipe, or nonzero-exit diagnostics.
    fn finish(&mut self, is_cancelled: &dyn Fn() -> bool) -> Result<(), SourceError> {
        let started = Instant::now();
        loop {
            cancelled(is_cancelled)?;
            if started.elapsed() >= PROBE_TIMEOUT {
                return Err(SourceError::new(
                    "source.process.timeout",
                    "decoder finalization exceeded its time bound",
                ));
            }
            match self
                .receiver
                .as_ref()
                .expect("live decoder receiver")
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(_) => {
                    return Err(SourceError::new(
                        "source.decode.frames",
                        "decoded stream exceeds the probed frame count",
                    ));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(status) = self
                        .child
                        .try_wait()
                        .map_err(|error| SourceError::new("source.process", error.to_string()))?
                    {
                        if status.success() {
                            return Ok(());
                        }
                        return Err(SourceError::new(
                            "source.decode",
                            format!("decoder failed: {}", bounded_diagnostic(&self.stderr)),
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    /// Stops, reaps, and joins the persistent decoder before releasing its private source.
    fn stop(&mut self) {
        self.receiver.take();
        let _ = self.child.kill();
        let start = Instant::now();
        while start.elapsed() < DECODE_STOP_TIMEOUT {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
            }
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for DecodeSession {
    /// Ensures cancellation/drop kills and reaps the child and joins both bounded readers.
    fn drop(&mut self) {
        self.stop();
    }
}

/// Advances or resets a persistent decoder and constructs the requested immutable direct-RGBA field.
///
/// # Errors
///
/// Returns decode, cancellation, pixel-bound, or direct-field construction diagnostics.
pub(crate) fn decode_selected(
    state: &mut DecodeState,
    tools: &MediaTools,
    file: &OwnedMediaFile,
    probe: &MovingProbe,
    source_fingerprint: &str,
    target: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SourceFrame, SourceError> {
    let DecodeState { session, current } = state;
    if let Some((index, frame)) = current.as_ref()
        && *index == target
    {
        cancelled(is_cancelled)?;
        return Ok(frame.clone());
    }
    let must_reset = session
        .as_ref()
        .is_none_or(|value| target < value.next_index);
    if must_reset {
        *session = None;
        *session = Some(DecodeSession::start(tools, file, probe)?);
    }
    let session_ref = session.as_mut().expect("decoder session exists");
    while session_ref.next_index <= target {
        let index = session_ref.next_index;
        let rgba = session_ref.next(is_cancelled)?;
        if index == target {
            if index + 1 == probe.frames.len() {
                session_ref.finish(is_cancelled)?;
            }
            let field = Arc::new(SourceField::from_straight_rgba8(
                probe.metadata.width,
                probe.metadata.height,
                rgba,
            )?);
            cancelled(is_cancelled)?;
            let timing = &probe.frames[index];
            let frame = SourceFrame {
                identity: FrameIdentity {
                    source_fingerprint: source_fingerprint.to_owned(),
                    stream_index: probe.metadata.stream_index,
                    original_pts: timing.original_pts,
                    time_base: probe.time_base,
                    decoder_contract: probe.decoder.map_or_else(
                        || DECODER_CONTRACT_ID.to_owned(),
                        |decoder| format!("{DECODER_CONTRACT_ID}:{decoder}"),
                    ),
                    color_policy: probe.color_policy.clone(),
                    decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
                },
                field,
                index: index as u64,
                normalized_time: timing.normalized_time,
            };
            *current = Some((index, frame.clone()));
            return Ok(frame);
        }
    }
    Err(SourceError::new(
        "source.decode",
        "decoder did not reach requested frame",
    ))
}

/// Couples one bounded decoder and its last published immutable frame.
#[derive(Debug, Default)]
pub(crate) struct DecodeState {
    pub session: Option<DecodeSession>,
    pub current: Option<(usize, SourceFrame)>,
}

/// Runs one subprocess while concurrently consuming bounded stdout and stderr.
///
/// # Errors
///
/// Returns spawn, timeout, cancellation, output-limit, wait, or nonzero-exit diagnostics.
fn run_bounded(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, SourceError> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = command.spawn().map_err(|error| {
        SourceError::new(
            "source.process",
            format!("could not start media tool: {error}"),
        )
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SourceError::new("source.process", "media tool stdout pipe is missing"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SourceError::new("source.process", "media tool stderr pipe is missing"))?;
    let stdout_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_flag = Arc::clone(&stdout_exceeded);
    let stdout_reader = thread::spawn(move || read_bounded(stdout, stdout_limit, stdout_flag));
    let stderr_exceeded = Arc::new(AtomicBool::new(false));
    let stderr_flag = Arc::clone(&stderr_exceeded);
    let stderr_reader = thread::spawn(move || read_bounded(stderr, stderr_limit, stderr_flag));
    let started = Instant::now();
    let mut failure_path = None;
    let status = loop {
        if is_cancelled() {
            failure_path = Some(("source.cancelled", "media operation cancelled"));
            let _ = child.kill();
        }
        if stdout_exceeded.load(AtomicOrdering::Relaxed) {
            failure_path = Some((
                "source.probe.bounds",
                "media tool output exceeded its bound",
            ));
            let _ = child.kill();
        }
        if started.elapsed() >= timeout {
            failure_path = Some((
                "source.process.timeout",
                "media tool exceeded its time bound",
            ));
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(SourceError::new(
                    "source.process",
                    format!("could not poll media tool: {error}"),
                ));
            }
        }
    };
    let stdout = stdout_reader.join();
    let stderr = stderr_reader.join();
    let status = status?;
    let stdout = stdout
        .map_err(|_| SourceError::new("source.process", "media stdout reader panicked"))??;
    let stderr = stderr
        .map_err(|_| SourceError::new("source.process", "media stderr reader panicked"))??;
    if let Some((path, message)) = failure_path {
        return Err(SourceError::new(path, message));
    }
    if stderr_exceeded.load(AtomicOrdering::Relaxed) {
        return Err(SourceError::new(
            "source.process.diagnostic",
            "media diagnostic exceeded its bound",
        ));
    }
    if !status.success() {
        return Err(SourceError::new(
            "source.process",
            format!(
                "media tool exited with {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        ));
    }
    Ok(stdout)
}

/// Reads and drains one process pipe while retaining only a bounded prefix.
///
/// # Errors
///
/// Returns the underlying pipe read error after draining stops.
fn read_bounded(
    mut reader: impl Read,
    limit: usize,
    exceeded: Arc<AtomicBool>,
) -> Result<Vec<u8>, SourceError> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer).map_err(|error| {
            SourceError::new(
                "source.process",
                format!("could not read media tool output: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..count.min(remaining)]);
        if count > remaining {
            exceeded.store(true, AtomicOrdering::Relaxed);
        }
    }
    Ok(output)
}

/// Reads fixed-size raw frames and delivers them through a capacity-one channel.
fn read_raw_frames(
    mut stdout: impl Read,
    frame_bytes: usize,
    sender: SyncSender<Result<Vec<u8>, String>>,
) {
    loop {
        let mut frame = Vec::new();
        if frame.try_reserve_exact(frame_bytes).is_err() {
            let _ = sender.send(Err("raw RGBA frame allocation failed".into()));
            return;
        }
        frame.resize(frame_bytes, 0);
        let mut offset = 0;
        while offset < frame_bytes {
            match stdout.read(&mut frame[offset..]) {
                Ok(0) if offset == 0 => return,
                Ok(0) => {
                    let _ = sender.send(Err("raw RGBA frame was truncated".to_owned()));
                    return;
                }
                Ok(count) => offset += count,
                Err(error) => {
                    let _ = sender.send(Err(format!("could not read raw RGBA frame: {error}")));
                    return;
                }
            }
        }
        if sender.send(Ok(frame)).is_err() {
            return;
        }
    }
}

/// Consumes stderr concurrently while retaining a 64 KiB diagnostic prefix.
fn read_diagnostic(mut stderr: impl Read, target: Arc<Mutex<Vec<u8>>>) {
    let mut buffer = [0u8; 4096];
    loop {
        let Ok(count) = stderr.read(&mut buffer) else {
            return;
        };
        if count == 0 {
            return;
        }
        let mut output = target
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let remaining = MAX_PROCESS_DIAGNOSTIC.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..count.min(remaining)]);
    }
}

/// Copies the bounded decoder diagnostic without exposing synchronization state.
fn bounded_diagnostic(stderr: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(
        &stderr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
    .into_owned()
}

/// Selects the alpha-capable software decoder for the first video stream's container declaration.
/// Ordinary opaque streams retain their existing decoder; codec choice never invents color tags.
fn alpha_decoder(stream: &ProbeStream) -> Option<&'static str> {
    if !stream
        .tags
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case("alpha_mode") && value == "1")
    {
        return None;
    }
    match stream.codec_name.as_deref() {
        Some("vp8") => Some("libvpx"),
        Some("vp9") => Some("libvpx-vp9"),
        _ => None,
    }
}

/// Probes stream metadata before choosing the first video or finite animated-image track.
/// A supplied software decoder applies to the first video stream, independently of audio order.
///
/// # Errors
///
/// Returns the same bounded subprocess and JSON diagnostics as the primary probe.
fn probe_streams(
    tools: &MediaTools,
    file: &OwnedMediaFile,
    decoder: Option<&str>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ProbeStream>, SourceError> {
    let mut command = Command::new(&tools.ffprobe);
    command.args(["-v", "error", "-show_entries", "stream=index,codec_type,codec_name,width,height,pix_fmt,sample_aspect_ratio,avg_frame_rate,r_frame_rate,time_base,start_pts,duration_ts,color_range,color_space,color_transfer,color_primaries:stream_side_data=rotation:stream_tags=alpha_mode", "-of", "json", "-protocol_whitelist", "file", "-format_whitelist", DEMUXERS]);
    if let Some(decoder) = decoder {
        command.args(["-c:v:0", decoder]);
    }
    command.arg(file.path());
    let output = run_bounded(
        &mut command,
        1024 * 1024,
        MAX_PROCESS_DIAGNOSTIC,
        PROBE_TIMEOUT,
        is_cancelled,
    )?;
    let root: ProbeRoot = serde_json::from_slice(&output).map_err(|error| {
        SourceError::new(
            "source.probe.json",
            format!("invalid stream probe JSON: {error}"),
        )
    })?;
    Ok(root.streams)
}

/// Parses one positive `numerator/denominator` exact time.
///
/// # Errors
///
/// Returns the supplied path for absent, malformed, negative, zero, or excessive values.
fn parse_positive_time(
    value: Option<&str>,
    path: &'static str,
) -> Result<RationalTime, SourceError> {
    let value = value.ok_or_else(|| SourceError::new(path, "exact rational value is missing"))?;
    let (numerator, denominator) = value
        .split_once('/')
        .ok_or_else(|| SourceError::new(path, "exact rational value is malformed"))?;
    let numerator = numerator
        .parse::<u64>()
        .map_err(|_| SourceError::new(path, "exact rational numerator is invalid"))?;
    let denominator = denominator
        .parse::<u64>()
        .map_err(|_| SourceError::new(path, "exact rational denominator is invalid"))?;
    if numerator == 0 {
        return Err(SourceError::new(
            path,
            "exact rational value must be positive",
        ));
    }
    Ok(RationalTime::new(numerator, denominator)?)
}

/// Parses one optional positive frame rate, ignoring conventional unknown `0/0` values.
fn parse_frame_rate(value: Option<&str>) -> Option<FrameRate> {
    let (numerator, denominator) = value?.split_once('/')?;
    FrameRate::new(numerator.parse().ok()?, denominator.parse().ok()?).ok()
}

/// Multiplies a positive exact time base by integer ticks with overflow checking.
///
/// # Errors
///
/// Returns `source.probe.time` if the numerator exceeds the domain rational bound.
fn multiply_time(time_base: RationalTime, ticks: u64) -> Result<RationalTime, SourceError> {
    RationalTime::new(
        time_base.numerator().checked_mul(ticks).ok_or_else(|| {
            SourceError::new("source.probe.time", "normalized timestamp overflowed")
        })?,
        time_base.denominator(),
    )
    .map_err(Into::into)
}

/// Rejects midstream dimension, pixel-layout, or color changes before starting the raw decoder.
///
/// # Errors
/// Returns a metadata diagnostic when a frame disagrees with the selected stream authority.
fn validate_frame_metadata(
    frame: &ProbeFrame,
    stream: &ProbeStream,
    width: u32,
    height: u32,
    _kind: SourceMediaKind,
) -> Result<(), SourceError> {
    crate::validate_dimensions(width, height)?;
    if frame.width.is_some_and(|value| value != width)
        || frame.height.is_some_and(|value| value != height)
    {
        return Err(SourceError::new(
            "source.probe.dimensions",
            "changing frame dimensions are unsupported",
        ));
    }
    for (actual, expected) in [
        (&frame.pix_fmt, &stream.pix_fmt),
        (&frame.color_range, &stream.color_range),
        (&frame.color_space, &stream.color_space),
        (&frame.color_transfer, &stream.color_transfer),
        (&frame.color_primaries, &stream.color_primaries),
    ] {
        if actual.is_some() && actual != expected {
            return Err(SourceError::new(
                "source.probe.color",
                "changing frame color or pixel layout is unsupported",
            ));
        }
    }
    Ok(())
}

/// Derives deterministic square-pixel dimensions and rotation filters under the pixel bound.
/// Packed RGB alpha moves through planar channels so nearest scaling preserves fractional alpha.
///
/// # Errors
///
/// Returns diagnostics for malformed SAR, unsupported rotation, or unsafe output dimensions.
fn spatial_filter(
    width: u32,
    height: u32,
    sar: Option<&str>,
    side_data: &[ProbeSideData],
    pixel_format: Option<&str>,
) -> Result<(u32, u32, String), SourceError> {
    let (sar_num, sar_den) = match sar.unwrap_or("1:1").split_once(':') {
        Some((left, right)) => (
            left.parse::<u64>().map_err(|_| {
                SourceError::new("source.probe.sar", "sample aspect ratio is invalid")
            })?,
            right.parse::<u64>().map_err(|_| {
                SourceError::new("source.probe.sar", "sample aspect ratio is invalid")
            })?,
        ),
        None => {
            return Err(SourceError::new(
                "source.probe.sar",
                "sample aspect ratio is malformed",
            ));
        }
    };
    if sar_num == 0 || sar_den == 0 {
        return Err(SourceError::new(
            "source.probe.sar",
            "sample aspect ratio must be positive",
        ));
    }
    let square_width_u64 =
        (u128::from(width) * u128::from(sar_num) + u128::from(sar_den) / 2) / u128::from(sar_den);
    let square_width = u32::try_from(square_width_u64)
        .map_err(|_| SourceError::new("source.dimensions", "square-pixel width is unsafe"))?;
    let rotation = side_data
        .iter()
        .find_map(|data| data.rotation)
        .unwrap_or(0)
        .rem_euclid(360);
    let (output_width, output_height) = if matches!(rotation, 90 | 270) {
        (height, square_width)
    } else {
        (square_width, height)
    };
    if !matches!(rotation, 0 | 90 | 180 | 270) {
        return Err(SourceError::new(
            "source.orientation",
            "only right-angle source rotation is supported",
        ));
    }
    let pixels = u64::from(output_width) * u64::from(output_height);
    if output_width == 0 || output_height == 0 || pixels > 64 * 1024 * 1024 {
        return Err(SourceError::new(
            "source.dimensions",
            "oriented square-pixel source exceeds the 64-megapixel bound",
        ));
    }
    let mut filters = Vec::new();
    if square_width != width {
        // Packed RGB scaling changes alpha 128 to 129 in swscale. Independent planar channels
        // retain exact nearest-neighbor values, including hidden RGB, before final RGBA output.
        match pixel_format {
            Some("rgba" | "bgra" | "argb" | "abgr") => filters.push("format=gbrap".to_owned()),
            Some(format) if format.starts_with("rgba64") || format.starts_with("bgra64") => {
                filters.push("format=gbrap16le".to_owned());
            }
            _ => {}
        }
        filters.push(format!("scale={square_width}:{height}:flags=neighbor"));
    }
    if sar_num != sar_den {
        filters.push("setsar=1".to_owned());
    }
    match rotation {
        90 => filters.push("transpose=cclock".to_owned()),
        180 => filters.push("hflip,vflip".to_owned()),
        270 => filters.push("transpose=clock".to_owned()),
        _ => {}
    }
    Ok((output_width, output_height, filters.join(",")))
}

/// Derives an explicit sRGB output conversion or format-defined animated-image fallback.
/// Validated stream tags supply input interpretation even when a decoder drops per-frame tags.
///
/// # Errors
///
/// Returns `source.color` for HDR, incomplete video tags, or unsupported SDR metadata.
fn color_filter(
    stream: &ProbeStream,
    kind: SourceMediaKind,
) -> Result<(String, String, bool), SourceError> {
    let pix_fmt = stream
        .pix_fmt
        .as_deref()
        .ok_or_else(|| SourceError::new("source.color", "pixel format is missing"))?;
    let transfer = stream.color_transfer.as_deref();
    if matches!(transfer, Some("smpte2084" | "arib-std-b67")) {
        return Err(SourceError::new(
            "source.color",
            "HDR transfer is unsupported without an explicit tone-map policy",
        ));
    }
    let has_alpha = matches!(pix_fmt, "rgba" | "bgra" | "argb" | "abgr" | "pal8")
        || ["gbrap", "yuva", "ya", "rgba64", "bgra64"]
            .iter()
            .any(|prefix| pix_fmt.starts_with(prefix));
    let already_srgb = stream.color_space.as_deref() == Some("gbr")
        && matches!(transfer, Some("iec61966-2-1" | "srgb"))
        && stream.color_primaries.as_deref() == Some("bt709");
    if already_srgb {
        return Ok((
            "declared-full-range-srgb-rgb".to_owned(),
            "format=rgba".to_owned(),
            false,
        ));
    }
    if matches!(
        pix_fmt,
        "rgb24" | "bgr24" | "rgba" | "bgra" | "argb" | "abgr"
    ) && matches!(stream.color_space.as_deref(), None | Some("gbr"))
        && matches!(stream.color_range.as_deref(), None | Some("pc" | "jpeg"))
        && transfer.is_none()
        && stream.color_primaries.is_none()
    {
        // Untagged full-range RGB follows the same explicit sRGB interpretation as still
        // image pixels. In particular FFV1/BGRA may omit transfer and primaries tags.
        return Ok((
            "untagged-full-range-rgb-assumed-srgb".into(),
            "format=rgba".into(),
            false,
        ));
    }
    if kind == SourceMediaKind::AnimatedImage
        && matches!(pix_fmt, "rgba" | "bgra" | "rgb24" | "bgr24" | "pal8")
        && transfer.is_none()
        && stream.color_primaries.is_none()
        && matches!(stream.color_space.as_deref(), None | Some("gbr"))
        && matches!(stream.color_range.as_deref(), None | Some("pc" | "jpeg"))
    {
        return Ok((
            "animated-image-format-defined-srgb".to_owned(),
            "format=rgba".to_owned(),
            false,
        ));
    }
    let range = stream.color_range.as_deref().ok_or_else(|| {
        SourceError::new("source.color", "video color range metadata is required")
    })?;
    let space = stream.color_space.as_deref().ok_or_else(|| {
        SourceError::new("source.color", "video color matrix metadata is required")
    })?;
    let transfer = transfer
        .ok_or_else(|| SourceError::new("source.color", "video transfer metadata is required"))?;
    let primaries = stream
        .color_primaries
        .as_deref()
        .ok_or_else(|| SourceError::new("source.color", "video primaries metadata is required"))?;
    if !matches!(range, "tv" | "mpeg" | "pc" | "jpeg")
        || !matches!(
            space,
            "bt709" | "bt470bg" | "smpte170m" | "smpte240m" | "bt2020nc"
        )
        || !matches!(
            transfer,
            "bt709"
                | "bt470m"
                | "gamma22"
                | "bt470bg"
                | "gamma28"
                | "smpte170m"
                | "smpte240m"
                | "linear"
                | "iec61966-2-1"
                | "srgb"
                | "bt2020-10"
                | "bt2020-12"
        )
        || !matches!(
            primaries,
            "bt709" | "bt470m" | "bt470bg" | "smpte170m" | "smpte240m" | "bt2020"
        )
    {
        return Err(SourceError::new(
            "source.color",
            "declared SDR color metadata is unsupported",
        ));
    }
    let policy =
        format!("ffmpeg-colorspace:{range}:{space}:{transfer}:{primaries}->bt709:srgb:rgba8");
    // Decoder/filter frames may omit tags (notably VP8). The validated stream supplies input
    // interpretation explicitly; alpha bypasses this conversion rather than becoming color.
    let conversion = format!(
        "colorspace=ispace={space}:itrc={transfer}:iprimaries={primaries}:irange={range}:all=bt709:trc=srgb:format=yuv444p12,format=rgba"
    );
    Ok((policy, conversion, has_alpha))
}
