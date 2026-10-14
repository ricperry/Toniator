//! Native JPEG XL decoding into the existing straight-sRGB source authority.

use super::frame_source::{cancelled, fingerprint, validate_compressed_size, validate_frame_count};
use super::{
    FrameIdentity, FrameSource, SourceError, SourceFrame, SourceMediaKind, SourceMediaMetadata,
};
use crate::{SamplingError, SourceField, SourceFormat, validate_dimensions};
use jxl_oxide::{
    AllocTracker, EnumColourEncoding, InitializeResult, JxlImage, Moxcms, RenderingIntent,
};
use std::sync::Arc;
use toniator_domain::{FrameRate, RationalTime};

/// Bounds decoder-owned tracked grids, including animation reference/render caches.
const DECODER_ALLOCATION_LIMIT: usize = 512 * 1024 * 1024;

/// Retains native frame dependencies and one canonical decoded field within decoder grid bounds.
pub(super) struct JpegXlSource {
    image: JxlImage,
    metadata: SourceMediaMetadata,
    times: Vec<RationalTime>,
    fingerprint: String,
    current: Option<SourceFrame>,
}

/// Parses the native image header to select still versus finite animation without flattening.
///
/// # Errors
/// Rejects malformed bytes, compressed-source bounds, unsupported alpha, and unsafe dimensions.
pub(super) fn is_animated(bytes: &[u8]) -> Result<bool, SourceError> {
    let (image, _) = header(bytes, &|| false)?;
    Ok(image.image_header().metadata.animation.is_some())
}

/// Initializes the header before loading frame data, enforcing native allocation and pixel bounds.
///
/// # Errors
/// Rejects wrong signatures, truncation, cancellation, unsafe dimensions, or decoder errors.
fn header(bytes: &[u8], is_cancelled: &dyn Fn() -> bool) -> Result<(JxlImage, usize), SourceError> {
    validate_compressed_size(bytes.len())?;
    if !bytes.starts_with(&[0xff, 0x0a]) && !bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        return Err(SourceError::new(
            "source.format",
            "bytes do not match JPEG XL format hint",
        ));
    }
    let mut uninit = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(DECODER_ALLOCATION_LIMIT))
        .build_uninit();
    let mut offset = 0;
    loop {
        cancelled(is_cancelled)?;
        let end = (offset + 256).min(bytes.len());
        if end == offset {
            return Err(SourceError::new(
                "source.jpegxl.decode",
                "truncated JPEG XL header",
            ));
        }
        let consumed = uninit
            .feed_bytes(&bytes[offset..end])
            .map_err(decode_error)?;
        if consumed == 0 {
            return Err(SourceError::new(
                "source.jpegxl.decode",
                "truncated JPEG XL container header",
            ));
        }
        offset += consumed;
        match uninit.try_init().map_err(decode_error)? {
            InitializeResult::NeedMoreData(value) => uninit = value,
            InitializeResult::Initialized(mut image) => {
                validate_dimensions(image.width(), image.height())?;
                if image
                    .image_header()
                    .metadata
                    .ec_info
                    .iter()
                    .find_map(|channel| channel.alpha_associated())
                    == Some(true)
                {
                    return Err(SourceError::new(
                        "source.jpegxl.alpha",
                        "associated-alpha JPEG XL is unsupported; use straight alpha",
                    ));
                }
                image.set_cms(Moxcms);
                image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
                return Ok((image, offset));
            }
        }
    }
}

impl JpegXlSource {
    /// Loads a complete finite source, retaining exact presentation times and native frame blending.
    ///
    /// Loop flags never repeat playback. Hidden reference/zero-duration frames remain decoder-owned.
    ///
    /// # Errors
    /// Rejects malformed/truncated data, cancellation, resource limits, or invalid animation timing.
    pub(super) fn new(bytes: &[u8], is_cancelled: &dyn Fn() -> bool) -> Result<Self, SourceError> {
        let (mut image, mut offset) = header(bytes, is_cancelled)?;
        while offset < bytes.len() {
            cancelled(is_cancelled)?;
            let end = (offset + 4096).min(bytes.len());
            let consumed = image
                .feed_bytes(&bytes[offset..end])
                .map_err(decode_error)?;
            if consumed == 0 {
                return Err(SourceError::new(
                    "source.jpegxl.decode",
                    "JPEG XL decoder made no progress",
                ));
            }
            offset += consumed;
            validate_frame_count(image.num_loaded_frames().max(1))?;
        }
        image.finalize().map_err(decode_error)?;
        if !image.is_loading_done() {
            return Err(SourceError::new(
                "source.jpegxl.decode",
                "truncated JPEG XL source",
            ));
        }
        validate_frame_count(image.num_loaded_keyframes())?;
        let animation = image.image_header().metadata.animation.as_ref();
        let mut times = Vec::new();
        let mut ticks = 0u64;
        let mut first_delay = None;
        let mut variable_frame_rate = false;
        if let Some(animation) = animation {
            if animation.tps_numerator == 0 || animation.tps_denominator == 0 {
                return Err(SourceError::new(
                    "source.animation.time",
                    "JPEG XL tick rate must be positive",
                ));
            }
            for index in 0..image.num_loaded_keyframes() {
                let delay = image.frame_header(index).expect("loaded keyframe").duration;
                times.push(RationalTime::new(
                    ticks
                        .checked_mul(u64::from(animation.tps_denominator))
                        .ok_or_else(time_error)?,
                    u64::from(animation.tps_numerator),
                )?);
                if delay != 0 {
                    variable_frame_rate |= first_delay.is_some_and(|first| first != delay);
                    first_delay.get_or_insert(delay);
                }
                ticks = ticks.checked_add(u64::from(delay)).ok_or_else(time_error)?;
            }
            if ticks == 0 {
                return Err(SourceError::new(
                    "source.animation.duration",
                    "JPEG XL animation must have a positive finite duration",
                ));
            }
        } else {
            times.push(RationalTime::default());
        }
        let duration = animation
            .map(|animation| -> Result<RationalTime, SourceError> {
                Ok(RationalTime::new(
                    ticks
                        .checked_mul(u64::from(animation.tps_denominator))
                        .ok_or_else(time_error)?,
                    u64::from(animation.tps_numerator),
                )?)
            })
            .transpose()?;
        let nominal_frame_rate = animation.and_then(|animation| {
            nominal_rate(
                animation.tps_numerator,
                animation.tps_denominator,
                first_delay?,
            )
        });
        let metadata = SourceMediaMetadata {
            kind: if animation.is_some() {
                SourceMediaKind::AnimatedImage
            } else {
                SourceMediaKind::StillImage
            },
            width: image.width(),
            height: image.height(),
            frame_count: times.len() as u64,
            duration,
            nominal_frame_rate,
            variable_frame_rate,
            has_audio: false,
            stream_index: 0,
        };
        Ok(Self {
            image,
            metadata,
            times,
            fingerprint: fingerprint(bytes),
            current: None,
        })
    }
}

impl FrameSource for JpegXlSource {
    /// Returns one-pass metadata; JPEG XL loop and timecode tags do not override domain timing.
    fn metadata(&self) -> &SourceMediaMetadata {
        &self.metadata
    }

    /// Selects the latest presentation at the requested time and decodes native blended pixels.
    ///
    /// # Errors
    /// Rejects cancellation, the half-open animation end, decode errors, or unsafe rendered buffers.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        cancelled(is_cancelled)?;
        if self
            .metadata
            .duration
            .is_some_and(|duration| position.checked_cmp(duration).is_ge())
        {
            return Err(SourceError::new(
                "source.range",
                "requested time lies outside the half-open animation duration",
            ));
        }
        let index = self
            .times
            .partition_point(|time| time.checked_cmp(position).is_le())
            .saturating_sub(1);
        if let Some(frame) = &self.current
            && frame.index == index as u64
        {
            return Ok(frame.clone());
        }
        let render = self.image.render_frame(index).map_err(decode_error)?;
        let mut stream = render.stream();
        let channels = stream.channels() as usize;
        if !(1..=4).contains(&channels)
            || (stream.width(), stream.height()) != (self.metadata.width, self.metadata.height)
        {
            return Err(SourceError::new(
                "source.jpegxl.decode",
                "unsupported JPEG XL rendered layout",
            ));
        }
        let count = self.metadata.width as usize * self.metadata.height as usize;
        let mut samples = vec![0u8; count * channels];
        if stream.write_to_buffer(&mut samples) != samples.len() {
            return Err(SourceError::new(
                "source.jpegxl.decode",
                "incomplete JPEG XL rendered pixels",
            ));
        }
        let mut rgba = Vec::with_capacity(count * 4);
        for pixel in samples.chunks_exact(channels) {
            match pixel {
                [gray] => rgba.extend([*gray, *gray, *gray, 255]),
                [gray, alpha] => rgba.extend([*gray, *gray, *gray, *alpha]),
                [r, g, b] => rgba.extend([*r, *g, *b, 255]),
                [r, g, b, a] => rgba.extend([*r, *g, *b, *a]),
                _ => unreachable!(),
            }
        }
        let mut field =
            SourceField::from_straight_rgba8(self.metadata.width, self.metadata.height, rgba)?;
        field.identity.format = SourceFormat::JpegXl;
        field.identity.content_hash = self.fingerprint.clone();
        let time = self.times[index];
        let frame = SourceFrame {
            identity: FrameIdentity {
                source_fingerprint: self.fingerprint.clone(),
                stream_index: 0,
                original_pts: i64::try_from(time.numerator()).map_err(|_| time_error())?,
                time_base: RationalTime::new(1, time.denominator())?,
                decoder_contract: "toniator-jxl-oxide-0.12.6-v1".into(),
                color_policy: "jpegxl-to-straight-srgb8".into(),
                decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
            },
            field: Arc::new(field),
            index: index as u64,
            normalized_time: time,
        };
        cancelled(is_cancelled)?;
        self.current = Some(frame.clone());
        Ok(frame)
    }
}

/// Decodes one JPEG XL still while rejecting animation at the still-only sampling boundary.
///
/// # Errors
/// Rejects animated, malformed, oversized, or unsupported source content without flattening.
pub(crate) fn decode_still(bytes: &[u8]) -> Result<SourceField, SamplingError> {
    let decode = || -> Result<SourceField, SourceError> {
        let mut source = JpegXlSource::new(bytes, &|| false)?;
        if source.metadata.kind != SourceMediaKind::StillImage {
            return Err(SourceError::new(
                "source.sequence",
                "animated JPEG XL requires a frame provider",
            ));
        }
        Ok((*source.frame_at(RationalTime::default(), &|| false)?.field).clone())
    };
    decode().map_err(|error| {
        SamplingError::new(
            error.path(),
            match error.path() {
                "source.jpegxl.alpha" => {
                    "associated-alpha JPEG XL is unsupported; use straight alpha"
                }
                "source.sequence" => "animated JPEG XL requires a frame provider",
                _ => "JPEG XL source decode failed",
            },
        )
    })
}

/// Preserves decoder diagnostics at the JPEG XL boundary.
fn decode_error(error: impl std::fmt::Display) -> SourceError {
    SourceError::new("source.jpegxl.decode", error.to_string())
}

/// Reports an exact-time arithmetic overflow without approximating source timestamps.
fn time_error() -> SourceError {
    SourceError::new(
        "source.animation.time",
        "JPEG XL duration exceeds the exact-time bound",
    )
}

/// Reduces the native tick ratio before projecting it into the domain's bounded frame rate.
/// Returns no rate only for zero/unsupported domain values; intermediate multiplication uses u64.
fn nominal_rate(numerator: u32, denominator: u32, delay: u32) -> Option<FrameRate> {
    let time = RationalTime::new(
        u64::from(denominator) * u64::from(delay),
        u64::from(numerator),
    )
    .ok()?;
    FrameRate::new(
        u32::try_from(time.denominator()).ok()?,
        u32::try_from(time.numerator()).ok()?,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    /// Checks exact reduction before the domain's u32 frame-rate boundary.
    ///
    /// # Panics
    /// Panics if a representable reduced native frame rate is lost through intermediate overflow.
    #[test]
    fn jpegxl_nominal_rate_reduces_before_narrowing() {
        assert_eq!(
            super::nominal_rate(1_000_000_000, 1_000_000_000, 10),
            Some(toniator_domain::FrameRate::new(1, 10).unwrap())
        );
    }
}
