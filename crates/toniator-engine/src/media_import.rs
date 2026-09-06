//! Shared local-media import and exact timing selection for CLI and desktop workers.

use crate::media::{open_source_media, still_hint};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::PathBuf,
};
use toniator_domain::{
    FrameRange, FrameRate, ProjectTiming, RationalTime, SourceReferenceId, TimeRange,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, SourceMediaManifest};
use toniator_sampling::media::{
    FrameSource, MediaTools, SourceError, SourceMediaMetadata, is_animated_image,
};

/// Transferable import result; frame providers stay inside their owning worker.
#[derive(Clone, Debug)]
pub struct ImportedMedia {
    pub sources: SourceBundle,
    pub source_id: SourceReferenceId,
    pub metadata: SourceMediaMetadata,
}

/// Imports one local source or explicitly ordered still sequence under portable-container bounds.
///
/// Paths are never expanded, sorted, or treated as network URLs. Canonical-path repetitions reuse
/// encoded bytes and IDs while retaining their sequence positions. Source probing remains in the
/// sampling layer. Every unique sequence image is decoded once for compatibility preflight with
/// the provider's one-frame cache. The caller commits no document state until this succeeds.
///
/// # Errors
/// Rejects nonregular files, unsupported media, missing sequence rate, bounds, decode, or cancellation.
pub fn import_source_media(
    paths: &[PathBuf],
    sequence_rate: Option<FrameRate>,
    tools: MediaTools,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<ImportedMedia, SourceError> {
    if paths.is_empty() || paths.len() > 1_000_000 || (paths.len() > 1 && sequence_rate.is_none()) {
        return Err(SourceError::new(
            "source.sequence",
            "supply one source or an ordered sequence with a frame rate",
        ));
    }
    let mut entries = Vec::new();
    let mut known: HashMap<PathBuf, SourceReferenceId> = HashMap::new();
    let mut order = Vec::with_capacity(paths.len());
    let mut total = 0u64;
    let mut single_kind = None;
    for path in paths {
        if is_cancelled() {
            return Err(SourceError::new(
                "source.cancelled",
                "source import was cancelled",
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| SourceError::new("source.file", error.to_string()))?;
        if let Some(id) = known.get(&canonical) {
            order.push(id.clone());
            continue;
        }
        if entries.len() >= toniator_io::MAX_SOURCE_ENTRIES {
            return Err(SourceError::new(
                "source.limits",
                "too many unique source entries",
            ));
        }
        let format = import_format(path)?;
        if !canonical
            .metadata()
            .map_err(|error| SourceError::new("source.file", error.to_string()))?
            .is_file()
        {
            return Err(SourceError::new(
                "source.file",
                "source must be a regular file",
            ));
        }
        let mut file = File::open(&canonical)
            .map_err(|error| SourceError::new("source.file", error.to_string()))?;
        let metadata = file
            .metadata()
            .map_err(|error| SourceError::new("source.file", error.to_string()))?;
        let remaining = toniator_io::MAX_SOURCE_BYTES - total;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > remaining {
            return Err(SourceError::new(
                "source.limits",
                "source must be a nonempty regular file within the 128 MiB aggregate limit",
            ));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(remaining + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| SourceError::new("source.file", error.to_string()))?;
        if bytes.len() as u64 > remaining {
            return Err(SourceError::new(
                "source.limits",
                "source exceeds the 128 MiB aggregate limit",
            ));
        }
        total += bytes.len() as u64;
        let animated = match format {
            EmbeddedSourceFormat::Video => false,
            EmbeddedSourceFormat::Gif => true,
            _ => is_animated_image(&bytes, still_hint(format)?)?,
        };
        if sequence_rate.is_some() && (animated || format == EmbeddedSourceFormat::Video) {
            return Err(SourceError::new(
                "source.sequence",
                "sequence entries must be still images",
            ));
        }
        let id = SourceReferenceId::new(format!("source-{}", entries.len() + 1))?;
        single_kind = Some(if format == EmbeddedSourceFormat::Video {
            SourceMediaManifest::Video {
                source_id: id.clone(),
            }
        } else if animated {
            SourceMediaManifest::AnimatedImage {
                source_id: id.clone(),
            }
        } else {
            SourceMediaManifest::StillImage {
                source_id: id.clone(),
            }
        });
        entries.push(
            EmbeddedSource::new(
                id.clone(),
                format,
                bytes,
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned),
            )
            .map_err(|error| SourceError::new(error.path(), error.context()))?,
        );
        known.insert(canonical, id.clone());
        order.push(id);
    }
    let source_id = order[0].clone();
    let manifest = match sequence_rate {
        Some(frame_rate) => SourceMediaManifest::ImageSequence {
            source_ids: order,
            frame_rate,
        },
        None => single_kind.ok_or_else(|| SourceError::new("source.media", "missing source"))?,
    };
    let sources = SourceBundle::new_media(entries, manifest)
        .map_err(|error| SourceError::new(error.path(), error.context()))?;
    let mut media = open_source_media(&sources, tools, is_cancelled)?;
    if let Some(SourceMediaManifest::ImageSequence {
        source_ids,
        frame_rate,
    }) = sources.media()
    {
        let mut validated = HashSet::new();
        for (index, source_id) in source_ids.iter().enumerate() {
            if is_cancelled() {
                return Err(SourceError::new(
                    "source.cancelled",
                    "source import was cancelled",
                ));
            }
            if validated.insert(source_id) {
                media.frame_at(frame_rate.time_for_frame(index as u64)?, is_cancelled)?;
            }
        }
    }
    Ok(ImportedMedia {
        sources,
        source_id,
        metadata: media.metadata().clone(),
    })
}

/// Maps supported local file suffixes to persisted format authority; decoders validate contents.
///
/// # Errors
/// Rejects missing/unsupported extensions without falling back to an unrelated decoder.
fn import_format(path: &std::path::Path) -> Result<EmbeddedSourceFormat, SourceError> {
    Ok(
        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png" | "apng") => EmbeddedSourceFormat::Png,
            Some("svg") => EmbeddedSourceFormat::Svg,
            Some("jpg" | "jpeg") => EmbeddedSourceFormat::Jpeg,
            Some("webp") => EmbeddedSourceFormat::Webp,
            Some("bmp") => EmbeddedSourceFormat::Bmp,
            Some("tif" | "tiff") => EmbeddedSourceFormat::Tiff,
            Some("exr") => EmbeddedSourceFormat::OpenExr,
            Some("avif" | "avifs") => EmbeddedSourceFormat::Avif,
            Some("gif") => EmbeddedSourceFormat::Gif,
            Some(
                "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "ogv" | "mpeg" | "mpg" | "ts",
            ) => EmbeddedSourceFormat::Video,
            _ => {
                return Err(SourceError::new(
                    "source.format",
                    "unsupported source file extension",
                ));
            }
        },
    )
}

/// Optional export/import timing projection, shared by both frontends.
#[derive(Clone, Debug, Default)]
pub struct MediaTimingSelection {
    pub frame_rate: Option<FrameRate>,
    pub start_frame: Option<u64>,
    /// Inclusive CLI/UI boundary, converted exactly once to the domain's half-open range.
    pub end_frame: Option<u64>,
    pub start_time: Option<RationalTime>,
    pub end_time: Option<RationalTime>,
}

/// Parses nonnegative exact seconds for both frontends without floating-point rounding.
///
/// # Errors
/// Rejects signs, exponents, malformed decimals/fractions and integer overflow.
pub fn parse_media_time(value: &str) -> Result<RationalTime, String> {
    let integer = |part: &str| -> Result<u64, String> {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("expected unsigned integer, decimal, or numerator/denominator".into());
        }
        part.parse()
            .map_err(|_| "exact value exceeds the supported integer range".into())
    };
    let (numerator, denominator) = if let Some((numerator, denominator)) = value.split_once('/') {
        (integer(numerator)?, integer(denominator)?)
    } else if let Some((whole, fraction)) = value.split_once('.') {
        let fractional = integer(fraction)?;
        let denominator = 10u64
            .checked_pow(u32::try_from(fraction.len()).map_err(|_| "decimal is too long")?)
            .ok_or("decimal precision exceeds the exact integer bound")?;
        let numerator = integer(whole)?
            .checked_mul(denominator)
            .and_then(|part| part.checked_add(fractional))
            .ok_or("decimal value exceeds the exact integer bound")?;
        (numerator, denominator)
    } else {
        (integer(value)?, 1)
    };
    RationalTime::new(numerator, denominator).map_err(|error| error.to_string())
}

/// Parses a positive exact frame rate within the domain's reduced u32 component bounds.
///
/// # Errors
/// Rejects malformed/nonpositive values and rates outside the domain representation.
pub fn parse_media_rate(value: &str) -> Result<FrameRate, String> {
    let time = parse_media_time(value)?;
    FrameRate::new(
        u32::try_from(time.numerator()).map_err(|_| "frame-rate numerator is too large")?,
        u32::try_from(time.denominator()).map_err(|_| "frame-rate denominator is too large")?,
    )
    .map_err(|error| error.to_string())
}

/// Resolves source defaults or overrides while preserving exact source speed and endpoint scope.
///
/// An unchanged project retains its authored timing. Rate-only changes preserve its selected
/// source interval. Explicit frame indices address the source at the selected output rate;
/// explicit times form a half-open interval. Stills default to five seconds at 30 fps.
///
/// # Errors
/// Rejects mixed selectors, empty/reversed/out-of-source ranges, over one million frames, and
/// exact integer overflow. This function never edits a document or decodes a frame.
pub fn select_media_timing(
    metadata: &SourceMediaMetadata,
    current: Option<&ProjectTiming>,
    selection: &MediaTimingSelection,
) -> Result<ProjectTiming, SourceError> {
    let frames = selection.start_frame.is_some() || selection.end_frame.is_some();
    let times = selection.start_time.is_some() || selection.end_time.is_some();
    if frames && times {
        return Err(SourceError::new(
            "temporal.selection",
            "frame and time range selectors conflict",
        ));
    }
    if !frames
        && !times
        && selection.frame_rate.is_none()
        && let Some(current) = current
    {
        return validate_timing(current.clone(), metadata);
    }
    let rate = selection
        .frame_rate
        .or_else(|| current.map(ProjectTiming::frame_rate))
        .or(metadata.nominal_frame_rate)
        .unwrap_or_default();
    let base = match current {
        Some(timing) => match timing.source_time_range() {
            Some(range) => range,
            None => TimeRange::new(
                timing
                    .frame_rate()
                    .time_for_frame(timing.frame_range().start())?,
                timing
                    .frame_rate()
                    .time_for_frame(timing.frame_range().end_exclusive())?,
            )?,
        },
        None => TimeRange::new(
            RationalTime::default(),
            metadata.duration.unwrap_or(RationalTime::new(5, 1)?),
        )?,
    };
    let timing = if frames {
        let count =
            ProjectTiming::new(rate, FrameRange::default()).frames_for_duration(base.end())?;
        let start = selection.start_frame.unwrap_or(0);
        let end = selection
            .end_frame
            .map(|value| {
                value.checked_add(1).ok_or_else(|| {
                    SourceError::new("temporal.frame_range", "inclusive end frame overflowed")
                })
            })
            .transpose()?
            .unwrap_or(count);
        ProjectTiming::new(rate, FrameRange::new(start, end)?)
    } else {
        let range = TimeRange::new(
            selection.start_time.unwrap_or(base.start()),
            selection.end_time.unwrap_or(base.end()),
        )?;
        let count = ProjectTiming::new(rate, FrameRange::default())
            .frames_for_duration(range.duration()?)?;
        ProjectTiming::new(rate, FrameRange::new(0, count)?).with_source_time_range(range)
    };
    validate_timing(timing, metadata)
}

/// Checks shared source bounds before an import or export snapshot can be committed.
///
/// # Errors
/// Rejects excessive frame counts and requests reaching past a finite source's exclusive end.
fn validate_timing(
    timing: ProjectTiming,
    metadata: &SourceMediaMetadata,
) -> Result<ProjectTiming, SourceError> {
    if let Some(range) = timing.source_time_range()
        && timing.frames_for_duration(range.duration()?)? != timing.frame_range().frame_count()
    {
        return Err(SourceError::new(
            "temporal.frame_range",
            "frame count must equal ceil(selected duration * frame rate)",
        ));
    }
    if timing.frame_range().frame_count() > 1_000_000 {
        return Err(SourceError::new(
            "temporal.frame_range",
            "output exceeds one million frames",
        ));
    }
    if let Some(duration) = metadata.duration
        && (timing
            .source_time_range()
            .is_some_and(|range| range.end().checked_cmp(duration).is_gt())
            || timing
                .source_time_for_frame(timing.frame_range().end_exclusive() - 1)?
                .checked_cmp(duration)
                .is_ge())
    {
        return Err(SourceError::new(
            "temporal.source_range",
            "selected range extends beyond the source duration",
        ));
    }
    Ok(timing)
}

#[cfg(test)]
#[path = "media_import_tests.rs"]
mod tests;
