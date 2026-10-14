use toniator_domain::RationalTime;
use toniator_sampling::media::{
    AnimatedImageFormat, AnimatedImageSource, FrameSource, MediaTools, is_animated_image,
};
use toniator_sampling::{SourceFormat, SourceFormatHint, decode_source};
const STILL: &[u8] = include_bytes!("fixtures/jpegxl/rgba-still.jxl");
const ANIMATION: &[u8] = include_bytes!("fixtures/jpegxl/rgba-animation.jxl");

/// Checks native still colors and independent alpha, including transparent pixels and strict hints.
///
/// # Panics
/// Panics if the generated lossless fixture changes its decoded contract or malformed data succeeds.
#[test]
fn jpegxl_still_preserves_straight_color_alpha_and_rejects_malformed_sources() {
    assert!(!is_animated_image(STILL, SourceFormatHint::JpegXl).unwrap());
    let field = decode_source(STILL, SourceFormatHint::JpegXl).unwrap();
    assert_eq!((field.identity().width, field.identity().height), (3, 2));
    assert_eq!(field.identity().format, SourceFormat::JpegXl);
    let red = field.pixel(0, 0).unwrap();
    assert_eq!((red.red, red.green, red.blue, red.alpha), (1., 0., 0., 1.));
    let green = field.pixel(1, 0).unwrap();
    assert_eq!(
        (green.red, green.green, green.blue, green.alpha),
        (0., 1., 0., 128. / 255.)
    );
    assert_eq!(field.pixel(2, 0).unwrap().alpha, 0.);
    assert!(decode_source(STILL, SourceFormatHint::Png).is_err());
    assert!(
        decode_source(
            include_bytes!("../../../assets/raster-sample.png"),
            SourceFormatHint::JpegXl
        )
        .is_err()
    );
    assert!(decode_source(&STILL[..STILL.len() / 2], SourceFormatHint::JpegXl).is_err());
    assert!(decode_source(&ANIMATION[..ANIMATION.len() / 2], SourceFormatHint::JpegXl).is_err());
}

/// Checks exact finite timing, random backward selection, alpha, cancellation, and no flattening.
///
/// # Panics
/// Panics if the generated animation fails its three-frame native source contract.
#[test]
fn jpegxl_animation_uses_finite_native_frames_and_exact_time() {
    assert!(is_animated_image(ANIMATION, SourceFormatHint::JpegXl).unwrap());
    assert!(decode_source(ANIMATION, SourceFormatHint::JpegXl).is_err());
    let mut source = AnimatedImageSource::new(
        ANIMATION,
        AnimatedImageFormat::JpegXl,
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.metadata().frame_count, 3);
    assert_eq!(
        source.metadata().duration,
        Some(RationalTime::new(3, 4).unwrap())
    );
    for (n, expected) in [
        (2, (0., 0., 1., 1.)),
        (0, (1., 0., 0., 1.)),
        (1, (0., 1., 0., 128. / 255.)),
    ] {
        let frame = source
            .frame_at(RationalTime::new(n, 4).unwrap(), &|| false)
            .unwrap();
        assert_eq!(frame.index, n);
        let pixel = frame.field.pixel(0, 0).unwrap();
        assert_eq!((pixel.red, pixel.green, pixel.blue, pixel.alpha), expected);
    }
    assert!(
        source
            .frame_at(RationalTime::new(3, 4).unwrap(), &|| false)
            .is_err()
    );
    assert!(source.frame_at(RationalTime::default(), &|| true).is_err());
}

/// Checks variable durations and ignores infinite loop tags while selecting the latest exact frame.
///
/// # Panics
/// Panics if native timing, grayscale, high-bit-depth, orientation, ICC conversion, or rejection drifts.
#[test]
fn jpegxl_native_metadata_color_profiles_and_explicit_alpha_limit() {
    let bytes = include_bytes!("fixtures/jpegxl/vfr-loop-animation.jxl");
    let mut source = AnimatedImageSource::new(
        bytes.as_slice(),
        AnimatedImageFormat::JpegXl,
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        source.metadata().duration,
        Some(RationalTime::new(3, 2).unwrap())
    );
    assert!(source.metadata().variable_frame_rate);
    for (n, d, index) in [(0, 1, 0), (1, 4, 1), (1, 2, 1), (3, 4, 2), (5, 4, 2)] {
        assert_eq!(
            source
                .frame_at(RationalTime::new(n, d).unwrap(), &|| false)
                .unwrap()
                .index,
            index
        );
    }
    assert!(
        source
            .frame_at(RationalTime::new(3, 2).unwrap(), &|| false)
            .is_err()
    );
    for bytes in [
        include_bytes!("fixtures/jpegxl/gray.jxl").as_slice(),
        include_bytes!("fixtures/jpegxl/rgba16.jxl").as_slice(),
    ] {
        let field = decode_source(bytes, SourceFormatHint::JpegXl).unwrap();
        assert_eq!(field.pixel(0, 0).unwrap().red, 1.);
    }
    let oriented = decode_source(
        include_bytes!("fixtures/jpegxl/oriented.jxl"),
        SourceFormatHint::JpegXl,
    )
    .unwrap();
    assert_eq!(
        (oriented.identity().width, oriented.identity().height),
        (2, 3)
    );
    assert_eq!(oriented.pixel(1, 0).unwrap().red, 1.);
    let moved = oriented.pixel(0, 0).unwrap();
    assert_eq!(
        (moved.red, moved.green, moved.blue, moved.alpha),
        (64. / 255., 128. / 255., 192. / 255., 1.)
    );
    let icc = decode_source(
        include_bytes!("fixtures/jpegxl/linear-icc.jxl"),
        SourceFormatHint::JpegXl,
    )
    .unwrap();
    let pixel = icc.pixel(0, 0).unwrap();
    assert!((pixel.red - 188. / 255.).abs() <= 1. / 255., "{pixel:?}");
    assert_eq!(pixel.alpha, 128. / 255.);
    let associated = include_bytes!("fixtures/jpegxl/associated-alpha.jxl");
    assert_eq!(
        is_animated_image(associated, SourceFormatHint::JpegXl)
            .unwrap_err()
            .path(),
        "source.jpegxl.alpha"
    );
    assert_eq!(
        decode_source(associated, SourceFormatHint::JpegXl)
            .unwrap_err()
            .path(),
        "source.jpegxl.alpha"
    );
}

/// Checks every container/codestream truncation boundary without allowing an importer hang.
///
/// # Panics
/// Panics on a source probe exceeding the timeout or accepting the known incomplete box header.
#[test]
fn jpegxl_truncated_container_probe_makes_progress() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let incomplete = b"\0\0\0\x0cJXL \r\n\x87\nX";
        assert!(is_animated_image(incomplete, SourceFormatHint::JpegXl).is_err());
        for bytes in [
            STILL,
            include_bytes!("fixtures/jpegxl/oriented.jxl").as_slice(),
        ] {
            for length in 0..bytes.len() {
                let _ = is_animated_image(&bytes[..length], SourceFormatHint::JpegXl);
            }
        }
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("truncated headers finish within five seconds");
}

/// Checks cropped alpha blending against saved references and hides a zero-duration intermediate.
///
/// # Panics
/// Panics if the native decoder exposes hidden layers as source frames or loses blended pixels.
#[test]
fn jpegxl_animation_preserves_reference_layers_and_zero_duration_blending() {
    let mut source = AnimatedImageSource::new(
        include_bytes!("fixtures/jpegxl/reference-layers.jxl").as_slice(),
        AnimatedImageFormat::JpegXl,
        MediaTools::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.metadata().frame_count, 2);
    assert_eq!(
        source.metadata().duration,
        Some(RationalTime::new(1, 2).unwrap())
    );
    let final_frame = source
        .frame_at(RationalTime::new(1, 4).unwrap(), &|| false)
        .unwrap();
    assert_eq!(final_frame.index, 1);
    let green = final_frame.field.pixel(1, 0).unwrap();
    assert_eq!(
        (green.red, green.green, green.blue, green.alpha),
        (0., 1., 0., 1.)
    );
    let blend = final_frame.field.pixel(2, 0).unwrap();
    assert!(
        (blend.red - 127. / 255.).abs() <= 1. / 255.
            && (blend.blue - 128. / 255.).abs() <= 1. / 255.,
        "{blend:?}"
    );
    assert_eq!(blend.alpha, 1.);
    let first = source.frame_at(RationalTime::default(), &|| false).unwrap();
    assert_eq!(first.field.pixel(1, 0).unwrap().red, 1.);
}
