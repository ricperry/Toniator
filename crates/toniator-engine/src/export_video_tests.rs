use super::*;
use crate::export::tests::fixture;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_io::EmbeddedSourceFormat;

/// Retains only native review artifacts; ordinary generated recovery fixtures remain disposable.
struct Scratch {
    path: PathBuf,
    keep: bool,
}
impl Scratch {
    /// Creates a unique evidence or temporary root without overwriting earlier outputs.
    ///
    /// # Panics
    /// Panics when the clock or test filesystem is unavailable.
    fn new(keep: bool) -> Self {
        let base = if keep {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/validation/stage22-video-export")
        } else {
            std::env::temp_dir()
        };
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self { path, keep }
    }
}
impl Drop for Scratch {
    /// Removes only this test's non-retained root after all owned processes have stopped.
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Supplies fixed software codec choices and explicit transparent/opaque output policy.
fn options(path: PathBuf, codec: VideoCodec) -> VideoExportOptions {
    VideoExportOptions {
        destination: path,
        codec,
        temporary_directory: None,
        background: Some(if codec == VideoCodec::Ffv1Matroska {
            RasterBackground::Transparent
        } else {
            RasterBackground::OpaqueBlack
        }),
        target: None,
        antialiasing: RasterAntialiasing::On,
        limits: EvaluationLimits::default(),
    }
}

/// Proves a failed retry preserves hidden RGB/alpha and exact fractional-rate intent for success.
///
/// # Panics
/// Panics if capability validation, retries, exact RGBA encoding or cleanup fails.
#[test]
fn fractional_rate_retry_preserves_hidden_rgba() {
    use image::ImageEncoder;
    use toniator_domain::FrameRange;
    let scratch = Scratch::new(false);
    let rate = FrameRate::new(30_000, 1_001).unwrap();
    let output = VideoOutput::reserve(&scratch.path.join("probe.mkv"), 262_144).unwrap();
    process::capability_probe(
        &MediaTools::default(),
        VideoCodec::Ffv1Matroska,
        VideoStreamSpec {
            width: 64,
            height: 64,
            frame_count: 1,
            frame_rate: rate,
        },
        &output,
        &AtomicBool::new(false),
    )
    .unwrap();
    drop(output);
    let colors = [
        [211, 73, 19, 0],
        [19, 57, 229, 31],
        [144, 80, 191, 127],
        [8, 196, 91, 255],
    ];
    let rgba = (0..64 * 64)
        .flat_map(|index| colors[index % 4])
        .collect::<Vec<u8>>();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, 64, 64, image::ColorType::Rgba8.into())
        .unwrap();
    let timing = ProjectTiming::new(rate, FrameRange::new(0, 2).unwrap());
    let workspace = RenderWorkspace::create(Path::new("/tmp")).unwrap();
    let mut writer = SequenceWriter::create(
        &workspace.frames_path(),
        SequenceManifest::new(SequenceFormat::Png, 64, 64, &timing),
        262_144,
    )
    .unwrap();
    writer.write_frame(&png).unwrap();
    writer.write_frame(&png).unwrap();
    writer.finish().unwrap();
    let path = scratch.path.join("fractional.mkv");
    let mut recovery = VideoRecovery {
        workspace: Some(workspace),
        options: options(path.clone(), VideoCodec::Ffv1Matroska),
        timing,
        width: 64,
        height: 64,
        frame_count: 2,
        estimated_bytes: 262_144,
    };
    assert!(
        recovery
            .retry(
                MediaTools {
                    ffmpeg: "/usr/bin/false".into(),
                    ..MediaTools::default()
                },
                &path,
                &AtomicBool::new(false),
                &|_| {}
            )
            .is_err()
    );
    recovery
        .retry(
            MediaTools::default(),
            &path,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
        .output()
        .unwrap();
    assert!(decoded.status.success());
    assert_eq!(decoded.stdout, [rgba.clone(), rgba].concat());
    assert!(recovery.frames_path().is_none());
}

/// Renders the supplied native video and verifies every encoded RGBA byte including alpha.
///
/// # Panics
/// Panics on current-media, encoder, validation, pixel mismatch or cleanup failure.
#[test]
fn native_ffv1_video_preserves_all_rendered_rgba_frames() {
    let scratch = Scratch::new(true);
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Video,
        include_bytes!("../../../assets/video-sample0001-0010.mp4"),
        1080,
        1920,
        10,
    );
    let destination = scratch.path.join("native-ffv1.mkv");
    println!("native video evidence: {}", scratch.path.display());
    let job = VideoExportJob::new(
        document,
        sources,
        options(destination.clone(), VideoCodec::Ffv1Matroska),
    )
    .unwrap();
    let events = Mutex::new(Vec::new());
    let cancelled = AtomicBool::new(false);
    let mut recovery = job
        .render_frames(MediaTools::default(), &cancelled, &|event| {
            events.lock().unwrap().push(event)
        })
        .unwrap();
    let frames = recovery.frames_path().unwrap();
    assert!(frames.starts_with("/tmp"));
    let mut hashes = Vec::new();
    for index in 0..10 {
        let png = frames.join(format!("frame-{index:06}.png"));
        let image = image::open(&png).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (1080, 1920));
        hashes.push(Sha256::digest(image.as_raw()));
        if matches!(index, 0 | 9) {
            fs::copy(&png, scratch.path.join(format!("frame-{index:06}.png"))).unwrap();
        }
    }
    let result = recovery
        .retry(MediaTools::default(), &destination, &cancelled, &|event| {
            events.lock().unwrap().push(event)
        })
        .unwrap();
    assert_eq!(result.frame_count, 10);
    assert!(result.cleanup_warning.is_none());
    assert!(!frames.exists());
    let mut decoder = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&destination)
        .args([
            "-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = decoder.stdout.take().unwrap();
    let mut rgba = vec![0; 1080 * 1920 * 4];
    for expected in hashes {
        stdout.read_exact(&mut rgba).unwrap();
        assert_eq!(Sha256::digest(&rgba), expected);
    }
    assert_eq!(stdout.read(&mut [0_u8; 1]).unwrap(), 0);
    let result = decoder.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let events = events.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.phase == VideoPhase::Encoding && event.completed_frames == 10)
    );
    assert!(events.iter().any(|event| event.encoded_time_micros > 0));
    assert_eq!(events.last().unwrap().phase, VideoPhase::Complete);
}

/// Keeps PNGs on failed/cancelled retries and supports Save PNG sequence and explicit Discard.
///
/// # Panics
/// Panics if recovery loses frames, leaves partial video output, or publishes over a collision.
#[test]
fn video_recovery_preserves_frames_until_successful_copy_or_discard() {
    let scratch = Scratch::new(false);
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png"),
        64,
        64,
        2,
    );
    let destination = scratch.path.join("output.mkv");
    let job = VideoExportJob::new(
        document,
        sources,
        options(destination.clone(), VideoCodec::Ffv1Matroska),
    )
    .unwrap();
    let cancelled = AtomicBool::new(false);
    let mut recovery = job
        .render_frames(MediaTools::default(), &cancelled, &|_| {})
        .unwrap();
    let retained = recovery.frames_path().unwrap();
    let bad_tools = MediaTools {
        ffmpeg: "/usr/bin/false".into(),
        ..MediaTools::default()
    };
    assert!(
        recovery
            .retry(bad_tools, &destination, &cancelled, &|_| {})
            .is_err()
    );
    assert!(!destination.exists());
    assert!(retained.join("frame-000001.png").exists());
    cancelled.store(true, Ordering::Release);
    assert!(
        recovery
            .retry(MediaTools::default(), &destination, &cancelled, &|_| {})
            .unwrap_err()
            .is_cancelled()
    );
    cancelled.store(false, Ordering::Release);
    fs::write(&destination, b"existing user video").unwrap();
    assert!(
        recovery
            .retry(MediaTools::default(), &destination, &cancelled, &|_| {})
            .is_err()
    );
    assert_eq!(fs::read(&destination).unwrap(), b"existing user video");
    let expected = fs::read(retained.join("frame-000001.png")).unwrap();
    let copied = recovery
        .save_png_sequence(&scratch.path.join("saved-frames"), &cancelled, &|_| {})
        .unwrap();
    assert_eq!(fs::read(copied.join("frame-000001.png")).unwrap(), expected);
    assert!(!retained.exists());
    assert!(recovery.frames_path().is_none());
    let mut second_job = job.clone();
    second_job.options.destination = scratch.path.join("discard.mkv");
    let mut discard = second_job
        .render_frames(MediaTools::default(), &cancelled, &|_| {})
        .unwrap();
    let path = discard.frames_path().unwrap();
    discard.discard().unwrap();
    assert!(!path.exists());
    let mut cancelled_job = job.clone();
    cancelled_job.options.destination = scratch.path.join("cancel-encoding.mkv");
    cancelled_job.options.temporary_directory = Some(scratch.path.clone());
    let stop = AtomicBool::new(false);
    let failure = cancelled_job
        .run(MediaTools::default(), &stop, &|event| {
            if event.phase == VideoPhase::Encoding {
                stop.store(true, Ordering::Release);
            }
        })
        .unwrap_err();
    assert!(failure.error.is_cancelled());
    assert!(failure.recovery.is_none());
    assert!(!cancelled_job.options.destination.exists());
    assert!(!fs::read_dir(&scratch.path).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("toniator-render-")
    }));
}

/// Encodes an explicitly opaque AV1 sharing artifact and verifies native output plus SDR color conversion.
///
/// # Panics
/// Panics if the selected software encoder, color policy, stream validation or matte rule fails.
#[test]
fn av1_sharing_requires_matte_and_preserves_sdr_patch_colors() {
    use image::ImageEncoder;
    use toniator_domain::{FrameRange, RationalTime};
    let scratch = Scratch::new(true);
    let (document, sources) = fixture(
        EmbeddedSourceFormat::Png,
        include_bytes!("../../../assets/raster-sample.png"),
        1024,
        1024,
        2,
    );
    let destination = scratch.path.join("sharing.webm");
    let mut choices = options(destination.clone(), VideoCodec::Av1Webm);
    choices.background = None;
    assert!(VideoExportJob::new(document.clone(), sources.clone(), choices).is_err());
    let job = VideoExportJob::new(
        document,
        sources,
        options(destination.clone(), VideoCodec::Av1Webm),
    )
    .unwrap();
    let result = job
        .run(MediaTools::default(), &AtomicBool::new(false), &|_| {})
        .unwrap();
    assert_eq!(
        (result.width, result.height, result.frame_count),
        (1024, 1024, 2)
    );
    println!("AV1 sharing evidence: {}", destination.display());

    let colors = [
        [128, 64, 32, 255],
        [32, 96, 160, 255],
        [80, 80, 80, 255],
        [180, 160, 90, 255],
    ];
    let mut rgba = Vec::new();
    for y in 0..128 {
        for x in 0..128 {
            rgba.extend_from_slice(&colors[(y / 64) * 2 + x / 64]);
        }
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, 128, 128, image::ColorType::Rgba8.into())
        .unwrap();
    let timing = ProjectTiming::new(
        FrameRate::new(6, 1).unwrap(),
        FrameRange::new(0, 1).unwrap(),
    );
    let workspace = RenderWorkspace::create(Path::new("/tmp")).unwrap();
    let mut writer = SequenceWriter::create(
        &workspace.frames_path(),
        SequenceManifest::new(SequenceFormat::Png, 128, 128, &timing),
        262_144,
    )
    .unwrap();
    writer.write_frame(&png).unwrap();
    writer.finish().unwrap();
    let path = scratch.path.join("color-patches.webm");
    let mut recovery = VideoRecovery {
        workspace: Some(workspace),
        options: options(path.clone(), VideoCodec::Av1Webm),
        timing,
        width: 128,
        height: 128,
        frame_count: 1,
        estimated_bytes: 262_144,
    };
    recovery
        .retry(
            MediaTools::default(),
            &path,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
    let mut source = toniator_sampling::media::VideoSource::new(
        fs::read(&path).unwrap(),
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    let frame = source.frame_at(RationalTime::default(), &|| false).unwrap();
    for (index, (x, y)) in [(32, 32), (96, 32), (32, 96), (96, 96)]
        .into_iter()
        .enumerate()
    {
        let pixel = frame.field.pixel(x, y).unwrap();
        for (actual, expected) in [pixel.red, pixel.green, pixel.blue]
            .into_iter()
            .zip(colors[index])
        {
            assert!(
                (actual * 255.0 - f64::from(expected)).abs() <= 5.0,
                "patch {index}: actual {actual}, expected {expected}"
            );
        }
    }
}
