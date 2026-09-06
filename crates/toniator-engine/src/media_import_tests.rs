use super::*;
use toniator_sampling::media::SourceMediaKind;

/// Owns only a uniquely allocated animation-import fixture directory.
struct Scratch(PathBuf);
impl Scratch {
    /// Creates an exclusive temporary fixture directory.
    ///
    /// # Panics
    /// Panics if the clock or local writable temporary filesystem is unavailable.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "toniator-media-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    /// Removes only this test's owned generated files.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Detects animated containers from contents, persists their kind, and evaluates both source states.
///
/// # Panics
/// Panics if local software encoders, shared import, container detection, or finite decoding fail.
#[test]
fn local_import_detects_png_webp_gif_and_avif_animation() {
    let scratch = Scratch::new();
    for extension in ["png", "webp", "gif", "avif"] {
        let path = scratch.0.join(format!("animation.{extension}"));
        let mut command = std::process::Command::new("ffmpeg");
        command.args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=32x32:rate=2",
            "-frames:v",
            "2",
        ]);
        match extension {
            "png" => {
                command.args(["-f", "apng", "-plays", "0"]);
            }
            "webp" => {
                command.args(["-c:v", "libwebp_anim", "-lossless", "1", "-loop", "0"]);
            }
            "avif" => {
                command.args([
                    "-c:v",
                    "libaom-av1",
                    "-cpu-used",
                    "8",
                    "-crf",
                    "0",
                    "-pix_fmt",
                    "yuv444p",
                    "-colorspace",
                    "bt709",
                    "-color_primaries",
                    "bt709",
                    "-color_trc",
                    "bt709",
                    "-color_range",
                    "tv",
                    "-aom-params",
                    "color-primaries=1:transfer-characteristics=1:matrix-coefficients=1",
                ]);
            }
            _ => {}
        }
        let output = command.arg(&path).output().unwrap();
        assert!(
            output.status.success(),
            "{extension}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let imported = import_source_media(&[path.clone()], None, MediaTools::default(), &|| false)
            .unwrap_or_else(|error| panic!("{extension}: {error}"));
        assert_eq!(
            imported.metadata.kind,
            SourceMediaKind::AnimatedImage,
            "{extension}"
        );
        assert_eq!(imported.metadata.frame_count, 2, "{extension}");
        let mut source =
            open_source_media(&imported.sources, MediaTools::default(), &|| false).unwrap();
        let first = source.frame_at(RationalTime::default(), &|| false).unwrap();
        let last = source
            .frame_at(RationalTime::new(1, 2).unwrap(), &|| false)
            .unwrap();
        assert_eq!((first.index, last.index), (0, 1));
        assert_ne!(
            first.field.identity().decoded_pixel_hash,
            last.field.identity().decoded_pixel_hash
        );
        assert!(
            source
                .frame_at(imported.metadata.duration.unwrap(), &|| false)
                .is_err()
        );
        assert!(
            import_source_media(
                &[path],
                Some(FrameRate::new(2, 1).unwrap()),
                MediaTools::default(),
                &|| false
            )
            .is_err()
        );
    }
}

/// Verifies exact trimming/rate semantics independently of any CLI syntax or desktop widget.
///
/// # Panics
/// Panics if frame counts, source timestamps, preserved ranges, or invalid-range diagnostics drift.
#[test]
fn timing_selection_preserves_source_speed_and_exact_bounds() {
    let metadata = SourceMediaMetadata {
        kind: SourceMediaKind::Video,
        width: 1080,
        height: 1920,
        frame_count: 10,
        duration: Some(RationalTime::new(5, 3).unwrap()),
        nominal_frame_rate: Some(FrameRate::new(6, 1).unwrap()),
        variable_frame_rate: false,
        has_audio: false,
        stream_index: 0,
    };
    let default = MediaTimingSelection::default();
    let full = select_media_timing(&metadata, None, &default).unwrap();
    assert_eq!(full.frame_rate(), FrameRate::new(6, 1).unwrap());
    assert_eq!(full.frame_range().frame_count(), 10);
    let inconsistent = ProjectTiming::new(full.frame_rate(), FrameRange::default())
        .with_source_time_range(full.source_time_range().unwrap());
    assert!(select_media_timing(&metadata, Some(&inconsistent), &default).is_err());
    assert_eq!(
        full.source_time_for_frame(9).unwrap(),
        RationalTime::new(3, 2).unwrap()
    );
    let trimmed = select_media_timing(
        &metadata,
        Some(&full),
        &MediaTimingSelection {
            start_frame: Some(2),
            end_frame: Some(4),
            ..default.clone()
        },
    )
    .unwrap();
    assert_eq!(trimmed.frame_range(), FrameRange::new(2, 5).unwrap());
    assert_eq!(
        trimmed.source_time_for_frame(2).unwrap(),
        RationalTime::new(1, 3).unwrap()
    );
    assert_eq!(trimmed.frame_range().progress(2).unwrap(), 0.0);
    assert_eq!(trimmed.frame_range().progress(4).unwrap(), 1.0);
    let one = select_media_timing(
        &metadata,
        None,
        &MediaTimingSelection {
            start_frame: Some(4),
            end_frame: Some(4),
            ..default.clone()
        },
    )
    .unwrap();
    assert_eq!(one.frame_range().progress(4).unwrap(), 0.0);
    assert_eq!(
        select_media_timing(&metadata, Some(&trimmed), &default).unwrap(),
        trimmed
    );
    let rate = select_media_timing(
        &metadata,
        Some(&trimmed),
        &MediaTimingSelection {
            frame_rate: Some(FrameRate::new(30000, 1001).unwrap()),
            ..default.clone()
        },
    )
    .unwrap();
    assert_eq!(rate.frame_range().frame_count(), 15);
    assert_eq!(
        rate.source_time_for_frame(0).unwrap(),
        RationalTime::new(1, 3).unwrap()
    );
    assert_eq!(
        rate.source_time_range().unwrap().end(),
        RationalTime::new(5, 6).unwrap()
    );
    assert!(
        rate.source_time_for_frame(14)
            .unwrap()
            .checked_cmp(RationalTime::new(5, 6).unwrap())
            .is_lt()
    );
    let time = select_media_timing(
        &metadata,
        None,
        &MediaTimingSelection {
            start_time: Some(RationalTime::new(1, 10).unwrap()),
            end_time: Some(RationalTime::new(3, 5).unwrap()),
            ..default.clone()
        },
    )
    .unwrap();
    assert_eq!(time.frame_range().frame_count(), 3);
    assert_eq!(
        time.source_time_for_frame(2).unwrap(),
        RationalTime::new(13, 30).unwrap()
    );
    for selection in [
        MediaTimingSelection {
            start_frame: Some(2),
            end_time: metadata.duration,
            ..default.clone()
        },
        MediaTimingSelection {
            end_frame: Some(10),
            ..default.clone()
        },
        MediaTimingSelection {
            end_frame: Some(u64::MAX),
            ..default.clone()
        },
        MediaTimingSelection {
            start_time: metadata.duration,
            ..default.clone()
        },
        MediaTimingSelection {
            end_time: Some(RationalTime::new(2, 1).unwrap()),
            ..default.clone()
        },
    ] {
        assert!(select_media_timing(&metadata, None, &selection).is_err());
    }
    let still = SourceMediaMetadata {
        kind: SourceMediaKind::StillImage,
        duration: None,
        nominal_frame_rate: None,
        frame_count: 1,
        ..metadata
    };
    let animation = select_media_timing(&still, None, &default).unwrap();
    assert_eq!(animation.frame_range().frame_count(), 150);
    assert_eq!(animation.frame_rate(), FrameRate::default());
    assert!(
        select_media_timing(
            &still,
            None,
            &MediaTimingSelection {
                end_frame: Some(1_000_000),
                ..default
            }
        )
        .is_err()
    );
}

/// Imports both immutable stills and the video, then proves sequence ordering and deduplication.
///
/// # Panics
/// Panics when current local fixtures, bounded providers, or configured FFmpeg tools fail.
#[test]
fn local_import_uses_shared_formats_and_explicit_sequence_order() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let raster = assets.join("raster-sample.png");
    let vector = assets.join("vector-sample.svg");
    for (path, kind, dimensions) in [
        (raster.clone(), SourceMediaKind::StillImage, (1024, 1024)),
        (vector.clone(), SourceMediaKind::StillImage, (900, 620)),
        (
            assets.join("video-sample0001-0010.mp4"),
            SourceMediaKind::Video,
            (1080, 1920),
        ),
    ] {
        let imported =
            import_source_media(&[path], None, MediaTools::default(), &|| false).unwrap();
        assert_eq!(imported.metadata.kind, kind);
        assert_eq!(
            (imported.metadata.width, imported.metadata.height),
            dimensions
        );
    }
    let imported = import_source_media(
        &[raster.clone(), raster.clone(), raster.clone()],
        Some(FrameRate::new(3, 1).unwrap()),
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(imported.sources.len(), 1);
    assert_eq!(imported.metadata.frame_count, 3);
    let SourceMediaManifest::ImageSequence { source_ids, .. } = imported.sources.media().unwrap()
    else {
        panic!("sequence manifest")
    };
    assert_eq!(source_ids, &vec![imported.source_id; 3]);
    assert!(
        import_source_media(
            &[raster.clone(), vector],
            None,
            MediaTools::default(),
            &|| false
        )
        .is_err()
    );
    assert!(import_source_media(&[raster], None, MediaTools::default(), &|| true).is_err());
    assert!(import_source_media(&[assets], None, MediaTools::default(), &|| false).is_err());
}
