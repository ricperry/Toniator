//! Focused Stage 20Q current-format persistence integration coverage.
//!
//! The current Stage 20O persistence target owns its private ZIP rewrite harness for v5 delta,
//! malformed-tag, foreign-output, obsolete-version, and derived-state rejection fault cases.

use std::fs;

use toniator_domain::{
    CoveragePolicy, PatternDefinitionDraft, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternStructureRecipe, PresetMetadata, PresetRecord, RegionGeometryResponse,
    RegionSamplingStrategy,
};
use toniator_io::{PRESET_FORMAT_VERSION, load_preset, save_preset};

/// Proves current preset persistence retains treatment and sampling tags through one v3 round trip.
#[test]
fn stage20q_preset_v3_round_trip_retains_typed_region_intent_only() {
    let mut recipe = PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: "persisted regions".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
        },
    ));
    recipe.output_settings[0].response =
        PatternGeometryResponse::Regions(RegionGeometryResponse::ConstantGap {
            sampling: RegionSamplingStrategy::AreaAverage,
            minimum_gap: -2.0,
            maximum_gap: 3.0,
        });
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20q-io".into(),
            name: "Stage 20Q IO".into(),
            category: "test".into(),
            description: "typed region persistence".into(),
            thumbnail: None,
        },
        recipe,
    };
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20q-io-{}.preset.json",
        std::process::id()
    ));
    save_preset(&path, &preset).expect("current v3 preset saves");
    let bytes = fs::read_to_string(&path).expect("current v3 preset reads");
    assert!(bytes.contains(&format!(
        "\"preset_format_version\": {}",
        PRESET_FORMAT_VERSION
    )));
    assert!(bytes.contains("\"treatment\": \"constant_gap\""));
    assert!(bytes.contains("\"sampling\": \"area_average\""));
    assert!(!bytes.contains("treated_regions"));
    assert_eq!(load_preset(&path).expect("current v3 preset loads"), preset);
    fs::remove_file(path).expect("test-only preset removes");
}
