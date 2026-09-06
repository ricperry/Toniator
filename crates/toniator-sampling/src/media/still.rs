use std::sync::Arc;

use toniator_domain::RationalTime;

use super::frame_source::{
    FrameIdentity, FrameSource, SourceError, SourceFrame, SourceMediaKind, SourceMediaMetadata,
    cancelled,
};
use crate::{DECODER_CONTRACT_ID, SourceFormatHint, decode_source};

/// A decoded still image that repeats for every nonnegative requested source time.
#[derive(Clone, Debug)]
pub struct StillImageSource {
    frame: SourceFrame,
    metadata: SourceMediaMetadata,
}

impl StillImageSource {
    /// Decodes one still source through the existing canonical source decoder.
    ///
    /// The compressed source is bounded by the shared 128 MiB media limit and the decoded field
    /// retains the existing 64-megapixel limit.
    ///
    /// # Errors
    ///
    /// Returns a stable source diagnostic when the bytes exceed the bound or still decoding fails.
    pub fn new(bytes: impl Into<Arc<[u8]>>, hint: SourceFormatHint) -> Result<Self, SourceError> {
        let bytes = bytes.into();
        super::frame_source::validate_compressed_size(bytes.len())?;
        let field = Arc::new(decode_source(&bytes, hint).map_err(SourceError::from)?);
        let identity = FrameIdentity {
            source_fingerprint: super::frame_source::fingerprint(&bytes),
            stream_index: 0,
            original_pts: 0,
            time_base: RationalTime::new(1, 1).expect("one-second time base is valid"),
            decoder_contract: DECODER_CONTRACT_ID.to_owned(),
            color_policy: "still-decoder-straight-srgb".to_owned(),
            decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
        };
        let metadata = SourceMediaMetadata {
            kind: SourceMediaKind::StillImage,
            width: field.identity().width,
            height: field.identity().height,
            frame_count: 1,
            duration: None,
            nominal_frame_rate: None,
            variable_frame_rate: false,
            has_audio: false,
            stream_index: 0,
        };
        Ok(Self {
            frame: SourceFrame {
                field,
                identity,
                index: 0,
                normalized_time: RationalTime::default(),
            },
            metadata,
        })
    }
}

impl FrameSource for StillImageSource {
    /// Returns immutable still metadata with no finite duration.
    fn metadata(&self) -> &SourceMediaMetadata {
        &self.metadata
    }

    /// Repeats the same immutable decoded field and identity for every requested source time.
    ///
    /// # Errors
    ///
    /// Returns `source.cancelled` when the cooperative cancellation callback is set.
    fn frame_at(
        &mut self,
        _position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        cancelled(is_cancelled)?;
        Ok(self.frame.clone())
    }
}
