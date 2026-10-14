//! Current-format independent artifact persistence and shipped archive reopen witnesses.
use std::{fs, path::PathBuf};
use toniator_domain::*;
use toniator_io::*;

/// Locates only this stage's derived artifacts without writing immutable input archives.
///
/// # Panics
/// Panics if the stage output directory cannot be created.
fn stage() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-animation-response-20261013/io");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Compares every numeric companion field bit-for-bit and retains measured provenance.
///
/// # Panics
/// Panics when any persisted numeric companion or profile/binding changes.
fn exact(a: &SavedCalibration, b: &SavedCalibration) {
    assert_eq!(a.profile(), b.profile());
    assert_eq!(a.binding(), b.binding());
    assert_eq!(
        a.binding().density_aspect().to_bits(),
        b.binding().density_aspect().to_bits()
    );
    assert_eq!(
        a.binding().shape_rotation_degrees().to_bits(),
        b.binding().shape_rotation_degrees().to_bits()
    );
    for (a, b) in a.raw().iter().zip(b.raw()) {
        assert_eq!(a.input.to_bits(), b.input.to_bits());
        assert_eq!(a.coverage.to_bits(), b.coverage.to_bits());
    }
    assert_eq!(a.raw().len(), b.raw().len());
    match (a.uniform(), b.uniform()) {
        (Some(a), Some(b)) => {
            assert_eq!(
                a.radius_per_nominal_spacing().to_bits(),
                b.radius_per_nominal_spacing().to_bits()
            );
            assert_eq!(a.inverse().len(), b.inverse().len());
            for (a, b) in a.inverse().iter().zip(b.inverse()) {
                assert_eq!(a.input.to_bits(), b.input.to_bits());
                assert_eq!(a.coverage.to_bits(), b.coverage.to_bits());
            }
        }
        (None, None) => {}
        _ => panic!("Uniform presence differs"),
    }
    assert_eq!(a.variation(), b.variation());
}

/// Preserves near-equal equal-binding assignments in current projects and source-free Presets.
///
/// # Panics
/// Panics on payload normalization, recipient fanout, schema changes, or any persistence error.
#[test]
fn independent_numeric_assignments_roundtrip_exactly() {
    let directory = stage();
    let id = SourceReferenceId::new("ton018-independent").unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 128.0,
            height: 128.0,
        },
        SourceReference::Assigned(id.clone()),
    )
    .unwrap();
    let saved = document
        .saved_calibration_for(ChannelId(1))
        .unwrap()
        .unwrap();
    let mut raw = saved.raw().to_vec();
    raw[1].coverage += 1e-13;
    let distinct = SavedCalibration::with_profile(
        saved.profile().into(),
        saved.binding().clone(),
        raw,
        Some(
            UniformCalibration::new(
                0.3750000000001,
                vec![
                    CalibrationKnot {
                        input: 0.0,
                        coverage: 0.0,
                    },
                    CalibrationKnot {
                        input: 0.3125000000001,
                        coverage: 0.5,
                    },
                    CalibrationKnot {
                        input: 1.0,
                        coverage: 1.0,
                    },
                ],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let document = document
        .with_saved_calibration(ChannelId(2), distinct)
        .unwrap();
    let document = document
        .edit_channel_start_configuration(&[(
            PropertyTarget::Channel(ChannelId(2)),
            PropertyFieldId::ShapeRotationDegrees,
            45.0,
        )])
        .unwrap()
        .bind(&document)
        .unwrap();
    let sources=SourceBundle::new([EmbeddedSource::new(id,EmbeddedSourceFormat::Svg,b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"128\" height=\"128\"><rect width=\"128\" height=\"128\" fill=\"#777\"/></svg>".to_vec(),Some("independent.svg".into())).unwrap()]).unwrap();
    let project = directory.join("independent.toniator");
    save(&project, &document, &sources).unwrap();
    let reopened = load(&project).unwrap();
    assert_eq!(reopened.document(), &document);
    let preset = directory.join("independent.toniator-preset");
    save_document_preset(
        &preset,
        &DocumentConfiguration::capture(&document),
        &capture_document_preset_destination(&preset).unwrap(),
    )
    .unwrap();
    let rebound = load_document_preset(&preset, &document)
        .unwrap()
        .bind(&document)
        .unwrap();
    for result in [reopened.document(), &rebound] {
        for old in document.saved_calibrations() {
            exact(
                &old.artifact,
                result
                    .saved_calibration_for(old.channel_id)
                    .unwrap()
                    .unwrap(),
            );
        }
        exact(
            document.baseline_saved_calibration().unwrap().unwrap(),
            result.baseline_saved_calibration().unwrap().unwrap(),
        );
    }
}

/// Reopens all shipped current Document Presets and the retained 64-family current project matrix.
///
/// # Panics
/// Panics on archive count drift or a current-format validation failure; input files stay read-only.
/// This opt-in evidence check requires retained TON-018 current family matrix validation
/// artifacts; those files are not shipped source fixtures.
#[test]
#[ignore = "requires retained TON-018 current family matrix validation artifacts"]
fn current_archives_reopen_without_schema_or_payload_changes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let destination = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut presets = 0;
    for entry in fs::read_dir(root.join("assets/document-presets")).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|value| value == "toniator-preset")
        {
            load_document_preset(&path, &destination)
                .unwrap()
                .bind(&destination)
                .unwrap()
                .require_saved_calibration_authority()
                .unwrap();
            presets += 1;
        }
    }
    let mut projects = 0;
    for entry in fs::read_dir(
        root.join("target/validation/ton018-family-regression-20261013/native-projects"),
    )
    .unwrap()
    {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|value| value == "toniator") {
            load(&path).unwrap().document().validate().unwrap();
            projects += 1;
        }
    }
    assert_eq!(presets, 14);
    assert_eq!(projects, 64);
    fs::write(stage().join("archive-reopen.txt"),format!("{presets} current Document Presets; {projects} current projects; read-only input loads\n")).unwrap();
}
