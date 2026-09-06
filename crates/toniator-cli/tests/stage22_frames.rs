use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    CanvasSpec, ChannelId, Document, Easing, FrameRange, FrameRate, ProjectTiming, PropertyFieldId,
    PropertyTarget, SourceReference, SourceReferenceId, TemporalEndpointEdit,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, save};

/// Owns only this test's exclusively created scratch directory.
struct Scratch(PathBuf);

/// Imports direct moving media and explicit still sequences into the current project contract.
///
/// # Panics
/// Panics if CLI defaults, portable source ordering, selected source time, or decoding fail.
#[test]
fn direct_media_create_and_render_share_timing_authority() {
    let directory = Scratch::new();
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let build_flags = [
        "--channel-model",
        "rgb",
        "--canvas",
        "48x48",
        "--density",
        "8",
        "--density-aspect",
        "1",
        "--rotation",
        "0",
        "--offset-x",
        "0",
        "--offset-y",
        "0",
        "--guard-steps",
        "1",
    ];
    let project = directory.0.join("direct-video.toniator");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["document", "create", "-i"])
        .arg(assets.join("video-sample0001-0010.mp4"))
        .arg("-o")
        .arg(&project)
        .args(build_flags)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let loaded = toniator_io::load(&project).unwrap();
    assert_eq!(
        loaded.document().project_timing().frame_rate(),
        FrameRate::new(6, 1).unwrap()
    );
    assert_eq!(
        loaded
            .document()
            .project_timing()
            .frame_range()
            .frame_count(),
        10
    );
    assert!(matches!(
        loaded.sources().media(),
        Some(toniator_io::SourceMediaManifest::Video { .. })
    ));
    let output = directory.0.join("direct-video");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["render", "-i"])
        .arg(assets.join("video-sample0001-0010.mp4"))
        .arg("-o")
        .arg(output.join("frame-%06d.svg"))
        .args(build_flags)
        .args(["--start-frame", "7", "--end-frame", "9"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.join("frame-000002.svg").exists());
    assert!(!output.join("frame-000003.svg").exists());

    let red = directory.0.join("red.png");
    let blue = directory.0.join("blue.png");
    image::RgbaImage::from_pixel(48, 48, image::Rgba([255, 0, 0, 255]))
        .save(&red)
        .unwrap();
    image::RgbaImage::from_pixel(48, 48, image::Rgba([0, 0, 255, 255]))
        .save(&blue)
        .unwrap();
    let project = directory.0.join("ordered.toniator");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["document", "create", "-i"])
        .arg(&red)
        .arg("--sequence-frame")
        .arg(&blue)
        .arg("--sequence-frame")
        .arg(&red)
        .args(["--fps", "1.5"])
        .arg("-o")
        .arg(&project)
        .args(build_flags)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let loaded = toniator_io::load(&project).unwrap();
    assert_eq!(loaded.sources().len(), 2);
    assert_eq!(
        loaded.document().project_timing().frame_rate(),
        FrameRate::new(3, 2).unwrap()
    );
    assert_eq!(
        loaded
            .document()
            .project_timing()
            .frame_range()
            .frame_count(),
        3
    );
    let Some(toniator_io::SourceMediaManifest::ImageSequence { source_ids, .. }) =
        loaded.sources().media()
    else {
        panic!("ordered sequence manifest")
    };
    assert_eq!(source_ids[0], source_ids[2]);
    assert_ne!(source_ids[0], source_ids[1]);
    let output = directory.0.join("ordered");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["render", "-i"])
        .arg(&project)
        .arg("-o")
        .arg(output.join("frame-%06d.png"))
        .args(["--background", "transparent"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let frames = (0..3)
        .map(|index| {
            image::open(output.join(format!("frame-{index:06}.png")))
                .unwrap()
                .to_rgba8()
        })
        .collect::<Vec<_>>();
    assert_eq!(frames[0], frames[2]);
    assert_ne!(frames[0], frames[1]);
}

impl Scratch {
    /// Creates a unique directory for CLI inputs and output without touching user assets.
    ///
    /// # Panics
    /// Panics when the test clock or scratch filesystem is unavailable.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "toniator-cli-frames-{}-{}",
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

impl Drop for Scratch {
    /// Removes only generated test files at scope exit.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Renders project endpoints and sequences, rejects bad frame bounds, and cooperatively cancels.
///
/// End opacity is zero on all channels; transparent PNG output proves CLI materializes the
/// requested animation endpoint instead of always rendering ordinary Start settings.
///
/// # Panics
/// Panics if the current fixture, local FFmpeg tools, CLI invocation, or pixel assertions fail.
#[test]
fn project_frame_render_uses_shared_media_and_authored_endpoint() {
    let directory = Scratch::new();
    let source_id = SourceReferenceId::new("video").unwrap();
    let mut document = Document::new_default_document(
        CanvasSpec {
            width: 48.0,
            height: 48.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let edits = (1..=3)
        .map(|id| TemporalEndpointEdit {
            target: PropertyTarget::Channel(ChannelId(id)),
            field: PropertyFieldId::Opacity,
            effective_end: 0.0,
            easing: Easing::Linear,
        })
        .collect::<Vec<_>>();
    let command = document.edit_effective_end_command(&edits).unwrap();
    document = document
        .with_temporal_authority(
            ProjectTiming::new(
                FrameRate::new(6, 1).unwrap(),
                FrameRange::new(0, 10).unwrap(),
            ),
            command.replacement().end_overrides.clone(),
        )
        .unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Video,
        include_bytes!("../../../assets/video-sample0001-0010.mp4").as_slice(),
        None,
    )
    .unwrap()])
    .unwrap();
    let input = directory.0.join("video.toniator");
    save(&input, &document, &sources).unwrap();
    for frame in [0, 9, 10] {
        let output = directory.0.join(format!("frame-{frame}.png"));
        let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .arg("render")
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .arg("--frame")
            .arg(frame.to_string())
            .args(["--background", "transparent"])
            .output()
            .unwrap();
        if frame == 10 {
            assert!(!result.status.success());
            assert!(!output.exists());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("temporal.frame:"),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        } else {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let raster = image::open(&output).unwrap().to_rgba8();
            assert_eq!(raster.dimensions(), (48, 48));
            let covered = raster.pixels().any(|pixel| pixel[3] != 0);
            assert_eq!(covered, frame == 0);
        }
    }
    assert_eq!(toniator_io::load(&input).unwrap().document(), &document);
    for (name, flags, count) in [
        (
            "trim-frames",
            vec!["--start-frame", "2", "--end-frame", "4"],
            3,
        ),
        (
            "trim-time",
            vec!["--start-time", "0.1", "--end-time", "0.6", "--fps", "6/1"],
            3,
        ),
        (
            "trim-one",
            vec!["--start-frame", "4", "--end-frame", "4"],
            1,
        ),
    ] {
        let output = directory.0.join(name);
        let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .arg("render")
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(output.join("frame-%06d.png"))
            .args(flags)
            .args(["--background", "transparent"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["completed_frames"], count);
        let first = image::open(output.join("frame-000000.png"))
            .unwrap()
            .to_rgba8();
        assert!(first.pixels().any(|pixel| pixel[3] != 0));
        if count > 1 {
            let last = image::open(output.join(format!("frame-{:06}.png", count - 1)))
                .unwrap()
                .to_rgba8();
            assert!(last.pixels().all(|pixel| pixel[3] == 0));
        }
    }
    for flags in [
        vec!["--start-frame", "1", "--end-time", "1"],
        vec!["--frame", "1", "--start-time", "0"],
        vec!["--end-frame", "10"],
        vec!["--fps", "NaN"],
        vec!["--fps", "1/0"],
        vec!["--start-time", "-1"],
    ] {
        let output = directory.0.join("invalid");
        let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .arg("render")
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(output.join("frame-%06d.png"))
            .args(flags)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
    assert_eq!(toniator_io::load(&input).unwrap().document(), &document);
    let sequence = directory.0.join("sequence");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .arg("render")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(sequence.join("frame-%06d.png"))
        .args(["--background", "transparent"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(sequence.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["complete"], true);
    assert_eq!(manifest["completed_frames"], 10);
    assert!(sequence.join("frame-000009.png").exists());
    let video = directory.0.join("video-output.mkv");
    let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .arg("render")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&video)
        .arg("--temporary-directory")
        .arg(&directory.0)
        .args(["--background", "transparent"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(video.metadata().unwrap().len() > 0);
    assert!(String::from_utf8_lossy(&result.stdout).contains("10 silent video frames"));
    assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("toniator-render-")
    }));
    let cancelled_sequence = directory.0.join("cancelled-sequence");
    let mut child = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .arg("render")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(cancelled_sequence.join("frame-%06d.png"))
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut ready = String::new();
    stderr.read_line(&mut ready).unwrap();
    assert!(ready.contains("Preparing frame export"));
    assert!(
        Command::new("kill")
            .arg("-TERM")
            .arg(child.id().to_string())
            .status()
            .unwrap()
            .success()
    );
    let mut diagnostic = String::new();
    stderr.read_to_string(&mut diagnostic).unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(diagnostic.contains("export.cancelled"), "{diagnostic}");
    if cancelled_sequence.exists() {
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(cancelled_sequence.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["complete"], false);
    }
}
