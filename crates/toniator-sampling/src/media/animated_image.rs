use std::{io::Cursor, sync::Arc};

use image::{AnimationDecoder, ImageDecoder};
use toniator_domain::RationalTime;

use super::frame_source::{
    FrameIdentity, cancelled, fingerprint, validate_compressed_size, validate_frame_count,
};
use super::{
    FrameSource, MediaTools, SourceError, SourceFrame, SourceMediaKind, SourceMediaMetadata,
    VideoSource,
};
use crate::{SourceField, validate_dimensions};

/// Explicit animated-image container selection; loop flags never repeat the source indefinitely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatedImageFormat {
    Png,
    Webp,
    Gif,
    Avif,
}

/// Detects animation using native PNG/WebP headers and the AVIF sequence brand.
///
/// This only selects a provider; that provider validates and decodes the complete finite source.
/// AVIF's `avis` brand describes an image sequence even when its filename ends in `.avif`.
///
/// # Errors
/// Rejects oversized data and malformed PNG, WebP, or AVIF container headers.
pub fn is_animated_image(
    bytes: &[u8],
    format: crate::SourceFormatHint,
) -> Result<bool, SourceError> {
    validate_compressed_size(bytes.len())?;
    match format {
        crate::SourceFormatHint::Png => image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .and_then(|decoder| decoder.is_apng())
            .map_err(image_error),
        crate::SourceFormatHint::Webp => image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .map(|decoder| decoder.has_animation())
            .map_err(image_error),
        crate::SourceFormatHint::Avif => {
            // ISO BMFF requires ftyp first for AVIF. Inspect brands, never scan image payloads.
            let invalid = || SourceError::new("source.avif.header", "invalid AVIF file-type box");
            let header = bytes.get(..16).ok_or_else(invalid)?;
            if &header[4..8] != b"ftyp" {
                return Err(invalid());
            }
            let size = u32::from_be_bytes(header[..4].try_into().map_err(|_| invalid())?) as usize;
            if size < 16 || !size.is_multiple_of(4) {
                return Err(invalid());
            }
            let brands = bytes.get(..size).ok_or_else(invalid)?;
            Ok(&brands[8..12] == b"avis"
                || brands[16..].chunks_exact(4).any(|brand| brand == b"avis"))
        }
        _ => Ok(false),
    }
}

/// Uses mature image decoders for APNG/WebP and the bounded FFmpeg provider for GIF/AVIF.
pub struct AnimatedImageSource {
    inner: AnimatedBackend,
}

enum AnimatedBackend {
    Native(Box<NativeAnimation>),
    Process(Box<VideoSource>),
}

struct NativeAnimation {
    bytes: Arc<[u8]>,
    format: AnimatedImageFormat,
    metadata: SourceMediaMetadata,
    times: Vec<RationalTime>,
    frames: image::Frames<'static>,
    next_index: usize,
    current: Option<SourceFrame>,
    fingerprint: String,
}

impl std::fmt::Debug for AnimatedImageSource {
    /// Reports metadata without dumping compressed artwork or decoder state.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimatedImageSource")
            .field("metadata", self.metadata())
            .finish()
    }
}

impl AnimatedImageSource {
    /// Probes one finite animation pass while retaining only bounded compressed data and timing.
    ///
    /// # Errors
    /// Rejects invalid/oversized images, excessive frame tables, unsupported colors, cancelled work,
    /// or missing decoder tools. Format-defined image colors enter the straight sRGB authority.
    pub fn new(
        bytes: impl Into<Arc<[u8]>>,
        format: AnimatedImageFormat,
        tools: MediaTools,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, SourceError> {
        let bytes = bytes.into();
        validate_compressed_size(bytes.len())?;
        cancelled(is_cancelled)?;
        if matches!(format, AnimatedImageFormat::Gif | AnimatedImageFormat::Avif) {
            return Ok(Self {
                inner: AnimatedBackend::Process(Box::new(VideoSource::open(
                    bytes,
                    tools,
                    SourceMediaKind::AnimatedImage,
                    is_cancelled,
                )?)),
            });
        }
        let (width, height, mut frames) = native_frames(Arc::clone(&bytes), format)?;
        let mut times = Vec::new();
        let mut duration = RationalTime::default();
        let mut first_delay = None;
        let mut variable_frame_rate = false;
        for frame in &mut frames {
            cancelled(is_cancelled)?;
            let frame = frame.map_err(image_error)?;
            validate_frame_count(times.len() + 1)?;
            times.push(duration);
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let delay = RationalTime::new(u64::from(numerator), u64::from(denominator) * 1000)?;
            variable_frame_rate |= first_delay.is_some_and(|first| first != delay);
            first_delay.get_or_insert(delay);
            duration = add_time(duration, delay)?;
        }
        validate_frame_count(times.len())?;
        if duration.numerator() == 0 {
            return Err(SourceError::new(
                "source.animation.duration",
                "animation duration must be positive",
            ));
        }
        let (_, _, frames) = native_frames(Arc::clone(&bytes), format)?;
        let metadata = SourceMediaMetadata {
            kind: SourceMediaKind::AnimatedImage,
            width,
            height,
            frame_count: times.len() as u64,
            duration: Some(duration),
            nominal_frame_rate: first_delay.and_then(|delay| {
                toniator_domain::FrameRate::new(
                    u32::try_from(delay.denominator()).ok()?,
                    u32::try_from(delay.numerator()).ok()?,
                )
                .ok()
            }),
            variable_frame_rate,
            has_audio: false,
            stream_index: 0,
        };
        let source_fingerprint = fingerprint(&bytes);
        Ok(Self {
            inner: AnimatedBackend::Native(Box::new(NativeAnimation {
                bytes,
                format,
                metadata,
                times,
                frames,
                next_index: 0,
                current: None,
                fingerprint: source_fingerprint,
            })),
        })
    }
}

impl FrameSource for AnimatedImageSource {
    /// Returns measured finite one-pass animation metadata.
    fn metadata(&self) -> &SourceMediaMetadata {
        match &self.inner {
            AnimatedBackend::Native(value) => &value.metadata,
            AnimatedBackend::Process(value) => value.metadata(),
        }
    }

    /// Selects the latest exact image presentation time with one cached decoded field.
    ///
    /// # Errors
    /// Returns cancellation, range, image decoding, bounds, or subprocess diagnostics.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        let AnimatedBackend::Native(value) = &mut self.inner else {
            let AnimatedBackend::Process(value) = &mut self.inner else {
                unreachable!()
            };
            return value.frame_at(position, is_cancelled);
        };
        cancelled(is_cancelled)?;
        if position
            .checked_cmp(value.metadata.duration.expect("finite animation"))
            .is_ge()
        {
            return Err(SourceError::new(
                "source.range",
                "requested time lies outside the half-open animation duration",
            ));
        }
        let target = value
            .times
            .partition_point(|time| time.checked_cmp(position).is_le())
            .saturating_sub(1);
        if let Some(frame) = &value.current
            && frame.index == target as u64
        {
            return Ok(frame.clone());
        }
        if target < value.next_index {
            value.frames = native_frames(Arc::clone(&value.bytes), value.format)?.2;
            value.next_index = 0;
        }
        while value.next_index <= target {
            cancelled(is_cancelled)?;
            let decoded = value
                .frames
                .next()
                .ok_or_else(|| {
                    SourceError::new(
                        "source.animation.decode",
                        "animation ended before the probed frame",
                    )
                })?
                .map_err(image_error)?;
            let index = value.next_index;
            value.next_index += 1;
            if index != target {
                continue;
            }
            let field = Arc::new(SourceField::from_straight_rgba8(
                value.metadata.width,
                value.metadata.height,
                decoded.into_buffer().into_raw(),
            )?);
            let time = value.times[index];
            let frame = SourceFrame {
                identity: FrameIdentity {
                    source_fingerprint: value.fingerprint.clone(),
                    stream_index: 0,
                    original_pts: i64::try_from(time.numerator()).map_err(|_| {
                        SourceError::new(
                            "source.animation.time",
                            "image timestamp exceeds the identity bound",
                        )
                    })?,
                    time_base: RationalTime::new(1, time.denominator())?,
                    decoder_contract: "toniator-animation-image-v1".into(),
                    color_policy: "image-format-straight-srgb".into(),
                    decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
                },
                field,
                index: index as u64,
                normalized_time: time,
            };
            cancelled(is_cancelled)?;
            value.current = Some(frame.clone());
            return Ok(frame);
        }
        Err(SourceError::new(
            "source.animation.decode",
            "animation did not reach the requested frame",
        ))
    }
}

/// Opens a bounded mature image animation decoder without a PNG handoff.
///
/// # Errors
/// Rejects unsupported containers, malformed images, non-animation PNGs, or oversized dimensions.
fn native_frames(
    bytes: Arc<[u8]>,
    format: AnimatedImageFormat,
) -> Result<(u32, u32, image::Frames<'static>), SourceError> {
    match format {
        AnimatedImageFormat::Png => {
            let mut decoder =
                image::codecs::png::PngDecoder::new(Cursor::new(bytes)).map_err(image_error)?;
            let (width, height) = decoder.dimensions();
            validate_dimensions(width, height)?;
            decoder
                .set_limits(image::Limits::default())
                .map_err(image_error)?;
            Ok((
                width,
                height,
                decoder.apng().map_err(image_error)?.into_frames(),
            ))
        }
        AnimatedImageFormat::Webp => {
            let decoder =
                image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(image_error)?;
            let (width, height) = decoder.dimensions();
            validate_dimensions(width, height)?;
            Ok((width, height, decoder.into_frames()))
        }
        _ => Err(SourceError::new(
            "source.animation.format",
            "this container requires the moving-media decoder",
        )),
    }
}

/// Preserves a mature image decoder failure at the animation boundary.
fn image_error(error: image::ImageError) -> SourceError {
    SourceError::new("source.animation.decode", error.to_string())
}

/// Adds reduced exact nonnegative times without intermediate u64 overflow.
///
/// # Errors
/// Rejects results whose reduced numerator or denominator exceeds the domain bound.
fn add_time(left: RationalTime, right: RationalTime) -> Result<RationalTime, SourceError> {
    let mut numerator = u128::from(left.numerator()) * u128::from(right.denominator())
        + u128::from(right.numerator()) * u128::from(left.denominator());
    let mut denominator = u128::from(left.denominator()) * u128::from(right.denominator());
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    numerator /= a;
    denominator /= a;
    Ok(RationalTime::new(
        u64::try_from(numerator).map_err(|_| {
            SourceError::new(
                "source.animation.time",
                "animation duration numerator overflowed",
            )
        })?,
        u64::try_from(denominator).map_err(|_| {
            SourceError::new(
                "source.animation.time",
                "animation duration denominator overflowed",
            )
        })?,
    )?)
}
