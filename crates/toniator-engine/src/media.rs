//! Shared persisted-media opening for frontend workers and export jobs.

use crate::{EvaluationRequest, ResolvedSource};
use toniator_domain::{DocumentSession, SourceReference};
use toniator_sampling::media::FrameSource;

/// Resolves one authored frame into the shared evaluator while preserving the session revision.
///
/// Domain timing and endpoint materialization own all time/interpolation decisions. The provider
/// supplies decoded pixels directly; this operation performs no rendering or cache publication.
///
/// # Errors
/// Rejects invalid frame settings, missing source authority, source timing/decode failures, and
/// cancellation before returning a complete immutable request.
pub fn frame_evaluation_request(
    session: &DocumentSession,
    media: &mut dyn FrameSource,
    frame: u64,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<EvaluationRequest, SourceError> {
    if is_cancelled() {
        return Err(SourceError::new(
            "source.cancelled",
            "frame request was cancelled",
        ));
    }
    let snapshot = session
        .document_evaluation_snapshot_at_frame(frame)
        .map_err(|error| SourceError::new(error.path(), error.to_string()))?;
    let SourceReference::Assigned(id) = snapshot.document().source() else {
        return Err(SourceError::new(
            "source.document",
            "frame request requires an assigned source",
        ));
    };
    let time = session
        .document()
        .project_timing()
        .source_time_for_frame(frame)
        .map_err(|error| SourceError::new(error.path(), error.to_string()))?;
    let source = ResolvedSource::from_frame(id.clone(), media.frame_at(time, is_cancelled)?)
        .map_err(|error| SourceError::new(error.path(), error.to_string()))?;
    Ok(EvaluationRequest::new(snapshot, source))
}

use toniator_io::{EmbeddedSourceFormat, SourceBundle, SourceMediaManifest};
use toniator_sampling::{
    SourceFormatHint,
    media::{
        AnimatedImageFormat, AnimatedImageSource, ImageSequenceEntry, ImageSequenceSource,
        MediaTools, SourceError, SourceMedia, StillImageSource, VideoSource,
    },
};

/// Opens a validated portable source bundle through the canonical sampling providers.
///
/// Construct and retain the provider inside its owning worker: native animation iterators are
/// not transferable between threads. Encoded bytes are shared, sequence repetitions retain their
/// authored order, and no document settings or frontend-specific frame policy is introduced.
///
/// # Errors
/// Returns source diagnostics for absent media, cancellation, unsupported format, malformed
/// encoded data, resource bounds, or unavailable/failed media tools before returning a provider.
pub fn open_source_media(
    sources: &SourceBundle,
    tools: MediaTools,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<SourceMedia, SourceError> {
    if is_cancelled() {
        return Err(SourceError::new(
            "source.cancelled",
            "media opening was cancelled",
        ));
    }
    let media = sources
        .media()
        .ok_or_else(|| SourceError::new("source.media", "source media manifest is missing"))?;
    let entry = |id| {
        sources
            .get(id)
            .ok_or_else(|| SourceError::new("source.media", "media source entry is missing"))
    };
    Ok(match media {
        SourceMediaManifest::StillImage { source_id } => {
            let source = entry(source_id)?;
            SourceMedia::StillImage(StillImageSource::new(
                source.shared_bytes(),
                still_hint(source.format())?,
            )?)
        }
        SourceMediaManifest::AnimatedImage { source_id } => {
            let source = entry(source_id)?;
            let format = match source.format() {
                EmbeddedSourceFormat::Gif => AnimatedImageFormat::Gif,
                EmbeddedSourceFormat::Png => AnimatedImageFormat::Png,
                EmbeddedSourceFormat::Webp => AnimatedImageFormat::Webp,
                EmbeddedSourceFormat::Avif => AnimatedImageFormat::Avif,
                _ => {
                    return Err(SourceError::new(
                        "source.media.format",
                        "unsupported animated-image format",
                    ));
                }
            };
            SourceMedia::AnimatedImage(AnimatedImageSource::new(
                source.shared_bytes(),
                format,
                tools,
                is_cancelled,
            )?)
        }
        SourceMediaManifest::Video { source_id } => SourceMedia::Video(Box::new(VideoSource::new(
            entry(source_id)?.shared_bytes(),
            tools,
            is_cancelled,
        )?)),
        SourceMediaManifest::ImageSequence {
            source_ids,
            frame_rate,
        } => {
            let mut entries = Vec::with_capacity(source_ids.len());
            for id in source_ids {
                if is_cancelled() {
                    return Err(SourceError::new(
                        "source.cancelled",
                        "media opening was cancelled",
                    ));
                }
                let source = entry(id)?;
                entries.push(ImageSequenceEntry {
                    bytes: source.shared_bytes(),
                    format: still_hint(source.format())?,
                });
            }
            SourceMedia::ImageSequence(ImageSequenceSource::new(entries, *frame_rate)?)
        }
    })
}

/// Maps persisted still formats onto decoder authority without treating moving media as stills.
///
/// # Errors
/// Rejects video/GIF containers, which require their explicit moving-media provider.
pub(crate) fn still_hint(format: EmbeddedSourceFormat) -> Result<SourceFormatHint, SourceError> {
    Ok(match format {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
        EmbeddedSourceFormat::Jpeg => SourceFormatHint::Jpeg,
        EmbeddedSourceFormat::Webp => SourceFormatHint::Webp,
        EmbeddedSourceFormat::Bmp => SourceFormatHint::Bmp,
        EmbeddedSourceFormat::Tiff => SourceFormatHint::Tiff,
        EmbeddedSourceFormat::OpenExr => SourceFormatHint::OpenExr,
        EmbeddedSourceFormat::Avif => SourceFormatHint::Avif,
        EmbeddedSourceFormat::Gif | EmbeddedSourceFormat::Video => {
            return Err(SourceError::new(
                "source.media.format",
                "moving media requires an explicit frame provider",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::{FrameRate, RationalTime, SourceReferenceId};
    use toniator_io::EmbeddedSource;
    use toniator_sampling::media::{FrameSource, SourceMediaKind};

    /// Checks explicit sequence order, repeated-frame identity, and both immutable still decoders.
    ///
    /// # Panics
    /// Panics when a current source fixture fails provider validation or produces wrong metadata.
    #[test]
    fn bundle_opens_stills_and_ordered_sequence() {
        let mut raster = None;
        for (format, bytes, width, height) in [
            (
                EmbeddedSourceFormat::Png,
                include_bytes!("../../../assets/raster-sample.png").as_slice(),
                1024,
                1024,
            ),
            (
                EmbeddedSourceFormat::Svg,
                include_bytes!("../../../assets/vector-sample.svg").as_slice(),
                900,
                620,
            ),
        ] {
            let id = SourceReferenceId::new("still").unwrap();
            let source = EmbeddedSource::new(id, format, bytes, None).unwrap();
            let bundle = SourceBundle::new([source.clone()]).unwrap();
            let mut media = open_source_media(&bundle, MediaTools::default(), &|| false).unwrap();
            assert_eq!(media.metadata().kind, SourceMediaKind::StillImage);
            assert_eq!(
                (media.metadata().width, media.metadata().height),
                (width, height)
            );
            let first = media.frame_at(RationalTime::default(), &|| false).unwrap();
            let later = media
                .frame_at(RationalTime::new(60, 1).unwrap(), &|| false)
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&first.field, &later.field));
            if format == EmbeddedSourceFormat::Png {
                raster = Some(source);
            }
        }
        let source = raster.unwrap();
        let sequence = SourceBundle::new_media(
            [source.clone()],
            SourceMediaManifest::ImageSequence {
                source_ids: vec![source.id().clone(); 3],
                frame_rate: FrameRate::new(3, 1).unwrap(),
            },
        )
        .unwrap();
        let mut media = open_source_media(&sequence, MediaTools::default(), &|| false).unwrap();
        assert_eq!(media.metadata().frame_count, 3);
        assert_eq!(
            media
                .frame_at(RationalTime::new(2, 3).unwrap(), &|| false)
                .unwrap()
                .index,
            2
        );
        assert!(
            media
                .frame_at(RationalTime::new(1, 1).unwrap(), &|| false)
                .is_err()
        );
        assert!(open_source_media(&sequence, MediaTools::default(), &|| true).is_err());
        assert!(
            open_source_media(
                &SourceBundle::new([]).unwrap(),
                MediaTools::default(),
                &|| false
            )
            .is_err()
        );
    }

    /// Opens the immutable video bundle with shared bytes and selects its last finite source frame.
    ///
    /// # Panics
    /// Panics if configured local FFmpeg tools cannot decode the current ten-frame test video.
    #[test]
    fn bundle_opens_video_without_frontend_format_projection() {
        let bundle = SourceBundle::new([EmbeddedSource::new(
            SourceReferenceId::new("video").unwrap(),
            EmbeddedSourceFormat::Video,
            include_bytes!("../../../assets/video-sample0001-0010.mp4").as_slice(),
            None,
        )
        .unwrap()])
        .unwrap();
        let mut media = open_source_media(&bundle, MediaTools::default(), &|| false).unwrap();
        assert_eq!(media.metadata().frame_count, 10);
        assert_eq!(media.metadata().kind, SourceMediaKind::Video);
        let end = media
            .frame_at(RationalTime::new(9, 6).unwrap(), &|| false)
            .unwrap();
        assert_eq!(end.index, 9);
        assert_eq!(
            (media.metadata().width, media.metadata().height),
            (1080, 1920)
        );
    }
}
