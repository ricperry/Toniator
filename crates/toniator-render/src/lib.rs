#![forbid(unsafe_code)]

//! Headless consumers for immutable canonical circle geometry.
//!
//! `RenderScene` deliberately knows nothing about source artwork, sampling, or
//! pattern settings. Raster compositing happens in linear premultiplied RGBA;
//! `RasterSurface` exposes only straight sRGBA bytes at the output boundary.

use std::{collections::HashSet, error::Error, fmt};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel};
use toniator_geometry::{
    CanonicalCircleMark, CanonicalFillRule, CanonicalMark, CanonicalStroke, CurveSegment, Point2,
};

const SUBPIXEL_GRID: u32 = 8;
const MAX_PREVIEW_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT_PIXELS: u64 = 64 * 1024 * 1024;
/// Exact Stage 20E2 upper bound for adaptive flattened edges in one raster request.
pub const DEFAULT_MAX_FLATTENED_RASTER_EDGES: usize = 4_194_304;

/// Bounded consumer-side resource limits for adaptive canonical-path rasterization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterizationLimits {
    max_flattened_edges: usize,
}

impl RasterizationLimits {
    /// Builds a nonzero flattened-edge limit without changing canonical scene identity.
    ///
    /// # Errors
    ///
    /// Returns a stable raster-limit diagnostic when the caller disables the required bound.
    pub fn new(max_flattened_edges: usize) -> Result<Self, RenderError> {
        if max_flattened_edges == 0 {
            return Err(RenderError::new(
                "raster.limits.flattened_edges",
                "flattened raster edge limit must be nonzero",
            ));
        }
        Ok(Self {
            max_flattened_edges,
        })
    }

    /// Returns the exact maximum number of concrete flattened edges accepted per request.
    pub const fn max_flattened_edges(self) -> usize {
        self.max_flattened_edges
    }
}

impl Default for RasterizationLimits {
    /// Supplies the Stage 20E2 finite adaptive-raster edge contract.
    fn default() -> Self {
        Self {
            max_flattened_edges: DEFAULT_MAX_FLATTENED_RASTER_EDGES,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    canvas: CanvasSpec,
    layers: Vec<RenderLayer>,
    /// `None` is the accepted Stage 5 single-layer scene contract. Modeled
    /// scenes opt into the fixed Stage 9C equations without reinterpreting
    /// existing callers before complete-document evaluation is authorized.
    model: Option<HalftoneChannelModel>,
    identity: SceneIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneIdentity {
    family_fingerprint: String,
    realization_fingerprint: String,
    scene_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderLayer {
    channel_id: ChannelId,
    visible: bool,
    /// Canonical linear RGBA. It is converted only at output boundaries.
    color: ColorValue,
    opacity: f64,
    geometry: GeometryOutput,
    /// SourceColorAlpha carries immutable straight-linear paint per canonical
    /// mark. Solid layers leave this as `None`.
    mark_paints: Option<Vec<ColorValue>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeometryOutput {
    CircularMarks(Vec<CanonicalCircleMark>),
    CanonicalMarks(Vec<CanonicalMark>),
    CanonicalStrokes(Vec<CanonicalStroke>),
}

/// Renderer-owned immutable source-colored circle. Stage 9D may adapt the
/// accepted Stage 9B realization into this DTO without making rendering depend
/// on pattern realization or source sampling crates.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceColorCircle {
    pub mark: CanonicalCircleMark,
    pub paint: ColorValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError {
    path: &'static str,
    message: &'static str,
}

impl RenderError {
    pub const fn new(path: &'static str, message: &'static str) -> Self {
        Self { path, message }
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RenderError {}

impl RenderScene {
    /// Builds one unmodeled scene after validating every layer and complete canonical identity.
    ///
    /// # Errors
    ///
    /// Returns stable canvas, layer, geometry, paint, or duplicate-channel diagnostics.
    pub fn new(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        Self::build(
            canvas,
            family_fingerprint,
            realization_fingerprint,
            None,
            layers,
        )
    }

    /// Constructs a Stage 9C scene with fixed, non-selectable model semantics.
    pub fn new_modeled(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        model: HalftoneChannelModel,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        Self::build(
            canvas,
            family_fingerprint,
            realization_fingerprint,
            Some(model),
            layers,
        )
    }

    /// Validates ordered scene authority and computes its complete canonical geometry fingerprint.
    ///
    /// # Errors
    ///
    /// Returns stable canvas, identity, model topology, geometry, paint, or channel diagnostics.
    fn build(
        canvas: CanvasSpec,
        family_fingerprint: String,
        realization_fingerprint: String,
        model: Option<HalftoneChannelModel>,
        layers: Vec<RenderLayer>,
    ) -> Result<Self, RenderError> {
        validate_canvas(&canvas)?;
        if family_fingerprint.is_empty() || realization_fingerprint.is_empty() {
            return Err(RenderError::new(
                "scene.identity",
                "family and realization identities must not be empty",
            ));
        }
        if layers.is_empty() {
            return Err(RenderError::new(
                "scene.layers",
                "at least one layer is required",
            ));
        }
        if matches!(model, Some(HalftoneChannelModel::SourceColorAlpha)) && layers.len() != 1 {
            return Err(RenderError::new(
                "scene.layers",
                "SourceColorAlpha requires exactly one ordered source-colored layer",
            ));
        }
        let mut channel_ids = HashSet::new();
        for layer in &layers {
            validate_layer(layer)?;
            match model {
                None => {
                    if layer.mark_paints.is_some() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "unmodeled legacy scenes cannot carry sampled per-mark paints",
                        ));
                    }
                }
                Some(HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk) => {
                    if layer.mark_paints.is_some() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "RGB and CMYK layers must use solid paint",
                        ));
                    }
                }
                Some(HalftoneChannelModel::SourceColorAlpha) => {
                    if layer.mark_paints.is_none() {
                        return Err(RenderError::new(
                            "scene.layers",
                            "SourceColorAlpha requires sampled per-mark paint",
                        ));
                    }
                }
            }
            if !channel_ids.insert(layer.channel_id) {
                return Err(RenderError::new(
                    "scene.layers",
                    "layer channel IDs must be unique while preserving supplied order",
                ));
            }
        }
        let scene_fingerprint = scene_fingerprint(
            &canvas,
            &family_fingerprint,
            &realization_fingerprint,
            model,
            &layers,
        );
        Ok(Self {
            canvas,
            layers,
            model,
            identity: SceneIdentity {
                family_fingerprint,
                realization_fingerprint,
                scene_fingerprint,
            },
        })
    }

    /// Counts every filled mark across retained-circle and generalized canonical geometry.
    pub fn circular_mark_count(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| match &layer.geometry {
                GeometryOutput::CircularMarks(marks) => marks.len(),
                GeometryOutput::CanonicalMarks(marks) => marks.len(),
                GeometryOutput::CanonicalStrokes(strokes) => strokes.len(),
            })
            .sum()
    }

    pub fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }
    pub fn layers(&self) -> &[RenderLayer] {
        &self.layers
    }
    pub const fn model(&self) -> Option<HalftoneChannelModel> {
        self.model
    }
    pub fn identity(&self) -> &SceneIdentity {
        &self.identity
    }
}

impl SceneIdentity {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn realization_fingerprint(&self) -> &str {
        &self.realization_fingerprint
    }
    pub fn scene_fingerprint(&self) -> &str {
        &self.scene_fingerprint
    }
}

impl RenderLayer {
    pub fn new(
        channel_id: ChannelId,
        visible: bool,
        color: ColorValue,
        opacity: f64,
        geometry: GeometryOutput,
    ) -> Result<Self, RenderError> {
        let layer = Self {
            channel_id,
            visible,
            color,
            opacity,
            geometry,
            mark_paints: None,
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    /// Builds the one SourceColorAlpha layer from Stage 9B's immutable marked
    /// paint. The per-mark source alpha has already determined inclusion and
    /// color sampling; it is applied here exactly once with layer opacity.
    pub fn new_source_color(
        channel_id: ChannelId,
        visible: bool,
        opacity: f64,
        marks: Vec<SourceColorCircle>,
    ) -> Result<Self, RenderError> {
        let geometry = GeometryOutput::CircularMarks(
            marks
                .iter()
                .map(|source_mark| source_mark.mark.clone())
                .collect(),
        );
        let mark_paints = marks
            .into_iter()
            .map(|source_mark| ColorValue {
                red: source_mark.paint.red,
                green: source_mark.paint.green,
                blue: source_mark.paint.blue,
                alpha: source_mark.paint.alpha,
            })
            .collect();
        let layer = Self {
            channel_id,
            visible,
            color: ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity,
            geometry,
            mark_paints: Some(mark_paints),
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    /// Builds one source-colored layer over generalized canonical mark geometry.
    ///
    /// # Errors
    ///
    /// Returns stable geometry/paint cardinality or presentation diagnostics.
    pub fn new_source_color_geometry(
        channel_id: ChannelId,
        visible: bool,
        opacity: f64,
        marks: Vec<CanonicalMark>,
        paints: Vec<ColorValue>,
    ) -> Result<Self, RenderError> {
        let layer = Self {
            channel_id,
            visible,
            color: ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity,
            geometry: GeometryOutput::CanonicalMarks(marks),
            mark_paints: Some(paints),
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }
    pub const fn visible(&self) -> bool {
        self.visible
    }
    pub fn color(&self) -> &ColorValue {
        &self.color
    }
    pub const fn opacity(&self) -> f64 {
        self.opacity
    }
    pub fn geometry(&self) -> &GeometryOutput {
        &self.geometry
    }

    fn mark_paint(&self, index: usize) -> &ColorValue {
        self.mark_paints
            .as_ref()
            .and_then(|paints| paints.get(index))
            .unwrap_or(&self.color)
    }
}

fn validate_canvas(canvas: &CanvasSpec) -> Result<(), RenderError> {
    if !canvas.width.is_finite()
        || !canvas.height.is_finite()
        || canvas.width <= 0.0
        || canvas.height <= 0.0
    {
        return Err(RenderError::new(
            "scene.canvas",
            "canvas dimensions must be positive and finite",
        ));
    }
    Ok(())
}

/// Validates one immutable layer's presentation, geometry, and optional per-mark paint contract.
///
/// # Errors
///
/// Returns the first stable non-finite, invalid-radius, paint-cardinality, or alpha diagnostic.
fn validate_layer(layer: &RenderLayer) -> Result<(), RenderError> {
    for value in [
        layer.color.red,
        layer.color.green,
        layer.color.blue,
        layer.color.alpha,
        layer.opacity,
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(RenderError::new(
                "scene.layer.presentation",
                "color and opacity must be finite values within 0.0..=1.0",
            ));
        }
    }
    match &layer.geometry {
        GeometryOutput::CircularMarks(marks) => {
            if marks.iter().any(|mark| {
                !mark.center.is_finite() || !mark.radius.is_finite() || mark.radius < 0.0
            }) {
                return Err(RenderError::new(
                    "scene.layer.geometry",
                    "canonical circle geometry must be finite and nonnegative",
                ));
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            if marks.iter().any(|mark| match mark {
                CanonicalMark::Circle { center, radius, .. } => {
                    !center.is_finite() || !radius.is_finite() || *radius < 0.0
                }
                CanonicalMark::ClosedPath(mark) => {
                    !mark.bounds.min.is_finite() || !mark.bounds.max.is_finite()
                }
            }) {
                return Err(RenderError::new(
                    "scene.layer.geometry",
                    "canonical mark geometry must be finite",
                ));
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            if strokes.iter().any(|stroke| {
                !stroke.nominal_basis.is_finite()
                    || stroke.nominal_basis <= 0.0
                    || stroke.path.bounds().is_err()
                    || stroke
                        .outline
                        .bounds
                        .is_some_and(|bounds| !bounds.min.is_finite() || !bounds.max.is_finite())
                    || stroke.profile.iter().any(|sample| {
                        !sample.center.is_finite()
                            || !sample.width.is_finite()
                            || sample.width < 0.0
                            || !sample.normalized_thickness.is_finite()
                            || !(0.0..=2.0).contains(&sample.normalized_thickness)
                    })
                    || stroke.outline.fill_rule != CanonicalFillRule::NonZero
                    || stroke
                        .outline
                        .contours
                        .iter()
                        .any(|contour| contour.segments.is_empty())
            }) {
                return Err(RenderError::new(
                    "scene.layer.geometry",
                    "canonical stroke geometry must be finite",
                ));
            }
        }
    }
    if let Some(paints) = &layer.mark_paints {
        if matches!(layer.geometry, GeometryOutput::CanonicalStrokes(_)) {
            return Err(RenderError::new(
                "scene.layer.source_color",
                "guide-path strokes require solid channel paint",
            ));
        }
        let marks = match &layer.geometry {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
            GeometryOutput::CanonicalStrokes(strokes) => strokes.len(),
        };
        if paints.len() != marks {
            return Err(RenderError::new(
                "scene.layer.source_color",
                "source-colored paint count must match canonical mark count",
            ));
        }
        for paint in paints {
            for value in [paint.red, paint.green, paint.blue, paint.alpha] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(RenderError::new(
                        "scene.layer.source_color",
                        "source-colored paint must be finite values within 0.0..=1.0",
                    ));
                }
            }
            if paint.alpha != 1.0 {
                return Err(RenderError::new(
                    "scene.layer.source_color",
                    "sampled per-mark paint alpha must be exactly 1.0",
                ));
            }
        }
    }
    Ok(())
}

/// Hashes ordered canonical geometry and presentation without deriving any renderer behavior.
///
/// Legacy circular adapters retain their accepted byte contract. Generalized marks include their
/// complete family-site provenance, explicit fill semantics, and construction geometry so scene
/// cache identity cannot conflate distinct authored-shape output.
fn scene_fingerprint(
    canvas: &CanvasSpec,
    family: &str,
    realization: &str,
    model: Option<HalftoneChannelModel>,
    layers: &[RenderLayer],
) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    add_scene_bytes(
        &mut hash,
        b"toniator-stage-5-render-scene-v2".iter().copied(),
    );
    add_scene_bytes(&mut hash, family.bytes());
    add_scene_bytes(&mut hash, realization.bytes());
    if let Some(model) = model {
        add_scene_bytes(
            &mut hash,
            [match model {
                HalftoneChannelModel::Rgb => 1,
                HalftoneChannelModel::Cmyk => 2,
                HalftoneChannelModel::SourceColorAlpha => 3,
            }],
        );
    }
    add_scene_bytes(&mut hash, canvas.width.to_bits().to_le_bytes());
    add_scene_bytes(&mut hash, canvas.height.to_bits().to_le_bytes());
    // The complete scene identity includes ordered presentation. Family and
    // realization identities remain the independent geometry identities.
    for layer in layers {
        add_scene_bytes(&mut hash, layer.channel_id.0.to_le_bytes());
        add_scene_bytes(&mut hash, [u8::from(layer.visible)]);
        add_scene_bytes(&mut hash, layer.color.red.to_bits().to_le_bytes());
        add_scene_bytes(&mut hash, layer.color.green.to_bits().to_le_bytes());
        add_scene_bytes(&mut hash, layer.color.blue.to_bits().to_le_bytes());
        add_scene_bytes(&mut hash, layer.color.alpha.to_bits().to_le_bytes());
        add_scene_bytes(&mut hash, layer.opacity.to_bits().to_le_bytes());
        match &layer.geometry {
            GeometryOutput::CircularMarks(marks) => {
                add_scene_bytes(&mut hash, (marks.len() as u64).to_le_bytes());
                for mark in marks {
                    add_scene_bytes(
                        &mut hash,
                        mark.source_site_id.first_dimension_id.to_le_bytes(),
                    );
                    add_scene_bytes(&mut hash, mark.source_site_id.first_index.to_le_bytes());
                    add_scene_bytes(
                        &mut hash,
                        mark.source_site_id.second_dimension_id.to_le_bytes(),
                    );
                    add_scene_bytes(&mut hash, mark.source_site_id.second_index.to_le_bytes());
                    add_scene_bytes(&mut hash, mark.center.x.to_bits().to_le_bytes());
                    add_scene_bytes(&mut hash, mark.center.y.to_bits().to_le_bytes());
                    add_scene_bytes(&mut hash, mark.radius.to_bits().to_le_bytes());
                    add_scene_bytes(
                        &mut hash,
                        [match mark.scope {
                            toniator_geometry::SiteScope::Canvas => 1,
                            toniator_geometry::SiteScope::Guard => 2,
                        }],
                    );
                    for contributor in &mark.provenance.contributors {
                        append_scene_guide_instance(&mut hash, *contributor);
                    }
                }
            }
            GeometryOutput::CanonicalMarks(marks) => {
                add_scene_bytes(&mut hash, (marks.len() as u64).to_le_bytes());
                for mark in marks {
                    match mark {
                        CanonicalMark::Circle {
                            source_site_id,
                            center,
                            radius,
                            scope,
                            provenance,
                            fill_rule,
                        } => {
                            add_scene_bytes(&mut hash, [1]);
                            append_scene_family_site_id(&mut hash, *source_site_id);
                            append_scene_scope(&mut hash, *scope);
                            append_scene_provenance(&mut hash, provenance);
                            append_scene_fill_rule(&mut hash, *fill_rule);
                            add_scene_bytes(&mut hash, [1]);
                            add_scene_bytes(&mut hash, 1_u64.to_le_bytes());
                            add_scene_bytes(&mut hash, [0]);
                            add_scene_bytes(&mut hash, center.x.to_bits().to_le_bytes());
                            add_scene_bytes(&mut hash, center.y.to_bits().to_le_bytes());
                            add_scene_bytes(&mut hash, radius.to_bits().to_le_bytes());
                        }
                        CanonicalMark::ClosedPath(mark) => {
                            add_scene_bytes(&mut hash, [2]);
                            append_scene_family_site_id(&mut hash, mark.source_site_id);
                            append_scene_scope(&mut hash, mark.scope);
                            append_scene_provenance(&mut hash, &mark.provenance);
                            append_scene_fill_rule(&mut hash, mark.fill_rule);
                            add_scene_bytes(&mut hash, [2]);
                            add_scene_bytes(
                                &mut hash,
                                u64::try_from(mark.path.segments().len())
                                    .expect("usize fits u64")
                                    .to_le_bytes(),
                            );
                            add_scene_bytes(
                                &mut hash,
                                [match mark.path.closure() {
                                    toniator_geometry::PathClosure::Open => 1,
                                    toniator_geometry::PathClosure::Closed => 2,
                                }],
                            );
                            for segment in mark.path.segments() {
                                match segment {
                                    CurveSegment::Line(line) => {
                                        add_scene_bytes(&mut hash, [1]);
                                        for point in [line.start(), line.end()] {
                                            add_scene_bytes(
                                                &mut hash,
                                                point.x.to_bits().to_le_bytes(),
                                            );
                                            add_scene_bytes(
                                                &mut hash,
                                                point.y.to_bits().to_le_bytes(),
                                            );
                                        }
                                    }
                                    CurveSegment::CubicBezier(cubic) => {
                                        add_scene_bytes(&mut hash, [2]);
                                        for point in [
                                            cubic.start(),
                                            cubic.control_1(),
                                            cubic.control_2(),
                                            cubic.end(),
                                        ] {
                                            add_scene_bytes(
                                                &mut hash,
                                                point.x.to_bits().to_le_bytes(),
                                            );
                                            add_scene_bytes(
                                                &mut hash,
                                                point.y.to_bits().to_le_bytes(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            GeometryOutput::CanonicalStrokes(strokes) => {
                add_scene_bytes(&mut hash, [3]);
                add_scene_bytes(&mut hash, (strokes.len() as u64).to_le_bytes());
                for stroke in strokes {
                    add_scene_bytes(&mut hash, stroke.source_guide_id.dimension_id.to_le_bytes());
                    add_scene_bytes(&mut hash, stroke.source_guide_id.index.to_le_bytes());
                    add_scene_bytes(
                        &mut hash,
                        stroke.source_guide_id.component_ordinal.to_le_bytes(),
                    );
                    add_scene_bytes(
                        &mut hash,
                        stroke
                            .source_structure_id
                            .map_or(0, |id| id.0)
                            .to_le_bytes(),
                    );
                    add_scene_bytes(&mut hash, stroke.nominal_basis.to_bits().to_le_bytes());
                    add_scene_bytes(
                        &mut hash,
                        [
                            match stroke.style.join {
                                toniator_domain::StrokeJoin::Round => 1,
                            },
                            match stroke.style.cap {
                                toniator_domain::StrokeCap::Round => 1,
                            },
                        ],
                    );
                    add_scene_bytes(
                        &mut hash,
                        [match stroke.path.closure() {
                            toniator_geometry::PathClosure::Open => 1,
                            toniator_geometry::PathClosure::Closed => 2,
                        }],
                    );
                    add_scene_bytes(
                        &mut hash,
                        (stroke.path.segments().len() as u64).to_le_bytes(),
                    );
                    for segment in stroke.path.segments() {
                        match segment {
                            CurveSegment::Line(line) => {
                                add_scene_bytes(&mut hash, [1]);
                                for point in [line.start(), line.end()] {
                                    add_scene_bytes(&mut hash, point.x.to_bits().to_le_bytes());
                                    add_scene_bytes(&mut hash, point.y.to_bits().to_le_bytes());
                                }
                            }
                            CurveSegment::CubicBezier(cubic) => {
                                add_scene_bytes(&mut hash, [2]);
                                for point in [
                                    cubic.start(),
                                    cubic.control_1(),
                                    cubic.control_2(),
                                    cubic.end(),
                                ] {
                                    add_scene_bytes(&mut hash, point.x.to_bits().to_le_bytes());
                                    add_scene_bytes(&mut hash, point.y.to_bits().to_le_bytes());
                                }
                            }
                        }
                    }
                    for sample in &stroke.profile {
                        add_scene_bytes(&mut hash, sample.center.x.to_bits().to_le_bytes());
                        add_scene_bytes(&mut hash, sample.center.y.to_bits().to_le_bytes());
                        add_scene_bytes(&mut hash, sample.location.segment_index().to_le_bytes());
                        add_scene_bytes(
                            &mut hash,
                            sample.location.parameter().to_bits().to_le_bytes(),
                        );
                        add_scene_bytes(
                            &mut hash,
                            sample.normalized_thickness.to_bits().to_le_bytes(),
                        );
                        add_scene_bytes(&mut hash, sample.width.to_bits().to_le_bytes());
                    }
                    append_scene_fill_rule(&mut hash, stroke.outline.fill_rule);
                    add_scene_bytes(
                        &mut hash,
                        (stroke.outline.contours.len() as u64).to_le_bytes(),
                    );
                    for contour in &stroke.outline.contours {
                        add_scene_bytes(&mut hash, (contour.segments.len() as u64).to_le_bytes());
                        for segment in &contour.segments {
                            append_scene_outline_segment(&mut hash, segment);
                        }
                    }
                }
            }
        }
        if model.is_some() {
            if let Some(paints) = &layer.mark_paints {
                add_scene_bytes(&mut hash, [1]);
                for paint in paints {
                    add_scene_bytes(&mut hash, paint.red.to_bits().to_le_bytes());
                    add_scene_bytes(&mut hash, paint.green.to_bits().to_le_bytes());
                    add_scene_bytes(&mut hash, paint.blue.to_bits().to_le_bytes());
                    add_scene_bytes(&mut hash, paint.alpha.to_bits().to_le_bytes());
                }
            } else {
                add_scene_bytes(&mut hash, [0]);
            }
        }
    }
    format!("fnv1a64:{hash:016x}")
}

/// Applies one FNV-1a byte sequence at the scene identity boundary.
fn add_scene_bytes(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
    for byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Appends a generalized canonical mark's evaluator-emission ID in a fixed binary form.
fn append_scene_family_site_id(hash: &mut u64, site_id: toniator_geometry::FamilySiteId) {
    add_scene_bytes(hash, site_id.mechanism_id.0.to_le_bytes());
    add_scene_bytes(
        hash,
        u64::try_from(site_id.ordinal)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
}

/// Appends the final canvas-scope discriminator retained by generalized canonical geometry.
fn append_scene_scope(hash: &mut u64, scope: toniator_geometry::SiteScope) {
    add_scene_bytes(
        hash,
        [match scope {
            toniator_geometry::SiteScope::Canvas => 1,
            toniator_geometry::SiteScope::Guard => 2,
        }],
    );
}

/// Appends the complete discriminant and ordered payload of truthful family-site provenance.
fn append_scene_provenance(hash: &mut u64, provenance: &toniator_geometry::FamilySiteProvenance) {
    match provenance {
        toniator_geometry::FamilySiteProvenance::GuideIntersection { contributors } => {
            add_scene_bytes(hash, [1]);
            append_scene_guide_instances(hash, contributors);
        }
        toniator_geometry::FamilySiteProvenance::AlongGuide {
            guide_id,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            add_scene_bytes(hash, [2]);
            append_scene_guide_instance(hash, *guide_id);
            add_scene_bytes(
                hash,
                u64::try_from(*guide_order)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            add_scene_bytes(hash, sequence.to_le_bytes());
            add_scene_bytes(hash, absolute_arc_position_bits.to_le_bytes());
            add_scene_bytes(hash, local_arc_position_bits.to_le_bytes());
        }
        toniator_geometry::FamilySiteProvenance::Random {
            candidate_ordinal,
            accepted_ordinal,
            exclusion_neighbor_ordinal,
        } => {
            add_scene_bytes(hash, [3]);
            for value in [candidate_ordinal, accepted_ordinal] {
                add_scene_bytes(
                    hash,
                    u64::try_from(*value).expect("usize fits u64").to_le_bytes(),
                );
            }
            match exclusion_neighbor_ordinal {
                Some(value) => {
                    add_scene_bytes(hash, [1]);
                    add_scene_bytes(
                        hash,
                        u64::try_from(*value).expect("usize fits u64").to_le_bytes(),
                    );
                }
                None => add_scene_bytes(hash, [0]),
            }
        }
        toniator_geometry::FamilySiteProvenance::CurveGuideIntersection { contributors } => {
            add_scene_bytes(hash, [4]);
            add_scene_bytes(
                hash,
                u64::try_from(contributors.len())
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            for contributor in contributors {
                append_scene_curve_location(hash, contributor);
            }
        }
        toniator_geometry::FamilySiteProvenance::CurveAlongGuide {
            location,
            guide_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            add_scene_bytes(hash, [5]);
            append_scene_curve_location(hash, location);
            add_scene_bytes(
                hash,
                u64::try_from(*guide_order)
                    .expect("usize fits u64")
                    .to_le_bytes(),
            );
            add_scene_bytes(hash, sequence.to_le_bytes());
            add_scene_bytes(hash, absolute_arc_position_bits.to_le_bytes());
            add_scene_bytes(hash, local_arc_position_bits.to_le_bytes());
        }
    }
}

/// Appends one ordered straight-guide contributor list with an explicit count delimiter.
fn append_scene_guide_instances(
    hash: &mut u64,
    contributors: &[toniator_geometry::GuideInstanceId],
) {
    add_scene_bytes(
        hash,
        u64::try_from(contributors.len())
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    for contributor in contributors {
        append_scene_guide_instance(hash, *contributor);
    }
}

/// Appends one complete dimension/index/component guide identity.
fn append_scene_guide_instance(hash: &mut u64, guide: toniator_geometry::GuideInstanceId) {
    add_scene_bytes(hash, guide.dimension_id.to_le_bytes());
    add_scene_bytes(hash, guide.index.to_le_bytes());
    add_scene_bytes(hash, guide.component_ordinal.to_le_bytes());
}

/// Appends one exact curve contributor location.
fn append_scene_curve_location(
    hash: &mut u64,
    location: &toniator_geometry::GuidePathLocationProvenance,
) {
    append_scene_guide_instance(hash, location.guide_id);
    add_scene_bytes(
        hash,
        u64::try_from(location.segment_index)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    add_scene_bytes(hash, location.parameter_bits.to_le_bytes());
}

/// Appends canonical fill semantics explicitly instead of relying on renderer defaults.
fn append_scene_fill_rule(hash: &mut u64, fill_rule: CanonicalFillRule) {
    add_scene_bytes(
        hash,
        [match fill_rule {
            CanonicalFillRule::EvenOdd => 1,
            CanonicalFillRule::NonZero => 2,
        }],
    );
}

/// Appends one derived outline segment in a stable scene-identity representation.
fn append_scene_outline_segment(hash: &mut u64, segment: &CurveSegment) {
    match segment {
        CurveSegment::Line(line) => {
            add_scene_bytes(hash, [1]);
            for point in [line.start(), line.end()] {
                add_scene_bytes(hash, point.x.to_bits().to_le_bytes());
                add_scene_bytes(hash, point.y.to_bits().to_le_bytes());
            }
        }
        CurveSegment::CubicBezier(cubic) => {
            add_scene_bytes(hash, [2]);
            for point in [
                cubic.start(),
                cubic.control_1(),
                cubic.control_2(),
                cubic.end(),
            ] {
                add_scene_bytes(hash, point.x.to_bits().to_le_bytes());
                add_scene_bytes(hash, point.y.to_bits().to_le_bytes());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterBackground {
    OpaqueBlack,
    OpaqueWhite,
    Transparent,
}

/// Final-consumer raster edge policy. This is deliberately absent from the
/// canonical scene: it affects only PNG consumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RasterAntialiasing {
    On,
    Off,
}

/// Request-local generalized-mark work state shared across all layers and marks.
struct RasterWork<'a> {
    remaining_edges: usize,
    antialiasing: RasterAntialiasing,
    is_cancelled: &'a dyn Fn() -> bool,
}

impl<'a> RasterWork<'a> {
    /// Initializes one nonzero request-wide edge budget and caller-selected sampling policy.
    fn new(
        limits: RasterizationLimits,
        antialiasing: RasterAntialiasing,
        is_cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            remaining_edges: limits.max_flattened_edges(),
            antialiasing,
            is_cancelled,
        }
    }

    /// Rejects cancellation before a bounded raster boundary can mutate a local surface.
    fn check(&self) -> Result<(), RenderError> {
        (!(self.is_cancelled)())
            .then_some(())
            .ok_or(RenderError::new(
                "evaluation.cancelled",
                "rasterization was cancelled",
            ))
    }

    /// Consumes one concrete flattened edge from the entire request budget.
    fn edge(&mut self) -> Result<(), RenderError> {
        self.check()?;
        self.remaining_edges = self.remaining_edges.checked_sub(1).ok_or(RenderError::new(
            "raster.limits.flattened_edges",
            "flattened raster edge limit exceeded",
        ))?;
        Ok(())
    }
}

/// Checked pixel extent for a final PNG consumer. It maps canonical document
/// coordinates to output pixels and never changes the scene canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputRasterTarget {
    width: u32,
    height: u32,
}

impl OutputRasterTarget {
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::new(
                "output.target",
                "dimensions must be positive",
            ));
        }
        if u64::from(width) * u64::from(height) > MAX_OUTPUT_PIXELS {
            return Err(RenderError::new(
                "output.target",
                "pixel count exceeds output safety limit",
            ));
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Checked, renderer-owned pixel extent for a derived transparent preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewRasterTarget {
    width: u32,
    height: u32,
}

impl PreviewRasterTarget {
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::new(
                "preview.target",
                "dimensions must be positive",
            ));
        }
        if u64::from(width) * u64::from(height) > MAX_PREVIEW_PIXELS {
            return Err(RenderError::new(
                "preview.target",
                "pixel count exceeds preview safety limit",
            ));
        }
        Ok(Self { width, height })
    }
    pub const fn width(self) -> u32 {
        self.width
    }
    pub const fn height(self) -> u32 {
        self.height
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterSurface {
    width: u32,
    height: u32,
    /// Straight, 8-bit sRGBA in row-major pixel order.
    pixels: Vec<u8>,
}

impl RasterSurface {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::new(
                "raster.surface",
                "dimensions must be positive",
            ));
        }
        if pixels.len() != width as usize * height as usize * 4 {
            return Err(RenderError::new(
                "raster.surface",
                "straight sRGBA buffer length does not match dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug)]
struct PremultipliedLinearPixel {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

/// Rasterizes one canonical scene at native integral canvas dimensions with transparent-aware AA.
///
/// # Errors
///
/// Returns stable target, cancellation, flattening, edge-limit, or surface diagnostics.
pub fn rasterize(
    scene: &RenderScene,
    background: RasterBackground,
) -> Result<RasterSurface, RenderError> {
    rasterize_cancellable(scene, background, RasterizationLimits::default(), &|| false)
}

/// Rasterizes canonical geometry with one caller-owned edge budget and cancellation probe.
///
/// # Errors
///
/// Returns cancellation or any stable target, flattening, edge-limit, or surface diagnostic.
pub fn rasterize_cancellable(
    scene: &RenderScene,
    background: RasterBackground,
    limits: RasterizationLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RasterSurface, RenderError> {
    let mut work = RasterWork::new(limits, RasterAntialiasing::On, is_cancelled);
    // All native document-canvas rasterization crosses the same checked final
    // consumer boundary as an explicit output target before allocation.
    let native_target = native_output_target(scene)?;
    if scene.model.is_none() {
        return rasterize_stage5(scene, background, &mut work);
    }

    let width = native_target.width;
    let height = native_target.height;
    let layer_pixels = scene
        .layers
        .iter()
        .map(|layer| rasterize_layer(layer, width, height, &mut work))
        .collect::<Result<Vec<_>, _>>()?;
    let mut linear_pixels = compose_model(scene.model.expect("modeled scene"), &layer_pixels);
    apply_background(&mut linear_pixels, background);
    pixels_from_linear(width, height, linear_pixels)
}

/// Rerasterizes immutable canonical geometry for a PNG consumer. The
/// accepted native call remains [`rasterize`] (no target, antialiasing on), so
/// existing preview and baseline PNG bytes retain their established path.
///
/// # Errors
///
/// Returns stable target, flattening, edge-limit, or surface diagnostics.
pub fn rasterize_output(
    scene: &RenderScene,
    background: RasterBackground,
    target: Option<OutputRasterTarget>,
    antialiasing: RasterAntialiasing,
) -> Result<RasterSurface, RenderError> {
    rasterize_output_cancellable(
        scene,
        background,
        target,
        antialiasing,
        RasterizationLimits::default(),
        &|| false,
    )
}

/// Rerasterizes explicit output with a caller-owned shared edge budget and cancellation probe.
///
/// # Errors
///
/// Returns cancellation or stable target, flattening, edge-limit, or surface diagnostics.
pub fn rasterize_output_cancellable(
    scene: &RenderScene,
    background: RasterBackground,
    target: Option<OutputRasterTarget>,
    antialiasing: RasterAntialiasing,
    limits: RasterizationLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RasterSurface, RenderError> {
    if target.is_none() && matches!(antialiasing, RasterAntialiasing::On) {
        return rasterize_cancellable(scene, background, limits, is_cancelled);
    }
    let target = match target {
        Some(target) => target,
        None => native_output_target(scene)?,
    };
    let transform = OutputTransform::for_scene(scene, target);
    let mut work = RasterWork::new(limits, antialiasing, is_cancelled);
    let layers = scene
        .layers
        .iter()
        .map(|layer| rasterize_layer_for_output(layer, target, transform, antialiasing, &mut work))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pixels = match scene.model {
        Some(model) => compose_model(model, &layers),
        None => {
            let mut destination = vec![
                background_pixel(RasterBackground::Transparent);
                target.width as usize * target.height as usize
            ];
            for layer in layers {
                for (destination_pixel, source) in destination.iter_mut().zip(layer) {
                    source_over(destination_pixel, source);
                }
            }
            destination
        }
    };
    apply_background(&mut pixels, background);
    pixels_from_linear(target.width, target.height, pixels)
}

/// Stable consumer-only identity suitable for a PNG cache. It intentionally
/// includes no source, document, family, realization, or scene mutation: the
/// scene fingerprint is the complete immutable canonical input.
pub fn raster_output_identity(
    scene: &RenderScene,
    background: RasterBackground,
    target: Option<OutputRasterTarget>,
    antialiasing: RasterAntialiasing,
) -> String {
    let background = match background {
        RasterBackground::OpaqueBlack => "black",
        RasterBackground::OpaqueWhite => "white",
        RasterBackground::Transparent => "transparent",
    };
    let target = target.map_or_else(
        || "native".to_owned(),
        |target| format!("{}x{}", target.width, target.height),
    );
    let antialiasing = match antialiasing {
        RasterAntialiasing::On => "on",
        RasterAntialiasing::Off => "off",
    };
    format!(
        "{}:toniator-raster-output-v1:{background}:{target}:{antialiasing}",
        scene.identity.scene_fingerprint
    )
}

/// Rerasterizes canonical scene geometry into a transparent fitted target.
/// This is intentionally not a resample of [`RasterSurface`].
///
/// # Errors
///
/// Returns stable preview-target, flattening, edge-limit, or surface diagnostics.
pub fn rasterize_preview(
    scene: &RenderScene,
    target: PreviewRasterTarget,
) -> Result<RasterSurface, RenderError> {
    rasterize_preview_cancellable(scene, target, RasterizationLimits::default(), &|| false)
}

/// Rasterizes a fitted preview with one caller-owned edge budget and cancellation probe.
///
/// # Errors
///
/// Returns cancellation or stable preview-target, flattening, edge-limit, or surface diagnostics.
pub fn rasterize_preview_cancellable(
    scene: &RenderScene,
    target: PreviewRasterTarget,
    limits: RasterizationLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RasterSurface, RenderError> {
    let transform = PreviewTransform::for_scene(scene, target);
    let width = target.width;
    let height = target.height;
    let mut work = RasterWork::new(limits, RasterAntialiasing::On, is_cancelled);
    let layers = scene
        .layers
        .iter()
        .map(|layer| rasterize_layer_with_transform(layer, width, height, transform, &mut work))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pixels = match scene.model {
        Some(model) => compose_model(model, &layers),
        None => {
            let mut destination = vec![
                background_pixel(RasterBackground::Transparent);
                width as usize * height as usize
            ];
            for layer in layers {
                for (destination_pixel, source) in destination.iter_mut().zip(layer) {
                    source_over(destination_pixel, source);
                }
            }
            destination
        }
    };
    apply_background(&mut pixels, RasterBackground::Transparent);
    pixels_from_linear(width, height, pixels)
}

#[derive(Clone, Copy)]
struct PreviewTransform {
    scale: f64,
    offset_x: f64,
    offset_y: f64,
    right: f64,
    bottom: f64,
}
impl PreviewTransform {
    fn for_scene(scene: &RenderScene, target: PreviewRasterTarget) -> Self {
        let scale = (f64::from(target.width) / scene.canvas.width)
            .min(f64::from(target.height) / scene.canvas.height);
        let offset_x = (f64::from(target.width) - scene.canvas.width * scale) / 2.0;
        let offset_y = (f64::from(target.height) - scene.canvas.height * scale) / 2.0;
        Self {
            scale,
            offset_x,
            offset_y,
            right: offset_x + scene.canvas.width * scale,
            bottom: offset_y + scene.canvas.height * scale,
        }
    }
}

/// Retains the accepted Stage 5 raster path byte-for-byte for callers which
/// have not opted into an explicit Stage 9C model.
///
/// # Errors
///
/// Returns target, cancellation, flattening, edge-limit, or surface diagnostics.
fn rasterize_stage5(
    scene: &RenderScene,
    background: RasterBackground,
    work: &mut RasterWork<'_>,
) -> Result<RasterSurface, RenderError> {
    let target = native_output_target(scene)?;
    let width = target.width;
    let height = target.height;
    let background = background_pixel(background);
    let mut linear_pixels = vec![background; width as usize * height as usize];
    for layer in &scene.layers {
        if !layer.visible {
            continue;
        }
        match &layer.geometry {
            GeometryOutput::CircularMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_circle(
                        &mut linear_pixels,
                        width,
                        height,
                        mark,
                        layer.mark_paint(index),
                        layer.opacity,
                        work,
                    )?;
                }
            }
            GeometryOutput::CanonicalMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_canonical_mark(
                        &mut linear_pixels,
                        width,
                        height,
                        mark,
                        layer.mark_paint(index),
                        layer.opacity,
                        CanonicalRasterTransform::native(),
                        work,
                    )?;
                }
            }
            GeometryOutput::CanonicalStrokes(strokes) => {
                for stroke in strokes {
                    composite_canonical_stroke(
                        &mut linear_pixels,
                        width,
                        height,
                        stroke,
                        &layer.color,
                        layer.opacity,
                        CanonicalRasterTransform::native(),
                        work,
                    )?;
                }
            }
        }
    }
    pixels_from_linear(width, height, linear_pixels)
}

fn native_output_target(scene: &RenderScene) -> Result<OutputRasterTarget, RenderError> {
    OutputRasterTarget::new(
        integral_dimension(scene.canvas.width)?,
        integral_dimension(scene.canvas.height)?,
    )
}

/// Converts one final-consumer background choice into premultiplied linear storage.
fn background_pixel(background: RasterBackground) -> PremultipliedLinearPixel {
    match background {
        RasterBackground::OpaqueBlack => PremultipliedLinearPixel {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        RasterBackground::OpaqueWhite => PremultipliedLinearPixel {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        },
        RasterBackground::Transparent => PremultipliedLinearPixel {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        },
    }
}

/// Rasterizes one native-coordinate layer into private premultiplied linear storage.
///
/// # Errors
///
/// Returns cancellation, flattening, or request-wide edge-limit diagnostics.
fn rasterize_layer(
    layer: &RenderLayer,
    width: u32,
    height: u32,
    work: &mut RasterWork<'_>,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let mut pixels =
        vec![background_pixel(RasterBackground::Transparent); width as usize * height as usize];
    if !layer.visible {
        return Ok(pixels);
    }
    match &layer.geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_circle(
                    &mut pixels,
                    width,
                    height,
                    mark,
                    layer.mark_paint(index),
                    layer.opacity,
                    work,
                )?;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_canonical_mark(
                    &mut pixels,
                    width,
                    height,
                    mark,
                    layer.mark_paint(index),
                    layer.opacity,
                    CanonicalRasterTransform::native(),
                    work,
                )?;
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for stroke in strokes {
                composite_canonical_stroke(
                    &mut pixels,
                    width,
                    height,
                    stroke,
                    &layer.color,
                    layer.opacity,
                    CanonicalRasterTransform::native(),
                    work,
                )?;
            }
        }
    }
    Ok(pixels)
}

/// Rasterizes one fitted-preview layer with final target clipping and shared work accounting.
///
/// # Errors
///
/// Returns cancellation, flattening, or request-wide edge-limit diagnostics.
fn rasterize_layer_with_transform(
    layer: &RenderLayer,
    width: u32,
    height: u32,
    transform: PreviewTransform,
    work: &mut RasterWork<'_>,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let mut pixels =
        vec![background_pixel(RasterBackground::Transparent); width as usize * height as usize];
    if !layer.visible {
        return Ok(pixels);
    }
    match &layer.geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_circle_transformed(
                    &mut pixels,
                    width,
                    height,
                    mark,
                    layer.mark_paint(index),
                    layer.opacity,
                    transform,
                    work,
                )?;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_canonical_mark(
                    &mut pixels,
                    width,
                    height,
                    mark,
                    layer.mark_paint(index),
                    layer.opacity,
                    CanonicalRasterTransform::preview(transform),
                    work,
                )?;
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for stroke in strokes {
                composite_canonical_stroke(
                    &mut pixels,
                    width,
                    height,
                    stroke,
                    &layer.color,
                    layer.opacity,
                    CanonicalRasterTransform::preview(transform),
                    work,
                )?;
            }
        }
    }
    Ok(pixels)
}

#[derive(Clone, Copy)]
struct OutputTransform {
    scale_x: f64,
    scale_y: f64,
}

/// Maps immutable canonical coordinates into one concrete raster target before adaptive flattening.
#[derive(Clone, Copy)]
struct CanonicalRasterTransform {
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
}

impl CanonicalRasterTransform {
    /// Returns the native canvas-to-pixel transform without reinterpreting legacy circle output.
    const fn native() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// Converts one preview fit transform into target-pixel coordinates.
    fn preview(value: PreviewTransform) -> Self {
        Self {
            scale_x: value.scale,
            scale_y: value.scale,
            offset_x: value.offset_x,
            offset_y: value.offset_y,
        }
    }

    /// Converts one explicit anisotropic output transform into target-pixel coordinates.
    const fn output(value: OutputTransform) -> Self {
        Self {
            scale_x: value.scale_x,
            scale_y: value.scale_y,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// Maps one finite canonical point into the final raster coordinate system.
    fn point(self, point: Point2) -> Point2 {
        Point2::new(
            self.offset_x + point.x * self.scale_x,
            self.offset_y + point.y * self.scale_y,
        )
    }
}

impl OutputTransform {
    /// Derives an anisotropic final-output scale without changing canonical geometry.
    fn for_scene(scene: &RenderScene, target: OutputRasterTarget) -> Self {
        Self {
            scale_x: f64::from(target.width) / scene.canvas.width,
            scale_y: f64::from(target.height) / scene.canvas.height,
        }
    }
}

/// Rasterizes one explicit-output layer in concrete target coordinates under shared work policy.
///
/// # Errors
///
/// Returns cancellation, flattening, or request-wide edge-limit diagnostics.
fn rasterize_layer_for_output(
    layer: &RenderLayer,
    target: OutputRasterTarget,
    transform: OutputTransform,
    antialiasing: RasterAntialiasing,
    work: &mut RasterWork<'_>,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let mut pixels = vec![
        background_pixel(RasterBackground::Transparent);
        target.width as usize * target.height as usize
    ];
    if !layer.visible {
        return Ok(pixels);
    }
    match &layer.geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_ellipse(
                    &mut pixels,
                    target.width,
                    target.height,
                    mark.center.x * transform.scale_x,
                    mark.center.y * transform.scale_y,
                    mark.radius * transform.scale_x,
                    mark.radius * transform.scale_y,
                    layer.mark_paint(index),
                    layer.opacity,
                    antialiasing,
                    None,
                )?;
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                composite_canonical_mark(
                    &mut pixels,
                    target.width,
                    target.height,
                    mark,
                    layer.mark_paint(index),
                    layer.opacity,
                    CanonicalRasterTransform::output(transform),
                    work,
                )?;
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for stroke in strokes {
                composite_canonical_stroke(
                    &mut pixels,
                    target.width,
                    target.height,
                    stroke,
                    &layer.color,
                    layer.opacity,
                    CanonicalRasterTransform::output(transform),
                    work,
                )?;
            }
        }
    }
    Ok(pixels)
}

fn compose_model(
    model: HalftoneChannelModel,
    layers: &[Vec<PremultipliedLinearPixel>],
) -> Vec<PremultipliedLinearPixel> {
    let count = layers.first().map_or(0, Vec::len);
    match model {
        HalftoneChannelModel::Rgb => (0..count)
            .map(|index| {
                let mut pixel = background_pixel(RasterBackground::Transparent);
                for layer in layers {
                    let source = layer[index];
                    pixel.red = (pixel.red + source.red).clamp(0.0, 1.0);
                    pixel.green = (pixel.green + source.green).clamp(0.0, 1.0);
                    pixel.blue = (pixel.blue + source.blue).clamp(0.0, 1.0);
                    pixel.alpha = (pixel.alpha + source.alpha).clamp(0.0, 1.0);
                }
                pixel
            })
            .collect(),
        HalftoneChannelModel::Cmyk => (0..count)
            .map(|index| {
                let mut transmittance = [1.0; 3];
                let mut uncovered = 1.0;
                for layer in layers {
                    let source = layer[index];
                    if source.alpha > 0.0 {
                        let straight = [
                            source.red / source.alpha,
                            source.green / source.alpha,
                            source.blue / source.alpha,
                        ];
                        for component in 0..3 {
                            transmittance[component] *=
                                1.0 - source.alpha * (1.0 - straight[component]);
                        }
                        uncovered *= 1.0 - source.alpha;
                    }
                }
                let alpha = (1.0 - uncovered).clamp(0.0, 1.0);
                PremultipliedLinearPixel {
                    red: boundary_clamp(transmittance[0] - (1.0 - alpha)),
                    green: boundary_clamp(transmittance[1] - (1.0 - alpha)),
                    blue: boundary_clamp(transmittance[2] - (1.0 - alpha)),
                    alpha,
                }
            })
            .collect(),
        HalftoneChannelModel::SourceColorAlpha => (0..count)
            .map(|index| {
                let mut destination = background_pixel(RasterBackground::Transparent);
                for layer in layers {
                    source_over(&mut destination, layer[index]);
                }
                destination
            })
            .collect(),
    }
}

fn boundary_clamp(value: f64) -> f64 {
    const EPSILON: f64 = 1e-12;
    if (-EPSILON..0.0).contains(&value) {
        0.0
    } else if (1.0..=1.0 + EPSILON).contains(&value) {
        1.0
    } else {
        value
    }
}

fn source_over(destination: &mut PremultipliedLinearPixel, source: PremultipliedLinearPixel) {
    let remaining = 1.0 - source.alpha;
    destination.red = source.red + destination.red * remaining;
    destination.green = source.green + destination.green * remaining;
    destination.blue = source.blue + destination.blue * remaining;
    destination.alpha = source.alpha + destination.alpha * remaining;
}

fn apply_background(pixels: &mut [PremultipliedLinearPixel], background: RasterBackground) {
    if matches!(background, RasterBackground::Transparent) {
        return;
    }
    let background = background_pixel(background);
    for pixel in pixels {
        let remaining = 1.0 - pixel.alpha;
        pixel.red += background.red * remaining;
        pixel.green += background.green * remaining;
        pixel.blue += background.blue * remaining;
        pixel.alpha = 1.0;
    }
}

fn pixels_from_linear(
    width: u32,
    height: u32,
    linear_pixels: Vec<PremultipliedLinearPixel>,
) -> Result<RasterSurface, RenderError> {
    let mut pixels = Vec::with_capacity(linear_pixels.len() * 4);
    for pixel in linear_pixels {
        let alpha = pixel.alpha.clamp(0.0, 1.0);
        let (red, green, blue) = if alpha == 0.0 {
            (0.0, 0.0, 0.0)
        } else {
            (pixel.red / alpha, pixel.green / alpha, pixel.blue / alpha)
        };
        pixels.extend([
            quantize_srgb(red),
            quantize_srgb(green),
            quantize_srgb(blue),
            quantize_linear(alpha),
        ]);
    }
    RasterSurface::new(width, height, pixels)
}

fn integral_dimension(value: f64) -> Result<u32, RenderError> {
    if !value.is_finite() || value <= 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(RenderError::new(
            "raster.canvas",
            "canvas dimensions must be positive finite integral document units",
        ));
    }
    Ok(value as u32)
}

/// Composites one retained native circle while polling the request cancellation authority.
///
/// # Errors
///
/// Returns cancellation before further local pixel mutation.
fn composite_circle(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    mark: &CanonicalCircleMark,
    color: &ColorValue,
    opacity: f64,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    let min_x = (mark.center.x - mark.radius - 1.0).floor().max(0.0) as u32;
    let min_y = (mark.center.y - mark.radius - 1.0).floor().max(0.0) as u32;
    let max_x = (mark.center.x + mark.radius + 1.0)
        .ceil()
        .min(f64::from(width)) as u32;
    let max_y = (mark.center.y + mark.radius + 1.0)
        .ceil()
        .min(f64::from(height)) as u32;
    for y in min_y..max_y {
        work.check()?;
        for x in min_x..max_x {
            let coverage = circle_coverage(mark, x, y, work)?;
            if coverage == 0.0 {
                continue;
            }
            let source_alpha = (color.alpha * opacity * coverage).clamp(0.0, 1.0);
            let destination = &mut pixels[y as usize * width as usize + x as usize];
            let remaining = 1.0 - source_alpha;
            destination.red = color.red * source_alpha + destination.red * remaining;
            destination.green = color.green * source_alpha + destination.green * remaining;
            destination.blue = color.blue * source_alpha + destination.blue * remaining;
            destination.alpha = source_alpha + destination.alpha * remaining;
        }
    }
    Ok(())
}

/// Rasterizes one generalized canonical mark with the fixed 8x8 coverage contract.
///
/// Closed paths are deterministically flattened to at most one sixty-fourth of
/// a native output pixel before even-odd sampling; clipping remains the pixel
/// bounds supplied by the final renderer consumer.
///
/// # Errors
///
/// Returns cancellation, numeric flattening, or request-wide edge-limit diagnostics.
#[allow(clippy::too_many_arguments)]
fn composite_canonical_mark(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    mark: &CanonicalMark,
    color: &ColorValue,
    opacity: f64,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    match mark {
        CanonicalMark::Circle { center, radius, .. } => {
            composite_ellipse(
                pixels,
                width,
                height,
                transform.offset_x + center.x * transform.scale_x,
                transform.offset_y + center.y * transform.scale_y,
                *radius * transform.scale_x,
                *radius * transform.scale_y,
                color,
                opacity,
                work.antialiasing,
                Some(work),
            )?;
            Ok(())
        }
        CanonicalMark::ClosedPath(mark) => {
            let edges = flattened_path_edges(&mark.path, transform, work)?;
            if edges.is_empty() {
                return Ok(());
            }
            let bounds_min = transform.point(mark.bounds.min);
            let bounds_max = transform.point(mark.bounds.max);
            let tolerance = 1.0 / 64.0;
            let min_x = (bounds_min.x - tolerance).floor().max(0.0) as u32;
            let min_y = (bounds_min.y - tolerance).floor().max(0.0) as u32;
            let max_x = (bounds_max.x + tolerance).ceil().min(f64::from(width)) as u32;
            let max_y = (bounds_max.y + tolerance).ceil().min(f64::from(height)) as u32;
            for y in min_y..max_y {
                work.check()?;
                for x in min_x..max_x {
                    let coverage = polygon_coverage_even_odd(&edges, x, y, work)?;
                    if coverage == 0.0 {
                        continue;
                    }
                    let source = PremultipliedLinearPixel {
                        red: color.red * color.alpha * opacity * coverage,
                        green: color.green * color.alpha * opacity * coverage,
                        blue: color.blue * color.alpha * opacity * coverage,
                        alpha: color.alpha * opacity * coverage,
                    };
                    source_over(
                        &mut pixels[y as usize * width as usize + x as usize],
                        source,
                    );
                }
            }
            Ok(())
        }
    }
}

/// Composites one canonical stroke once per pixel from its nonzero filled outline.
///
/// # Errors
///
/// Returns cancellation or finite raster bounds errors without changing the
/// canonical centerline or splitting it at the canvas boundary.
#[allow(clippy::too_many_arguments)]
fn composite_canonical_stroke(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    stroke: &CanonicalStroke,
    color: &ColorValue,
    opacity: f64,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    let Some(bounds) = stroke.outline.bounds else {
        return Ok(());
    };
    let min = transform.point(bounds.min);
    let max = transform.point(bounds.max);
    let min_x = (min.x - 1.0).floor().max(0.0) as u32;
    let min_y = (min.y - 1.0).floor().max(0.0) as u32;
    let max_x = (max.x + 1.0).ceil().min(f64::from(width)) as u32;
    let max_y = (max.y + 1.0).ceil().min(f64::from(height)) as u32;
    let edges = flattened_outline_edges(&stroke.outline, transform, work)?;
    if edges.is_empty() {
        return Ok(());
    }
    for y in min_y..max_y {
        work.check()?;
        for x in min_x..max_x {
            let mut covered = 0_u32;
            let samples = if matches!(work.antialiasing, RasterAntialiasing::On) {
                SUBPIXEL_GRID
            } else {
                1
            };
            for sy in 0..samples {
                for sx in 0..samples {
                    let point = Point2::new(
                        f64::from(x) + (f64::from(sx) + 0.5) / f64::from(samples),
                        f64::from(y) + (f64::from(sy) + 0.5) / f64::from(samples),
                    );
                    if point_in_nonzero_outline(&edges, point) {
                        covered += 1;
                    }
                }
            }
            let coverage = f64::from(covered) / f64::from(samples * samples);
            if coverage > 0.0 {
                let source = PremultipliedLinearPixel {
                    red: color.red * color.alpha * opacity * coverage,
                    green: color.green * color.alpha * opacity * coverage,
                    blue: color.blue * color.alpha * opacity * coverage,
                    alpha: color.alpha * opacity * coverage,
                };
                source_over(
                    &mut pixels[y as usize * width as usize + x as usize],
                    source,
                );
            }
        }
    }
    Ok(())
}

/// Flattens stored line and cubic geometry in authored order under the caller's checked edge bound.
///
/// # Errors
///
/// Returns cancellation, numeric subdivision, or request-wide edge-limit diagnostics.
fn flattened_path_edges(
    path: &toniator_geometry::CurvePath,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<Vec<(Point2, Point2)>, RenderError> {
    let mut points = Vec::new();
    for segment in path.segments() {
        match segment {
            CurveSegment::Line(line) => {
                if points.is_empty() {
                    points.push(transform.point(line.start()));
                }
                push_flattened_point(&mut points, transform.point(line.end()), work)?;
            }
            CurveSegment::CubicBezier(cubic) => {
                if points.is_empty() {
                    points.push(transform.point(cubic.start()));
                }
                flatten_cubic(
                    transform.point(cubic.start()),
                    transform.point(cubic.control_1()),
                    transform.point(cubic.control_2()),
                    transform.point(cubic.end()),
                    0,
                    &mut points,
                    work,
                )?;
            }
        }
    }
    Ok(points.windows(2).map(|pair| (pair[0], pair[1])).collect())
}

/// Flattens every independent canonical outline contour under the shared raster edge budget.
///
/// # Errors
///
/// Returns cancellation, numeric subdivision, or edge-limit diagnostics without changing
/// canonical outline topology or applying consumer clipping before raster sampling.
fn flattened_outline_edges(
    outline: &toniator_geometry::CanonicalFilledOutline,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<Vec<(Point2, Point2)>, RenderError> {
    let mut edges = Vec::new();
    for contour in &outline.contours {
        work.check()?;
        let mut points = Vec::new();
        for segment in &contour.segments {
            match segment {
                CurveSegment::Line(line) => {
                    if points.is_empty() {
                        points.push(transform.point(line.start()));
                    }
                    push_flattened_point(&mut points, transform.point(line.end()), work)?;
                }
                CurveSegment::CubicBezier(cubic) => {
                    if points.is_empty() {
                        points.push(transform.point(cubic.start()));
                    }
                    flatten_cubic(
                        transform.point(cubic.start()),
                        transform.point(cubic.control_1()),
                        transform.point(cubic.control_2()),
                        transform.point(cubic.end()),
                        0,
                        &mut points,
                        work,
                    )?;
                }
            }
        }
        edges.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
    }
    Ok(edges)
}

/// Tests nonzero winding coverage across every ordered outline contour.
fn point_in_nonzero_outline(edges: &[(Point2, Point2)], point: Point2) -> bool {
    let mut winding = 0_i32;
    for (start, end) in edges {
        if start.y <= point.y && end.y > point.y {
            let cross =
                (end.x - start.x) * (point.y - start.y) - (point.x - start.x) * (end.y - start.y);
            if cross > 0.0 {
                winding += 1;
            }
        } else if start.y > point.y && end.y <= point.y {
            let cross =
                (end.x - start.x) * (point.y - start.y) - (point.x - start.x) * (end.y - start.y);
            if cross < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

/// Recursively appends a deterministic cubic polyline whose control polygon is within 1/64 pixel flatness.
///
/// # Errors
///
/// Returns cancellation, numeric-stagnation/depth, or request-wide edge-limit diagnostics.
fn flatten_cubic(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
    depth: u8,
    points: &mut Vec<Point2>,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    let flatness = point_line_distance(b, a, d).max(point_line_distance(c, a, d));
    work.check()?;
    if flatness <= 1.0 / 64.0 {
        return push_flattened_point(points, d, work);
    }
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let mid = midpoint(abc, bcd);
    if depth >= 60 || mid == a || mid == d {
        return Err(RenderError::new(
            "raster.flatten.numeric",
            "cubic subdivision cannot meet output-pixel tolerance",
        ));
    }
    flatten_cubic(a, ab, abc, mid, depth + 1, points, work)?;
    flatten_cubic(mid, bcd, cd, d, depth + 1, points, work)
}

/// Appends one flattened endpoint only while the exact request edge budget remains available.
///
/// # Errors
///
/// Returns cancellation or the exact request-wide flattened-edge limit diagnostic.
fn push_flattened_point(
    points: &mut Vec<Point2>,
    point: Point2,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    work.edge()?;
    points.push(point);
    Ok(())
}

/// Measures one point's Euclidean distance from an infinite finite chord line.
fn point_line_distance(point: Point2, start: Point2, end: Point2) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return ((point.x - start.x).powi(2) + (point.y - start.y).powi(2)).sqrt();
    }
    ((dy * point.x - dx * point.y + end.x * start.y - end.y * start.x).abs()) / length
}

/// Returns the midpoint of two finite points without changing path topology.
fn midpoint(a: Point2, b: Point2) -> Point2 {
    Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// Computes accepted 8x8 even-odd coverage for one output pixel.
///
/// # Errors
///
/// Returns cancellation while sampling subpixels or traversing flattened edges.
fn polygon_coverage_even_odd(
    edges: &[(Point2, Point2)],
    x: u32,
    y: u32,
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    let mut inside = 0_u32;
    let samples = match work.antialiasing {
        RasterAntialiasing::On => SUBPIXEL_GRID,
        RasterAntialiasing::Off => 1,
    };
    for sub_y in 0..samples {
        for sub_x in 0..samples {
            work.check()?;
            let point = Point2::new(
                f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(samples),
                f64::from(y) + (f64::from(sub_y) + 0.5) / f64::from(samples),
            );
            let mut crossings = 0usize;
            for (a, b) in edges {
                work.check()?;
                if (a.y > point.y) != (b.y > point.y)
                    && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
                {
                    crossings += 1;
                }
            }
            if crossings % 2 == 1 {
                inside += 1;
            }
        }
    }
    Ok(f64::from(inside) / f64::from(samples * samples))
}

/// Composites one native or anisotropically transformed ellipse under optional shared cancellation.
///
/// # Errors
///
/// Returns cancellation while traversing bounded rows or subpixel samples.
#[allow(clippy::too_many_arguments)]
fn composite_ellipse(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
    color: &ColorValue,
    opacity: f64,
    antialiasing: RasterAntialiasing,
    work: Option<&RasterWork<'_>>,
) -> Result<(), RenderError> {
    let min_x = (center_x - radius_x - 1.0).floor().max(0.0) as u32;
    let min_y = (center_y - radius_y - 1.0).floor().max(0.0) as u32;
    let max_x = (center_x + radius_x + 1.0).ceil().min(f64::from(width)) as u32;
    let max_y = (center_y + radius_y + 1.0).ceil().min(f64::from(height)) as u32;
    for y in min_y..max_y {
        if let Some(work) = work {
            work.check()?;
        }
        for x in min_x..max_x {
            let coverage = ellipse_coverage(
                center_x,
                center_y,
                radius_x,
                radius_y,
                x,
                y,
                antialiasing,
                work,
            )?;
            if coverage == 0.0 {
                continue;
            }
            let source_alpha = (color.alpha * opacity * coverage).clamp(0.0, 1.0);
            let destination = &mut pixels[y as usize * width as usize + x as usize];
            let remaining = 1.0 - source_alpha;
            destination.red = color.red * source_alpha + destination.red * remaining;
            destination.green = color.green * source_alpha + destination.green * remaining;
            destination.blue = color.blue * source_alpha + destination.blue * remaining;
            destination.alpha = source_alpha + destination.alpha * remaining;
        }
    }
    Ok(())
}

/// Measures one ellipse's selected AA coverage at a concrete output pixel.
///
/// # Errors
///
/// Returns cancellation while traversing the selected subpixel grid.
#[allow(clippy::too_many_arguments)]
fn ellipse_coverage(
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
    x: u32,
    y: u32,
    antialiasing: RasterAntialiasing,
    work: Option<&RasterWork<'_>>,
) -> Result<f64, RenderError> {
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return Ok(0.0);
    }
    let samples = match antialiasing {
        RasterAntialiasing::On => SUBPIXEL_GRID,
        RasterAntialiasing::Off => 1,
    };
    let mut inside = 0_u32;
    for sample_y in 0..samples {
        for sample_x in 0..samples {
            if let Some(work) = work {
                work.check()?;
            }
            let point_x = f64::from(x) + (f64::from(sample_x) + 0.5) / f64::from(samples);
            let point_y = f64::from(y) + (f64::from(sample_y) + 0.5) / f64::from(samples);
            let dx = (point_x - center_x) / radius_x;
            let dy = (point_y - center_y) / radius_y;
            if dx.mul_add(dx, dy * dy) <= 1.0 {
                inside += 1;
            }
        }
    }
    Ok(f64::from(inside) / f64::from(samples * samples))
}

/// Measures one retained native circle through the accepted 8x8 coverage contract.
///
/// # Errors
///
/// Returns cancellation while traversing subpixel samples.
fn circle_coverage(
    mark: &CanonicalCircleMark,
    x: u32,
    y: u32,
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    circle_coverage_at(mark.center.x, mark.center.y, mark.radius, x, y, work)
}

/// Composites one retained preview circle inside fitted canvas bounds with cancellation polling.
///
/// # Errors
///
/// Returns cancellation while traversing bounded pixels or subpixel samples.
#[allow(clippy::too_many_arguments)]
fn composite_circle_transformed(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    mark: &CanonicalCircleMark,
    color: &ColorValue,
    opacity: f64,
    transform: PreviewTransform,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    let center_x = transform.offset_x + mark.center.x * transform.scale;
    let center_y = transform.offset_y + mark.center.y * transform.scale;
    let radius = mark.radius * transform.scale;
    let min_x = (center_x - radius - 1.0)
        .floor()
        .max(transform.offset_x.floor())
        .max(0.0) as u32;
    let min_y = (center_y - radius - 1.0)
        .floor()
        .max(transform.offset_y.floor())
        .max(0.0) as u32;
    let max_x = (center_x + radius + 1.0)
        .ceil()
        .min(transform.right.ceil())
        .min(f64::from(width)) as u32;
    let max_y = (center_y + radius + 1.0)
        .ceil()
        .min(transform.bottom.ceil())
        .min(f64::from(height)) as u32;
    for y in min_y..max_y {
        work.check()?;
        for x in min_x..max_x {
            let coverage =
                circle_coverage_clipped(center_x, center_y, radius, x, y, transform, work)?;
            if coverage == 0.0 {
                continue;
            }
            let source_alpha = (color.alpha * opacity * coverage).clamp(0.0, 1.0);
            let destination = &mut pixels[y as usize * width as usize + x as usize];
            let remaining = 1.0 - source_alpha;
            destination.red = color.red * source_alpha + destination.red * remaining;
            destination.green = color.green * source_alpha + destination.green * remaining;
            destination.blue = color.blue * source_alpha + destination.blue * remaining;
            destination.alpha = source_alpha + destination.alpha * remaining;
        }
    }
    Ok(())
}

/// Measures accepted 8x8 circle coverage at one native/output pixel.
///
/// # Errors
///
/// Returns cancellation while traversing subpixel samples.
fn circle_coverage_at(
    center_x: f64,
    center_y: f64,
    radius: f64,
    x: u32,
    y: u32,
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    let mut inside = 0_u32;
    for sample_y in 0..SUBPIXEL_GRID {
        for sample_x in 0..SUBPIXEL_GRID {
            work.check()?;
            let point_x = f64::from(x) + (f64::from(sample_x) + 0.5) / f64::from(SUBPIXEL_GRID);
            let point_y = f64::from(y) + (f64::from(sample_y) + 0.5) / f64::from(SUBPIXEL_GRID);
            let dx = point_x - center_x;
            let dy = point_y - center_y;
            if dx.mul_add(dx, dy * dy) <= radius * radius {
                inside += 1;
            }
        }
    }
    Ok(f64::from(inside) / f64::from(SUBPIXEL_GRID * SUBPIXEL_GRID))
}

/// Measures accepted 8x8 preview-circle coverage clipped to the fitted canvas rectangle.
///
/// # Errors
///
/// Returns cancellation while traversing subpixel samples.
fn circle_coverage_clipped(
    center_x: f64,
    center_y: f64,
    radius: f64,
    x: u32,
    y: u32,
    transform: PreviewTransform,
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    let mut inside = 0_u32;
    for sample_y in 0..SUBPIXEL_GRID {
        for sample_x in 0..SUBPIXEL_GRID {
            work.check()?;
            let point_x = f64::from(x) + (f64::from(sample_x) + 0.5) / f64::from(SUBPIXEL_GRID);
            let point_y = f64::from(y) + (f64::from(sample_y) + 0.5) / f64::from(SUBPIXEL_GRID);
            let dx = point_x - center_x;
            let dy = point_y - center_y;
            if point_x >= transform.offset_x
                && point_x < transform.right
                && point_y >= transform.offset_y
                && point_y < transform.bottom
                && dx.mul_add(dx, dy * dy) <= radius * radius
            {
                inside += 1;
            }
        }
    }
    Ok(f64::from(inside) / f64::from(SUBPIXEL_GRID * SUBPIXEL_GRID))
}

pub fn encode_png(surface: &RasterSurface) -> Result<Vec<u8>, RenderError> {
    let mut output = Vec::new();
    PngEncoder::new(&mut output)
        .write_image(
            surface.pixels(),
            surface.width(),
            surface.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|_| RenderError::new("png.encode", "could not encode RasterSurface"))?;
    Ok(output)
}

/// Converts canonical linear RGB to sRGB at a presentation/output boundary.
pub fn linear_to_srgb(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts sRGB to canonical linear RGB at a presentation/input boundary.
pub fn srgb_to_linear(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn quantize_srgb(value: f64) -> u8 {
    (linear_to_srgb(value) * 255.0).round() as u8
}
fn quantize_linear(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn write_svg(scene: &RenderScene) -> String {
    match scene.model {
        None => write_stage5_svg(scene),
        Some(model) => write_modeled_svg(scene, model),
    }
}

/// Serializes one unmodeled canonical scene with one final canvas clip and editable mark geometry.
fn write_stage5_svg(scene: &RenderScene) -> String {
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let mut document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<metadata>family={};realization={};scene={}</metadata>\n<defs><clipPath id=\"canvas-clip\"><rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\"/></clipPath></defs>\n",
        xml_escape(&scene.identity.family_fingerprint),
        xml_escape(&scene.identity.realization_fingerprint),
        xml_escape(&scene.identity.scene_fingerprint),
    );
    for layer in &scene.layers {
        if !layer.visible {
            continue;
        }
        let color = color_hex(&layer.color);
        let opacity = compact_number(layer.color.alpha * layer.opacity);
        document.push_str(&format!("<g id=\"channel-{}\" clip-path=\"url(#canvas-clip)\" fill=\"{color}\" fill-opacity=\"{opacity}\">\n", layer.channel_id.0));
        write_svg_geometry(
            &mut document,
            &layer.geometry,
            layer.channel_id.0,
            None,
            &scene.canvas,
        );
        document.push_str("</g>\n");
    }
    document.push_str("</svg>\n");
    document
}

fn write_modeled_svg(scene: &RenderScene, model: HalftoneChannelModel) -> String {
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let title = match model {
        HalftoneChannelModel::Rgb => "Toniator RGB halftone",
        HalftoneChannelModel::Cmyk => "Toniator CMYK halftone",
        HalftoneChannelModel::SourceColorAlpha => "Toniator source-colored halftone",
    };
    let mut document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<title>{title}</title>\n<metadata>family={};realization={};scene={}</metadata>\n<defs><clipPath id=\"canvas-clip\"><rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\"/></clipPath></defs>\n",
        xml_escape(&scene.identity.family_fingerprint),
        xml_escape(&scene.identity.realization_fingerprint),
        xml_escape(&scene.identity.scene_fingerprint),
    );
    document.push_str(
        "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">\n",
    );
    let blend_mode = match model {
        HalftoneChannelModel::Rgb => Some("screen"),
        HalftoneChannelModel::Cmyk => Some("multiply"),
        HalftoneChannelModel::SourceColorAlpha => None,
    };
    for layer in &scene.layers {
        write_svg_channel_group(&mut document, layer, blend_mode, &scene.canvas);
    }
    document.push_str("</g>\n");
    document.push_str("</svg>\n");
    document
}

/// Appends one modeled channel group with immutable presentation and per-mark paint semantics.
fn write_svg_channel_group(
    document: &mut String,
    layer: &RenderLayer,
    blend_mode: Option<&str>,
    canvas: &CanvasSpec,
) {
    let mut styles = Vec::new();
    if let Some(mode) = blend_mode {
        styles.push(format!("mix-blend-mode:{mode}"));
    }
    if !layer.visible {
        styles.push("display:none".to_owned());
    }
    let style = (!styles.is_empty()).then(|| format!(" style=\"{}\"", styles.join(";")));
    document.push_str(&format!(
        "<g id=\"channel-{}\"{}>\n",
        layer.channel_id.0,
        style.unwrap_or_default(),
    ));
    write_svg_geometry(
        document,
        &layer.geometry,
        layer.channel_id.0,
        Some(layer),
        canvas,
    );
    document.push_str("</g>\n");
}

/// Writes editable canonical circle or cubic path geometry without resolving document resources.
fn write_svg_geometry(
    document: &mut String,
    geometry: &GeometryOutput,
    channel_id: u64,
    layer: Option<&RenderLayer>,
    canvas: &CanvasSpec,
) {
    match geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                let paint = layer.map_or_else(String::new, |layer| {
                    format!(" fill=\"{}\"", color_hex(layer.mark_paint(index)))
                });
                let opacity = layer.map_or_else(
                    || "".to_owned(),
                    |layer| {
                        format!(
                            " fill-opacity=\"{}\"",
                            compact_number(layer.mark_paint(index).alpha * layer.opacity)
                        )
                    },
                );
                let id = layer.map_or_else(String::new, |_| {
                    format!(" id=\"channel-{channel_id}-mark-{index}\"")
                });
                document.push_str(&format!(
                    "<circle{id} cx=\"{}\" cy=\"{}\" r=\"{}\"{paint}{opacity}/>\n",
                    compact_number(mark.center.x),
                    compact_number(mark.center.y),
                    compact_number(mark.radius)
                ));
            }
        }
        GeometryOutput::CanonicalMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                let paint = layer.map_or_else(String::new, |layer| {
                    format!(" fill=\"{}\"", color_hex(layer.mark_paint(index)))
                });
                let opacity = layer.map_or_else(
                    || "".to_owned(),
                    |layer| {
                        format!(
                            " fill-opacity=\"{}\"",
                            compact_number(layer.mark_paint(index).alpha * layer.opacity)
                        )
                    },
                );
                match mark {
                    CanonicalMark::Circle { center, radius, .. } => {
                        let id = layer.map_or_else(String::new, |_| {
                            format!(" id=\"channel-{channel_id}-mark-{index}\"")
                        });
                        document.push_str(&format!("<circle{id} cx=\"{}\" cy=\"{}\" r=\"{}\"{paint} fill-rule=\"evenodd\"{opacity}/>\n", compact_number(center.x), compact_number(center.y), compact_number(*radius)))
                    }
                    CanonicalMark::ClosedPath(mark) => {
                        let mut data = String::new();
                        for (segment_index, segment) in mark.path.segments().iter().enumerate() {
                            match segment {
                                CurveSegment::Line(line) => {
                                    if segment_index == 0 {
                                        data.push_str(&format!(
                                            "M {} {} ",
                                            compact_number(line.start().x),
                                            compact_number(line.start().y)
                                        ));
                                    }
                                    data.push_str(&format!(
                                        "L {} {} ",
                                        compact_number(line.end().x),
                                        compact_number(line.end().y)
                                    ));
                                }
                                CurveSegment::CubicBezier(cubic) => {
                                    if segment_index == 0 {
                                        data.push_str(&format!(
                                            "M {} {} ",
                                            compact_number(cubic.start().x),
                                            compact_number(cubic.start().y)
                                        ));
                                    }
                                    data.push_str(&format!(
                                        "C {} {},{} {},{} {} ",
                                        compact_number(cubic.control_1().x),
                                        compact_number(cubic.control_1().y),
                                        compact_number(cubic.control_2().x),
                                        compact_number(cubic.control_2().y),
                                        compact_number(cubic.end().x),
                                        compact_number(cubic.end().y)
                                    ));
                                }
                            }
                        }
                        data.push('Z');
                        let id = layer.map_or_else(String::new, |_| {
                            format!(" id=\"channel-{channel_id}-mark-{index}\"")
                        });
                        document.push_str(&format!(
                            "<path{id} d=\"{data}\"{paint} fill-rule=\"evenodd\"{opacity}/>\n"
                        ));
                    }
                }
            }
        }
        GeometryOutput::CanonicalStrokes(strokes) => {
            for (index, stroke) in strokes.iter().enumerate() {
                let id = format!("channel-{channel_id}-stroke-{index}");
                if stroke.outline.contours.is_empty() {
                    continue;
                }
                if !outline_intersects_canvas(&stroke.outline, canvas) {
                    continue;
                }
                let data = svg_outline_path_data(&stroke.outline);
                let paint = layer.map_or_else(String::new, |value| {
                    format!(
                        " fill=\"{}\" fill-opacity=\"{}\"",
                        color_hex(&value.color),
                        compact_number(value.color.alpha * value.opacity)
                    )
                });
                document.push_str(&format!(
                    "<path id=\"{id}\" d=\"{data}\" fill-rule=\"nonzero\"{paint}/>\n"
                ));
            }
        }
    }
}

/// Reports whether derived outline bounds reach the final SVG canvas without clipping geometry.
fn outline_intersects_canvas(
    outline: &toniator_geometry::CanonicalFilledOutline,
    canvas: &CanvasSpec,
) -> bool {
    outline.bounds.is_some_and(|bounds| {
        bounds.max.x >= 0.0
            && bounds.max.y >= 0.0
            && bounds.min.x <= canvas.width
            && bounds.min.y <= canvas.height
    })
}

/// Serializes independent canonical outline contours as one direct SVG path payload.
fn svg_outline_path_data(outline: &toniator_geometry::CanonicalFilledOutline) -> String {
    let mut data = String::new();
    for contour in &outline.contours {
        for (index, segment) in contour.segments.iter().enumerate() {
            match segment {
                CurveSegment::Line(line) => {
                    if index == 0 {
                        data.push_str(&format!(
                            "M {} {} ",
                            compact_number(line.start().x),
                            compact_number(line.start().y)
                        ));
                    }
                    data.push_str(&format!(
                        "L {} {} ",
                        compact_number(line.end().x),
                        compact_number(line.end().y)
                    ));
                }
                CurveSegment::CubicBezier(cubic) => {
                    if index == 0 {
                        data.push_str(&format!(
                            "M {} {} ",
                            compact_number(cubic.start().x),
                            compact_number(cubic.start().y)
                        ));
                    }
                    data.push_str(&format!(
                        "C {} {},{} {},{} {} ",
                        compact_number(cubic.control_1().x),
                        compact_number(cubic.control_1().y),
                        compact_number(cubic.control_2().x),
                        compact_number(cubic.control_2().y),
                        compact_number(cubic.end().x),
                        compact_number(cubic.end().y)
                    ));
                }
            }
        }
        data.push('Z');
    }
    data
}

fn color_hex(color: &ColorValue) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        quantize_srgb(color.red),
        quantize_srgb(color.green),
        quantize_srgb(color.blue)
    )
}

fn compact_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod stage20e2_limit_tests {
    use std::cell::Cell;

    use super::*;
    use toniator_domain::PatternMechanismId;
    use toniator_geometry::{
        CanonicalPathMark, CubicBezierSegment, CurvePath, CurveSegment, FamilySiteId,
        FamilySiteProvenance, PathClosure,
    };

    /// Builds one truthful even-odd canonical path mark from exact closed construction geometry.
    fn path_mark(ordinal: usize, path: CurvePath) -> CanonicalMark {
        CanonicalMark::ClosedPath(
            CanonicalPathMark::new(
                FamilySiteId {
                    mechanism_id: PatternMechanismId(41),
                    ordinal,
                },
                path,
                toniator_geometry::SiteScope::Canvas,
                FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
                CanonicalFillRule::EvenOdd,
            )
            .expect("focused canonical path fixture is finite and closed"),
        )
    }

    /// Builds a small unmodeled canonical scene so request-wide raster work is isolated.
    fn canonical_scene(marks: Vec<CanonicalMark>) -> RenderScene {
        RenderScene::new(
            CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            "stage-20e2-family".into(),
            "stage-20e2-realization".into(),
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 0.25,
                        green: 0.5,
                        blue: 0.75,
                        alpha: 1.0,
                    },
                    1.0,
                    GeometryOutput::CanonicalMarks(marks),
                )
                .expect("focused canonical layer validates"),
            ],
        )
        .expect("focused canonical scene validates")
    }

    /// Measures Euclidean distance to a finite flattened edge for the output-pixel witness.
    fn point_segment_distance(point: Point2, start: Point2, end: Point2) -> f64 {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length_squared = dx.mul_add(dx, dy * dy);
        if length_squared == 0.0 {
            return (point.x - start.x).hypot(point.y - start.y);
        }
        let parameter = (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared)
            .clamp(0.0, 1.0);
        let projected = Point2::new(start.x + parameter * dx, start.y + parameter * dy);
        (point.x - projected.x).hypot(point.y - projected.y)
    }

    /// Fixes the exact nonzero shared flattened-edge default and rejects disabled raster work.
    #[test]
    fn flattened_edge_limit_default_and_zero_rejection_are_exact() {
        assert_eq!(
            RasterizationLimits::default().max_flattened_edges(),
            DEFAULT_MAX_FLATTENED_RASTER_EDGES
        );
        assert_eq!(
            RasterizationLimits::new(0).unwrap_err().path(),
            "raster.limits.flattened_edges"
        );
    }

    /// Proves one flattened-edge budget is shared across marks and accepts the exact boundary.
    #[test]
    fn request_wide_flattened_edge_limit_is_exact_across_marks() {
        let square = |offset: f64| {
            CurvePath::polyline(
                vec![
                    Point2::new(offset + 0.5, 1.0),
                    Point2::new(offset + 2.5, 1.0),
                    Point2::new(offset + 2.5, 3.0),
                    Point2::new(offset + 0.5, 3.0),
                ],
                PathClosure::Closed,
            )
            .expect("square fixture is closed")
        };
        let scene = canonical_scene(vec![path_mark(0, square(0.0)), path_mark(1, square(5.0))]);

        rasterize_cancellable(
            &scene,
            RasterBackground::Transparent,
            RasterizationLimits::new(8).expect("exact eight-edge limit is enabled"),
            &|| false,
        )
        .expect("two four-edge paths fit the exact request-wide limit");
        let error = rasterize_cancellable(
            &scene,
            RasterBackground::Transparent,
            RasterizationLimits::new(7).expect("seven-edge limit is enabled"),
            &|| false,
        )
        .expect_err("the second path must exhaust the shared request budget");
        assert_eq!(error.path(), "raster.limits.flattened_edges");
    }

    /// Proves anisotropic target flattening stays within 1/64 output pixel and AA-off is binary.
    #[test]
    fn anisotropic_output_flattening_and_antialiasing_use_concrete_pixels() {
        let cubic = CubicBezierSegment::new(
            Point2::new(1.0, 2.0),
            Point2::new(2.0, 9.0),
            Point2::new(8.0, 9.0),
            Point2::new(1.0, 2.0),
        )
        .expect("cubic fixture is finite");
        let path = CurvePath::new(vec![CurveSegment::CubicBezier(cubic)], PathClosure::Closed)
            .expect("loop fixture is closed");
        let scene = canonical_scene(vec![path_mark(0, path.clone())]);
        let target = OutputRasterTarget::new(80, 20).expect("anisotropic output is bounded");
        let transform = CanonicalRasterTransform {
            scale_x: 8.0,
            scale_y: 2.0,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let mut work = RasterWork::new(
            RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
            RasterAntialiasing::On,
            &|| false,
        );
        let edges = flattened_path_edges(&path, transform, &mut work)
            .expect("anisotropic cubic flattens in output coordinates");
        for step in 0..=4_096 {
            let parameter = f64::from(step) / 4_096.0;
            let point = CurveSegment::CubicBezier(cubic)
                .point_at(parameter)
                .map(|point| transform.point(point))
                .expect("sampled exact cubic point remains finite");
            let distance = edges
                .iter()
                .map(|(start, end)| point_segment_distance(point, *start, *end))
                .fold(f64::INFINITY, f64::min);
            assert!(
                distance <= 1.0 / 64.0 + 1e-10,
                "flattened output error {distance} exceeded 1/64 pixel"
            );
        }

        let hard_edges = rasterize_output_cancellable(
            &scene,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off,
            RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
            &|| false,
        )
        .expect("explicit anisotropic output rasterizes");
        assert!(
            hard_edges
                .pixels()
                .chunks_exact(4)
                .all(|pixel| matches!(pixel[3], 0 | 255)),
            "AA-off canonical paths use one center sample"
        );
        assert_eq!(
            rasterize_preview_cancellable(
                &scene,
                PreviewRasterTarget::new(40, 20).expect("preview target is bounded"),
                RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
                &|| false,
            )
            .expect("preview rerasterizes canonical geometry")
            .pixels()
            .len(),
            40 * 20 * 4
        );
    }

    /// Proves complete cubic construction bits affect scene identity while SVG retains one editable
    /// closed path, explicit even-odd fill, and the existing final-canvas clip authority.
    #[test]
    fn canonical_cubic_identity_and_structural_svg_are_complete() {
        let cubic_path = |control_y: f64| {
            CurvePath::new(
                vec![CurveSegment::CubicBezier(
                    CubicBezierSegment::new(
                        Point2::new(1.0, 1.0),
                        Point2::new(2.0, control_y),
                        Point2::new(8.0, 9.0),
                        Point2::new(1.0, 1.0),
                    )
                    .expect("cubic identity fixture is finite"),
                )],
                PathClosure::Closed,
            )
            .expect("cubic identity fixture is explicitly closed")
        };
        let first = canonical_scene(vec![path_mark(0, cubic_path(8.0))]);
        let second = canonical_scene(vec![path_mark(0, cubic_path(8.5))]);
        assert_eq!(
            first.identity().family_fingerprint(),
            second.identity().family_fingerprint()
        );
        assert_eq!(
            first.identity().realization_fingerprint(),
            second.identity().realization_fingerprint()
        );
        assert_ne!(
            first.identity().scene_fingerprint(),
            second.identity().scene_fingerprint(),
            "one cubic control-point bit change must invalidate canonical scene identity"
        );
        let svg = write_svg(&first);
        assert_eq!(svg.matches("<clipPath ").count(), 1);
        assert_eq!(svg.matches("<path ").count(), 1);
        assert!(svg.contains(" C "));
        assert!(svg.contains("fill-rule=\"evenodd\""));
        assert!(svg.contains("clip-path=\"url(#canvas-clip)\""));
    }

    /// Proves cancellation interrupts subdivision/edge/pixel work and canonical ellipse sampling.
    #[test]
    fn canonical_raster_work_observes_cancellation_without_a_surface() {
        let cubic = CubicBezierSegment::new(
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 9.0),
            Point2::new(9.0, 9.0),
            Point2::new(1.0, 1.0),
        )
        .expect("cubic fixture is finite");
        let path_scene = canonical_scene(vec![path_mark(
            0,
            CurvePath::new(vec![CurveSegment::CubicBezier(cubic)], PathClosure::Closed)
                .expect("loop fixture is closed"),
        )]);
        let polls = Cell::new(0_u32);
        let error = rasterize_output_cancellable(
            &path_scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(80, 80).expect("output target is bounded")),
            RasterAntialiasing::On,
            RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
            &|| {
                let next = polls.get() + 1;
                polls.set(next);
                next > 64
            },
        )
        .expect_err("path raster work must stop when the probe cancels");
        assert_eq!(error.path(), "evaluation.cancelled");

        let circle = CanonicalMark::Circle {
            source_site_id: FamilySiteId {
                mechanism_id: PatternMechanismId(41),
                ordinal: 0,
            },
            center: Point2::new(5.0, 5.0),
            radius: 4.5,
            scope: toniator_geometry::SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::EvenOdd,
        };
        let circle_scene = canonical_scene(vec![circle]);
        let polls = Cell::new(0_u32);
        let error = rasterize_cancellable(
            &circle_scene,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| {
                let next = polls.get() + 1;
                polls.set(next);
                next > 8
            },
        )
        .expect_err("canonical ellipse sampling must stop when the probe cancels");
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}
