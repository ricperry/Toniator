use crate::model::{
    AlternateTileTransform, CubicCurveSegment, CurveLayout, CurvePath, CurvePoint, Document, Ink,
    MotifCoverage, RenderVariant, ValueMode, WebCurveChannel, WebCurveChannels, WebCurveSettings,
    WebShape, WebShapeChannel, WebShapeChannels, WebShapeDeltas, WebShapeSettings, parse_hex_color,
};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

const WEB_WIDTH: u32 = 900;
const WEB_HEIGHT: u32 = 600;
const WEB_CELLS: f64 = 90.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTreatment {
    pub render: RenderVariant,
    pub native_settings: Option<crate::model::Settings>,
}

pub fn parse_treatment(bytes: &[u8], source_dimensions: (u32, u32)) -> Result<ParsedTreatment> {
    let header: PresetHeader =
        serde_json::from_slice(bytes).context("Could not read this treatment preset")?;
    if header.version == 2 {
        ensure!(
            header.format == "toniator-preset",
            "This is not a Toniator treatment preset"
        );
        let preset: NativePresetV2 =
            serde_json::from_slice(bytes).context("Could not read this native treatment preset")?;
        validate_render_variant(&preset.render)?;
        return Ok(ParsedTreatment {
            render: preset.render,
            native_settings: Some(preset.settings.sanitized()),
        });
    }
    Ok(ParsedTreatment {
        render: parse_tntr(bytes, source_dimensions)?,
        native_settings: None,
    })
}

pub fn parse_tntr(bytes: &[u8], source_dimensions: (u32, u32)) -> Result<RenderVariant> {
    let header: PresetHeader =
        serde_json::from_slice(bytes).context("Could not read this treatment preset")?;
    ensure!(
        header.format == "toniator-preset",
        "This is not a Toniator treatment preset"
    );
    if header.version == 2 {
        let preset: NativePresetV2 =
            serde_json::from_slice(bytes).context("Could not read this native treatment preset")?;
        let candidate = preset.render;
        validate_render_variant(&candidate)?;
        return Ok(candidate);
    }
    let preset: PresetV1 =
        serde_json::from_slice(bytes).context("Could not read this treatment preset")?;
    ensure!(
        preset.format == "toniator-preset",
        "This is not a Toniator treatment preset"
    );
    ensure!(
        preset.version == 1,
        "Unsupported treatment preset version {}",
        preset.version
    );
    if let Some(render) = preset.native_render.clone() {
        validate_render_variant(&render)?;
        return Ok(render);
    }
    let settings = preset.settings.clone().unwrap_or_default();
    match settings.mark_mode.as_deref().unwrap_or("shape") {
        "shape" => {}
        "curve" => return parse_curve_preset(&preset, &settings, source_dimensions),
        value => bail!("This preset uses unknown treatment mode '{value}'. Nothing was changed."),
    }
    let shared = match settings.geometry_mode.as_deref().unwrap_or("shared") {
        "shared" => true,
        "independent" => false,
        value => bail!("This preset uses unknown geometry '{value}'. Nothing was changed."),
    };
    let shared_shape = if shared {
        parse_shape(settings.shared_preset.as_deref().unwrap_or("circle"))?
    } else {
        WebShape::Circle
    };
    if shared
        && settings
            .shared_path
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty())
    {
        bail!("Custom shape paths are not available in the desktop app yet. Nothing was changed.");
    }

    let output_width = settings.output_width.unwrap_or(WEB_WIDTH);
    let mut output_height = settings.output_height.unwrap_or(WEB_HEIGHT);
    if settings.preserve_aspect.unwrap_or(true) {
        let (source_width, source_height) = source_dimensions;
        ensure!(
            source_width > 0 && source_height > 0,
            "Source artwork has no usable size"
        );
        output_height = ((output_width as f64) / (source_width as f64 / source_height as f64))
            .round()
            .max(1.0) as u32;
    }
    ensure!(
        (32..=4000).contains(&output_width),
        "Preset output width is outside the supported range"
    );
    ensure!(
        (1..=4000).contains(&output_height),
        "Preset output height is outside the supported range"
    );

    let mut channels = WebShapeChannels::default();
    for ink in Ink::ALL {
        let raw = preset
            .channels
            .as_ref()
            .and_then(|channels| channels.get(ink));
        *channels.get_mut(ink) = parse_channel(raw, ink, shared, shared_shape)?;
    }

    let mut result = WebShapeSettings {
        output_width,
        output_height,
        long_edge_cells: bounded(
            settings.long_edge_cells.unwrap_or(WEB_CELLS),
            8.0,
            220.0,
            "cell count",
        )?,
        grid_scale: bounded(
            settings.grid_scale.unwrap_or(92.0),
            10.0,
            160.0,
            "cell fill",
        )?,
        min_mark: bounded(settings.min_mark.unwrap_or(0.0), 0.0, 100.0, "minimum mark")?,
        max_mark: bounded(
            settings.max_mark.unwrap_or(85.0),
            1.0,
            200.0,
            "maximum mark",
        )?,
        value_mode: parse_value_mode(settings.value_mode.as_deref().unwrap_or("cmyk"))?,
        single_channel: parse_ink(settings.single_channel.as_deref().unwrap_or("k"))?,
        use_shared_mark: shared,
        shared_shape,
        channels,
    };
    ensure!(
        result.min_mark <= result.max_mark,
        "Preset mark range is invalid"
    );
    let deltas = preset.cmyk_deltas.unwrap_or_default();
    let threshold_delta = finite(deltas.threshold_delta.unwrap_or(0.0), "threshold delta")? / 100.0;
    result.apply_deltas(WebShapeDeltas {
        rotation_delta: finite(deltas.rotation_delta.unwrap_or(0.0), "rotation delta")?,
        grid_rotation_delta: finite(
            deltas.grid_rotation_delta.unwrap_or(0.0),
            "grid rotation delta",
        )?,
        grid_pivot_x_delta: finite(deltas.grid_pivot_x_delta.unwrap_or(0.0), "pivot delta")?,
        grid_pivot_y_delta: finite(deltas.grid_pivot_y_delta.unwrap_or(0.0), "pivot delta")?,
        scale_multiplier: positive(deltas.scale_multiplier.unwrap_or(1.0), "scale multiplier")?,
        resolution_multiplier: positive(
            deltas.resolution_multiplier.unwrap_or(1.0),
            "resolution multiplier",
        )?,
        threshold_delta,
        max_size_multiplier: positive(
            deltas.max_size_multiplier.unwrap_or(1.0),
            "maximum size multiplier",
        )?,
        opacity_multiplier: positive_or_zero(
            deltas.opacity_multiplier.unwrap_or(1.0),
            "opacity multiplier",
        )?,
        offset_x_delta: finite(deltas.offset_x_delta.unwrap_or(0.0), "phase delta")?,
        offset_y_delta: finite(deltas.offset_y_delta.unwrap_or(0.0), "phase delta")?,
    });
    let candidate = RenderVariant::WebShapeV1 {
        settings: Box::new(result),
    };
    let mut cloned = Document::new(crate::model::SourceArtwork {
        name: "validation".into(),
        media_type: "application/octet-stream".into(),
        bytes: std::sync::Arc::from([1]),
    });
    cloned.render = candidate.clone();
    cloned.validate()?;
    Ok(candidate)
}

pub fn treatment_preset_bytes(name: &str, render: &RenderVariant) -> Result<Vec<u8>> {
    let name = name.trim();
    ensure!(!name.is_empty(), "Treatment name cannot be empty");
    let value = match render {
        RenderVariant::NativeBasicV1 => serde_json::json!({
            "format": "toniator-preset",
            "version": 2,
            "name": name,
            "render": render,
        }),
        RenderVariant::WebShapeV1 { settings } => shape_preset_value(name, settings),
        RenderVariant::WebCurveV1 { settings } => curve_preset_value(name, settings),
    };
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn document_treatment_preset_bytes(name: &str, document: &Document) -> Result<Vec<u8>> {
    if document.render != RenderVariant::NativeBasicV1 {
        return treatment_preset_bytes(name, &document.render);
    }
    let name = name.trim();
    ensure!(!name.is_empty(), "Treatment name cannot be empty");
    let value = serde_json::json!({
        "format": "toniator-preset",
        "version": 2,
        "name": name,
        "settings": document.settings,
        "render": document.render,
    });
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_render_variant(render: &RenderVariant) -> Result<()> {
    let mut document = Document::new(crate::model::SourceArtwork {
        name: "preset-validation".into(),
        media_type: "application/octet-stream".into(),
        bytes: std::sync::Arc::from([1]),
    });
    document.render = render.clone();
    document.validate()
}

fn identity_deltas() -> serde_json::Value {
    serde_json::json!({
        "rotationDelta": 0, "gridRotationDelta": 0,
        "gridPivotXDelta": 0, "gridPivotYDelta": 0,
        "scaleMultiplier": 1, "curveScaleMultiplier": 1,
        "resolutionMultiplier": 1, "outputQualityMultiplier": 1,
        "thresholdDelta": 0, "maxSizeMultiplier": 1, "opacityMultiplier": 1,
        "motifCoverageModeOverride": "", "motifBleedDelta": 0,
        "offsetXDelta": 0, "offsetYDelta": 0,
        "tileCountDelta": 0, "tileSpacingDelta": 0, "tileAngleDelta": 0,
        "tileOffsetDelta": 0, "alternateTileTransformOverride": "",
        "stackCountDelta": 0, "stackSpacingDelta": 0, "stackAngleDelta": 0,
        "stackOffsetDelta": 0, "alternateStackOffsetDelta": 0
    })
}

fn shape_name(shape: WebShape) -> &'static str {
    match shape {
        WebShape::Circle => "circle",
        WebShape::Rectangle => "rectangle",
        WebShape::Triangle => "triangle",
        WebShape::Pentagon => "pentagon",
        WebShape::Hexagon => "hexagon",
    }
}

fn value_mode_name(mode: ValueMode) -> &'static str {
    match mode {
        ValueMode::Cmyk => "cmyk",
        ValueMode::Luminance => "luminance",
        ValueMode::CrosshatchLuminance => "crosshatch-luminance",
        ValueMode::InvertedLuminance => "inverted-luminance",
        ValueMode::SingleChannel => "single-channel",
    }
}

fn ink_name(ink: Ink) -> &'static str {
    match ink {
        Ink::Cyan => "c",
        Ink::Magenta => "m",
        Ink::Yellow => "y",
        Ink::Black => "k",
    }
}

fn shape_preset_value(name: &str, settings: &WebShapeSettings) -> serde_json::Value {
    let mut channels = serde_json::Map::new();
    for ink in Ink::ALL {
        let channel = settings.channels.get(ink);
        channels.insert(
            ink_name(ink).into(),
            serde_json::json!({
                "enabled": channel.enabled, "color": channel.color,
                "rotation": channel.rotation, "gridRotation": channel.grid_rotation,
                "gridPivotX": channel.grid_pivot_x, "gridPivotY": channel.grid_pivot_y,
                "scale": channel.scale, "curveScale": 32,
                "motifCoverageMode": "auto", "motifBleed": 2,
                "tileCount": 1, "tileSpacing": 0, "tileAngle": 0, "tileOffset": 0,
                "stackCount": 1, "stackSpacing": 36, "stackAngle": 0,
                "stackOffset": 0, "alternateStackOffset": 0,
                "alternateTileTransform": "none", "outputQuality": 1,
                "threshold": channel.threshold, "maxSize": channel.max_size,
                "resolutionScale": channel.resolution_scale,
                "offsetX": channel.offset_x, "offsetY": channel.offset_y,
                "opacity": channel.opacity, "preset": shape_name(channel.shape),
                "customPath": "", "connectEndpoints": false,
                "smoothSeamTangents": false
            }),
        );
    }
    serde_json::json!({
        "format": "toniator-preset", "version": 1, "name": name,
        "nativeRender": RenderVariant::WebShapeV1 { settings: Box::new(settings.clone()) },
        "settings": {
            "outputWidth": settings.output_width, "outputHeight": settings.output_height,
            "longEdgeCells": settings.long_edge_cells, "gridScale": settings.grid_scale,
            "minMark": settings.min_mark, "maxMark": settings.max_mark,
            "valueMode": value_mode_name(settings.value_mode),
            "singleChannel": ink_name(settings.single_channel), "markMode": "shape",
            "curveSpan": "full-width", "syncCurveChannels": true,
            "sharedConnectEndpoints": false, "sharedSmoothSeamTangents": false,
            "showBackground": false,
            "geometryMode": if settings.use_shared_mark { "shared" } else { "independent" },
            "useSharedMark": settings.use_shared_mark,
            "sharedPreset": shape_name(settings.shared_shape), "sharedPath": "",
            "preserveAspect": false
        },
        "cmykDeltas": identity_deltas(), "channels": channels
    })
}

fn curve_path_data(path: &CurvePath) -> String {
    use std::fmt::Write as _;
    let mut data = format!("M {} {}", path.start.x, path.start.y);
    for segment in &path.segments {
        let _ = write!(
            data,
            " C {} {} {} {} {} {}",
            segment.control_1.x,
            segment.control_1.y,
            segment.control_2.x,
            segment.control_2.y,
            segment.end.x,
            segment.end.y
        );
    }
    data
}

fn curve_preset_value(name: &str, settings: &WebCurveSettings) -> serde_json::Value {
    let mut channels = serde_json::Map::new();
    for ink in Ink::ALL {
        let channel = settings.channels.get(ink);
        channels.insert(
            ink_name(ink).into(),
            serde_json::json!({
                "enabled": channel.enabled, "color": channel.color, "rotation": 0,
                "gridRotation": channel.grid_rotation,
                "gridPivotX": channel.grid_pivot_x, "gridPivotY": channel.grid_pivot_y,
                "scale": channel.scale, "curveScale": channel.curve_scale,
                "motifCoverageMode": match channel.motif_coverage { MotifCoverage::Auto => "auto", MotifCoverage::Manual => "manual" },
                "motifBleed": channel.motif_bleed, "tileCount": channel.tile_count,
                "tileSpacing": channel.tile_spacing, "tileAngle": channel.tile_angle,
                "tileOffset": channel.tile_offset, "stackCount": channel.stack_count,
                "stackSpacing": channel.stack_spacing, "stackAngle": channel.stack_angle,
                "stackOffset": channel.stack_offset,
                "alternateStackOffset": channel.alternate_stack_offset,
                "alternateTileTransform": match channel.alternate_tile_transform { AlternateTileTransform::None => "none", AlternateTileTransform::Flip => "flip", AlternateTileTransform::Rotate180 => "rotate-180" },
                "outputQuality": channel.output_quality, "threshold": channel.threshold,
                "maxSize": channel.max_size, "resolutionScale": channel.resolution_scale,
                "offsetX": channel.offset_x, "offsetY": channel.offset_y,
                "opacity": channel.opacity, "preset": "line",
                "customPath": curve_path_data(&channel.path),
                "connectEndpoints": channel.close_ends,
                "smoothSeamTangents": channel.smooth_join
            }),
        );
    }
    serde_json::json!({
        "format": "toniator-preset", "version": 1, "name": name,
        "nativeRender": RenderVariant::WebCurveV1 { settings: Box::new(settings.clone()) },
        "settings": {
            "outputWidth": settings.output_width, "outputHeight": settings.output_height,
            "longEdgeCells": settings.long_edge_cells, "gridScale": 92,
            "minMark": settings.min_mark, "maxMark": settings.max_mark,
            "valueMode": value_mode_name(settings.value_mode),
            "singleChannel": ink_name(settings.single_channel), "markMode": "curve",
            "curveSpan": match settings.layout { CurveLayout::FullWidth => "full-width", CurveLayout::MotifPattern => "motif-pattern" },
            "syncCurveChannels": settings.use_shared_curve,
            "sharedConnectEndpoints": settings.shared_close_ends,
            "sharedSmoothSeamTangents": settings.shared_smooth_join,
            "showBackground": settings.show_background, "geometryMode": "shared",
            "useSharedMark": false, "sharedPreset": "line",
            "sharedPath": curve_path_data(&settings.shared_path), "preserveAspect": false
        },
        "cmykDeltas": identity_deltas(), "channels": channels
    })
}

fn parse_curve_preset(
    preset: &PresetV1,
    raw_settings: &RawSettings,
    source_dimensions: (u32, u32),
) -> Result<RenderVariant> {
    let layout = match raw_settings.curve_span.as_deref().unwrap_or("full-width") {
        "full-width" => CurveLayout::FullWidth,
        "motif-pattern" => CurveLayout::MotifPattern,
        value => {
            bail!("This preset uses unsupported curve coverage '{value}'. Nothing was changed.")
        }
    };
    let output_width = raw_settings.output_width.unwrap_or(WEB_WIDTH);
    let mut output_height = raw_settings.output_height.unwrap_or(WEB_HEIGHT);
    if raw_settings.preserve_aspect.unwrap_or(true) {
        let (source_width, source_height) = source_dimensions;
        ensure!(
            source_width > 0 && source_height > 0,
            "Source artwork has no usable size"
        );
        output_height = ((output_width as f64) / (source_width as f64 / source_height as f64))
            .round()
            .max(1.0) as u32;
    }
    ensure!(
        (32..=4000).contains(&output_width) && (1..=4000).contains(&output_height),
        "Preset curve artboard is outside the supported range"
    );

    let use_shared_curve = raw_settings.sync_curve_channels.unwrap_or(true);
    let shared_source = if let Some(path) = raw_settings
        .shared_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        path
    } else {
        curve_preset_path(raw_settings.shared_preset.as_deref().unwrap_or("line"))?
    };
    let shared_path = parse_curve_path(shared_source)?;
    let shared_close_ends = raw_settings.shared_connect_endpoints.unwrap_or(false);
    let shared_smooth_join = raw_settings.shared_smooth_seam_tangents.unwrap_or(false);

    let mut channels = WebCurveChannels::default();
    for ink in Ink::ALL {
        let raw = preset
            .channels
            .as_ref()
            .and_then(|channels| channels.get(ink));
        *channels.get_mut(ink) = parse_curve_channel(
            raw,
            ink,
            use_shared_curve.then_some(&shared_path),
            shared_close_ends,
            shared_smooth_join,
        )?;
    }

    let mut result = WebCurveSettings {
        output_width,
        output_height,
        long_edge_cells: bounded(
            raw_settings.long_edge_cells.unwrap_or(WEB_CELLS),
            8.0,
            220.0,
            "cell count",
        )?,
        min_mark: bounded(
            raw_settings.min_mark.unwrap_or(0.0),
            0.0,
            100.0,
            "minimum mark",
        )?,
        max_mark: bounded(
            raw_settings.max_mark.unwrap_or(85.0),
            1.0,
            200.0,
            "maximum mark",
        )?,
        value_mode: parse_value_mode(raw_settings.value_mode.as_deref().unwrap_or("cmyk"))?,
        single_channel: parse_ink(raw_settings.single_channel.as_deref().unwrap_or("k"))?,
        layout,
        use_shared_curve,
        shared_path,
        shared_close_ends,
        shared_smooth_join,
        show_background: raw_settings.show_background.unwrap_or(true),
        channels,
    };
    ensure!(
        result.min_mark <= result.max_mark,
        "Preset mark range is invalid"
    );
    let deltas = preset.cmyk_deltas.as_ref();
    let threshold_delta = finite(
        deltas
            .and_then(|value| value.threshold_delta)
            .unwrap_or(0.0),
        "threshold delta",
    )? / 100.0;
    let shape_deltas = WebShapeDeltas {
        rotation_delta: finite(
            deltas.and_then(|value| value.rotation_delta).unwrap_or(0.0),
            "rotation delta",
        )?,
        grid_rotation_delta: finite(
            deltas
                .and_then(|value| value.grid_rotation_delta)
                .unwrap_or(0.0),
            "grid rotation delta",
        )?,
        grid_pivot_x_delta: finite(
            deltas
                .and_then(|value| value.grid_pivot_x_delta)
                .unwrap_or(0.0),
            "pivot delta",
        )?,
        grid_pivot_y_delta: finite(
            deltas
                .and_then(|value| value.grid_pivot_y_delta)
                .unwrap_or(0.0),
            "pivot delta",
        )?,
        scale_multiplier: positive(
            deltas
                .and_then(|value| value.scale_multiplier)
                .unwrap_or(1.0),
            "scale multiplier",
        )?,
        resolution_multiplier: positive(
            deltas
                .and_then(|value| value.resolution_multiplier)
                .unwrap_or(1.0),
            "resolution multiplier",
        )?,
        threshold_delta,
        max_size_multiplier: positive(
            deltas
                .and_then(|value| value.max_size_multiplier)
                .unwrap_or(1.0),
            "maximum size multiplier",
        )?,
        opacity_multiplier: positive_or_zero(
            deltas
                .and_then(|value| value.opacity_multiplier)
                .unwrap_or(1.0),
            "opacity multiplier",
        )?,
        offset_x_delta: finite(
            deltas.and_then(|value| value.offset_x_delta).unwrap_or(0.0),
            "phase delta",
        )?,
        offset_y_delta: finite(
            deltas.and_then(|value| value.offset_y_delta).unwrap_or(0.0),
            "phase delta",
        )?,
    };
    let output_quality_multiplier = positive(
        deltas
            .and_then(|value| value.output_quality_multiplier)
            .unwrap_or(1.0),
        "output quality multiplier",
    )?;
    result.apply_deltas(shape_deltas, output_quality_multiplier);
    let curve_scale_multiplier = positive(
        deltas
            .and_then(|value| value.curve_scale_multiplier)
            .unwrap_or(1.0),
        "curve scale multiplier",
    )?;
    let coverage_override = deltas
        .and_then(|value| value.motif_coverage_mode_override.as_deref())
        .filter(|value| !value.is_empty());
    let alternate_override = deltas
        .and_then(|value| value.alternate_tile_transform_override.as_deref())
        .filter(|value| !value.is_empty());
    for ink in Ink::ALL {
        let channel = result.channels.get_mut(ink);
        channel.curve_scale *= curve_scale_multiplier;
        channel.motif_bleed += finite(
            deltas
                .and_then(|value| value.motif_bleed_delta)
                .unwrap_or(0.0),
            "edge overlap delta",
        )?;
        channel.tile_count = add_count(
            channel.tile_count,
            deltas.and_then(|value| value.tile_count_delta).unwrap_or(0),
            "column count",
        )?;
        channel.tile_spacing += finite(
            deltas
                .and_then(|value| value.tile_spacing_delta)
                .unwrap_or(0.0),
            "horizontal spacing delta",
        )?;
        channel.tile_angle += finite(
            deltas
                .and_then(|value| value.tile_angle_delta)
                .unwrap_or(0.0),
            "pattern angle delta",
        )?;
        channel.tile_offset += finite(
            deltas
                .and_then(|value| value.tile_offset_delta)
                .unwrap_or(0.0),
            "tile offset delta",
        )?;
        channel.stack_count = add_count(
            channel.stack_count,
            deltas
                .and_then(|value| value.stack_count_delta)
                .unwrap_or(0),
            "row count",
        )?;
        channel.stack_spacing += finite(
            deltas
                .and_then(|value| value.stack_spacing_delta)
                .unwrap_or(0.0),
            "vertical spacing delta",
        )?;
        channel.stack_angle += finite(
            deltas
                .and_then(|value| value.stack_angle_delta)
                .unwrap_or(0.0),
            "layer turn delta",
        )?;
        channel.stack_offset += finite(
            deltas
                .and_then(|value| value.stack_offset_delta)
                .unwrap_or(0.0),
            "layer shift delta",
        )?;
        channel.alternate_stack_offset += finite(
            deltas
                .and_then(|value| value.alternate_stack_offset_delta)
                .unwrap_or(0.0),
            "row stagger delta",
        )?;
        if let Some(value) = coverage_override {
            channel.motif_coverage = parse_motif_coverage(value)?;
        }
        if let Some(value) = alternate_override {
            channel.alternate_tile_transform = parse_alternate_transform(value)?;
        }
    }
    let candidate = RenderVariant::WebCurveV1 {
        settings: Box::new(result),
    };
    let mut cloned = Document::new(crate::model::SourceArtwork {
        name: "validation".into(),
        media_type: "application/octet-stream".into(),
        bytes: std::sync::Arc::from([1]),
    });
    cloned.render = candidate.clone();
    cloned.validate()?;
    Ok(candidate)
}

fn parse_curve_channel(
    raw: Option<&RawChannel>,
    ink: Ink,
    shared_path: Option<&CurvePath>,
    shared_close_ends: bool,
    shared_smooth_join: bool,
) -> Result<WebCurveChannel> {
    let defaults = default_curve_channel(ink);
    let Some(raw) = raw else {
        let mut channel = defaults;
        if let Some(path) = shared_path {
            channel.path = path.clone();
            channel.close_ends = shared_close_ends;
            channel.smooth_join = shared_smooth_join;
        }
        return Ok(channel);
    };
    let color = raw.color.clone().unwrap_or(defaults.color);
    ensure!(
        parse_hex_color(&color).is_some(),
        "Preset has an invalid {} color",
        ink.label()
    );
    let path = if let Some(path) = shared_path {
        path.clone()
    } else {
        let source = if let Some(path) = raw
            .custom_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
        {
            path
        } else {
            curve_preset_path(raw.preset.as_deref().unwrap_or("line"))?
        };
        parse_curve_path(source)?
    };
    Ok(WebCurveChannel {
        enabled: raw.enabled.unwrap_or(true),
        color,
        grid_rotation: finite(raw.grid_rotation.unwrap_or(0.0), "screen angle")?,
        grid_pivot_x: bounded(
            raw.grid_pivot_x.unwrap_or(0.0),
            -4000.0,
            4000.0,
            "grid pivot",
        )?,
        grid_pivot_y: bounded(
            raw.grid_pivot_y.unwrap_or(0.0),
            -4000.0,
            4000.0,
            "grid pivot",
        )?,
        scale: bounded(raw.scale.unwrap_or(1.0), 0.0, 5.0, "coverage")?,
        threshold: unit_or_percent(raw.threshold.unwrap_or(0.04), "threshold")?,
        max_size: bounded(raw.max_size.unwrap_or(100.0), 0.0, 1000.0, "maximum size")?,
        resolution_scale: bounded(raw.resolution_scale.unwrap_or(1.0), 0.1, 100.0, "detail")?,
        offset_x: bounded(raw.offset_x.unwrap_or(0.0), -4000.0, 4000.0, "phase")?,
        offset_y: bounded(raw.offset_y.unwrap_or(0.0), -4000.0, 4000.0, "phase")?,
        opacity: unit_or_percent(raw.opacity.unwrap_or(0.92), "opacity")?,
        output_quality: bounded(
            raw.output_quality.unwrap_or(1.0),
            0.1,
            20.0,
            "output quality",
        )?,
        curve_scale: bounded(raw.curve_scale.unwrap_or(32.0), 0.1, 500.0, "motif size")?,
        motif_coverage: parse_motif_coverage(raw.motif_coverage_mode.as_deref().unwrap_or("auto"))?,
        motif_bleed: bounded(raw.motif_bleed.unwrap_or(2.0), 0.0, 100.0, "edge overlap")?,
        tile_count: bounded_count(raw.tile_count.unwrap_or(2), "column count")?,
        tile_spacing: bounded(
            raw.tile_spacing.unwrap_or(0.0),
            -10_000.0,
            10_000.0,
            "horizontal spacing",
        )?,
        tile_angle: finite(raw.tile_angle.unwrap_or(0.0), "pattern angle")?,
        tile_offset: bounded(
            raw.tile_offset.unwrap_or(0.0),
            -10_000.0,
            10_000.0,
            "tile offset",
        )?,
        stack_count: bounded_count(raw.stack_count.unwrap_or(2), "row count")?,
        stack_spacing: bounded(
            raw.stack_spacing.unwrap_or(36.0),
            -10_000.0,
            10_000.0,
            "vertical spacing",
        )?,
        stack_angle: finite(raw.stack_angle.unwrap_or(0.0), "layer turn")?,
        stack_offset: bounded(
            raw.stack_offset.unwrap_or(0.0),
            -10_000.0,
            10_000.0,
            "layer shift",
        )?,
        alternate_stack_offset: bounded(
            raw.alternate_stack_offset.unwrap_or(0.0),
            -10_000.0,
            10_000.0,
            "row stagger",
        )?,
        alternate_tile_transform: parse_alternate_transform(
            raw.alternate_tile_transform.as_deref().unwrap_or("none"),
        )?,
        path,
        close_ends: shared_path
            .map(|_| shared_close_ends)
            .unwrap_or_else(|| raw.connect_endpoints.unwrap_or(false)),
        smooth_join: shared_path
            .map(|_| shared_smooth_join)
            .unwrap_or_else(|| raw.smooth_seam_tangents.unwrap_or(false)),
    })
}

fn default_curve_channel(ink: Ink) -> WebCurveChannel {
    let color = match ink {
        Ink::Cyan => "#00aeef",
        Ink::Magenta => "#ec008c",
        Ink::Yellow => "#ffd400",
        Ink::Black => "#111111",
    };
    WebCurveChannel {
        color: color.into(),
        grid_rotation: 0.0,
        ..Default::default()
    }
}

fn curve_preset_path(name: &str) -> Result<&'static str> {
    Ok(match name {
        "line" => "M -0.45 0 L 0.45 0",
        "slash" => "M -0.42 0.42 L 0.42 -0.42",
        "arc" => "M -0.45 0.18 C -0.25 -0.35 0.25 -0.35 0.45 0.18",
        "wave" => "M -0.5 0 C -0.32 -0.4 -0.18 -0.4 0 0 C 0.18 0.4 0.32 0.4 0.5 0",
        "curve" => "M -0.45 0.32 C -0.22 -0.38 0.22 -0.38 0.45 0.32",
        "v" => "M -0.42 -0.3 L 0 0.34 L 0.42 -0.3",
        "loop" => "M -0.45 0 C -0.35 -0.35 -0.05 -0.35 0 0 C 0.05 0.35 0.35 0.35 0.45 0",
        value => bail!("This preset uses unknown curve profile '{value}'. Nothing was changed."),
    })
}

fn parse_curve_path(source: &str) -> Result<CurvePath> {
    use svgtypes::{SimplePathSegment, SimplifyingPathParser};
    let mut start = None;
    let mut current = CurvePoint::default();
    let mut segments = Vec::new();
    for segment in SimplifyingPathParser::from(source) {
        match segment.context("Preset curve path is invalid")? {
            SimplePathSegment::MoveTo { x, y } => {
                ensure!(start.is_none(), "Preset curve must contain one open path");
                current = CurvePoint { x, y };
                start = Some(current);
            }
            SimplePathSegment::LineTo { x, y } => {
                ensure!(start.is_some(), "Preset curve must begin with a move");
                let end = CurvePoint { x, y };
                segments.push(CubicCurveSegment {
                    control_1: lerp_curve_point(current, end, 1.0 / 3.0),
                    control_2: lerp_curve_point(current, end, 2.0 / 3.0),
                    end,
                });
                current = end;
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                ensure!(start.is_some(), "Preset curve must begin with a move");
                let end = CurvePoint { x, y };
                segments.push(CubicCurveSegment {
                    control_1: CurvePoint { x: x1, y: y1 },
                    control_2: CurvePoint { x: x2, y: y2 },
                    end,
                });
                current = end;
            }
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                ensure!(start.is_some(), "Preset curve must begin with a move");
                let control = CurvePoint { x: x1, y: y1 };
                let end = CurvePoint { x, y };
                segments.push(CubicCurveSegment {
                    control_1: lerp_curve_point(current, control, 2.0 / 3.0),
                    control_2: lerp_curve_point(end, control, 2.0 / 3.0),
                    end,
                });
                current = end;
            }
            SimplePathSegment::ClosePath => {
                bail!("Closed preset curves are not available yet. Nothing was changed.")
            }
        }
    }
    let path = CurvePath {
        start: start.context("Preset curve path is empty")?,
        segments,
    };
    ensure!(
        !path.segments.is_empty() && path.segments.len() <= 64,
        "Preset curve has an unsupported number of segments"
    );
    Ok(path)
}

fn lerp_curve_point(a: CurvePoint, b: CurvePoint, amount: f64) -> CurvePoint {
    CurvePoint {
        x: a.x + (b.x - a.x) * amount,
        y: a.y + (b.y - a.y) * amount,
    }
}

fn parse_channel(
    raw: Option<&RawChannel>,
    ink: Ink,
    shared: bool,
    shared_shape: WebShape,
) -> Result<WebShapeChannel> {
    let defaults = default_channel(ink);
    let Some(raw) = raw else { return Ok(defaults) };
    let enabled = raw.enabled.unwrap_or(true);
    let color = raw.color.clone().unwrap_or(defaults.color);
    ensure!(
        parse_hex_color(&color).is_some(),
        "Preset has an invalid {} color",
        ink.label()
    );
    let shape = if shared || !enabled {
        shared_shape
    } else {
        if raw
            .custom_path
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty())
        {
            bail!(
                "The {} ink uses a custom shape path that is not available yet. Nothing was changed.",
                ink.label()
            );
        }
        parse_shape(raw.preset.as_deref().unwrap_or("circle"))?
    };
    Ok(WebShapeChannel {
        enabled,
        color,
        rotation: finite(raw.rotation.unwrap_or(0.0), "mark rotation")?,
        grid_rotation: finite(raw.grid_rotation.unwrap_or(0.0), "screen angle")?,
        grid_pivot_x: bounded(
            raw.grid_pivot_x.unwrap_or(0.0),
            -4000.0,
            4000.0,
            "grid pivot",
        )?,
        grid_pivot_y: bounded(
            raw.grid_pivot_y.unwrap_or(0.0),
            -4000.0,
            4000.0,
            "grid pivot",
        )?,
        scale: bounded(raw.scale.unwrap_or(1.0), 0.0, 5.0, "coverage")?,
        threshold: unit_or_percent(raw.threshold.unwrap_or(0.04), "threshold")?,
        max_size: bounded(raw.max_size.unwrap_or(100.0), 0.0, 1000.0, "maximum size")?,
        resolution_scale: bounded(raw.resolution_scale.unwrap_or(1.0), 0.1, 100.0, "detail")?,
        offset_x: bounded(raw.offset_x.unwrap_or(0.0), -4000.0, 4000.0, "phase")?,
        offset_y: bounded(raw.offset_y.unwrap_or(0.0), -4000.0, 4000.0, "phase")?,
        opacity: unit_or_percent(raw.opacity.unwrap_or(0.92), "opacity")?,
        shape,
    })
}

fn default_channel(ink: Ink) -> WebShapeChannel {
    let color = match ink {
        Ink::Cyan => "#00aeef",
        Ink::Magenta => "#ec008c",
        Ink::Yellow => "#ffd400",
        Ink::Black => "#111111",
    };
    WebShapeChannel {
        color: color.into(),
        threshold: 0.04,
        opacity: 0.92,
        grid_rotation: 0.0,
        ..Default::default()
    }
}

fn parse_shape(value: &str) -> Result<WebShape> {
    match value {
        "circle" => Ok(WebShape::Circle),
        "rectangle" => Ok(WebShape::Rectangle),
        "triangle" => Ok(WebShape::Triangle),
        "pentagon" => Ok(WebShape::Pentagon),
        "hexagon" => Ok(WebShape::Hexagon),
        value => bail!("This preset uses unknown shape '{value}'. Nothing was changed."),
    }
}
fn parse_value_mode(value: &str) -> Result<ValueMode> {
    match value {
        "cmyk" => Ok(ValueMode::Cmyk),
        "luminance" => Ok(ValueMode::Luminance),
        "crosshatch-luminance" => Ok(ValueMode::CrosshatchLuminance),
        "inverted-luminance" => Ok(ValueMode::InvertedLuminance),
        "single-channel" => Ok(ValueMode::SingleChannel),
        _ => bail!("Preset has an unknown color interpretation"),
    }
}
fn parse_ink(value: &str) -> Result<Ink> {
    match value {
        "c" => Ok(Ink::Cyan),
        "m" => Ok(Ink::Magenta),
        "y" => Ok(Ink::Yellow),
        "k" => Ok(Ink::Black),
        _ => bail!("Preset has an unknown output ink"),
    }
}
fn finite(value: f64, name: &str) -> Result<f64> {
    ensure!(value.is_finite(), "Preset {name} is invalid");
    Ok(value)
}
fn bounded(value: f64, min: f64, max: f64, name: &str) -> Result<f64> {
    let value = finite(value, name)?;
    ensure!(
        (min..=max).contains(&value),
        "Preset {name} is outside the supported range"
    );
    Ok(value)
}
fn bounded_count(value: u32, name: &str) -> Result<u32> {
    ensure!(
        (1..=10_000).contains(&value),
        "Preset {name} is outside the supported range"
    );
    Ok(value)
}
fn add_count(value: u32, delta: i32, name: &str) -> Result<u32> {
    let result = i64::from(value) + i64::from(delta);
    ensure!(
        (1..=10_000).contains(&result),
        "Preset {name} is outside the supported range"
    );
    Ok(result as u32)
}
fn parse_motif_coverage(value: &str) -> Result<MotifCoverage> {
    match value {
        "auto" => Ok(MotifCoverage::Auto),
        "manual" => Ok(MotifCoverage::Manual),
        value => bail!("Preset has an unknown motif coverage mode '{value}'"),
    }
}
fn parse_alternate_transform(value: &str) -> Result<AlternateTileTransform> {
    match value {
        "none" => Ok(AlternateTileTransform::None),
        "flip" => Ok(AlternateTileTransform::Flip),
        "rotate-180" => Ok(AlternateTileTransform::Rotate180),
        value => bail!("Preset has an unknown alternate-copy transform '{value}'"),
    }
}
fn positive(value: f64, name: &str) -> Result<f64> {
    let value = finite(value, name)?;
    ensure!(
        value > 0.0 && value <= 100.0,
        "Preset {name} is outside the supported range"
    );
    Ok(value)
}
fn positive_or_zero(value: f64, name: &str) -> Result<f64> {
    let value = finite(value, name)?;
    ensure!(
        (0.0..=100.0).contains(&value),
        "Preset {name} is outside the supported range"
    );
    Ok(value)
}
fn unit_or_percent(value: f64, name: &str) -> Result<f64> {
    let value = finite(value, name)?;
    let unit = if value > 1.0 { value / 100.0 } else { value };
    ensure!(
        (0.0..=1.0).contains(&unit),
        "Preset {name} is outside the supported range"
    );
    Ok(unit)
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSettings {
    output_width: Option<u32>,
    output_height: Option<u32>,
    long_edge_cells: Option<f64>,
    grid_scale: Option<f64>,
    min_mark: Option<f64>,
    max_mark: Option<f64>,
    value_mode: Option<String>,
    single_channel: Option<String>,
    mark_mode: Option<String>,
    curve_span: Option<String>,
    sync_curve_channels: Option<bool>,
    shared_connect_endpoints: Option<bool>,
    shared_smooth_seam_tangents: Option<bool>,
    show_background: Option<bool>,
    geometry_mode: Option<String>,
    shared_preset: Option<String>,
    shared_path: Option<String>,
    preserve_aspect: Option<bool>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDeltas {
    rotation_delta: Option<f64>,
    grid_rotation_delta: Option<f64>,
    grid_pivot_x_delta: Option<f64>,
    grid_pivot_y_delta: Option<f64>,
    scale_multiplier: Option<f64>,
    resolution_multiplier: Option<f64>,
    threshold_delta: Option<f64>,
    max_size_multiplier: Option<f64>,
    opacity_multiplier: Option<f64>,
    offset_x_delta: Option<f64>,
    offset_y_delta: Option<f64>,
    output_quality_multiplier: Option<f64>,
    curve_scale_multiplier: Option<f64>,
    motif_coverage_mode_override: Option<String>,
    motif_bleed_delta: Option<f64>,
    tile_count_delta: Option<i32>,
    tile_spacing_delta: Option<f64>,
    tile_angle_delta: Option<f64>,
    tile_offset_delta: Option<f64>,
    alternate_tile_transform_override: Option<String>,
    stack_count_delta: Option<i32>,
    stack_spacing_delta: Option<f64>,
    stack_angle_delta: Option<f64>,
    stack_offset_delta: Option<f64>,
    alternate_stack_offset_delta: Option<f64>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChannel {
    enabled: Option<bool>,
    color: Option<String>,
    rotation: Option<f64>,
    grid_rotation: Option<f64>,
    grid_pivot_x: Option<f64>,
    grid_pivot_y: Option<f64>,
    scale: Option<f64>,
    threshold: Option<f64>,
    max_size: Option<f64>,
    resolution_scale: Option<f64>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
    opacity: Option<f64>,
    preset: Option<String>,
    custom_path: Option<String>,
    output_quality: Option<f64>,
    curve_scale: Option<f64>,
    motif_coverage_mode: Option<String>,
    motif_bleed: Option<f64>,
    tile_count: Option<u32>,
    tile_spacing: Option<f64>,
    tile_angle: Option<f64>,
    tile_offset: Option<f64>,
    stack_count: Option<u32>,
    stack_spacing: Option<f64>,
    stack_angle: Option<f64>,
    stack_offset: Option<f64>,
    alternate_stack_offset: Option<f64>,
    alternate_tile_transform: Option<String>,
    connect_endpoints: Option<bool>,
    smooth_seam_tangents: Option<bool>,
}
#[derive(Deserialize)]
struct RawChannels {
    c: Option<RawChannel>,
    m: Option<RawChannel>,
    y: Option<RawChannel>,
    k: Option<RawChannel>,
}
impl RawChannels {
    fn get(&self, ink: Ink) -> Option<&RawChannel> {
        match ink {
            Ink::Cyan => self.c.as_ref(),
            Ink::Magenta => self.m.as_ref(),
            Ink::Yellow => self.y.as_ref(),
            Ink::Black => self.k.as_ref(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresetV1 {
    format: String,
    version: u32,
    settings: Option<RawSettings>,
    cmyk_deltas: Option<RawDeltas>,
    channels: Option<RawChannels>,
    native_render: Option<RenderVariant>,
}

#[derive(Deserialize)]
struct PresetHeader {
    format: String,
    version: u32,
}

#[derive(Deserialize)]
struct NativePresetV2 {
    #[serde(default)]
    settings: crate::model::Settings,
    render: RenderVariant,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_bundled_shape_curve_and_motif_presets_import() {
        for name in [
            "ComicBook.tntr",
            "Chunky Fingerprints.tntr",
            "Skinny Curve.tntr",
            "Tiled Stacked Motif Stress Test.tntr",
        ] {
            let bytes = std::fs::read(format!("../presets/{name}")).unwrap();
            let result = parse_tntr(&bytes, (960, 680));
            assert!(result.is_ok(), "{name}: {result:?}");
            if let Ok(RenderVariant::WebShapeV1 { settings }) = result {
                assert_eq!((settings.output_width, settings.output_height), (900, 620));
                assert_eq!(settings.long_edge_cells, 92.0);
                assert_eq!(
                    (settings.grid_scale, settings.min_mark, settings.max_mark),
                    (76.0, 4.0, 90.0)
                );
                assert_eq!(settings.channels.c.resolution_scale, 2.0);
                assert_eq!(settings.channels.c.max_size, 300.0);
                assert_eq!(settings.channels.c.grid_rotation, 15.0);
                assert_eq!(settings.channels.m.grid_rotation, 75.0);
                assert_eq!(settings.channels.y.grid_rotation, -10.0);
                assert_eq!(settings.channels.k.grid_rotation, 45.0);
            } else if let Ok(RenderVariant::WebCurveV1 { settings }) = result {
                assert_eq!((settings.output_width, settings.output_height), (900, 638));
                assert_eq!(settings.long_edge_cells, 90.0);
                assert!(settings.use_shared_curve);
                assert_eq!(settings.shared_path.segments.len(), 1);
                if name.starts_with("Tiled") {
                    assert_eq!(settings.layout, CurveLayout::MotifPattern);
                    assert_eq!(settings.channels.c.curve_scale, 32.0);
                    assert_eq!(settings.channels.c.motif_coverage, MotifCoverage::Auto);
                    assert_eq!(settings.channels.c.motif_bleed, 2.0);
                    assert_eq!(settings.channels.c.stack_spacing, 6.0);
                    assert_eq!(settings.channels.c.grid_rotation, 30.0);
                    assert_eq!(settings.channels.m.grid_rotation, 60.0);
                    assert_eq!(settings.channels.y.grid_rotation, 0.0);
                    assert_eq!(settings.channels.k.grid_rotation, 90.0);
                } else if name.starts_with("Chunky") {
                    assert_eq!(settings.layout, CurveLayout::FullWidth);
                    assert_eq!(settings.channels.c.grid_rotation, 51.0);
                    assert_eq!(settings.channels.m.grid_rotation, 76.0);
                    assert_eq!(settings.channels.y.grid_rotation, 91.0);
                    assert_eq!(settings.channels.k.grid_rotation, -14.0);
                    assert_eq!(settings.channels.c.resolution_scale, 0.6);
                } else {
                    assert_eq!(settings.channels.c.grid_rotation, 0.0);
                    assert_eq!(settings.channels.k.grid_rotation, 0.0);
                    assert_eq!(settings.channels.c.resolution_scale, 1.6);
                }
                if !name.starts_with("Tiled") {
                    assert_eq!(settings.channels.c.max_size, 170.0);
                }
            }
        }
    }
    #[test]
    fn web_defaults_percent_values_aspect_and_threshold_delta_bug_are_preserved() {
        let json = br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","preserveAspect":true},"cmykDeltas":{"thresholdDelta":10,"opacityMultiplier":2},"channels":{"c":{"threshold":4,"opacity":50}}}"##;
        let RenderVariant::WebShapeV1 { settings } = parse_tntr(json, (2, 1)).unwrap() else {
            panic!()
        };
        assert_eq!((settings.output_width, settings.output_height), (900, 450));
        assert_eq!(settings.long_edge_cells, 90.0);
        assert_eq!(settings.channels.c.grid_rotation, 0.0);
        assert_eq!(settings.channels.c.threshold, 0.14);
        assert_eq!(settings.channels.c.opacity, 1.0);
        assert_eq!(settings.channels.m.threshold, 0.14);
        assert_eq!(settings.channels.m.opacity, 1.0);
    }
    #[test]
    fn active_custom_rejects_but_disabled_independent_geometry_is_normalized() {
        let active = br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","geometryMode":"independent"},"channels":{"c":{"enabled":true,"customPath":"M0 0"}}}"##;
        assert!(
            parse_tntr(active, (1, 1))
                .unwrap_err()
                .to_string()
                .ends_with("Nothing was changed.")
        );
        let disabled = br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","geometryMode":"independent"},"channels":{"c":{"enabled":false,"preset":"unknown","customPath":"M0 0"}}}"##;
        let RenderVariant::WebShapeV1 { settings } = parse_tntr(disabled, (1, 1)).unwrap() else {
            panic!()
        };
        assert_eq!(settings.channels.c.shape, WebShape::Circle);
    }
    #[test]
    fn malformed_version_color_enum_and_range_reject() {
        for json in [b"{" as &[u8], br##"{"format":"toniator-preset","version":2}"##, br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","valueMode":"wat"}}"##, br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape"},"channels":{"k":{"color":"red"}}}"##, br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","outputWidth":99999}}"##] { assert!(parse_tntr(json, (1, 1)).is_err()); }
    }

    #[test]
    fn parsed_preset_installs_as_one_undo_step_and_rejection_has_no_side_effects() {
        use crate::model::{DocumentEditor, SourceArtwork};
        let document = Document::new(SourceArtwork {
            name: "art.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1, 2, 3]),
        });
        let mut editor = DocumentEditor::new(document);
        let original = serde_json::to_vec(editor.document()).unwrap();
        let rejected = br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"curve","curveSpan":"unknown-layout"}}"##;
        assert!(parse_tntr(rejected, (100, 100)).is_err());
        assert_eq!(serde_json::to_vec(editor.document()).unwrap(), original);
        assert!(!editor.can_undo());
        assert!(!editor.is_dirty());

        let accepted = br##"{"format":"toniator-preset","version":1,"settings":{"markMode":"shape","preserveAspect":false}}"##;
        let treatment = parse_tntr(accepted, (100, 100)).unwrap();
        assert!(editor.set_render_variant(treatment));
        assert!(editor.is_dirty());
        assert!(editor.undo());
        assert_eq!(serde_json::to_vec(editor.document()).unwrap(), original);
        assert!(!editor.can_undo());
    }

    #[test]
    fn native_treatment_serialization_roundtrips_every_render_variant() {
        let mut shape = WebShapeSettings {
            use_shared_mark: false,
            ..Default::default()
        };
        shape.channels.c.shape = WebShape::Triangle;
        shape.channels.m.grid_pivot_x = 17.5;
        let mut curve = WebCurveSettings {
            layout: CurveLayout::MotifPattern,
            use_shared_curve: false,
            ..Default::default()
        };
        curve.channels.c.curve_scale = 47.0;
        curve.channels.c.tile_count = 3;
        curve.channels.c.stack_count = 2;
        curve.channels.c.alternate_tile_transform = AlternateTileTransform::Flip;
        curve.channels.m.path = CurvePath::deep_wave();
        for render in [
            RenderVariant::NativeBasicV1,
            RenderVariant::WebShapeV1 {
                settings: Box::new(shape),
            },
            RenderVariant::WebCurveV1 {
                settings: Box::new(curve),
            },
        ] {
            let bytes = treatment_preset_bytes("My Treatment", &render).unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(text.ends_with('\n'));
            assert!(text.contains("\"name\": \"My Treatment\""));
            assert!(!text.contains("source"));
            assert!(!text.contains("document_id"));
            assert_eq!(parse_tntr(&bytes, (333, 222)).unwrap(), render);
        }
    }

    #[test]
    fn web_compatible_save_uses_effective_bases_and_identity_deltas() {
        let render = RenderVariant::WebCurveV1 {
            settings: Box::new(WebCurveSettings::default()),
        };
        let bytes = treatment_preset_bytes("Curve", &render).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["settings"]["preserveAspect"], false);
        assert_eq!(value["cmykDeltas"]["scaleMultiplier"], 1);
        assert_eq!(value["cmykDeltas"]["stackSpacingDelta"], 0);
        assert!(value.get("nativeRender").is_some());
    }

    #[test]
    fn native_document_treatment_preserves_creative_settings_exactly() {
        let mut document = Document::new(crate::model::SourceArtwork {
            name: "art.png".into(),
            media_type: "image/png".into(),
            bytes: std::sync::Arc::from([1, 2, 3]),
        });
        document.settings = crate::model::Settings {
            treatment: crate::model::Treatment::Lines,
            detail: 73.0,
            coverage: 118.0,
            contrast: 142.0,
            angle: -17.0,
        };
        let bytes = document_treatment_preset_bytes("Lines", &document).unwrap();
        let parsed = parse_treatment(&bytes, (400, 300)).unwrap();
        assert_eq!(parsed.render, RenderVariant::NativeBasicV1);
        assert_eq!(parsed.native_settings, Some(document.settings));
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("art.png"));
        assert!(!text.contains("document_id"));
        assert!(!text.contains("bytes"));
    }
}
