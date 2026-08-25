//! Stage 20P current document and preset persistence witnesses.

use std::{fs, io::Read};

use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    GeneralizedSiteProductDraft, GuideDimensionDraft, MarkOrientationDraft,
    PatternDefinitionRecipe, PatternGeometryResponse, PatternStructureRecipe, PresetMetadata,
    PresetRecord, RegionGeometryResponse, RegionSourceIntent, SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};

/// Proves the complete Guide Faces recipe round-trips through current preset format v3 without derived state.
#[test]
fn guide_face_recipe_round_trips_current_preset_format() {
    let structure = PatternStructureRecipe::GeneralizedStraightGuides {
        name: "guide faces".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 60.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 120.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1],
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    };
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stage20p".into(),
            name: "Guide Faces".into(),
            category: "test".into(),
            description: "round trip".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::guide_faces(structure, vec![0, 1, 2]),
    };
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20p-{}-{}.json",
        std::process::id(),
        20
    ));
    save_preset(&path, &preset).expect("save current preset");
    assert_eq!(load_preset(&path).expect("load current preset"), preset);
    fs::remove_file(path).expect("remove temp preset");
}

/// Builds one current v5 document with a keyed three-dimension Guide Faces bundle.
fn guide_face_document(source_id: SourceReferenceId) -> Document {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 120.0,
        },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("document session"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .expect("selected definition")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: PatternDefinitionRecipe::guide_faces(guide_face_structure(), vec![0, 1, 2]),
        })
        .expect("Guide Faces recipe installs");
    history.document().clone()
}

/// Builds the reusable ID-free three-guide structural recipe used by document persistence.
fn guide_face_structure() -> PatternStructureRecipe {
    PatternStructureRecipe::GeneralizedStraightGuides {
        name: "persisted guide faces".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 60.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 120.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product: GeneralizedSiteProductDraft::Intersections {
            dimension_indices: vec![0, 1],
            merge_epsilon: 0.0,
        },
        orientation: MarkOrientationDraft::Fixed,
    }
}

/// Proves a v5 Guide Faces document persists keyed settings only and reconstructs no derived evaluation state.
#[test]
fn guide_face_document_v5_round_trips_without_derived_state() {
    let source_id = SourceReferenceId::new("stage20p-persistence").expect("source ID");
    let document = guide_face_document(source_id.clone());
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Png,
        vec![137, 80, 78, 71],
        Some("minimal-payload.png".into()),
    )
    .expect("embedded source")])
    .expect("source bundle");
    let path = std::env::temp_dir().join(format!(
        "toniator-stage20p-doc-{}.toniator",
        std::process::id()
    ));
    save(&path, &document, &sources).expect("v5 document saves");
    let loaded = load(&path).expect("v5 document loads");
    assert_eq!(loaded.versions().document(), 5);
    assert_eq!(loaded.document(), &document);
    let bundle = loaded
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == loaded.document().pattern_settings().definition_id)
        .expect("selected bundle");
    assert!(matches!(
        &bundle.definition.output_layers[0],
        toniator_domain::PatternOutputLayer::Regions {
            source: RegionSourceIntent::GuideFaces { dimensions, .. },
            ..
        } if dimensions.len() == 3
    ));
    assert!(matches!(
        bundle.output_settings[0].response,
        PatternGeometryResponse::Regions(RegionGeometryResponse::Full)
    ));
    let file = fs::File::open(&path).expect("archive opens");
    let mut archive = zip::ZipArchive::new(file).expect("current archive");
    let mut document_json = String::new();
    archive
        .by_name("document.json")
        .expect("document entry")
        .read_to_string(&mut document_json)
        .expect("document JSON reads");
    for derived_key in [
        "centroids",
        "diagnostics",
        "guide_face_limits",
        "cache",
        "scheduler",
        "canonical_regions",
    ] {
        assert!(
            !document_json.contains(derived_key),
            "v5 document excludes derived {derived_key}",
        );
    }
    fs::remove_file(path).expect("temporary document removes");
}
