//! G2a exact-pair evidence against the current final-output evaluator.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use sha2::{Digest, Sha256};
use toniator_domain::{
    CanvasSpec, Document, DocumentCommand, DocumentHistory, DocumentSession, SourceReference,
};
use toniator_engine::{
    EvaluationLimits, MediaTools, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    RasterSurface, encode_png, evaluate_cancellable_with_limits, frame_evaluation_request,
    import_source_media, open_source_media,
    print_preflight::{
        PreflightLimits, PreflightRasterOutcome, PreflightRasterReport, PreflightSelection,
        preflight_current_frame_with_raster,
    },
};
use toniator_io::{SourceBundle, load};

/// Finds one workspace-root path without mutating any project fixture.
fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Evaluates the ordinary final-output raster for one exact selection and backing.
fn normal_output(
    session: &DocumentSession,
    sources: &SourceBundle,
    frame: u64,
    target: OutputRasterTarget,
    aa: RasterAntialiasing,
    background: RasterBackground,
) -> RasterSurface {
    let cancelled = AtomicBool::new(false);
    let mut media = open_source_media(sources, MediaTools::default(), &|| false).unwrap();
    let request = frame_evaluation_request(session, &mut media, frame, &|| false)
        .unwrap()
        .for_output(background, Some(target), aa);
    evaluate_cancellable_with_limits(request, EvaluationLimits::default(), &cancelled)
        .unwrap()
        .raster()
        .clone()
}

/// Obtains the inseparable pair from one actual transparent frame evaluation.
fn pair(
    session: &DocumentSession,
    sources: &SourceBundle,
    target: OutputRasterTarget,
    aa: RasterAntialiasing,
    background: RasterBackground,
) -> Box<PreflightRasterReport> {
    let selection = PreflightSelection {
        frame: session.document().project_timing().frame_range().start(),
        target,
        antialiasing: aa,
        selected_background: background,
        settings: session.document().print_preparation().clone(),
    };
    let cancelled = AtomicBool::new(false);
    let mut media = open_source_media(sources, MediaTools::default(), &|| false).unwrap();
    let outcome = preflight_current_frame_with_raster(
        session,
        &mut media,
        &selection,
        EvaluationLimits::default(),
        PreflightLimits::default(),
        &cancelled,
    )
    .unwrap();
    let PreflightRasterOutcome::Complete(pair) = outcome else {
        panic!("expected completed exact pair")
    };
    assert!(
        pair.report()
            .identity
            .is_current(session, &mut media, &selection, &cancelled)
            .unwrap()
    );
    pair
}

/// Verifies report inventory/hash and exact transparent PNG bytes for one immutable pair.
fn assert_pair_exact(pair: &PreflightRasterReport, expected: &RasterSurface) {
    assert_eq!(pair.raster(), expected);
    let pixels = pair.raster().pixels();
    let mut histogram = vec![0_u64; 256];
    let mut alpha = Sha256::new();
    for pixel in pixels.chunks_exact(4) {
        histogram[usize::from(pixel[3])] += 1;
        alpha.update([pixel[3]]);
    }
    assert_eq!(pair.report().alpha_histogram, histogram);
    assert_eq!(
        pair.report().identity.alpha_sha256(),
        format!("{:x}", alpha.finalize())
    );
    assert_eq!(pair.report().identity.target().width(), expected.width());
    assert_eq!(pair.report().identity.target().height(), expected.height());
    let encoded = encode_png(pair.raster()).unwrap();
    assert_eq!(
        image::load_from_memory(&encoded)
            .unwrap()
            .to_rgba8()
            .as_raw(),
        pixels
    );
}

/// Tests native and 2× final targets with both AA modes against existing transparent rendering.
///
/// # Panics
/// Panics if report alpha, RGBA, target, AA, or encoded transparent PNG diverges.
#[test]
fn exact_pair_matches_existing_transparent_output_matrix() {
    let loaded = load(&workspace_path(
        "crates/toniator-io/tests/fixtures/garment-schema10-clean-fade.toniator",
    ))
    .unwrap();
    let session = DocumentSession::new(loaded.document().clone()).unwrap();
    for (width, height) in [(256, 96), (512, 192)] {
        let target = OutputRasterTarget::new(width, height).unwrap();
        for aa in [RasterAntialiasing::On, RasterAntialiasing::Off] {
            for backing in [
                RasterBackground::Transparent,
                RasterBackground::OpaqueWhite,
                RasterBackground::OpaqueBlack,
            ] {
                let pair = pair(&session, loaded.sources(), target, aa, backing);
                let transparent = normal_output(
                    &session,
                    loaded.sources(),
                    0,
                    target,
                    aa,
                    RasterBackground::Transparent,
                );
                assert_pair_exact(&pair, &transparent);
                assert_eq!(pair.report().identity.antialiasing(), aa);
                assert_eq!(pair.report().identity.selected_background(), backing);
                if backing != RasterBackground::Transparent {
                    let backed = normal_output(&session, loaded.sources(), 0, target, aa, backing);
                    assert_eq!(backed.width(), pair.raster().width());
                    assert_eq!(backed.height(), pair.raster().height());
                    assert!(backed.pixels().chunks_exact(4).all(|pixel| pixel[3] == 255));
                }
            }
        }
    }
}

/// Imports one immutable source at natural dimensions through normal source authority.
fn imported_asset(path: &Path) -> (DocumentSession, SourceBundle, OutputRasterTarget) {
    let imported =
        import_source_media(&[path.to_owned()], None, MediaTools::default(), &|| false).unwrap();
    let target =
        OutputRasterTarget::new(imported.metadata.width, imported.metadata.height).unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: f64::from(imported.metadata.width),
            height: f64::from(imported.metadata.height),
        },
        SourceReference::Assigned(imported.source_id),
    )
    .unwrap();
    (
        DocumentSession::new(document).unwrap(),
        imported.sources,
        target,
    )
}

/// Exercises both immutable artwork baselines at native size and writes raw PNG evidence.
///
/// White/black files come from the existing backed renderer independently of
/// the transparent pair; their pixels are not an 8-bit RGBA blend surrogate.
/// SVG live text is decoded normally but is not an exact text-pixel golden.
///
/// # Panics
/// Panics if either input changes, native report/pixels diverge, or PNG decode differs.
#[test]
fn immutable_assets_native_pair_and_backed_png_artifacts() {
    let output = workspace_path("target/validation/garment-g2a-20261004/native-png");
    fs::create_dir_all(&output).unwrap();
    for (name, expected_hash) in [
        (
            "raster-sample.png",
            "324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2",
        ),
        (
            "vector-sample.svg",
            "42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e",
        ),
    ] {
        let input = workspace_path(&format!("assets/{name}"));
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&input).unwrap())),
            expected_hash
        );
        let (session, sources, target) = imported_asset(&input);
        let frame = session.document().project_timing().frame_range().start();
        let aa = RasterAntialiasing::On;
        let pair = pair(
            &session,
            &sources,
            target,
            aa,
            RasterBackground::Transparent,
        );
        let transparent = normal_output(
            &session,
            &sources,
            frame,
            target,
            aa,
            RasterBackground::Transparent,
        );
        assert_pair_exact(&pair, &transparent);
        let stem = name.split('.').next().unwrap();
        fs::write(
            output.join(format!("{stem}-transparent.png")),
            encode_png(pair.raster()).unwrap(),
        )
        .unwrap();
        for (label, backing) in [
            ("white", RasterBackground::OpaqueWhite),
            ("black", RasterBackground::OpaqueBlack),
        ] {
            let backed = normal_output(&session, &sources, frame, target, aa, backing);
            assert!(backed.pixels().chunks_exact(4).all(|pixel| pixel[3] == 255));
            let encoded = encode_png(&backed).unwrap();
            assert_eq!(
                image::load_from_memory(&encoded)
                    .unwrap()
                    .to_rgba8()
                    .as_raw(),
                backed.pixels()
            );
            fs::write(output.join(format!("{stem}-{label}.png")), encoded).unwrap();
        }
    }
}

/// Proves a faint channel contribution overlapping substantial final coverage is not isolated.
///
/// # Panics
/// Panics if analysis classifies channel-local faintness instead of composite alpha.
#[test]
fn faint_channel_overlap_uses_final_composite_alpha() {
    let source_path = workspace_path("target/validation/garment-g2a-20261004/overlap-source.png");
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        64,
        64,
        image::Rgba([128, 128, 128, 255]),
    ))
    .save(&source_path)
    .unwrap();
    let (session, sources, _) = imported_asset(&source_path);
    let mut history = DocumentHistory::new(session);
    let channels = history.document().channel_topology().unwrap().channels();
    assert!(channels.len() >= 2, "fixture needs overlapping channels");
    let faint = channels[0].id;
    let strong = channels[1].id;
    let remaining: Vec<_> = channels.iter().skip(2).map(|channel| channel.id).collect();
    history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: faint,
            opacity: 0.25,
        })
        .unwrap();
    for id in remaining {
        history
            .apply(&DocumentCommand::SetVisibility {
                channel_id: id,
                visible: false,
            })
            .unwrap();
    }
    let full = history.session().clone();
    let mut strong_only = DocumentHistory::new(full.clone());
    strong_only
        .apply(&DocumentCommand::SetVisibility {
            channel_id: faint,
            visible: false,
        })
        .unwrap();
    let mut faint_only = DocumentHistory::new(full.clone());
    faint_only
        .apply(&DocumentCommand::SetVisibility {
            channel_id: strong,
            visible: false,
        })
        .unwrap();
    let target = OutputRasterTarget::new(256, 256).unwrap();
    let aa = RasterAntialiasing::On;
    let full_pair = pair(&full, &sources, target, aa, RasterBackground::Transparent);
    let strong_raster = normal_output(
        strong_only.session(),
        &sources,
        0,
        target,
        aa,
        RasterBackground::Transparent,
    );
    let faint_raster = normal_output(
        faint_only.session(),
        &sources,
        0,
        target,
        aa,
        RasterBackground::Transparent,
    );
    let overlap = full_pair
        .raster()
        .pixels()
        .chunks_exact(4)
        .zip(strong_raster.pixels().chunks_exact(4))
        .zip(faint_raster.pixels().chunks_exact(4))
        .enumerate()
        .find(|(_, ((full, strong), faint))| {
            faint[3] > 0 && faint[3] < 128 && strong[3] >= 128 && full[3] >= 128
        })
        .map(|(index, _)| index)
        .expect("a faint contribution overlaps substantial other-channel coverage");
    let x = u32::try_from(overlap % usize::try_from(target.width()).unwrap()).unwrap();
    let y = u32::try_from(overlap / usize::try_from(target.width()).unwrap()).unwrap();
    let component = full_pair
        .report()
        .support_components
        .iter()
        .find(|component| {
            component
                .runs
                .iter()
                .any(|run| run.y == y && run.x0 <= x && x < run.x1_exclusive)
        })
        .expect("overlap belongs to one final support component");
    assert!(component.core_pixels > 0);
}
