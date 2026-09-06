use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

use toniator_domain::{FrameRate, RationalTime};

use super::frame_source::{
    FrameIdentity, FrameSource, SourceError, SourceFrame, SourceMediaKind, SourceMediaMetadata,
    cancelled, validate_compressed_size,
};
use crate::{DECODER_CONTRACT_ID, SourceField, SourceFormatHint, decode_source};

/// One explicit ordered compressed still image in an image sequence.
#[derive(Clone, Debug)]
pub struct ImageSequenceEntry {
    pub bytes: Arc<[u8]>,
    pub format: SourceFormatHint,
}

/// An explicitly ordered image sequence with an assigned constant frame rate and one-frame cache.
#[derive(Clone, Debug)]
pub struct ImageSequenceSource {
    entries: Vec<ImageSequenceEntry>,
    fingerprint: String,
    frame_rate: FrameRate,
    metadata: SourceMediaMetadata,
    cached: Option<(usize, Arc<SourceField>)>,
}

impl ImageSequenceSource {
    /// Validates an ordered sequence without decoding or preloading every image.
    ///
    /// Unique shared compressed allocations are capped at 128 MiB, frame count is bounded, and
    /// the first frame establishes dimensions that every lazily decoded entry must match.
    /// Repeated references count once toward resident bytes and hash once per allocation.
    ///
    /// # Errors
    ///
    /// Returns a source diagnostic for an empty/oversized sequence, invalid first image, unsafe
    /// exact duration, or a frame-count overflow.
    pub fn new(
        entries: Vec<ImageSequenceEntry>,
        frame_rate: FrameRate,
    ) -> Result<Self, SourceError> {
        if entries.is_empty() {
            return Err(SourceError::new(
                "source.sequence",
                "image sequence must not be empty",
            ));
        }
        super::frame_source::validate_frame_count(entries.len())?;
        let mut unique_hashes = HashMap::new();
        let mut total = 0usize;
        for entry in &entries {
            let key = (entry.bytes.as_ptr() as usize, entry.bytes.len());
            if let std::collections::hash_map::Entry::Vacant(slot) = unique_hashes.entry(key) {
                validate_compressed_size(entry.bytes.len())?;
                total = total.checked_add(entry.bytes.len()).ok_or_else(|| {
                    SourceError::new("source.size", "aggregate compressed source size overflowed")
                })?;
                validate_compressed_size(total)?;
                slot.insert(Sha256::digest(&entry.bytes));
            }
        }
        let first = Arc::new(decode_source(&entries[0].bytes, entries[0].format)?);
        let duration = RationalTime::new(
            u64::try_from(entries.len()).map_err(|_| {
                SourceError::new("source.sequence", "image sequence frame count is unsafe")
            })? * u64::from(frame_rate.denominator()),
            u64::from(frame_rate.numerator()),
        )?;
        let mut identity = Sha256::new();
        identity.update(b"toniator-image-sequence-v2");
        identity.update(frame_rate.numerator().to_le_bytes());
        identity.update(frame_rate.denominator().to_le_bytes());
        for entry in &entries {
            identity.update([entry.format as u8]);
            identity.update((entry.bytes.len() as u64).to_le_bytes());
            identity.update(unique_hashes[&(entry.bytes.as_ptr() as usize, entry.bytes.len())]);
        }
        let fingerprint = format!("sha256:{:x}", identity.finalize());
        let metadata = SourceMediaMetadata {
            kind: SourceMediaKind::ImageSequence,
            width: first.identity().width,
            height: first.identity().height,
            frame_count: entries.len() as u64,
            duration: Some(duration),
            nominal_frame_rate: Some(frame_rate),
            variable_frame_rate: false,
            has_audio: false,
            stream_index: 0,
        };
        Ok(Self {
            entries,
            fingerprint,
            frame_rate,
            metadata,
            cached: Some((0, first)),
        })
    }

    /// Converts a half-open sequence position to its latest assigned frame without floating point.
    ///
    /// # Errors
    ///
    /// Returns `source.range` at or beyond the finite sequence duration.
    fn index_at(&self, position: RationalTime) -> Result<usize, SourceError> {
        let duration = self.metadata.duration.expect("sequence duration exists");
        if position.checked_cmp(duration).is_ge() {
            return Err(SourceError::new(
                "source.range",
                "requested time lies outside the half-open source duration",
            ));
        }
        let numerator = u128::from(position.numerator()) * u128::from(self.frame_rate.numerator());
        let denominator =
            u128::from(position.denominator()) * u128::from(self.frame_rate.denominator());
        usize::try_from(numerator / denominator)
            .map_err(|_| SourceError::new("source.range", "requested frame index is unsafe"))
    }
}

impl FrameSource for ImageSequenceSource {
    /// Returns finite assigned-rate sequence metadata.
    fn metadata(&self) -> &SourceMediaMetadata {
        &self.metadata
    }

    /// Lazily decodes the latest assigned frame at or before the requested exact source time.
    ///
    /// Only the last decoded field is cached. Duplicate ordered entries remain distinct frames.
    ///
    /// # Errors
    ///
    /// Returns cancellation, half-open range, decode, dimension mismatch, or exact-time errors.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        cancelled(is_cancelled)?;
        let index = self.index_at(position)?;
        let field = if let Some((cached_index, field)) = &self.cached
            && *cached_index == index
        {
            Arc::clone(field)
        } else {
            let field = Arc::new(decode_source(
                &self.entries[index].bytes,
                self.entries[index].format,
            )?);
            if field.identity().width != self.metadata.width
                || field.identity().height != self.metadata.height
            {
                return Err(SourceError::new(
                    "source.sequence.dimensions",
                    "every image sequence frame must have identical dimensions",
                ));
            }
            self.cached = Some((index, Arc::clone(&field)));
            field
        };
        cancelled(is_cancelled)?;
        let normalized_time = self.frame_rate.time_for_frame(index as u64)?;
        let time_base = RationalTime::new(
            u64::from(self.frame_rate.denominator()),
            u64::from(self.frame_rate.numerator()),
        )?;
        Ok(SourceFrame {
            identity: FrameIdentity {
                source_fingerprint: self.fingerprint.clone(),
                stream_index: 0,
                original_pts: index as i64,
                time_base,
                decoder_contract: DECODER_CONTRACT_ID.to_owned(),
                color_policy: "still-decoder-straight-srgb".to_owned(),
                decoded_pixel_hash: field.identity().decoded_pixel_hash.clone(),
            },
            field,
            index: index as u64,
            normalized_time,
        })
    }
}
