use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::RationalTime;
use toniator_sampling::{FrameSource, MediaTools, VideoSource};

/// Creates an exclusively owned source witness with opaque, fractional and zero-alpha bands.
///
/// # Panics
/// Panics if the validation directory or exact test PNG cannot be created.
fn witness() -> (PathBuf, Vec<u8>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage22-alpha-sar")
        .join(format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    let rgba: Vec<u8> = (0..32)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                [
                    if x < 32 { 210 } else { 35 },
                    if y < 16 { 65 } else { 195 },
                    120,
                    [0, 64, 128, 255][x / 16],
                ]
            })
        })
        .collect();
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&rgba, 64, 32, ColorType::Rgba8.into())
        .unwrap();
    fs::write(root.join("source.png"), png).unwrap();
    (root, rgba)
}

/// Checks one source-generation process without accepting partial or silently substituted output.
///
/// # Panics
/// Panics with FFmpeg diagnostics if encoding or the selected software codec is unavailable.
fn encode(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Proves software VP8/VP9 and FFV1 retain declared alpha through SDR conversion and both frames.
/// Missing alpha-capable decoders fail explicitly; they never fall back to opaque decoding.
///
/// # Panics
/// Panics if source generation, exact alpha reconstruction, decoder identity or rejection fails.
#[test]
fn video_alpha_uses_matching_software_probe_and_decode() {
    let (root, rgba) = witness();
    for (name, codec, format, extra) in [
        ("ffv1.mkv", "ffv1", "yuv444p", vec!["-level", "3"]),
        ("vp9.webm", "libvpx-vp9", "yuv420p", vec!["-lossless", "1"]),
        (
            "vp8.webm",
            "libvpx",
            "yuv420p",
            vec!["-auto-alt-ref", "0", "-crf", "4", "-b:v", "1M"],
        ),
    ] {
        let filter = format!(
            "[0:v]split[color][alpha];[alpha]alphaextract[mask];[color]scale=in_range=full:out_range=full:out_color_matrix=bt709,format=yuv444p12le,colorspace=iall=bt709:itrc=srgb:irange=pc:all=bt709:range=tv:format={format}[converted];[converted][mask]alphamerge,setparams=range=limited:color_primaries=bt709:color_trc=bt709:colorspace=bt709[out]"
        );
        let path = root.join(name);
        let mut command = Command::new("ffmpeg");
        command
            .args(["-v", "error", "-n", "-loop", "1", "-framerate", "2", "-i"])
            .arg(root.join("source.png"))
            .args([
                "-filter_complex_threads",
                "1",
                "-filter_complex",
                &filter,
                "-map",
                "[out]",
                "-frames:v",
                "2",
                "-an",
                "-c:v",
                codec,
            ])
            .args(extra)
            .arg(&path);
        encode(&mut command);
        let bytes = fs::read(&path).unwrap();
        let mut source = VideoSource::new(bytes.clone(), MediaTools::default(), &|| false).unwrap();
        assert_eq!(
            (
                source.metadata().width,
                source.metadata().height,
                source.metadata().frame_count
            ),
            (64, 32, 2)
        );
        for index in 0..2 {
            let frame = source
                .frame_at(RationalTime::new(index, 2).unwrap(), &|| false)
                .unwrap();
            assert_eq!(frame.index, index);
            for y in 0..32 {
                for x in 0..64 {
                    let expected = f64::from(rgba[((y * 64 + x) * 4 + 3) as usize]) / 255.0;
                    assert!(
                        (frame.field.pixel(x, y).unwrap().alpha - expected).abs() < 1e-12,
                        "{name} alpha at {x},{y}"
                    );
                }
            }
            if codec.starts_with("libvpx") {
                assert!(frame.identity.decoder_contract.ends_with(codec));
            }
        }
        if codec == "libvpx-vp9" {
            let wrapper = root.join("no-vpx-ffprobe");
            fs::write(&wrapper, "#!/bin/sh\nfor arg do\n case \"$arg\" in libvpx*) echo 'Unknown decoder' >&2; exit 1;; esac\ndone\nexec ffprobe \"$@\"\n").unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
            let tools = MediaTools {
                ffprobe: wrapper,
                ..MediaTools::default()
            };
            assert!(VideoSource::new(bytes, tools, &|| false).is_err());
        }
    }
}

/// Proves non-square sample aspect expands the decoded field once while retaining exact RGBA.
///
/// # Panics
/// Panics if FFV1 encoding or the canonical square-pixel nearest-neighbor projection differs.
#[test]
fn video_sample_aspect_preserves_square_pixel_color_and_alpha() {
    let (root, rgba) = witness();
    let path = root.join("sar.mkv");
    let mut command = Command::new("ffmpeg");
    command
        .args(["-v", "error", "-n", "-loop", "1", "-framerate", "2", "-i"])
        .arg(root.join("source.png"))
        .args([
            "-frames:v",
            "2",
            "-an",
            "-vf",
            "setsar=2/1",
            "-c:v",
            "ffv1",
            "-level",
            "3",
            "-pix_fmt",
            "bgra",
        ])
        .arg(path.clone());
    encode(&mut command);
    let mut source =
        VideoSource::new(fs::read(path).unwrap(), MediaTools::default(), &|| false).unwrap();
    assert_eq!(
        (source.metadata().width, source.metadata().height),
        (128, 32)
    );
    let frame = source
        .frame_at(RationalTime::new(1, 2).unwrap(), &|| false)
        .unwrap();
    let mut expanded = Vec::new();
    for pixel in rgba.chunks_exact(4) {
        expanded.extend_from_slice(pixel);
        expanded.extend_from_slice(pixel);
    }
    let expected = toniator_sampling::SourceField::from_straight_rgba8(128, 32, expanded).unwrap();
    assert_eq!(
        frame.field.identity().decoded_pixel_hash,
        expected.identity().decoded_pixel_hash
    );
}
