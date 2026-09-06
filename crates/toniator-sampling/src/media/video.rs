use std::sync::Arc;

use toniator_domain::RationalTime;

use super::frame_source::{
    DecodeState, FrameSource, MediaTools, MovingProbe, OwnedMediaFile, SourceError, SourceFrame,
    SourceMediaKind, SourceMediaMetadata, cancelled, decode_selected, fingerprint, probe_moving,
    select_frame, validate_compressed_size,
};

/// Owns bounded compressed input, presentation timing, and one sequential software decoder.
#[derive(Debug)]
pub struct VideoSource {
    // Drop the process and its readers before removing the private input file.
    state: DecodeState,
    file: OwnedMediaFile,
    tools: MediaTools,
    probe: MovingProbe,
    fingerprint: String,
}

impl VideoSource {
    /// Opens the first video stream from immutable local bytes and probes exact presentation times.
    ///
    /// # Errors
    /// Rejects oversized input, unsupported color/orientation, invalid timing, cancelled work,
    /// unavailable tools, or a failed bounded subprocess. Audio is reported but never decoded.
    pub fn new(
        bytes: impl Into<Arc<[u8]>>,
        tools: MediaTools,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, SourceError> {
        Self::open(bytes.into(), tools, SourceMediaKind::Video, is_cancelled)
    }

    /// Opens one encoded moving source using the same bounded provider for video and GIF.
    ///
    /// # Errors
    /// Returns compressed-size, cancellation, private-storage, probe, or color diagnostics.
    pub(super) fn open(
        bytes: Arc<[u8]>,
        tools: MediaTools,
        kind: SourceMediaKind,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, SourceError> {
        cancelled(is_cancelled)?;
        validate_compressed_size(bytes.len())?;
        let file = OwnedMediaFile::create(&bytes, "media")?;
        let probe = probe_moving(&tools, &file, kind, is_cancelled)?;
        Ok(Self {
            state: DecodeState::default(),
            file,
            tools,
            probe,
            fingerprint: fingerprint(&bytes),
        })
    }
}

impl FrameSource for VideoSource {
    /// Returns metadata measured from the immutable source, without decoding audio.
    fn metadata(&self) -> &SourceMediaMetadata {
        &self.probe.metadata
    }

    /// Selects by exact PTS, advances sequentially, and restarts decoding for a backward request.
    ///
    /// # Errors
    /// Returns range, cancellation, or decode diagnostics; failures stop and reap the decoder.
    fn frame_at(
        &mut self,
        position: RationalTime,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceFrame, SourceError> {
        let target = select_frame(&self.probe, position)?;
        let result = decode_selected(
            &mut self.state,
            &self.tools,
            &self.file,
            &self.probe,
            &self.fingerprint,
            target,
            is_cancelled,
        );
        if result.is_err() {
            self.state.session = None;
        }
        result
    }
}
