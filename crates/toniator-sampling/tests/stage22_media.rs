use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use toniator_domain::{FrameRate, RationalTime};
use toniator_sampling::{
    AnimatedImageFormat, AnimatedImageSource, FrameSource, ImageSequenceEntry, ImageSequenceSource,
    MediaTools, SourceField, SourceFormatHint, StillImageSource, VideoSource,
};

/// Returns one immutable repository input, never a historical generated artifact.
fn asset(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(name),
    )
    .unwrap()
}

/// Encodes a small literal RGBA witness for the existing still decoder.
fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, ColorType::Rgba8.into())
        .unwrap();
    bytes
}

/// Allocates an exclusively owned test directory and removes only its own witnesses.
struct Temporary(PathBuf);
impl Temporary {
    /// Creates a collision-resistant local test directory without overwriting existing files.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "toniator-media-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temporary {
    /// Removes the unique test directory and its generated fixtures.
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Proves native PNG/SVG dimensions, deterministic still repetition, and direct hidden-RGB retention.
#[test]
fn still_sources_repeat_native_fields_and_rgba_retains_hidden_color() {
    for (name, hint, dimensions) in [
        ("raster-sample.png", SourceFormatHint::Png, (1024, 1024)),
        ("vector-sample.svg", SourceFormatHint::Svg, (900, 620)),
    ] {
        let mut source = StillImageSource::new(asset(name), hint).unwrap();
        assert_eq!(
            (source.metadata().width, source.metadata().height),
            dimensions
        );
        let first = source.frame_at(RationalTime::default(), &|| false).unwrap();
        let late = source
            .frame_at(RationalTime::new(100, 1).unwrap(), &|| false)
            .unwrap();
        assert!(Arc::ptr_eq(&first.field, &late.field));
        assert_eq!(first.identity, late.identity);
        assert_eq!(
            source
                .frame_at(RationalTime::default(), &|| true)
                .unwrap_err()
                .path(),
            "source.cancelled"
        );
    }
    let field =
        SourceField::from_straight_rgba8(2, 1, vec![255, 0, 40, 0, 0, 150, 255, 64]).unwrap();
    assert_eq!(field.pixel(0, 0).unwrap().red, 1.0);
    assert_eq!(field.pixel(0, 0).unwrap().alpha, 0.0);
    assert_eq!(field.pixel(1, 0).unwrap().alpha, 64.0 / 255.0);
    assert!(SourceField::from_straight_rgba8(1, 1, vec![0; 3]).is_err());
}

/// Proves explicit sequence ordering, exact fractional selection, repeat entries, and half-open bounds.
///
/// # Panics
/// Panics if valid sequence frames fail decoding or violate ordering, identity, or memory bounds.
#[test]
fn ordered_sequence_uses_assigned_rational_time() {
    let bytes: Arc<[u8]> = include_bytes!("../../../assets/raster-sample.png")
        .as_slice()
        .into();
    let repeats = 128 * 1024 * 1024 / bytes.len() + 1;
    let repeated = ImageSequenceSource::new(
        vec![
            ImageSequenceEntry {
                bytes,
                format: SourceFormatHint::Png
            };
            repeats
        ],
        FrameRate::new(30, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(repeated.metadata().frame_count, repeats as u64);
    let red = png(1, 1, &[255, 0, 0, 255]);
    let blue = png(1, 1, &[0, 0, 255, 255]);
    let mut source = ImageSequenceSource::new(
        [red.clone(), blue, red]
            .into_iter()
            .map(|bytes| ImageSequenceEntry {
                bytes: bytes.into(),
                format: SourceFormatHint::Png,
            })
            .collect(),
        FrameRate::new(3, 2).unwrap(),
    )
    .unwrap();
    assert_eq!(
        source.metadata().duration,
        Some(RationalTime::new(2, 1).unwrap())
    );
    assert_eq!(
        source
            .frame_at(RationalTime::new(2, 3).unwrap(), &|| false)
            .unwrap()
            .field
            .pixel(0, 0)
            .unwrap()
            .blue,
        1.0
    );
    assert_eq!(
        source
            .frame_at(RationalTime::new(1333, 1000).unwrap(), &|| false)
            .unwrap()
            .index,
        1
    );
    let third = source
        .frame_at(RationalTime::new(4, 3).unwrap(), &|| false)
        .unwrap();
    let first = source.frame_at(RationalTime::default(), &|| false).unwrap();
    assert_eq!(first.field, third.field);
    assert_ne!(first.identity, third.identity);
    assert_eq!(
        source
            .frame_at(RationalTime::new(2, 1).unwrap(), &|| false)
            .unwrap_err()
            .path(),
        "source.range"
    );
}

/// Exercises the actual ten-frame decoder, SDR conversion, sequential/backward agreement, and cancellation.
#[test]
fn supplied_video_has_exact_frames_and_reaps_on_cancel() {
    let mut source = VideoSource::new(
        asset("video-sample0001-0010.mp4"),
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        (
            source.metadata().width,
            source.metadata().height,
            source.metadata().frame_count
        ),
        (1080, 1920, 10)
    );
    assert_eq!(
        source.metadata().nominal_frame_rate,
        Some(FrameRate::new(6, 1).unwrap())
    );
    assert_eq!(
        source.metadata().duration,
        Some(RationalTime::new(5, 3).unwrap())
    );
    assert!(!source.metadata().has_audio);
    let mut hashes = Vec::new();
    for index in 0..10 {
        let frame = source
            .frame_at(RationalTime::new(index, 6).unwrap(), &|| false)
            .unwrap();
        assert_eq!(frame.index, index);
        hashes.push(frame.identity.decoded_pixel_hash);
    }
    assert_eq!(
        hashes[0],
        "sha256:64f179301a8ba175b3a0f9fb279d93151b0caa9ccd85b6c3eb61a3a1b9da7c8e"
    );
    let repeated = source
        .frame_at(RationalTime::new(3, 2).unwrap(), &|| false)
        .unwrap();
    assert_eq!(repeated.identity.decoded_pixel_hash, hashes[9]);
    assert_eq!(
        source
            .frame_at(RationalTime::default(), &|| false)
            .unwrap()
            .identity
            .decoded_pixel_hash,
        hashes[0]
    );
    let start = Instant::now();
    assert_eq!(
        source
            .frame_at(RationalTime::new(1, 6).unwrap(), &|| true)
            .unwrap_err()
            .path(),
        "source.cancelled"
    );
    drop(source);
    assert!(
        start.elapsed().as_secs() < 5,
        "cancel/drop must reap a backpressured decoder"
    );
}

/// Checks finite APNG/GIF playback and preserves transparent RGBA through declared-sRGB FFV1 input.
#[test]
fn animated_images_play_once_and_ffv1_alpha_is_exact() {
    let temporary = Temporary::new();
    for (extension, format) in [
        ("png", AnimatedImageFormat::Png),
        ("gif", AnimatedImageFormat::Gif),
        ("webp", AnimatedImageFormat::Webp),
    ] {
        let path = temporary.0.join(format!("animation.{extension}"));
        let mut command = Command::new("ffmpeg");
        command.args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=16x16:rate=2",
            "-frames:v",
            "2",
        ]);
        if extension == "png" {
            command.args(["-f", "apng", "-plays", "0"]);
        }
        if extension == "webp" {
            command.args(["-c:v", "libwebp_anim", "-lossless", "1", "-loop", "0"]);
        }
        command.arg(&path);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut source = AnimatedImageSource::new(
            fs::read(&path).unwrap(),
            format,
            MediaTools::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(source.metadata().frame_count, 2);
        let duration = source.metadata().duration.unwrap();
        let first = source.frame_at(RationalTime::default(), &|| false).unwrap();
        let second = source
            .frame_at(RationalTime::new(1, 2).unwrap(), &|| false)
            .unwrap();
        assert_eq!(second.index, 1);
        assert_ne!(first.identity, second.identity);
        assert!(source.frame_at(duration, &|| false).is_err());
        assert_eq!(
            source
                .frame_at(RationalTime::default(), &|| false)
                .unwrap()
                .field,
            first.field
        );
    }
    let rgba: Vec<u8> = (0..256)
        .flat_map(|value| [value as u8, 30, 230, value as u8])
        .collect();
    let input = temporary.0.join("alpha.png");
    let video = temporary.0.join("alpha.mkv");
    fs::write(&input, png(16, 16, &rgba)).unwrap();
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-loop", "1", "-i"])
        .arg(&input)
        .args([
            "-frames:v",
            "2",
            "-r",
            "2",
            "-c:v",
            "ffv1",
            "-level",
            "3",
            "-pix_fmt",
            "bgra",
            "-colorspace",
            "rgb",
            "-color_trc",
            "iec61966-2-1",
            "-color_primaries",
            "bt709",
            "-color_range",
            "pc",
        ])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut source =
        VideoSource::new(fs::read(&video).unwrap(), MediaTools::default(), &|| false).unwrap();
    let frame = source.frame_at(RationalTime::default(), &|| false).unwrap();
    assert_eq!(
        *frame.field,
        SourceField::from_straight_rgba8(16, 16, rgba).unwrap()
    );
}

/// Compares source rotation with FFmpeg's native display-matrix interpretation.
#[test]
fn rotated_video_matches_native_orientation() {
    let temporary = Temporary::new();
    let input = temporary.0.join("input.mp4");
    let rotated = temporary.0.join("rotated.mp4");
    fs::write(&input, asset("video-sample0001-0010.mp4")).unwrap();
    let encoded = Command::new("ffmpeg")
        .args(["-v", "error", "-display_rotation:v:0", "90", "-i"])
        .arg(&input)
        .args(["-c", "copy"])
        .arg(&rotated)
        .output()
        .unwrap();
    assert!(encoded.status.success());
    let reference = Command::new("ffmpeg")
        .args(["-v", "error", "-filter_threads", "1", "-i"])
        .arg(&rotated)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "colorspace=all=bt709:trc=srgb:format=yuv444p12,format=rgba",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        reference.status.success(),
        "{}",
        String::from_utf8_lossy(&reference.stderr)
    );
    let expected = SourceField::from_straight_rgba8(1920, 1080, reference.stdout).unwrap();
    let mut source = VideoSource::new(fs::read(&rotated).unwrap(), MediaTools::default(), &|| {
        false
    })
    .unwrap();
    let frame = source.frame_at(RationalTime::default(), &|| false).unwrap();
    assert_eq!(
        (source.metadata().width, source.metadata().height),
        (1920, 1080)
    );
    assert_eq!(
        frame.identity.decoded_pixel_hash,
        expected.identity().decoded_pixel_hash
    );
}

/// Exercises unequal source intervals through the actual FFmpeg provider and exact domain time.
#[test]
fn variable_frame_rate_selects_latest_presentation_timestamp() {
    let temporary = Temporary::new();
    for (index, color) in [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]]
        .into_iter()
        .enumerate()
    {
        fs::write(temporary.0.join(format!("{index}.png")), png(1, 1, &color)).unwrap();
    }
    let list = temporary.0.join("inputs.txt");
    fs::write(
        &list,
        "file 0.png\nduration 0.2\nfile 1.png\nduration 0.6\nfile 2.png\n",
    )
    .unwrap();
    let video = temporary.0.join("vfr.mkv");
    let encoded = Command::new("ffmpeg")
        .args(["-v", "error", "-f", "concat", "-safe", "1", "-i"])
        .arg(&list)
        .args(["-fps_mode", "vfr", "-c:v", "ffv1", "-pix_fmt", "bgra"])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        encoded.status.success(),
        "{}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    let mut source =
        VideoSource::new(fs::read(&video).unwrap(), MediaTools::default(), &|| false).unwrap();
    assert!(source.metadata().variable_frame_rate);
    for (milliseconds, index) in [(0, 0), (199, 0), (200, 1), (799, 1), (800, 2)] {
        assert_eq!(
            source
                .frame_at(RationalTime::new(milliseconds, 1000).unwrap(), &|| false)
                .unwrap()
                .index,
            index
        );
    }
    assert!(
        source
            .frame_at(source.metadata().duration.unwrap(), &|| false)
            .is_err()
    );
}
