use toniator_engine::{SourceFormatHint, resolve_source_identity};

const RASTER: &[u8] = include_bytes!("../../../assets/raster-sample.png");
const VECTOR: &[u8] = include_bytes!("../../../assets/vector-sample.svg");
const REDDIT_PNG: &[u8] = include_bytes!("../../../assets/Reddit.png");
const REDDIT_SVG: &[u8] = include_bytes!("../../../assets/Reddit.svg");

#[test]
fn identity_delegates_to_the_png_decoder() {
    let identity = resolve_source_identity(RASTER, SourceFormatHint::Png).unwrap();
    assert_eq!((identity.width, identity.height), (1024, 1024));
    assert_eq!(identity.format, toniator_engine::SourceFormat::Png);
    assert!(identity.svg_text.is_none());
}

#[test]
fn identity_delegates_to_the_svg_decoder_and_preserves_live_text_diagnostic() {
    let identity = resolve_source_identity(VECTOR, SourceFormatHint::Svg).unwrap();
    assert_eq!((identity.width, identity.height), (900, 620));
    let diagnostic = identity
        .svg_text
        .expect("SVG identity must retain text diagnostic");
    assert!(diagnostic.has_live_text_node);
    assert!(!diagnostic.font_policy.is_empty());
}

#[test]
fn identity_preserves_stable_decoder_failures() {
    for (bytes, hint, path) in [
        (&[][..], SourceFormatHint::Png, "source.bytes"),
        (
            b"not a png".as_slice(),
            SourceFormatHint::Png,
            "source.format",
        ),
        (
            b"ignored".as_slice(),
            SourceFormatHint::Unsupported,
            "source.format",
        ),
    ] {
        assert_eq!(
            resolve_source_identity(bytes, hint).unwrap_err().path(),
            path
        );
    }
}

#[test]
fn small_preview_regression_identities_are_decoder_authoritative() {
    let png = resolve_source_identity(REDDIT_PNG, SourceFormatHint::Png).unwrap();
    assert_eq!((png.width, png.height), (128, 128));
    let svg = resolve_source_identity(REDDIT_SVG, SourceFormatHint::Svg).unwrap();
    assert_eq!((svg.width, svg.height), (14, 14));
    assert!(!svg.svg_text.unwrap().has_live_text_node);
}
