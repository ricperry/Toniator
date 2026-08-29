//! Explicit current-registry structural regression for the adopted Stage 20S icon subset.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentCommand, DocumentHistory, DocumentSession,
    HalftoneChannelModel, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationRequest, ResolvedSource, SourceFormatHint, evaluate, resolve_source_identity,
    write_svg,
};
use toniator_patterns::PresetRegistry;

/// Builds one source-assigned default RGB history whose ordinary channels receive one preset independently.
///
/// # Panics
///
/// Panics when the fixed icon document/session/source assignment is invalid, or when the default
/// topology is no longer exactly the three authoritative RGB channels.
fn icon_history(source_id: SourceReferenceId) -> (DocumentHistory, [ChannelId; 3]) {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .expect("icon document validates");
    assert_eq!(document.channel_model(), Some(HalftoneChannelModel::Rgb));
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("icon session"));
    history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(source_id),
        })
        .expect("icon source assignment succeeds");
    let channels = history
        .document()
        .channel_topology()
        .expect("default RGB topology exists")
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    assert_eq!(channels, vec![ChannelId(1), ChannelId(2), ChannelId(3)]);
    (
        history,
        channels.try_into().expect("exact RGB channel count"),
    )
}

/// Freezes the historic Stage 20S gallery density without changing current new-document defaults.
///
/// The original stored icon subset and the accepted Curve Motif validation use a density of 10.0.
/// This generator-only command therefore preserves that presentation cadence while retaining the
/// document's existing aspect and leaves no product document state behind outside its private
/// icon history.
///
/// # Panics
///
/// Panics if the document's current pattern settings cannot accept the historic density command.
fn freeze_stage20s_icon_density(history: &mut DocumentHistory) {
    let base = history.document().pattern_settings().clone();
    let mut settings = base.clone();
    settings.density.density = 10.0;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
        .expect("historic Stage 20S icon density applies");
}

/// Adds the icon-only black presentation rectangle before modeled RGB channel groups.
///
/// The returned SVG preserves canonical metadata and geometry bytes while keeping ordinary SVG
/// export transparent; failure to find the canonical canvas-group boundary is a generator defect.
///
/// # Panics
///
/// Panics when canonical SVG serialization no longer contains exactly one expected canvas group.
fn add_rgb_icon_background(svg: String, title: &str) -> String {
    let canvas = "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">\n";
    assert_eq!(
        svg.matches(canvas).count(),
        1,
        "canonical RGB canvas is unique"
    );
    let svg = svg.replacen(
        canvas,
        &format!(
            "{canvas}<rect id=\"icon-background\" x=\"0\" y=\"0\" width=\"100\" height=\"100\" fill=\"#000000\"/>\n"
        ),
        1,
    );
    svg.replacen(
        "<title>Toniator RGB halftone</title>",
        &format!("<title>{title}</title>"),
        1,
    )
}

/// Checks one adopted stored icon against the current registry and production SVG decoder.
///
/// The stored assets are the authority for the adopted 16 historical icons, so this helper never
/// regenerates or compares their recipe bytes. It verifies only their stable ID, 100×100 source
/// identity, and icon-only black rectangle ordering before the first ordinary RGB channel.
///
/// # Panics
///
/// Panics when the asset cannot be read or UTF-8 decoded, fails production SVG decoding, or no
/// longer satisfies the adopted icon dimensions/background/channel-order invariants.
fn assert_stored_icon_structure(path: &Path, preset_id: &str) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{preset_id} icon reads: {error}"));
    let identity = resolve_source_identity(&bytes, SourceFormatHint::Svg)
        .unwrap_or_else(|error| panic!("{preset_id} icon must parse: {error:?}"));
    assert_eq!(
        (identity.width, identity.height),
        (100, 100),
        "{preset_id} icon retains intrinsic dimensions"
    );
    let text = std::str::from_utf8(&bytes).expect("stored icon is UTF-8");
    assert_eq!(text.matches("id=\"icon-background\"").count(), 1);
    assert!(
        text.contains("<rect")
            && text.contains("id=\"icon-background\"")
            && text.contains("fill=\"#000000\"")
    );
    let background_position = text
        .find("id=\"icon-background\"")
        .expect("stored icon retains its presentation background");
    let first_channel_position = text
        .find("id=\"channel-1\"")
        .expect("stored icon retains its first ordinary RGB channel");
    assert!(
        background_position < first_channel_position,
        "{preset_id} icon background precedes ordinary RGB channel geometry"
    );
}

/// Rasterizes one actual stored SVG with Inkscape for non-authoritative visual evidence.
///
/// The external renderer consumes the supplied SVG bytes without recomputing canonical geometry.
/// This writer-only helper panics when Inkscape cannot produce the requested bounded PNG, keeping
/// artifact failure visible rather than silently substituting an in-process raster path.
///
/// # Panics
///
/// Panics when the Inkscape executable cannot start or returns a non-success exit status.
fn rasterize_stored_svg_with_inkscape(input: &Path, output: &Path, width: u32, height: u32) {
    let status = Command::new("inkscape")
        .arg(input)
        .arg("--export-type=png")
        .arg(format!("--export-filename={}", output.display()))
        .arg(format!("--export-width={width}"))
        .arg(format!("--export-height={height}"))
        .status()
        .expect("Inkscape is required for stored icon evidence");
    assert!(status.success(), "Inkscape rasterizes stored icon evidence");
}

/// Writes an SVG contact source that embeds the actual stored built-in assets in registry order.
///
/// This validation-only source contains file URI references to the adopted icon bytes and never
/// regenerates scenes. The fixed five-column grid has enough 120px rows for all 17 entries; text
/// labels are informative only and remain subject to the documented system-font caveat.
///
/// # Panics
///
/// Panics when a registry-listed stored asset is absent or cannot canonicalize, or when the
/// validation-only contact source cannot be written.
fn write_stored_icon_contact_sheet(
    output: &Path,
    registry: &PresetRegistry,
    source: &Path,
) -> PathBuf {
    let columns = 5_usize;
    let cell_width = 100_usize;
    let cell_height = 120_usize;
    let rows = registry.entries().len().div_ceil(columns);
    let mut sheet = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#1e1e1e\"/>\n",
        columns * cell_width,
        rows * cell_height,
        columns * cell_width,
        rows * cell_height,
    );
    for (index, record) in registry.entries().iter().enumerate() {
        let x = (index % columns) * cell_width;
        let y = (index / columns) * cell_height;
        let path = source.join(format!("{}.svg", record.metadata.id));
        assert!(path.is_file(), "{} stored icon exists", record.metadata.id);
        let path = path.canonicalize().expect("stored icon path canonicalizes");
        sheet.push_str(&format!(
            "<image href=\"file://{}\" x=\"{x}\" y=\"{y}\" width=\"100\" height=\"100\"/>\n<text x=\"{x}\" y=\"{}\" fill=\"#ffffff\" font-family=\"sans\" font-size=\"6\">{}</text>\n",
            path.display(),
            y + 110,
            record.metadata.id,
        ));
    }
    sheet.push_str("</svg>\n");
    let path = output.join("stage20s-17-icon-contact-sheet.svg");
    fs::write(&path, sheet).expect("stored-icon contact sheet source writes");
    path
}

/// Validates the stored synthetic source and exact current 17-icon built-in inventory without writing files.
///
/// Every SVG must parse through the production source decoder at 100×100, retain one icon-only
/// black rectangle before its ordinary channel groups, and correspond to one current catalog ID.
/// Curve Motif is stored by the same generator/settings as every other built-in; the retired
/// region-plus-mark debug tool is explicitly excluded.
///
/// # Panics
///
/// Panics when the synthetic source or any stored icon cannot be read/decoded, the current
/// registry/inventory differs from the adopted 17-card contract, or an icon violates its
/// presentation-structure invariants.
#[test]
fn stored_stage20s_preset_icons_are_exact_and_parseable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_path = root.join("assets/stage20s-preset-icon-source.svg");
    let source_bytes = fs::read(&source_path).expect("gradient icon source reads");
    let source_identity = resolve_source_identity(&source_bytes, SourceFormatHint::Svg)
        .expect("gradient source parses through production SVG decoding");
    assert_eq!((source_identity.width, source_identity.height), (100, 100));
    let source_text = std::str::from_utf8(&source_bytes).expect("gradient source is UTF-8");
    assert!(source_text.contains("<linearGradient id=\"black-to-white\""));
    assert!(source_text.contains("<stop offset=\"0\" stop-color=\"#000000\"/>"));
    assert!(source_text.contains("<stop offset=\"1\" stop-color=\"#ffffff\"/>"));

    let registry = PresetRegistry::bundled();
    assert_eq!(registry.version(), 3);
    assert_eq!(registry.entries().len(), 17);
    assert!(registry.find("curve-motif-rows").is_some());
    assert!(registry.find("regions-plus-marks").is_none());
    let output = root.join("assets/stage20s-preset-icons");
    let expected = registry
        .entries()
        .iter()
        .map(|record| format!("{}.svg", record.metadata.id))
        .collect::<BTreeSet<_>>();
    let actual = fs::read_dir(&output)
        .expect("temporary icon directory reads")
        .map(|entry| {
            entry
                .expect("icon directory entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "stored icons are the exact current catalog"
    );

    for record in registry.entries().iter() {
        let path = output.join(format!("{}.svg", record.metadata.id));
        assert_stored_icon_structure(&path, &record.metadata.id);
    }
}

/// Generates the missing Curve Motif SVG and visual evidence while preserving the adopted icon subset.
///
/// This ignored writer is run only on explicit request because it writes the new product asset and
/// correction evidence. It validates the existing 16 stored icons structurally, then materializes
/// only Curve Motif through the historic 10.0-density icon document, ordinary authoritative
/// evaluation, SVG serialization, and icon-background helper. It never regenerates, rewrites, or
/// byte-compares the adopted 16 historical assets; visual evidence is rasterized from the actual
/// stored SVG files, and the retired region-plus-mark debug tool remains absent.
///
/// # Panics
///
/// Panics when the source, registry, adopted stored assets, recipe materialization, historic
/// density command, authoritative evaluation, SVG serialization, output writes, or Inkscape
/// evidence rasterization fails; it also panics on any 17-icon inventory/invariant mismatch.
#[test]
#[ignore = "writes the explicitly requested Stage 20S built-in icon assets and correction evidence"]
fn generate_stage20s_preset_icons() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_path = root.join("assets/stage20s-preset-icon-source.svg");
    let output = root.join("assets/stage20s-preset-icons");
    let validation = root.join("target/validation/stage21b-gate2/curve-motif-icon-correction");
    let source_bytes = fs::read(&source_path).expect("gradient icon source reads");
    let source_text = std::str::from_utf8(&source_bytes).expect("gradient source is UTF-8");
    assert!(source_text.contains("<linearGradient id=\"black-to-white\""));
    assert!(source_text.contains("width=\"100\" height=\"100\" viewBox=\"0 0 100 100\""));

    let registry = PresetRegistry::bundled();
    assert_eq!(registry.version(), 3);
    assert_eq!(registry.entries().len(), 17);
    assert!(registry.find("curve-motif-rows").is_some());
    assert!(registry.find("regions-plus-marks").is_none());
    fs::create_dir_all(&output).expect("temporary icon directory creates");
    fs::create_dir_all(&validation).expect("Curve Motif correction directory creates");

    let curve_id = "curve-motif-rows";
    for record in registry.entries().iter() {
        if record.metadata.id != curve_id {
            assert_stored_icon_structure(
                &output.join(format!("{}.svg", record.metadata.id)),
                &record.metadata.id,
            );
        }
    }

    let source_id = SourceReferenceId::new(format!("stage20s-icon-{curve_id}"))
        .expect("Curve Motif icon source ID validates");
    let source = ResolvedSource::new(source_id.clone(), source_bytes, SourceFormatHint::Svg)
        .expect("gradient source resolves");
    let (mut history, channels) = icon_history(source_id);
    for channel_id in channels {
        let result = registry
            .apply_to_selected(&mut history, channel_id, curve_id)
            .expect("Curve Motif recipe materializes for icon channel");
        assert_eq!(result.affected_channels, vec![channel_id]);
    }
    freeze_stage20s_icon_density(&mut history);
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .unwrap_or_else(|error| panic!("{curve_id} icon evaluation failed: {error:?}"));
    assert_eq!(result.scene().canvas().width, 100.0);
    assert_eq!(result.scene().canvas().height, 100.0);
    assert_eq!(result.scene().model(), Some(HalftoneChannelModel::Rgb));
    let icon = add_rgb_icon_background(write_svg(result.scene()), "Curve Motif");
    assert!(icon.contains("width=\"100\" height=\"100\" viewBox=\"0 0 100 100\""));
    assert_eq!(icon.matches("id=\"icon-background\"").count(), 1);
    let curve_path = output.join("curve-motif-rows.svg");
    fs::write(&curve_path, &icon).expect("Curve Motif icon writes");
    assert_stored_icon_structure(&curve_path, curve_id);

    let raw_curve = validation.join("curve-motif-rows.raw.svg");
    fs::write(&raw_curve, &icon).expect("raw Curve Motif SVG evidence writes");
    rasterize_stored_svg_with_inkscape(
        &raw_curve,
        &validation.join("curve-motif-rows-100x100-inkscape-raster.png"),
        100,
        100,
    );
    let contact_sheet = write_stored_icon_contact_sheet(&validation, &registry, &output);
    rasterize_stored_svg_with_inkscape(
        &contact_sheet,
        &validation.join("stage20s-17-icon-contact-sheet-500x480-inkscape-raster.png"),
        500,
        480,
    );

    let expected = registry
        .entries()
        .iter()
        .map(|record| format!("{}.svg", record.metadata.id))
        .collect::<BTreeSet<_>>();
    let actual = fs::read_dir(&output)
        .expect("temporary icon directory reads")
        .map(|entry| {
            entry
                .expect("icon directory entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "icon directory is the exact 17-card built-in set"
    );
}
