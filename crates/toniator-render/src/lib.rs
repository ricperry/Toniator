#![forbid(unsafe_code)]

//! Headless consumers for immutable canonical circle geometry.
//!
//! `RenderScene` deliberately knows nothing about source artwork, sampling, or
//! pattern settings. Raster compositing happens in linear premultiplied RGBA;
//! `RasterSurface` exposes only straight sRGBA bytes at the output boundary.

use std::{collections::HashSet, error::Error, fmt, sync::Arc};

use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use rayon::prelude::*;
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel};
use toniator_geometry::{
    CanonicalCircleMark, CanonicalFillRule, CanonicalMark, CanonicalRegionSet, CanonicalStroke,
    CurveSegment, Point2, StructuralPathInstanceId, StructuralPathLocationProvenance,
    StructuralPathSourceId,
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

#[cfg(test)]
mod stage20r_composite_tests {
    use super::*;
    use toniator_domain::PatternMechanismId;
    use toniator_geometry::{
        CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSourceGroup,
        CanonicalRegionSourceId, CurvePath, FamilySiteId, FamilySiteProvenance, PathClosure,
        SiteScope, build_canonical_regions_cancellable,
    };

    /// Builds an empty-geometry witness whose output IDs make painter ordering directly inspectable.
    fn composite_scene(output_ids: [u64; 2]) -> RenderScene {
        let outputs = output_ids
            .into_iter()
            .map(|id| {
                RenderOutputLayer::new(
                    toniator_domain::PatternOutputLayerId(id),
                    GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
                        source_site_id: FamilySiteId {
                            mechanism_id: PatternMechanismId(3),
                            ordinal: usize::try_from(id).expect("small fixture ID"),
                        },
                        center: Point2::new(id as f64, 12.0),
                        radius: 1.0,
                        scope: SiteScope::Canvas,
                        provenance: FamilySiteProvenance::Random {
                            candidate_ordinal: usize::try_from(id).expect("small fixture ID"),
                            accepted_ordinal: usize::try_from(id).expect("small fixture ID"),
                            exclusion_neighbor_ordinal: None,
                        },
                        fill_rule: CanonicalFillRule::EvenOdd,
                    }]),
                    None,
                )
            })
            .collect();
        RenderScene::new(
            CanvasSpec {
                width: 32.0,
                height: 24.0,
            },
            "stage-20r-family".into(),
            format!("stage-20r-realization-{output_ids:?}"),
            vec![
                RenderLayer::new_outputs(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 0.1,
                        green: 0.2,
                        blue: 0.3,
                        alpha: 1.0,
                    },
                    0.75,
                    outputs,
                )
                .expect("composite layer validates"),
            ],
        )
        .expect("composite scene validates")
    }

    /// Proves SVG consumes authored painter order and a swap changes presentation identity only.
    #[test]
    fn svg_and_scene_preserve_composite_painter_order() {
        let first = composite_scene([9, 7]);
        let swapped = composite_scene([7, 9]);
        assert_eq!(
            first.layers()[0]
                .outputs()
                .iter()
                .map(|output| output.output_layer_id.0)
                .collect::<Vec<_>>(),
            vec![9, 7]
        );
        let svg = write_svg(&first);
        let first_position = svg.find("cx=\"9\"").expect("first output geometry");
        let second_position = svg.find("cx=\"7\"").expect("second output geometry");
        assert!(first_position < second_position);
        assert_ne!(first.identity(), swapped.identity());
        assert_eq!(
            rasterize(&first, RasterBackground::Transparent)
                .expect("empty composite rasterizes")
                .pixels(),
            rasterize(&swapped, RasterBackground::Transparent)
                .expect("swapped empty composite rasterizes")
                .pixels()
        );
    }

    /// Proves the legacy geometry accessor borrows the first ordered output without a backing clone.
    ///
    /// This focused ownership witness keeps the ordered-output contract intact:
    /// it projects output zero only, counts every output in the diagnostic
    /// total, and performs no rasterization, I/O, or unsafe pointer operation.
    #[test]
    fn legacy_geometry_projects_first_ordered_output_without_duplicate_backing_storage() {
        let marks = (0..4_096)
            .map(|ordinal| CanonicalMark::Circle {
                source_site_id: FamilySiteId {
                    mechanism_id: PatternMechanismId(3),
                    ordinal,
                },
                center: Point2::new(ordinal as f64, 12.0),
                radius: 1.0,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
                fill_rule: CanonicalFillRule::EvenOdd,
            })
            .collect::<Vec<_>>();
        let backing = marks.as_ptr();
        let layer = RenderLayer::new_outputs(
            ChannelId(1),
            true,
            ColorValue {
                red: 0.1,
                green: 0.2,
                blue: 0.3,
                alpha: 1.0,
            },
            0.75,
            vec![
                RenderOutputLayer::new(
                    toniator_domain::PatternOutputLayerId(9),
                    GeometryOutput::CanonicalMarks(marks),
                    None,
                ),
                RenderOutputLayer::new(
                    toniator_domain::PatternOutputLayerId(7),
                    GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
                        source_site_id: FamilySiteId {
                            mechanism_id: PatternMechanismId(3),
                            ordinal: 4_096,
                        },
                        center: Point2::new(4_096.0, 12.0),
                        radius: 1.0,
                        scope: SiteScope::Canvas,
                        provenance: FamilySiteProvenance::Random {
                            candidate_ordinal: 4_096,
                            accepted_ordinal: 4_096,
                            exclusion_neighbor_ordinal: None,
                        },
                        fill_rule: CanonicalFillRule::EvenOdd,
                    }]),
                    None,
                ),
            ],
        )
        .expect("ordered ownership witness validates");
        let GeometryOutput::CanonicalMarks(projected) = layer.geometry() else {
            panic!("legacy projection must remain the first canonical-mark output");
        };
        let GeometryOutput::CanonicalMarks(first) = layer.outputs()[0].geometry() else {
            panic!("first ordered output remains canonical marks");
        };
        assert_eq!(projected.as_ptr(), backing);
        assert_eq!(projected.as_ptr(), first.as_ptr());
        let scene = RenderScene::new(
            CanvasSpec {
                width: 8_192.0,
                height: 24.0,
            },
            "legacy-projection-family".into(),
            "legacy-projection-realization".into(),
            vec![layer],
        )
        .expect("legacy projection scene validates");
        assert_eq!(scene.circular_mark_count(), 4_097);
    }

    /// Proves sampled channel authority rejects a partially solid ordered output collection.
    #[test]
    fn sampled_composite_requires_paint_for_every_output() {
        let base = composite_scene([9, 7]);
        let mut layer = base.layers()[0].clone();
        layer.outputs[0].primitive_paints = Some(Arc::new(vec![ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }]));
        let error = RenderScene::new_modeled(
            CanvasSpec {
                width: 32.0,
                height: 24.0,
            },
            "stage-20r-sampled-family".into(),
            "stage-20r-sampled-realization".into(),
            HalftoneChannelModel::SourceColorAlpha,
            vec![layer],
        )
        .expect_err("partially sampled composite rejects atomically");
        assert_eq!(error.path(), "scene.layers");
        assert_eq!(
            error.message(),
            "SourceColorAlpha requires sampled paint for every ordered output"
        );
    }

    /// Proves heterogeneous sampled regions and marks preserve painter and paint alignment.
    #[test]
    fn heterogeneous_region_and_mark_outputs_composite_in_authored_order() {
        let site_id = FamilySiteId {
            mechanism_id: PatternMechanismId(3),
            ordinal: 1,
        };
        let regions = build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: toniator_domain::PatternOutputLayerId(9),
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: CanonicalRegionSourceId::SiteOwners(vec![site_id]),
                    components: vec![
                        CurvePath::polyline(
                            vec![
                                Point2::new(4.0, 4.0),
                                Point2::new(28.0, 4.0),
                                Point2::new(16.0, 21.0),
                            ],
                            PathClosure::Closed,
                        )
                        .expect("region closes"),
                    ],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .expect("region canonicalizes")
        .0;
        let region_output = RenderOutputLayer::new(
            toniator_domain::PatternOutputLayerId(9),
            GeometryOutput::CanonicalRegions(regions),
            Some(vec![ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            }]),
        );
        let mark_output = RenderOutputLayer::new(
            toniator_domain::PatternOutputLayerId(7),
            GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
                source_site_id: site_id,
                center: Point2::new(16.0, 12.0),
                radius: 6.0,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: 1,
                    accepted_ordinal: 1,
                    exclusion_neighbor_ordinal: None,
                },
                fill_rule: CanonicalFillRule::EvenOdd,
            }]),
            Some(vec![ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 1.0,
            }]),
        );
        let empty_output = RenderOutputLayer::new(
            toniator_domain::PatternOutputLayerId(8),
            GeometryOutput::CanonicalRegions(CanonicalRegionSet::empty()),
            Some(Vec::new()),
        );
        let build = |outputs| {
            RenderScene::new_modeled(
                CanvasSpec {
                    width: 32.0,
                    height: 24.0,
                },
                "stage-20r-heterogeneous-family".into(),
                "stage-20r-heterogeneous-realization".into(),
                HalftoneChannelModel::SourceColorAlpha,
                vec![
                    RenderLayer::new_outputs(
                        ChannelId(1),
                        true,
                        ColorValue {
                            red: 0.0,
                            green: 0.0,
                            blue: 1.0,
                            alpha: 1.0,
                        },
                        0.6,
                        outputs,
                    )
                    .expect("heterogeneous layer validates"),
                ],
            )
            .expect("heterogeneous scene validates")
        };
        let region_then_mark = build(vec![
            region_output.clone(),
            empty_output.clone(),
            mark_output.clone(),
        ]);
        let mark_then_region = build(vec![mark_output, empty_output, region_output]);
        assert_eq!(
            region_then_mark.layers()[0]
                .outputs()
                .iter()
                .map(|output| output.output_layer_id.0)
                .collect::<Vec<_>>(),
            vec![9, 8, 7]
        );
        assert_ne!(
            rasterize(&region_then_mark, RasterBackground::Transparent)
                .expect("region-first scene rasterizes")
                .pixels(),
            rasterize(&mark_then_region, RasterBackground::Transparent)
                .expect("mark-first scene rasterizes")
                .pixels()
        );
        let svg = write_svg(&region_then_mark);
        assert!(
            svg.find("<path").expect("sampled region path emits")
                < svg.find("<circle").expect("solid mark emits")
        );
    }
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
    outputs: Vec<RenderOutputLayer>,
}

/// One output-owned canonical geometry payload in painter order within a channel.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderOutputLayer {
    /// Stable structural output identity supplied by the domain/pattern pipeline.
    pub output_layer_id: toniator_domain::PatternOutputLayerId,
    /// Canonical geometry consumed only by raster and SVG final consumers.
    pub geometry: Arc<GeometryOutput>,
    /// Optional sampled paint, cardinally aligned to canonical marks or canonical regions.
    pub primitive_paints: Option<Arc<Vec<ColorValue>>>,
}

impl RenderOutputLayer {
    /// Wraps one renderer-owned geometry and optional paint payload for immutable scene sharing.
    pub fn new(
        output_layer_id: toniator_domain::PatternOutputLayerId,
        geometry: GeometryOutput,
        primitive_paints: Option<Vec<ColorValue>>,
    ) -> Self {
        Self {
            output_layer_id,
            geometry: Arc::new(geometry),
            primitive_paints: primitive_paints.map(Arc::new),
        }
    }

    /// Retains already shared cache payloads without cloning canonical geometry or sampled paint.
    pub fn from_shared(
        output_layer_id: toniator_domain::PatternOutputLayerId,
        geometry: Arc<GeometryOutput>,
        primitive_paints: Option<Arc<Vec<ColorValue>>>,
    ) -> Self {
        Self {
            output_layer_id,
            geometry,
            primitive_paints,
        }
    }

    /// Returns the immutable canonical geometry behind this painter-ordered output.
    pub fn geometry(&self) -> &GeometryOutput {
        self.geometry.as_ref()
    }

    /// Returns sampled primitive paint without exposing shared-storage mutation.
    pub fn primitive_paints(&self) -> Option<&[ColorValue]> {
        self.primitive_paints.as_deref().map(Vec::as_slice)
    }

    /// Clones only the immutable geometry handle for cache/scene publication.
    pub fn shared_geometry(&self) -> Arc<GeometryOutput> {
        Arc::clone(&self.geometry)
    }

    /// Clones only the optional immutable paint handle for cache/scene publication.
    pub fn shared_primitive_paints(&self) -> Option<Arc<Vec<ColorValue>>> {
        self.primitive_paints.as_ref().map(Arc::clone)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeometryOutput {
    CircularMarks(Vec<CanonicalCircleMark>),
    CanonicalMarks(Vec<CanonicalMark>),
    CanonicalStrokes(Vec<CanonicalStroke>),
    /// Geometry-owned, closed positive regions with fixed nonzero fill semantics.
    CanonicalRegions(CanonicalRegionSet),
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
                    if layer
                        .outputs
                        .iter()
                        .any(|output| output.primitive_paints.is_some())
                    {
                        return Err(RenderError::new(
                            "scene.layers",
                            "unmodeled legacy scenes cannot carry sampled per-mark paints",
                        ));
                    }
                }
                Some(HalftoneChannelModel::Rgb | HalftoneChannelModel::Cmyk) => {
                    if layer
                        .outputs
                        .iter()
                        .any(|output| output.primitive_paints.is_some())
                    {
                        return Err(RenderError::new(
                            "scene.layers",
                            "RGB and CMYK layers must use solid paint",
                        ));
                    }
                }
                Some(HalftoneChannelModel::SourceColorAlpha) => {
                    if layer
                        .outputs
                        .iter()
                        .any(|output| output.primitive_paints.is_none())
                    {
                        return Err(RenderError::new(
                            "scene.layers",
                            "SourceColorAlpha requires sampled paint for every ordered output",
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

    /// Counts every retained primitive across every ordered channel output.
    ///
    /// The count is a diagnostic projection only: it neither merges ordered
    /// outputs nor changes their painter order or canonical geometry.
    pub fn circular_mark_count(&self) -> usize {
        self.layers
            .iter()
            .flat_map(|layer| layer.outputs())
            .map(|output| match output.geometry() {
                GeometryOutput::CircularMarks(marks) => marks.len(),
                GeometryOutput::CanonicalMarks(marks) => marks.len(),
                GeometryOutput::CanonicalStrokes(strokes) => strokes.len(),
                GeometryOutput::CanonicalRegions(regions) => regions.regions().len(),
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
    /// Builds one solid-paint output layer with its structural output identity retained for rendering.
    ///
    /// # Errors
    ///
    /// Returns stable presentation or geometry diagnostics before scene construction.
    pub fn new_for_output(
        channel_id: ChannelId,
        visible: bool,
        color: ColorValue,
        opacity: f64,
        output_layer_id: toniator_domain::PatternOutputLayerId,
        geometry: GeometryOutput,
    ) -> Result<Self, RenderError> {
        Self::new_outputs(
            channel_id,
            visible,
            color,
            opacity,
            vec![RenderOutputLayer::new(output_layer_id, geometry, None)],
        )
    }

    /// Builds one legacy single-output layer without retaining a duplicate geometry projection.
    ///
    /// # Errors
    ///
    /// Returns stable presentation or geometry diagnostics before scene
    /// construction. The legacy [`Self::geometry`] accessor projects the sole
    /// normalized output by reference.
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
            outputs: vec![RenderOutputLayer::new(
                toniator_domain::PatternOutputLayerId(0),
                geometry,
                None,
            )],
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
        let mark_paints: Vec<ColorValue> = marks
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
            outputs: vec![RenderOutputLayer::new(
                toniator_domain::PatternOutputLayerId(0),
                geometry,
                Some(mark_paints),
            )],
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
            outputs: vec![RenderOutputLayer::new(
                toniator_domain::PatternOutputLayerId(0),
                GeometryOutput::CanonicalMarks(marks),
                Some(paints),
            )],
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
    /// Returns every channel output in deterministic painter order.
    pub fn outputs(&self) -> &[RenderOutputLayer] {
        &self.outputs
    }
    /// Returns the first ordered output geometry as the legacy read-only projection.
    ///
    /// The reference aliases the canonical first output; this compatibility
    /// accessor never retains or clones a second geometry payload.
    pub fn geometry(&self) -> &GeometryOutput {
        self.outputs[0].geometry()
    }

    /// Builds an ordered output layer with channel-owned presentation authority.
    ///
    /// # Errors
    ///
    /// Returns stable presentation, output identity, geometry, or sampled-paint diagnostics.
    pub fn new_outputs(
        channel_id: ChannelId,
        visible: bool,
        color: ColorValue,
        opacity: f64,
        outputs: Vec<RenderOutputLayer>,
    ) -> Result<Self, RenderError> {
        if outputs.is_empty() {
            return Err(RenderError::new(
                "scene.layer.outputs",
                "at least one ordered output is required",
            ));
        }
        let layer = Self {
            channel_id,
            visible,
            color,
            opacity,
            outputs,
        };
        validate_layer(&layer)?;
        Ok(layer)
    }

    /// Retargets the sole legacy output projection to an explicit structural output ID.
    ///
    /// # Errors
    ///
    /// Returns an output validation diagnostic if the retained layer invariant is violated.
    pub fn with_output_layer_id(
        mut self,
        output_layer_id: toniator_domain::PatternOutputLayerId,
    ) -> Result<Self, RenderError> {
        self.outputs[0].output_layer_id = output_layer_id;
        validate_layer(&self)?;
        Ok(self)
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
    if layer.outputs.is_empty() {
        return Err(RenderError::new(
            "scene.layer.outputs",
            "at least one ordered output is required",
        ));
    }
    let mut ids = HashSet::new();
    for output in &layer.outputs {
        if !ids.insert(output.output_layer_id) {
            return Err(RenderError::new(
                "scene.layer.outputs",
                "output layer IDs must be unique in painter order",
            ));
        }
        validate_output_geometry(output)?;
    }
    Ok(())
}

/// Validates one output geometry and its optional sampled paint without changing channel presentation.
///
/// # Errors
///
/// Returns stable geometry or sampled-paint diagnostics before scene construction.
fn validate_output_geometry(output: &RenderOutputLayer) -> Result<(), RenderError> {
    match output.geometry() {
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
        GeometryOutput::CanonicalRegions(regions) => {
            if regions.regions().iter().any(|region| {
                region.area <= 0.0
                    || !region.area.is_finite()
                    || !region.bounds.min.is_finite()
                    || !region.bounds.max.is_finite()
            }) {
                return Err(RenderError::new(
                    "scene.layer.geometry",
                    "canonical regions must be finite positive-area geometry",
                ));
            }
        }
    }
    if let Some(paints) = &output.primitive_paints {
        if matches!(output.geometry(), GeometryOutput::CanonicalStrokes(_)) {
            return Err(RenderError::new(
                "scene.layer.source_color",
                "canonical strokes require solid channel paint",
            ));
        }
        let primitives = match output.geometry() {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
            GeometryOutput::CanonicalRegions(regions) => regions.regions().len(),
            GeometryOutput::CanonicalStrokes(_) => 0,
        };
        if paints.len() != primitives {
            return Err(RenderError::new(
                "scene.layer.source_color",
                "source-colored paint count must match canonical primitive count",
            ));
        }
        for paint in paints.iter() {
            for value in [paint.red, paint.green, paint.blue, paint.alpha] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(RenderError::new(
                        "scene.layer.source_color",
                        "source-colored paint must be finite values within 0.0..=1.0",
                    ));
                }
            }
            if !matches!(output.geometry(), GeometryOutput::CanonicalRegions(_))
                && paint.alpha != 1.0
            {
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
        if layer.outputs.len() > 1 {
            add_scene_bytes(&mut hash, [0x4f]);
            add_scene_bytes(&mut hash, (layer.outputs.len() as u64).to_le_bytes());
        }
        for output in &layer.outputs {
            if layer.outputs.len() > 1 {
                add_scene_bytes(&mut hash, output.output_layer_id.0.to_le_bytes());
            }
            match output.geometry() {
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
                        match &stroke.source_id {
                            toniator_geometry::CanonicalStrokeSourceId::Structural(id) => {
                                append_scene_structural_path_instance(&mut hash, *id);
                            }
                            toniator_geometry::CanonicalStrokeSourceId::Connection(id) => {
                                add_scene_bytes(&mut hash, [0x43]);
                                add_scene_bytes(&mut hash, id.output_layer_id.0.to_le_bytes());
                                add_scene_bytes(
                                    &mut hash,
                                    id.component_minimum.mechanism_id.0.to_le_bytes(),
                                );
                                add_scene_bytes(
                                    &mut hash,
                                    (id.component_minimum.ordinal as u64).to_le_bytes(),
                                );
                                add_scene_bytes(&mut hash, id.component_ordinal.to_le_bytes());
                                add_scene_bytes(
                                    &mut hash,
                                    id.first_endpoint.mechanism_id.0.to_le_bytes(),
                                );
                                add_scene_bytes(
                                    &mut hash,
                                    (id.first_endpoint.ordinal as u64).to_le_bytes(),
                                );
                                add_scene_bytes(
                                    &mut hash,
                                    id.last_endpoint.mechanism_id.0.to_le_bytes(),
                                );
                                add_scene_bytes(
                                    &mut hash,
                                    (id.last_endpoint.ordinal as u64).to_le_bytes(),
                                );
                                add_scene_bytes(&mut hash, id.ordinal.to_le_bytes());
                            }
                            toniator_geometry::CanonicalStrokeSourceId::Maze(id) => {
                                add_scene_bytes(&mut hash, [0x4d]);
                                add_scene_bytes(&mut hash, id.output_layer_id.0.to_le_bytes());
                                add_scene_bytes(&mut hash, id.wall.first.0.to_le_bytes());
                                add_scene_bytes(&mut hash, id.wall.second.0.to_le_bytes());
                            }
                        }
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
                            add_scene_bytes(
                                &mut hash,
                                sample.location.segment_index().to_le_bytes(),
                            );
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
                            add_scene_bytes(
                                &mut hash,
                                (contour.segments.len() as u64).to_le_bytes(),
                            );
                            for segment in &contour.segments {
                                append_scene_outline_segment(&mut hash, segment);
                            }
                        }
                    }
                }
                GeometryOutput::CanonicalRegions(regions) => {
                    add_scene_bytes(&mut hash, [4]);
                    add_scene_bytes(&mut hash, regions.fingerprint().bytes());
                }
            }
            if model.is_some() {
                if let Some(paints) = &output.primitive_paints {
                    add_scene_bytes(&mut hash, [1]);
                    for paint in paints.iter() {
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
        toniator_geometry::FamilySiteProvenance::AlongParametricCurve {
            location,
            path_order,
            sequence,
            absolute_arc_position_bits,
            local_arc_position_bits,
        } => {
            add_scene_bytes(hash, [6]);
            append_scene_curve_location(hash, location);
            add_scene_bytes(
                hash,
                u64::try_from(*path_order)
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
fn append_scene_curve_location(hash: &mut u64, location: &StructuralPathLocationProvenance) {
    append_scene_structural_path_instance(hash, location.path);
    add_scene_bytes(
        hash,
        u64::try_from(location.segment_index)
            .expect("usize fits u64")
            .to_le_bytes(),
    );
    add_scene_bytes(hash, location.parameter_bits.to_le_bytes());
}

/// Appends the typed source and ordered component identity used by path-derived scene geometry.
fn append_scene_structural_path_instance(hash: &mut u64, path: StructuralPathInstanceId) {
    match path.source {
        StructuralPathSourceId::GuideDimension(id) => {
            add_scene_bytes(hash, [1]);
            add_scene_bytes(hash, id.0.to_le_bytes());
        }
        StructuralPathSourceId::ParametricCurve(id) => {
            add_scene_bytes(hash, [2]);
            add_scene_bytes(hash, id.0.to_le_bytes());
        }
    }
    add_scene_bytes(hash, path.repetition_index.to_le_bytes());
    add_scene_bytes(hash, path.component_ordinal.to_le_bytes());
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

impl RasterBackground {
    /// Selects the final-consumer PNG backing used when the caller supplies no override.
    ///
    /// This policy reads only the authoritative channel model and never enters document,
    /// canonical-scene, renderer-geometry, or cache identity. Unmodeled legacy scenes retain
    /// their historical transparent default.
    pub const fn default_for_model(model: Option<HalftoneChannelModel>) -> Self {
        match model {
            Some(HalftoneChannelModel::Rgb) => Self::OpaqueBlack,
            Some(HalftoneChannelModel::Cmyk) => Self::OpaqueWhite,
            Some(HalftoneChannelModel::SourceColorAlpha) | None => Self::Transparent,
        }
    }
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
    is_cancelled: &'a (dyn Fn() -> bool + Sync),
    completed_units: usize,
    total_units: usize,
    report_progress: &'a (dyn Fn(usize, usize) + Sync),
}

/// Discards optional raster progress for compatibility entry points.
fn ignore_raster_progress(_completed: usize, _total: usize) {}

impl<'a> RasterWork<'a> {
    /// Initializes one nonzero request-wide edge budget and caller-selected sampling policy.
    fn new(
        limits: RasterizationLimits,
        antialiasing: RasterAntialiasing,
        is_cancelled: &'a (dyn Fn() -> bool + Sync),
    ) -> Self {
        Self::with_progress(
            limits,
            antialiasing,
            is_cancelled,
            0,
            &ignore_raster_progress,
        )
    }

    /// Initializes request-wide edge and primitive-progress accounting.
    fn with_progress(
        limits: RasterizationLimits,
        antialiasing: RasterAntialiasing,
        is_cancelled: &'a (dyn Fn() -> bool + Sync),
        total_units: usize,
        report_progress: &'a (dyn Fn(usize, usize) + Sync),
    ) -> Self {
        Self {
            remaining_edges: limits.max_flattened_edges(),
            antialiasing,
            is_cancelled,
            completed_units: 0,
            total_units,
            report_progress,
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

    /// Reports completion of one canonical primitive after its pixels are fully composited.
    fn primitive(&mut self) {
        self.completed_units = self.completed_units.saturating_add(1);
        (self.report_progress)(self.completed_units.min(self.total_units), self.total_units);
    }

    /// Reports completion of one parallel composition, background, or quantization phase.
    fn parallel_phase(&mut self) {
        self.primitive();
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
    /// Builds one validated straight-sRGBA raster surface from an already allocated byte buffer.
    ///
    /// # Errors
    ///
    /// Returns `raster.surface` when dimensions are invalid or their checked
    /// byte count does not match the supplied buffer. This constructor never
    /// reallocates or changes caller-owned pixel bytes.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::new(
                "raster.surface",
                "dimensions must be positive",
            ));
        }
        let expected_byte_count = raster_byte_count(width, height)?;
        if pixels.len() != expected_byte_count {
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
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<RasterSurface, RenderError> {
    rasterize_cancellable_with_progress(
        scene,
        background,
        limits,
        is_cancelled,
        &ignore_raster_progress,
    )
}

/// Rasterizes canonical geometry while reporting completed primitive units.
///
/// Progress is consumer-only and excluded from pixels, scene identity, cache
/// identity, edge policy, and cancellation. A primitive is reported only after
/// all of its pixels have been composited into private layer storage.
///
/// # Errors
///
/// Returns the same cancellation, target, flattening, edge-limit, or surface
/// diagnostics as [`rasterize_cancellable`] without publishing partial pixels.
pub fn rasterize_cancellable_with_progress(
    scene: &RenderScene,
    background: RasterBackground,
    limits: RasterizationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<RasterSurface, RenderError> {
    let total_primitives = raster_progress_unit_count(scene);
    let mut work = RasterWork::with_progress(
        limits,
        RasterAntialiasing::On,
        is_cancelled,
        total_primitives,
        report_progress,
    );
    // All native document-canvas rasterization crosses the same checked final
    // consumer boundary as an explicit output target before allocation.
    let native_target = native_output_target(scene)?;
    if scene.model.is_none() {
        return rasterize_stage5(scene, background, &mut work);
    }

    let width = native_target.width;
    let height = native_target.height;
    let model = scene.model.expect("modeled scene");
    let mut linear_pixels = begin_model_composition(model, width, height)?;
    for layer in &scene.layers {
        let layer_pixels = rasterize_layer(layer, width, height, &mut work)?;
        compose_model_layer(model, &mut linear_pixels, &layer_pixels, is_cancelled)?;
    }
    finish_model_composition(model, &mut linear_pixels, is_cancelled)?;
    work.parallel_phase();
    apply_background(&mut linear_pixels, background, is_cancelled)?;
    work.parallel_phase();
    let surface = pixels_from_linear(width, height, linear_pixels, is_cancelled)?;
    work.parallel_phase();
    Ok(surface)
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
    is_cancelled: &(dyn Fn() -> bool + Sync),
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
    let model = scene.model;
    let mut pixels = match model {
        Some(model) => begin_model_composition(model, target.width, target.height)?,
        None => begin_source_over_composition(target.width, target.height)?,
    };
    for layer in &scene.layers {
        let layer_pixels =
            rasterize_layer_for_output(layer, target, transform, antialiasing, &mut work)?;
        match model {
            Some(model) => compose_model_layer(model, &mut pixels, &layer_pixels, is_cancelled)?,
            None => compose_source_over_layer(&mut pixels, &layer_pixels, is_cancelled)?,
        }
    }
    if let Some(model) = model {
        finish_model_composition(model, &mut pixels, is_cancelled)?;
    }
    apply_background(&mut pixels, background, is_cancelled)?;
    pixels_from_linear(target.width, target.height, pixels, is_cancelled)
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
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<RasterSurface, RenderError> {
    rasterize_preview_cancellable_with_progress(
        scene,
        target,
        limits,
        is_cancelled,
        &ignore_raster_progress,
    )
}

/// Rasterizes a fitted preview while reporting completed canonical primitives.
///
/// Progress is non-authoritative and may be emitted from worker-owned raster
/// work; it never changes target fitting, pixels, scene identity, or caching.
///
/// # Errors
///
/// Returns the same cancellation, target, flattening, edge-limit, or surface
/// diagnostics as [`rasterize_preview_cancellable`] without partial output.
pub fn rasterize_preview_cancellable_with_progress(
    scene: &RenderScene,
    target: PreviewRasterTarget,
    limits: RasterizationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    report_progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<RasterSurface, RenderError> {
    let transform = PreviewTransform::for_scene(scene, target);
    let width = target.width;
    let height = target.height;
    let mut work = RasterWork::with_progress(
        limits,
        RasterAntialiasing::On,
        is_cancelled,
        raster_progress_unit_count(scene),
        report_progress,
    );
    let model = scene.model;
    let mut pixels = match model {
        Some(model) => begin_model_composition(model, width, height)?,
        None => begin_source_over_composition(width, height)?,
    };
    for layer in &scene.layers {
        let layer_pixels =
            rasterize_layer_with_transform(layer, width, height, transform, &mut work)?;
        match model {
            Some(model) => compose_model_layer(model, &mut pixels, &layer_pixels, is_cancelled)?,
            None => compose_source_over_layer(&mut pixels, &layer_pixels, is_cancelled)?,
        }
    }
    if let Some(model) = model {
        finish_model_composition(model, &mut pixels, is_cancelled)?;
    }
    work.parallel_phase();
    apply_background(&mut pixels, RasterBackground::Transparent, is_cancelled)?;
    work.parallel_phase();
    let surface = pixels_from_linear(width, height, pixels, is_cancelled)?;
    work.parallel_phase();
    Ok(surface)
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
        for output in &layer.outputs {
            match output.geometry() {
                GeometryOutput::CircularMarks(marks) => {
                    for (index, mark) in marks.iter().enumerate() {
                        composite_circle(
                            &mut linear_pixels,
                            width,
                            height,
                            mark,
                            output
                                .primitive_paints
                                .as_ref()
                                .and_then(|paints| paints.get(index))
                                .unwrap_or(&layer.color),
                            layer.opacity,
                            work,
                        )?;
                        work.primitive();
                    }
                }
                GeometryOutput::CanonicalMarks(marks) => {
                    for (index, mark) in marks.iter().enumerate() {
                        composite_canonical_mark(
                            &mut linear_pixels,
                            width,
                            height,
                            mark,
                            output
                                .primitive_paints
                                .as_ref()
                                .and_then(|paints| paints.get(index))
                                .unwrap_or(&layer.color),
                            layer.opacity,
                            CanonicalRasterTransform::native(),
                            work,
                        )?;
                        work.primitive();
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
                        work.primitive();
                    }
                }
                GeometryOutput::CanonicalRegions(regions) => {
                    for (index, region) in regions.regions().iter().enumerate() {
                        composite_canonical_region(
                            &mut linear_pixels,
                            width,
                            height,
                            region,
                            output_primitive_paint(layer, Some(output), index),
                            layer.opacity,
                            CanonicalRasterTransform::native(),
                            work,
                        )?;
                        work.primitive();
                    }
                }
            }
        }
    }
    let surface = pixels_from_linear(width, height, linear_pixels, work.is_cancelled)?;
    work.parallel_phase();
    Ok(surface)
}

/// Counts canonical primitives and final parallel phases for raster progress.
fn raster_progress_unit_count(scene: &RenderScene) -> usize {
    let primitives = scene
        .layers
        .iter()
        .filter(|layer| layer.visible)
        .flat_map(|layer| &layer.outputs)
        .fold(0_usize, |total, output| {
            let count = match output.geometry() {
                GeometryOutput::CircularMarks(marks) => marks.len(),
                GeometryOutput::CanonicalMarks(marks) => marks.len(),
                GeometryOutput::CanonicalStrokes(strokes) => strokes.len(),
                GeometryOutput::CanonicalRegions(regions) => regions.regions().len(),
            };
            total.saturating_add(count)
        });
    primitives.saturating_add(3)
}

/// Resolves integral native output dimensions without changing scene geometry.
///
/// # Errors
///
/// Returns the stable output-target diagnostic when either finite canvas
/// dimension is not representable as a positive integral raster dimension.
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

/// Returns the checked count of complete pixels in one raster target.
///
/// # Errors
///
/// Returns `raster.allocation` when the dimensions cannot be represented by a
/// `usize` count on the current target. It imposes no product-size ceiling.
fn raster_pixel_count(width: u32, height: u32) -> Result<usize, RenderError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(RenderError::new(
            "raster.allocation",
            "raster pixel count overflows",
        ))
}

/// Returns the checked byte count for one straight-sRGBA raster target.
///
/// # Errors
///
/// Returns `raster.allocation` when the target pixel count or four-byte
/// conversion overflows; it never allocates storage itself.
fn raster_byte_count(width: u32, height: u32) -> Result<usize, RenderError> {
    raster_pixel_count(width, height)?
        .checked_mul(4)
        .ok_or(RenderError::new(
            "raster.allocation",
            "raster byte count overflows",
        ))
}

/// Allocates one complete premultiplied linear pixel buffer through fallible reservation.
///
/// # Errors
///
/// Returns `raster.allocation` for a non-representable target or failed
/// allocation before publishing any partial raster storage.
fn allocate_linear_pixels(
    width: u32,
    height: u32,
    initial: PremultipliedLinearPixel,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let count = raster_pixel_count(width, height)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(count).map_err(|_| {
        RenderError::new("raster.allocation", "raster linear pixel allocation failed")
    })?;
    pixels.resize(count, initial);
    Ok(pixels)
}

/// Allocates one final straight-sRGBA byte buffer through fallible reservation.
///
/// # Errors
///
/// Returns `raster.allocation` for a non-representable target or failed byte
/// allocation before quantization begins.
fn allocate_raster_bytes(width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    let count = raster_byte_count(width, height)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| RenderError::new("raster.allocation", "raster byte allocation failed"))?;
    pixels.resize(count, 0);
    Ok(pixels)
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
    let mut pixels = allocate_linear_pixels(
        width,
        height,
        background_pixel(RasterBackground::Transparent),
    )?;
    if !layer.visible {
        return Ok(pixels);
    }
    for output in &layer.outputs {
        match output.geometry() {
            GeometryOutput::CircularMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_circle(
                        &mut pixels,
                        width,
                        height,
                        mark,
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
                        layer.opacity,
                        work,
                    )?;
                    work.primitive();
                }
            }
            GeometryOutput::CanonicalMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_canonical_mark(
                        &mut pixels,
                        width,
                        height,
                        mark,
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
                        layer.opacity,
                        CanonicalRasterTransform::native(),
                        work,
                    )?;
                    work.primitive();
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
                    work.primitive();
                }
            }
            GeometryOutput::CanonicalRegions(regions) => {
                for (index, region) in regions.regions().iter().enumerate() {
                    composite_canonical_region(
                        &mut pixels,
                        width,
                        height,
                        region,
                        output_primitive_paint(layer, Some(output), index),
                        layer.opacity,
                        CanonicalRasterTransform::native(),
                        work,
                    )?;
                    work.primitive();
                }
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
    let mut pixels = allocate_linear_pixels(
        width,
        height,
        background_pixel(RasterBackground::Transparent),
    )?;
    if !layer.visible {
        return Ok(pixels);
    }
    for output in &layer.outputs {
        match output.geometry() {
            GeometryOutput::CircularMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_circle_transformed(
                        &mut pixels,
                        width,
                        height,
                        mark,
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
                        layer.opacity,
                        transform,
                        work,
                    )?;
                    work.primitive();
                }
            }
            GeometryOutput::CanonicalMarks(marks) => {
                for (index, mark) in marks.iter().enumerate() {
                    composite_canonical_mark(
                        &mut pixels,
                        width,
                        height,
                        mark,
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
                        layer.opacity,
                        CanonicalRasterTransform::preview(transform),
                        work,
                    )?;
                    work.primitive();
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
                    work.primitive();
                }
            }
            GeometryOutput::CanonicalRegions(regions) => {
                for (index, region) in regions.regions().iter().enumerate() {
                    composite_canonical_region(
                        &mut pixels,
                        width,
                        height,
                        region,
                        output_primitive_paint(layer, Some(output), index),
                        layer.opacity,
                        CanonicalRasterTransform::preview(transform),
                        work,
                    )?;
                    work.primitive();
                }
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
    let mut pixels = allocate_linear_pixels(
        target.width,
        target.height,
        background_pixel(RasterBackground::Transparent),
    )?;
    if !layer.visible {
        return Ok(pixels);
    }
    for output in &layer.outputs {
        match output.geometry() {
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
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
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
                        output
                            .primitive_paints
                            .as_ref()
                            .and_then(|paints| paints.get(index))
                            .unwrap_or(&layer.color),
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
            GeometryOutput::CanonicalRegions(regions) => {
                for (index, region) in regions.regions().iter().enumerate() {
                    composite_canonical_region(
                        &mut pixels,
                        target.width,
                        target.height,
                        region,
                        output_primitive_paint(layer, Some(output), index),
                        layer.opacity,
                        CanonicalRasterTransform::output(transform),
                        work,
                    )?;
                }
            }
        }
    }
    Ok(pixels)
}

/// Allocates the single streaming accumulator for one modeled rasterization.
///
/// CMYK uses the same storage while layers stream, but retains transmittance
/// in RGB and uncovered coverage in alpha until finalization. The other models
/// retain their final premultiplied meaning throughout.
///
/// # Errors
///
/// Returns `raster.allocation` before layer rasterization if the accumulator
/// cannot be represented or reserved.
fn begin_model_composition(
    model: HalftoneChannelModel,
    width: u32,
    height: u32,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let initial = match model {
        HalftoneChannelModel::Cmyk => PremultipliedLinearPixel {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        },
        HalftoneChannelModel::Rgb | HalftoneChannelModel::SourceColorAlpha => {
            background_pixel(RasterBackground::Transparent)
        }
    };
    allocate_linear_pixels(width, height, initial)
}

/// Allocates the single ordinary source-over accumulator for an unmodeled scene.
///
/// # Errors
///
/// Returns `raster.allocation` before layer rasterization if the accumulator
/// cannot be represented or reserved.
fn begin_source_over_composition(
    width: u32,
    height: u32,
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    allocate_linear_pixels(
        width,
        height,
        background_pixel(RasterBackground::Transparent),
    )
}

/// Folds one just-rasterized layer into the modeled accumulator in authored order.
///
/// # Errors
///
/// Returns cancellation or an internal raster-shape diagnostic without
/// accepting a partially composited final surface.
fn compose_model_layer(
    model: HalftoneChannelModel,
    accumulator: &mut [PremultipliedLinearPixel],
    layer: &[PremultipliedLinearPixel],
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if accumulator.len() != layer.len() {
        return Err(RenderError::new(
            "raster.composition",
            "layer dimensions do not match the composition accumulator",
        ));
    }
    accumulator
        .par_iter_mut()
        .zip(layer.par_iter())
        .try_for_each(|(destination, source)| {
            check_parallel_raster_cancellation(is_cancelled)?;
            match model {
                HalftoneChannelModel::Rgb => {
                    destination.red = (destination.red + source.red).clamp(0.0, 1.0);
                    destination.green = (destination.green + source.green).clamp(0.0, 1.0);
                    destination.blue = (destination.blue + source.blue).clamp(0.0, 1.0);
                    destination.alpha = (destination.alpha + source.alpha).clamp(0.0, 1.0);
                }
                HalftoneChannelModel::Cmyk if source.alpha > 0.0 => {
                    let straight = [
                        source.red / source.alpha,
                        source.green / source.alpha,
                        source.blue / source.alpha,
                    ];
                    destination.red *= 1.0 - source.alpha * (1.0 - straight[0]);
                    destination.green *= 1.0 - source.alpha * (1.0 - straight[1]);
                    destination.blue *= 1.0 - source.alpha * (1.0 - straight[2]);
                    destination.alpha *= 1.0 - source.alpha;
                }
                HalftoneChannelModel::Cmyk => {}
                HalftoneChannelModel::SourceColorAlpha => source_over(destination, *source),
            }
            Ok(())
        })
}

/// Finalizes a modeled streaming accumulator into premultiplied linear output.
///
/// # Errors
///
/// Returns cancellation before mutating a further independent accumulator
/// pixel. RGB and SourceColorAlpha require no conversion but still poll the
/// canonical cancellation authority.
fn finish_model_composition(
    model: HalftoneChannelModel,
    accumulator: &mut [PremultipliedLinearPixel],
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if model != HalftoneChannelModel::Cmyk {
        return check_parallel_raster_cancellation(is_cancelled);
    }
    accumulator.par_iter_mut().try_for_each(|pixel| {
        check_parallel_raster_cancellation(is_cancelled)?;
        let alpha = (1.0 - pixel.alpha).clamp(0.0, 1.0);
        pixel.red = boundary_clamp(pixel.red - (1.0 - alpha));
        pixel.green = boundary_clamp(pixel.green - (1.0 - alpha));
        pixel.blue = boundary_clamp(pixel.blue - (1.0 - alpha));
        pixel.alpha = alpha;
        Ok(())
    })
}

/// Folds one just-rasterized layer into an ordinary source-over accumulator.
///
/// # Errors
///
/// Returns cancellation or an internal raster-shape diagnostic without
/// accepting a partially composited final surface.
fn compose_source_over_layer(
    accumulator: &mut [PremultipliedLinearPixel],
    layer: &[PremultipliedLinearPixel],
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if accumulator.len() != layer.len() {
        return Err(RenderError::new(
            "raster.composition",
            "layer dimensions do not match the composition accumulator",
        ));
    }
    accumulator
        .par_iter_mut()
        .zip(layer.par_iter())
        .try_for_each(|(destination, source)| {
            check_parallel_raster_cancellation(is_cancelled)?;
            source_over(destination, *source);
            Ok(())
        })
}

/// Composes supplied test layers through the same streaming model accumulator.
///
/// # Errors
///
/// Returns the same cancellation and composition diagnostics as production
/// streaming rasterization without retaining a second composed buffer.
#[cfg(test)]
fn compose_model(
    model: HalftoneChannelModel,
    layers: &[Vec<PremultipliedLinearPixel>],
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<PremultipliedLinearPixel>, RenderError> {
    let count = layers.first().map_or(0, Vec::len);
    let width = u32::try_from(count).map_err(|_| {
        RenderError::new(
            "raster.allocation",
            "test composition width is not representable",
        )
    })?;
    let mut accumulator = begin_model_composition(model, width, 1)?;
    for layer in layers {
        compose_model_layer(model, &mut accumulator, layer, is_cancelled)?;
    }
    finish_model_composition(model, &mut accumulator, is_cancelled)?;
    Ok(accumulator)
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

/// Applies one final background independently to each premultiplied pixel.
///
/// # Errors
///
/// Returns canonical cancellation before a worker mutates its next local pixel.
fn apply_background(
    pixels: &mut [PremultipliedLinearPixel],
    background: RasterBackground,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    if matches!(background, RasterBackground::Transparent) {
        return check_parallel_raster_cancellation(is_cancelled);
    }
    let background = background_pixel(background);
    pixels.par_iter_mut().try_for_each(|pixel| {
        check_parallel_raster_cancellation(is_cancelled)?;
        let remaining = 1.0 - pixel.alpha;
        pixel.red += background.red * remaining;
        pixel.green += background.green * remaining;
        pixel.blue += background.blue * remaining;
        pixel.alpha = 1.0;
        Ok(())
    })
}

/// Quantizes a complete premultiplied linear buffer into straight row-major sRGBA bytes.
///
/// # Errors
///
/// Returns allocation, cancellation, or final surface validation failures without publication.
fn pixels_from_linear(
    width: u32,
    height: u32,
    linear_pixels: Vec<PremultipliedLinearPixel>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<RasterSurface, RenderError> {
    let expected_linear_count = raster_pixel_count(width, height)?;
    if linear_pixels.len() != expected_linear_count {
        return Err(RenderError::new(
            "raster.composition",
            "linear pixel count does not match raster dimensions",
        ));
    }
    let mut pixels = allocate_raster_bytes(width, height)?;
    pixels
        .par_chunks_mut(4)
        .zip(linear_pixels.par_iter())
        .try_for_each(|(output, pixel)| {
            check_parallel_raster_cancellation(is_cancelled)?;
            let alpha = pixel.alpha.clamp(0.0, 1.0);
            let (red, green, blue) = if alpha == 0.0 {
                (0.0, 0.0, 0.0)
            } else {
                (pixel.red / alpha, pixel.green / alpha, pixel.blue / alpha)
            };
            output.copy_from_slice(&[
                quantize_srgb(red),
                quantize_srgb(green),
                quantize_srgb(blue),
                quantize_linear(alpha),
            ]);
            Ok(())
        })?;
    RasterSurface::new(width, height, pixels)
}

/// Polls cancellation from an indexed raster worker using the canonical diagnostic.
///
/// # Errors
///
/// Returns `evaluation.cancelled` when the caller has superseded this raster request.
fn check_parallel_raster_cancellation(
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), RenderError> {
    (!is_cancelled()).then_some(()).ok_or(RenderError::new(
        "evaluation.cancelled",
        "rasterization was cancelled",
    ))
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

/// Rasterizes one geometry-owned region with fixed nonzero fill and final pixel clipping only.
///
/// # Errors
///
/// Returns cancellation, flattening, or request-wide edge-limit diagnostics without changing the
/// already-closed region ring or constructing renderer topology.
#[allow(clippy::too_many_arguments)]
fn composite_canonical_region(
    pixels: &mut [PremultipliedLinearPixel],
    width: u32,
    height: u32,
    region: &toniator_geometry::CanonicalRegion,
    color: &ColorValue,
    opacity: f64,
    transform: CanonicalRasterTransform,
    work: &mut RasterWork<'_>,
) -> Result<(), RenderError> {
    let edges = flattened_path_edges(&region.ring, transform, work)?;
    if edges.is_empty() {
        return Ok(());
    }
    let min = transform.point(region.bounds.min);
    let max = transform.point(region.bounds.max);
    let min_x = (min.x - 1.0).floor().max(0.0) as u32;
    let min_y = (min.y - 1.0).floor().max(0.0) as u32;
    let max_x = (max.x + 1.0).ceil().min(f64::from(width)) as u32;
    let max_y = (max.y + 1.0).ceil().min(f64::from(height)) as u32;
    for y in min_y..max_y {
        work.check()?;
        for x in min_x..max_x {
            let coverage = polygon_coverage_nonzero(&edges, x, y, work)?;
            if coverage == 0.0 {
                continue;
            }
            source_over(
                &mut pixels[y as usize * width as usize + x as usize],
                PremultipliedLinearPixel {
                    red: color.red * color.alpha * opacity * coverage,
                    green: color.green * color.alpha * opacity * coverage,
                    blue: color.blue * color.alpha * opacity * coverage,
                    alpha: color.alpha * opacity * coverage,
                },
            );
        }
    }
    Ok(())
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
    let row_starts = outline_row_edge_starts(&edges, min_y, max_y, work)?;
    if row_starts.is_empty() {
        return Ok(());
    }
    let mut covered = Vec::new();
    let mut crossings = Vec::new();
    crossings
        .try_reserve_exact(edges.len().min(256))
        .map_err(|_| {
            RenderError::new(
                "raster.allocation",
                "stroke scanline crossing allocation failed",
            )
        })?;
    let mut row_edges = Vec::new();
    row_edges
        .try_reserve_exact(row_starts.len())
        .map_err(|_| RenderError::new("raster.allocation", "stroke row-edge allocation failed"))?;
    let mut row_edge_end_rows = Vec::new();
    row_edge_end_rows
        .try_reserve_exact(row_starts.len())
        .map_err(|_| RenderError::new("raster.allocation", "stroke row-end allocation failed"))?;
    let samples = if matches!(work.antialiasing, RasterAntialiasing::On) {
        SUBPIXEL_GRID
    } else {
        1
    };
    let mut next_row_start = 0_usize;
    for y in min_y..max_y {
        work.check()?;
        advance_outline_edges_for_pixel_row(
            &row_starts,
            &mut next_row_start,
            y,
            &mut row_edges,
            &mut row_edge_end_rows,
        );
        if row_edges.is_empty() {
            continue;
        }
        let _scanline_work = accumulate_nonzero_scanline_coverage(
            &row_edges,
            y,
            min_x,
            max_x,
            samples,
            &mut covered,
            &mut crossings,
            work,
        )?;
        for (x, covered) in covered.iter().copied() {
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

/// One flattened outline edge scheduled for the bounded pixel-row interval it can sample.
///
/// The interval is half-open and derives only from finite flattened geometry. It is request-local
/// traversal state, never canonical geometry, clipping authority, or cache identity.
#[derive(Clone, Copy, Debug)]
struct OutlineRowEdgeStart {
    start_row: u32,
    end_row: u32,
    ordinal: usize,
    edge: (Point2, Point2),
}

/// Carries test-only traversal counts while resolving active nonzero scanline spans.
///
/// In test builds the counter describes raster traversal; production builds retain no counter
/// field or increment cost. It never changes canonical outline coverage or forms part of scene,
/// raster, or cache identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ScanlineCoverageWork {
    #[cfg(test)]
    visited_subpixel_samples: usize,
    #[cfg(test)]
    visited_active_edges: usize,
}

/// Bins each flattened outline edge at the first pixel row that can contain one of its samples.
///
/// The returned half-open row interval exactly satisfies the existing sample predicate
/// `edge_min_y < row + 1 && edge_max_y > row`. This turns per-row traversal into edge-row overlap
/// work while preserving deterministic activation order (source order for equal start rows) and
/// strict half-open crossing behavior.
///
/// # Errors
///
/// Returns cancellation or `raster.allocation` without mutating canonical geometry or a raster.
fn outline_row_edge_starts(
    edges: &[(Point2, Point2)],
    min_row: u32,
    max_row: u32,
    work: &RasterWork<'_>,
) -> Result<Vec<OutlineRowEdgeStart>, RenderError> {
    let mut starts = Vec::new();
    starts
        .try_reserve_exact(edges.len())
        .map_err(|_| RenderError::new("raster.allocation", "stroke row-edge allocation failed"))?;
    for (index, edge) in edges.iter().copied().enumerate() {
        if index % 128 == 0 {
            work.check()?;
        }
        let edge_min_y = edge.0.y.min(edge.1.y);
        let edge_max_y = edge.0.y.max(edge.1.y);
        let start_row = edge_min_y
            .floor()
            .max(f64::from(min_row))
            .min(f64::from(max_row)) as u32;
        let end_row = edge_max_y
            .ceil()
            .max(f64::from(min_row))
            .min(f64::from(max_row)) as u32;
        if start_row < end_row {
            starts.push(OutlineRowEdgeStart {
                start_row,
                end_row,
                ordinal: index,
                edge,
            });
        }
    }
    starts.sort_unstable_by_key(|entry| (entry.start_row, entry.ordinal));
    Ok(starts)
}

/// Advances one reusable row-active edge set without scanning inactive flattened edges.
///
/// `row_starts` stays sorted by row and both mutable vectors retain one entry per currently active
/// edge in identical order. The caller reserves their maximum possible capacity before calling, so
/// this hot path performs no allocation and does not alter nonzero-winding authority.
fn advance_outline_edges_for_pixel_row(
    row_starts: &[OutlineRowEdgeStart],
    next_row_start: &mut usize,
    row: u32,
    row_edges: &mut Vec<(Point2, Point2)>,
    row_edge_end_rows: &mut Vec<u32>,
) {
    while *next_row_start < row_starts.len() && row_starts[*next_row_start].start_row <= row {
        let entry = row_starts[*next_row_start];
        row_edges.push(entry.edge);
        row_edge_end_rows.push(entry.end_row);
        *next_row_start += 1;
    }
    let mut retained = 0_usize;
    for index in 0..row_edges.len() {
        if row_edge_end_rows[index] > row {
            if retained != index {
                row_edges[retained] = row_edges[index];
                row_edge_end_rows[retained] = row_edge_end_rows[index];
            }
            retained += 1;
        }
    }
    row_edges.truncate(retained);
    row_edge_end_rows.truncate(retained);
}

/// Accumulates exact nonzero 8x8 sample coverage for one pixel row from sorted crossings.
///
/// This preserves strict half-open winding tests at every sample center while visiting only the
/// active spans between nonzero crossings. The sparse caller-owned coverage buffer is reused and
/// contains only pixels that receive one or more samples; no geometry, clipping, or antialiasing
/// authority changes.
///
/// # Errors
///
/// Returns cancellation or fallible sparse-buffer allocation errors without publishing a partial
/// surface.
#[allow(clippy::too_many_arguments)]
fn accumulate_nonzero_scanline_coverage(
    edges: &[(Point2, Point2)],
    row: u32,
    min_x: u32,
    max_x: u32,
    samples: u32,
    covered: &mut Vec<(u32, u16)>,
    crossings: &mut Vec<(f64, i32)>,
    work: &RasterWork<'_>,
) -> Result<ScanlineCoverageWork, RenderError> {
    covered.clear();
    let mut coverage_work = ScanlineCoverageWork::default();
    for sub_y in 0..samples {
        work.check()?;
        crossings.clear();
        let sample_y = f64::from(row) + (f64::from(sub_y) + 0.5) / f64::from(samples);
        #[cfg(test)]
        {
            coverage_work.visited_active_edges = coverage_work
                .visited_active_edges
                .saturating_add(edges.len());
        }
        for (start, end) in edges {
            let contribution = if start.y <= sample_y && end.y > sample_y {
                1
            } else if start.y > sample_y && end.y <= sample_y {
                -1
            } else {
                continue;
            };
            let intersection_x =
                start.x + (end.x - start.x) * (sample_y - start.y) / (end.y - start.y);
            if crossings.len() == crossings.capacity() {
                crossings
                    .try_reserve(edges.len().saturating_sub(crossings.len()).clamp(1, 256))
                    .map_err(|_| {
                        RenderError::new(
                            "raster.allocation",
                            "stroke scanline crossing allocation failed",
                        )
                    })?;
            }
            crossings.push((intersection_x, contribution));
        }
        if crossings.is_empty() {
            continue;
        }
        crossings.sort_unstable_by(|first, second| first.0.total_cmp(&second.0));
        let mut winding = crossings
            .iter()
            .map(|(_, contribution)| *contribution)
            .sum::<i32>();
        let mut span_start = (winding != 0).then_some(f64::from(min_x));
        let mut crossing_index = 0_usize;
        while crossing_index < crossings.len() {
            let crossing_x = crossings[crossing_index].0;
            let prior_winding = winding;
            while crossing_index < crossings.len() && crossings[crossing_index].0 == crossing_x {
                winding -= crossings[crossing_index].1;
                crossing_index += 1;
            }
            match (prior_winding == 0, winding == 0) {
                (true, false) => span_start = Some(crossing_x),
                (false, true) => {
                    accumulate_nonzero_span_coverage(
                        span_start.unwrap_or(f64::from(min_x)),
                        crossing_x,
                        min_x,
                        max_x,
                        samples,
                        covered,
                        &mut coverage_work,
                        work,
                    )?;
                    span_start = None;
                }
                _ => {}
            }
        }
        if let Some(span_start) = span_start {
            accumulate_nonzero_span_coverage(
                span_start,
                f64::from(max_x),
                min_x,
                max_x,
                samples,
                covered,
                &mut coverage_work,
                work,
            )?;
        }
    }
    Ok(coverage_work)
}

/// Adds the exact subpixel samples in one active nonzero span to sparse row coverage.
///
/// The span remains half-open (`start <= sample < end`) so its ownership matches the historical
/// crossing rule. Iteration is clipped only at the final raster row bounds; canonical outline
/// geometry is not clipped or rewritten.
///
/// # Errors
///
/// Returns cancellation during bounded active-span traversal or `raster.allocation` when a newly
/// active pixel cannot be appended to the reusable sparse coverage collection.
#[allow(clippy::too_many_arguments)]
fn accumulate_nonzero_span_coverage(
    start: f64,
    end: f64,
    min_x: u32,
    max_x: u32,
    samples: u32,
    covered: &mut Vec<(u32, u16)>,
    coverage_work: &mut ScanlineCoverageWork,
    work: &RasterWork<'_>,
) -> Result<(), RenderError> {
    #[cfg(not(test))]
    let _ = coverage_work;
    let clipped_start = start.max(f64::from(min_x));
    let clipped_end = end.min(f64::from(max_x));
    if clipped_end <= clipped_start {
        return Ok(());
    }
    let first_pixel = clipped_start.floor().max(f64::from(min_x)) as u32;
    let end_pixel = clipped_end.ceil().min(f64::from(max_x)) as u32;
    for (offset, x) in (first_pixel..end_pixel).enumerate() {
        if offset % 64 == 0 {
            work.check()?;
        }
        for sub_x in 0..samples {
            #[cfg(test)]
            {
                coverage_work.visited_subpixel_samples =
                    coverage_work.visited_subpixel_samples.saturating_add(1);
            }
            let sample_x = f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(samples);
            if sample_x >= clipped_start && sample_x < clipped_end {
                record_sparse_coverage(covered, x)?;
            }
        }
    }
    Ok(())
}

/// Increments one active output pixel in a sparse, X-sorted row-coverage collection.
///
/// The collection stores only sampled pixels and preserves stable X order for deterministic
/// source-over compositing. Per-pixel coverage is bounded by the fixed antialiasing grid.
///
/// # Errors
///
/// Returns `raster.allocation` if adding a newly covered pixel cannot grow the reusable vector.
fn record_sparse_coverage(covered: &mut Vec<(u32, u16)>, x: u32) -> Result<(), RenderError> {
    match covered.binary_search_by_key(&x, |(covered_x, _)| *covered_x) {
        Ok(index) => covered[index].1 += 1,
        Err(index) => {
            if covered.len() == covered.capacity() {
                covered.try_reserve(1).map_err(|_| {
                    RenderError::new(
                        "raster.allocation",
                        "stroke sparse scanline coverage allocation failed",
                    )
                })?;
            }
            covered.insert(index, (x, 1));
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
    const PIXEL_TOLERANCE: f64 = 1.0 / 64.0;
    if control_polygon_extent([a, b, c, d]) <= PIXEL_TOLERANCE {
        work.check()?;
        return push_flattened_point(points, d, work);
    }
    let flatness = point_line_distance(b, a, d).max(point_line_distance(c, a, d));
    work.check()?;
    if flatness <= PIXEL_TOLERANCE {
        return push_flattened_point(points, d, work);
    }
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let mid = midpoint(abc, bcd);
    if depth >= 60 || mid == a || mid == d {
        if control_polygon_extent([a, b, c, d]) <= PIXEL_TOLERANCE {
            return push_flattened_point(points, d, work);
        }
        return Err(RenderError::new(
            "raster.flatten.numeric",
            "cubic subdivision cannot meet output-pixel tolerance",
        ));
    }
    flatten_cubic(a, ab, abc, mid, depth + 1, points, work)?;
    flatten_cubic(mid, bcd, cd, d, depth + 1, points, work)
}

/// Returns the finite transformed control-polygon diameter before chord-based flattening.
fn control_polygon_extent(points: [Point2; 4]) -> f64 {
    points
        .iter()
        .enumerate()
        .flat_map(|(index, first)| {
            points[index + 1..]
                .iter()
                .map(move |second| (first.x - second.x).hypot(first.y - second.y))
        })
        .fold(0.0_f64, f64::max)
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

/// Samples one closed canonical region through fixed nonzero winding coverage.
///
/// # Errors
///
/// Returns cancellation while visiting finite subpixel and edge work.
fn polygon_coverage_nonzero(
    edges: &[(Point2, Point2)],
    x: u32,
    y: u32,
    work: &RasterWork<'_>,
) -> Result<f64, RenderError> {
    let samples = if matches!(work.antialiasing, RasterAntialiasing::On) {
        SUBPIXEL_GRID
    } else {
        1
    };
    let mut inside = 0_u32;
    for sub_y in 0..samples {
        for sub_x in 0..samples {
            work.check()?;
            let point = Point2::new(
                f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(samples),
                f64::from(y) + (f64::from(sub_y) + 0.5) / f64::from(samples),
            );
            if point_in_nonzero_outline(edges, point) {
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
        for output in &layer.outputs {
            write_svg_geometry(
                &mut document,
                output.geometry(),
                layer.channel_id.0,
                None,
                None,
                &scene.canvas,
            );
        }
        document.push_str("</g>\n");
    }
    document.push_str("</svg>\n");
    document
}

/// Serializes one modeled scene as deterministic editable SVG with its fixed model compositor.
///
/// RGB retains its accepted editable screen-blend structure, CMYK emits explicit linear-light
/// same-document filter inputs, and SourceColorAlpha retains ordinary ordered source-over.
fn write_modeled_svg(scene: &RenderScene, model: HalftoneChannelModel) -> String {
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let title = match model {
        HalftoneChannelModel::Rgb => "Toniator RGB halftone",
        HalftoneChannelModel::Cmyk => "Toniator CMYK halftone",
        HalftoneChannelModel::SourceColorAlpha => "Toniator source-colored halftone",
    };
    let mut document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<title>{title}</title>\n<metadata>family={};realization={};scene={}</metadata>\n<defs><clipPath id=\"canvas-clip\"><rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\"/></clipPath>\n",
        xml_escape(&scene.identity.family_fingerprint),
        xml_escape(&scene.identity.realization_fingerprint),
        xml_escape(&scene.identity.scene_fingerprint),
    );
    if model == HalftoneChannelModel::Cmyk {
        write_cmyk_svg_filter_definitions(&mut document, scene);
    }
    document.push_str("</defs>\n");
    document.push_str(
        "<g id=\"canvas\" clip-path=\"url(#canvas-clip)\" style=\"isolation:isolate\">\n",
    );
    let blend_mode = match model {
        HalftoneChannelModel::Rgb => Some("screen"),
        HalftoneChannelModel::Cmyk | HalftoneChannelModel::SourceColorAlpha => None,
    };
    if model == HalftoneChannelModel::Cmyk {
        document
            .push_str("<g id=\"cmyk-composition\" filter=\"url(#cmyk-fixed-transmittance)\">\n");
    }
    for layer in &scene.layers {
        let filter_id = (model == HalftoneChannelModel::Cmyk)
            .then(|| format!("channel-{}-atlas-slot", layer.channel_id.0));
        write_svg_channel_group(
            &mut document,
            layer,
            blend_mode,
            filter_id.as_deref(),
            model == HalftoneChannelModel::Cmyk,
            &scene.canvas,
        );
    }
    if model == HalftoneChannelModel::Cmyk {
        document.push_str("</g>\n");
    }
    document.push_str("</g>\n");
    document.push_str("</svg>\n");
    document
}

/// Appends the same-document CMYK channel atlas and fixed-transmittance filter graph.
///
/// Each live channel group remains in canvas coordinates and uses a nested filter to move only
/// its rendered layer-local result into a disjoint atlas slot. The parent filter extracts those
/// slots from one `SourceGraphic`, restores them to the canvas origin, and applies the protected
/// linear-light transmittance and coverage-union equations. This avoids fragment `feImage`
/// references, embedded rasters, viewer blend modes, and duplicate proxy geometry.
fn write_cmyk_svg_filter_definitions(document: &mut String, scene: &RenderScene) {
    let column_count = scene.layers.len().min(2);
    let row_count = scene.layers.len().div_ceil(column_count);
    let atlas_width = scene.canvas.width * column_count as f64;
    let atlas_height = scene.canvas.height * row_count as f64;
    let width = compact_number(scene.canvas.width);
    let height = compact_number(scene.canvas.height);
    let atlas_width = compact_number(atlas_width);
    let atlas_height = compact_number(atlas_height);

    for (index, layer) in scene.layers.iter().enumerate() {
        let slot_x = scene.canvas.width * (index % column_count) as f64;
        let slot_y = scene.canvas.height * (index / column_count) as f64;
        document.push_str(&format!(
            "<filter id=\"channel-{}-atlas-slot\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{atlas_width}\" height=\"{atlas_height}\" color-interpolation-filters=\"linearRGB\"><feOffset in=\"SourceGraphic\" dx=\"{}\" dy=\"{}\"/></filter>\n",
            layer.channel_id.0,
            compact_number(slot_x),
            compact_number(slot_y),
        ));
    }

    document.push_str(&format!(
        "<filter id=\"cmyk-fixed-transmittance\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{atlas_width}\" height=\"{atlas_height}\" color-interpolation-filters=\"linearRGB\">\n<feFlood x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" flood-color=\"#ffffff\" result=\"canvas-white\"/>\n"
    ));
    for (index, layer) in scene.layers.iter().enumerate() {
        let slot_x = scene.canvas.width * (index % column_count) as f64;
        let slot_y = scene.canvas.height * (index / column_count) as f64;
        let channel_id = layer.channel_id.0;
        document.push_str(&format!(
            "<feFlood x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{height}\" flood-color=\"#ffffff\" result=\"channel-{channel_id}-slot\"/>\n<feComposite in=\"SourceGraphic\" in2=\"channel-{channel_id}-slot\" operator=\"in\" result=\"channel-{channel_id}-atlas\"/>\n<feOffset in=\"channel-{channel_id}-atlas\" dx=\"{}\" dy=\"{}\" result=\"channel-{channel_id}-layer\"/>\n<feComposite in=\"channel-{channel_id}-layer\" in2=\"canvas-white\" operator=\"over\" result=\"channel-{channel_id}-factor\"/>\n<feComposite in=\"canvas-white\" in2=\"channel-{channel_id}-layer\" operator=\"in\" result=\"channel-{channel_id}-coverage\"/>\n",
            compact_number(slot_x),
            compact_number(slot_y),
            compact_number(if slot_x == 0.0 { 0.0 } else { -slot_x }),
            compact_number(if slot_y == 0.0 { 0.0 } else { -slot_y }),
        ));
    }

    let first_channel = scene.layers[0].channel_id.0;
    let mut transmittance = format!("channel-{first_channel}-factor");
    let mut coverage = format!("channel-{first_channel}-coverage");
    for (index, layer) in scene.layers.iter().enumerate().skip(1) {
        let channel_id = layer.channel_id.0;
        let next_transmittance = format!("cmyk-transmittance-{index}");
        let next_coverage = format!("cmyk-coverage-{index}");
        document.push_str(&format!(
            "<feBlend in=\"{transmittance}\" in2=\"channel-{channel_id}-factor\" mode=\"multiply\" result=\"{next_transmittance}\"/>\n<feComposite in=\"channel-{channel_id}-coverage\" in2=\"{coverage}\" operator=\"over\" result=\"{next_coverage}\"/>\n"
        ));
        transmittance = next_transmittance;
        coverage = next_coverage;
    }
    document.push_str(&format!(
        "<feComposite in=\"{transmittance}\" in2=\"{coverage}\" operator=\"arithmetic\" k2=\"1\" k3=\"1\" k4=\"-1\"/>\n</filter>\n"
    ));
}

/// Appends one modeled channel group with immutable presentation and per-mark paint semantics.
///
/// An optional filter moves only the rendered group result into a same-document model-compositor
/// input while all child geometry retains its canonical canvas coordinates and stable IDs. The
/// optional inner canvas clip bounds a CMYK layer before the filter moves it into its atlas slot.
fn write_svg_channel_group(
    document: &mut String,
    layer: &RenderLayer,
    blend_mode: Option<&str>,
    filter_id: Option<&str>,
    clip_layer_to_canvas: bool,
    canvas: &CanvasSpec,
) {
    let mut styles = Vec::new();
    if let Some(mode) = blend_mode {
        styles.push(format!("mix-blend-mode:{mode}"));
    }
    if !layer.visible {
        styles.push("display:none".to_owned());
    }
    let filter = filter_id.map_or_else(String::new, |filter_id| {
        format!(" filter=\"url(#{filter_id})\"")
    });
    let style = (!styles.is_empty()).then(|| format!(" style=\"{}\"", styles.join(";")));
    document.push_str(&format!(
        "<g id=\"channel-{}\"{filter}{}>\n",
        layer.channel_id.0,
        style.unwrap_or_default(),
    ));
    if clip_layer_to_canvas {
        document.push_str("<g clip-path=\"url(#canvas-clip)\">\n");
    }
    for output in &layer.outputs {
        write_svg_geometry(
            document,
            output.geometry(),
            layer.channel_id.0,
            Some(layer),
            Some(output),
            canvas,
        );
    }
    if clip_layer_to_canvas {
        document.push_str("</g>\n");
    }
    document.push_str("</g>\n");
}

/// Writes editable canonical circle or cubic path geometry without resolving document resources.
fn write_svg_geometry(
    document: &mut String,
    geometry: &GeometryOutput,
    channel_id: u64,
    layer: Option<&RenderLayer>,
    output: Option<&RenderOutputLayer>,
    canvas: &CanvasSpec,
) {
    match geometry {
        GeometryOutput::CircularMarks(marks) => {
            for (index, mark) in marks.iter().enumerate() {
                let paint = layer.map_or_else(String::new, |layer| {
                    format!(
                        " fill=\"{}\"",
                        color_hex(output_primitive_paint(layer, output, index))
                    )
                });
                let opacity = layer.map_or_else(
                    || "".to_owned(),
                    |layer| {
                        format!(
                            " fill-opacity=\"{}\"",
                            compact_number(
                                output_primitive_paint(layer, output, index).alpha * layer.opacity
                            )
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
                    format!(
                        " fill=\"{}\"",
                        color_hex(output_primitive_paint(layer, output, index))
                    )
                });
                let opacity = layer.map_or_else(
                    || "".to_owned(),
                    |layer| {
                        format!(
                            " fill-opacity=\"{}\"",
                            compact_number(
                                output_primitive_paint(layer, output, index).alpha * layer.opacity
                            )
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
        GeometryOutput::CanonicalRegions(regions) => {
            for (index, region) in regions.regions().iter().enumerate() {
                if region.bounds.max.x < 0.0
                    || region.bounds.max.y < 0.0
                    || region.bounds.min.x > canvas.width
                    || region.bounds.min.y > canvas.height
                {
                    continue;
                }
                let data = svg_curve_path_data(&region.ring);
                let paint = layer.map_or_else(String::new, |value| {
                    let paint = output_primitive_paint(value, output, index);
                    format!(
                        " fill=\"{}\" fill-opacity=\"{}\"",
                        color_hex(paint),
                        compact_number(paint.alpha * value.opacity)
                    )
                });
                document.push_str(&format!("<path id=\"channel-{channel_id}-region-{index}\" d=\"{data}\" fill-rule=\"nonzero\"{paint}/>\n"));
            }
        }
    }
}

/// Resolves one output-local sampled paint while preserving channel-owned solid-paint fallback.
///
/// Region and mark consumers share this lookup only after validation has established exact
/// primitive cardinality; it never synthesizes, associates, or repairs paint data.
fn output_primitive_paint<'a>(
    layer: &'a RenderLayer,
    output: Option<&'a RenderOutputLayer>,
    index: usize,
) -> &'a ColorValue {
    output
        .and_then(|output| output.primitive_paints.as_ref())
        .and_then(|paints| paints.get(index))
        .unwrap_or(&layer.color)
}

/// Serializes one already-closed canonical curve path without constructing or repairing topology.
fn svg_curve_path_data(path: &toniator_geometry::CurvePath) -> String {
    let mut data = String::new();
    for (index, segment) in path.segments().iter().enumerate() {
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
    data
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
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use super::*;
    use toniator_domain::PatternMechanismId;
    use toniator_geometry::{
        CanonicalPathMark, CubicBezierSegment, CurvePath, CurveSegment, FamilySiteId,
        FamilySiteProvenance, PathClosure,
    };

    /// Proves omitted PNG backing follows only modeled output authority and preserves legacy transparency.
    #[test]
    fn omitted_png_background_follows_channel_model() {
        assert_eq!(
            RasterBackground::default_for_model(Some(HalftoneChannelModel::Rgb)),
            RasterBackground::OpaqueBlack
        );
        assert_eq!(
            RasterBackground::default_for_model(Some(HalftoneChannelModel::Cmyk)),
            RasterBackground::OpaqueWhite
        );
        assert_eq!(
            RasterBackground::default_for_model(Some(HalftoneChannelModel::SourceColorAlpha)),
            RasterBackground::Transparent
        );
        assert_eq!(
            RasterBackground::default_for_model(None),
            RasterBackground::Transparent
        );
    }

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

    /// Builds two deterministic premultiplied channel buffers for modeled pixel-stage tests.
    fn modeled_linear_layers() -> Vec<Vec<PremultipliedLinearPixel>> {
        let count = 64 * 64;
        let first = (0..count)
            .map(|index| {
                let alpha = 0.25 + f64::from((index % 5) as u8) * 0.05;
                PremultipliedLinearPixel {
                    red: alpha * 0.8,
                    green: alpha * 0.2,
                    blue: alpha * 0.1,
                    alpha,
                }
            })
            .collect();
        let second = (0..count)
            .map(|index| {
                let alpha = 0.15 + f64::from((index % 7) as u8) * 0.04;
                PremultipliedLinearPixel {
                    red: alpha * 0.1,
                    green: alpha * 0.6,
                    blue: alpha * 0.9,
                    alpha,
                }
            })
            .collect();
        vec![first, second]
    }

    /// Builds a probe that cancels only after execution enters a Rayon worker.
    fn worker_cancellation_probe(observed: &AtomicBool) -> impl Fn() -> bool + Sync + '_ {
        move || {
            let in_worker = rayon::current_thread_index().is_some();
            observed.fetch_or(in_worker, Ordering::Relaxed);
            in_worker
        }
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

    /// Proves observed preview rasterization reports primitives and parallel phases without changing pixels.
    #[test]
    fn preview_raster_progress_is_complete_and_pixel_neutral() {
        let square = CurvePath::polyline(
            vec![
                Point2::new(1.0, 1.0),
                Point2::new(4.0, 1.0),
                Point2::new(4.0, 4.0),
                Point2::new(1.0, 4.0),
            ],
            PathClosure::Closed,
        )
        .expect("square fixture is closed");
        let legacy = canonical_scene(vec![path_mark(0, square)]);
        let scene = RenderScene::new_modeled(
            legacy.canvas.clone(),
            "progress-family".into(),
            "progress-realization".into(),
            HalftoneChannelModel::Rgb,
            legacy.layers,
        )
        .expect("modeled progress scene validates");
        let target = PreviewRasterTarget::new(40, 40).expect("preview target is bounded");
        let expected =
            rasterize_preview_cancellable(&scene, target, RasterizationLimits::default(), &|| {
                false
            })
            .expect("ordinary preview rasterizes");
        let progress = std::sync::Mutex::new(Vec::new());
        let observed = rasterize_preview_cancellable_with_progress(
            &scene,
            target,
            RasterizationLimits::default(),
            &|| false,
            &|completed, total| progress.lock().unwrap().push((completed, total)),
        )
        .expect("observed preview rasterizes");
        assert_eq!(observed, expected);
        let progress = progress.into_inner().unwrap();
        assert_eq!(progress.len(), 4);
        assert!(progress.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let &(completed, total) = progress.last().expect("progress completes");
        assert_eq!(completed, total);
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

    /// Proves scanline winding coverage matches full-outline membership at every subpixel sample.
    #[test]
    fn scanline_nonzero_coverage_preserves_exact_subpixel_membership() {
        let edges = vec![
            (Point2::new(1.0, 1.0), Point2::new(7.0, 2.0)),
            (Point2::new(7.0, 2.0), Point2::new(6.0, 7.0)),
            (Point2::new(6.0, 7.0), Point2::new(2.0, 6.0)),
            (Point2::new(2.0, 6.0), Point2::new(1.0, 1.0)),
        ];
        let is_cancelled = || false;
        let work = RasterWork::new(
            RasterizationLimits::new(100).expect("focused edge budget is nonzero"),
            RasterAntialiasing::On,
            &is_cancelled,
        );
        let mut covered = Vec::new();
        let mut crossings = Vec::with_capacity(edges.len());
        for row in 0..8 {
            accumulate_nonzero_scanline_coverage(
                &edges,
                row,
                0,
                8,
                SUBPIXEL_GRID,
                &mut covered,
                &mut crossings,
                &work,
            )
            .expect("finite scanline coverage succeeds");
            for x in 0..8 {
                let mut direct = 0_u16;
                for sub_y in 0..SUBPIXEL_GRID {
                    for sub_x in 0..SUBPIXEL_GRID {
                        let point = Point2::new(
                            f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(SUBPIXEL_GRID),
                            f64::from(row) + (f64::from(sub_y) + 0.5) / f64::from(SUBPIXEL_GRID),
                        );
                        direct += u16::from(point_in_nonzero_outline(&edges, point));
                    }
                }
                assert_eq!(
                    covered
                        .iter()
                        .find_map(|(covered_x, coverage)| (*covered_x == x).then_some(*coverage))
                        .unwrap_or(0),
                    direct
                );
            }
        }
    }

    /// Builds one explicitly oriented rectangular contour for nonzero-winding scanline fixtures.
    fn rectangle_outline(
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        clockwise: bool,
    ) -> Vec<(Point2, Point2)> {
        let points = if clockwise {
            vec![
                Point2::new(min_x, min_y),
                Point2::new(min_x, max_y),
                Point2::new(max_x, max_y),
                Point2::new(max_x, min_y),
                Point2::new(min_x, min_y),
            ]
        } else {
            vec![
                Point2::new(min_x, min_y),
                Point2::new(max_x, min_y),
                Point2::new(max_x, max_y),
                Point2::new(min_x, max_y),
                Point2::new(min_x, min_y),
            ]
        };
        points
            .windows(2)
            .map(|points| (points[0], points[1]))
            .collect()
    }

    /// Verifies row-active sparse coverage against the retained point-membership winding authority.
    ///
    /// The bounded input range is interpreted as final raster clipping only; direct membership
    /// always receives the complete unmodified outline. The returned test-only work counts expose
    /// edge-row traversal without affecting production pixels or identities.
    #[allow(clippy::too_many_arguments)]
    fn assert_row_active_coverage_matches_point_membership(
        edges: &[(Point2, Point2)],
        min_row: u32,
        max_row: u32,
        min_x: u32,
        max_x: u32,
        samples: u32,
    ) -> ScanlineCoverageWork {
        let is_cancelled = || false;
        let work = RasterWork::new(
            RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
            if samples == 1 {
                RasterAntialiasing::Off
            } else {
                RasterAntialiasing::On
            },
            &is_cancelled,
        );
        let row_starts = outline_row_edge_starts(edges, min_row, max_row, &work)
            .expect("finite fixture schedules rows");
        let mut covered = Vec::new();
        let mut crossings = Vec::with_capacity(edges.len());
        let mut active_edges = Vec::with_capacity(row_starts.len());
        let mut active_edge_end_rows = Vec::with_capacity(row_starts.len());
        let mut next_row_start = 0_usize;
        let mut total_work = ScanlineCoverageWork::default();
        for row in min_row..max_row {
            advance_outline_edges_for_pixel_row(
                &row_starts,
                &mut next_row_start,
                row,
                &mut active_edges,
                &mut active_edge_end_rows,
            );
            let scanline_work = accumulate_nonzero_scanline_coverage(
                &active_edges,
                row,
                min_x,
                max_x,
                samples,
                &mut covered,
                &mut crossings,
                &work,
            )
            .expect("finite row coverage succeeds");
            total_work.visited_subpixel_samples = total_work
                .visited_subpixel_samples
                .saturating_add(scanline_work.visited_subpixel_samples);
            total_work.visited_active_edges = total_work
                .visited_active_edges
                .saturating_add(scanline_work.visited_active_edges);
            assert!(
                covered
                    .iter()
                    .all(|(covered_x, _)| *covered_x >= min_x && *covered_x < max_x),
                "sparse coverage remains final-raster clipped"
            );
            for x in min_x..max_x {
                let mut direct = 0_u16;
                for sub_y in 0..samples {
                    for sub_x in 0..samples {
                        let point = Point2::new(
                            f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(samples),
                            f64::from(row) + (f64::from(sub_y) + 0.5) / f64::from(samples),
                        );
                        direct += u16::from(point_in_nonzero_outline(edges, point));
                    }
                }
                assert_eq!(
                    covered
                        .iter()
                        .find_map(|(covered_x, coverage)| (*covered_x == x).then_some(*coverage))
                        .unwrap_or(0),
                    direct,
                    "row={row}, x={x}, samples={samples}"
                );
            }
        }
        total_work
    }

    /// Proves row-active coverage preserves disjoint, nested, coincident, binary-AA, and clipped
    /// nonzero-winding cases against the existing point-membership authority.
    #[test]
    fn row_active_scanlines_preserve_nonzero_winding_parity_at_boundary_cases() {
        let mut disjoint = rectangle_outline(1.0, 1.0, 3.0, 4.0, false);
        disjoint.extend(rectangle_outline(5.0, 1.0, 7.0, 4.0, false));
        assert_row_active_coverage_matches_point_membership(&disjoint, 0, 6, 0, 8, SUBPIXEL_GRID);

        let mut nested_same = rectangle_outline(1.0, 1.0, 7.0, 7.0, false);
        nested_same.extend(rectangle_outline(3.0, 3.0, 5.0, 5.0, false));
        assert_row_active_coverage_matches_point_membership(
            &nested_same,
            0,
            8,
            0,
            8,
            SUBPIXEL_GRID,
        );

        let mut nested_opposite = rectangle_outline(1.0, 1.0, 7.0, 7.0, false);
        nested_opposite.extend(rectangle_outline(3.0, 3.0, 5.0, 5.0, true));
        assert_row_active_coverage_matches_point_membership(
            &nested_opposite,
            0,
            8,
            0,
            8,
            SUBPIXEL_GRID,
        );

        let mut coincident_opposite = rectangle_outline(1.0, 1.0, 7.0, 7.0, false);
        coincident_opposite.extend(rectangle_outline(1.0, 1.0, 7.0, 7.0, true));
        assert_row_active_coverage_matches_point_membership(&coincident_opposite, 0, 8, 0, 8, 1);

        let mut clipped = rectangle_outline(-3.0, 1.0, 3.0, 5.0, false);
        clipped.extend(rectangle_outline(7.0, 1.0, 12.0, 5.0, false));
        assert_row_active_coverage_matches_point_membership(&clipped, 0, 6, 0, 10, 1);
    }

    /// Proves a rotated thin multi-contour outline performs active edge-row traversal rather than
    /// rescanning every flattened edge for every row while retaining exact sparse coverage.
    #[test]
    fn row_active_sweep_avoids_full_edge_scans_for_rotated_thin_multisegment_outline() {
        let mut edges = Vec::new();
        for band in 0..96_u32 {
            let y = f64::from(band * 4);
            edges.extend_from_slice(&[
                (Point2::new(0.0, y), Point2::new(2.0, y)),
                (Point2::new(2.0, y), Point2::new(62.0, y + 2.0)),
                (Point2::new(62.0, y + 2.0), Point2::new(60.0, y + 2.0)),
                (Point2::new(60.0, y + 2.0), Point2::new(0.0, y)),
            ]);
        }
        let work = assert_row_active_coverage_matches_point_membership(&edges, 0, 384, 0, 64, 1);
        let full_edge_scans = 384_usize * edges.len();
        assert!(
            work.visited_active_edges < full_edge_scans / 32,
            "row-active sweep visited {} of {full_edge_scans} full edge scans",
            work.visited_active_edges
        );
    }

    /// Proves a rotated thin outline visits only crossing-derived spans while retaining exact
    /// nonzero 8x8 coverage across its much wider axis-aligned raster bounding box.
    #[test]
    fn rotated_thin_stroke_scanline_visits_only_active_crossing_spans() {
        let edges = vec![
            (Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)),
            (Point2::new(2.0, 0.0), Point2::new(202.0, 200.0)),
            (Point2::new(202.0, 200.0), Point2::new(200.0, 200.0)),
            (Point2::new(200.0, 200.0), Point2::new(0.0, 0.0)),
        ];
        let is_cancelled = || false;
        let work = RasterWork::new(
            RasterizationLimits::new(1_000).expect("focused edge budget is nonzero"),
            RasterAntialiasing::On,
            &is_cancelled,
        );
        let mut covered = Vec::new();
        let mut crossings = Vec::with_capacity(edges.len());
        let scanline_work = accumulate_nonzero_scanline_coverage(
            &edges,
            100,
            0,
            202,
            SUBPIXEL_GRID,
            &mut covered,
            &mut crossings,
            &work,
        )
        .expect("finite diagonal outline scanline succeeds");

        for x in 0..202 {
            let mut direct = 0_u16;
            for sub_y in 0..SUBPIXEL_GRID {
                for sub_x in 0..SUBPIXEL_GRID {
                    let point = Point2::new(
                        f64::from(x) + (f64::from(sub_x) + 0.5) / f64::from(SUBPIXEL_GRID),
                        100.0 + (f64::from(sub_y) + 0.5) / f64::from(SUBPIXEL_GRID),
                    );
                    direct += u16::from(point_in_nonzero_outline(&edges, point));
                }
            }
            assert_eq!(
                covered
                    .iter()
                    .find_map(|(covered_x, coverage)| (*covered_x == x).then_some(*coverage))
                    .unwrap_or(0),
                direct,
                "x={x}"
            );
        }
        let full_aabb_subpixel_tests = 202_usize
            * usize::try_from(SUBPIXEL_GRID).expect("fixed grid fits usize")
            * usize::try_from(SUBPIXEL_GRID).expect("fixed grid fits usize");
        assert!(
            scanline_work.visited_subpixel_samples < full_aabb_subpixel_tests / 10,
            "rotated thin stroke visited {} of {full_aabb_subpixel_tests} full-AABB samples",
            scanline_work.visited_subpixel_samples
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
        let polls = AtomicU32::new(0);
        let error = rasterize_output_cancellable(
            &path_scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(80, 80).expect("output target is bounded")),
            RasterAntialiasing::On,
            RasterizationLimits::new(100_000).expect("focused edge budget is bounded"),
            &|| {
                let next = polls.fetch_add(1, Ordering::Relaxed) + 1;
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
        let polls = AtomicU32::new(0);
        let error = rasterize_cancellable(
            &circle_scene,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| {
                let next = polls.fetch_add(1, Ordering::Relaxed) + 1;
                next > 8
            },
        )
        .expect_err("canonical ellipse sampling must stop when the probe cancels");
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Proves indexed pixel composition and quantization preserve exact native RGBA bytes.
    #[test]
    fn parallel_raster_output_matches_single_worker_bytes() {
        let scene = canonical_scene(vec![CanonicalMark::Circle {
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
        }]);
        let run = || {
            rasterize_output_cancellable(
                &scene,
                RasterBackground::Transparent,
                Some(OutputRasterTarget::new(160, 160).expect("target is bounded")),
                RasterAntialiasing::On,
                RasterizationLimits::default(),
                &|| false,
            )
            .expect("rasterization completes")
        };
        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("one-worker pool builds")
            .install(run);
        let many = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .expect("four-worker pool builds")
            .install(run);
        assert_eq!(one, many);
        assert_eq!(one.pixels(), many.pixels());
    }

    /// Proves modeled CMYK/source-alpha composition, backgrounds, and quantization are byte exact.
    #[test]
    fn modeled_pixel_stages_match_one_worker_for_transparent_and_opaque_outputs() {
        let layers = modeled_linear_layers();
        for model in [
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            for background in [RasterBackground::Transparent, RasterBackground::OpaqueWhite] {
                let run = || {
                    let mut pixels = compose_model(model, &layers, &|| false)
                        .expect("modeled composition completes");
                    apply_background(&mut pixels, background, &|| false)
                        .expect("background composition completes");
                    pixels_from_linear(64, 64, pixels, &|| false)
                        .expect("modeled quantization completes")
                };
                let one = rayon::ThreadPoolBuilder::new()
                    .num_threads(1)
                    .build()
                    .unwrap()
                    .install(run);
                let many = rayon::ThreadPoolBuilder::new()
                    .num_threads(4)
                    .build()
                    .unwrap()
                    .install(run);
                assert_eq!(one, many);
                assert_eq!(one.pixels(), many.pixels());
            }
        }
    }

    /// Proves the streaming accumulator preserves the former per-pixel modeled equations byte-for-byte.
    ///
    /// # Panics
    ///
    /// Panics when a modeled RGB, CMYK, or source-over layer order changes its
    /// transparent final raster bytes.
    #[test]
    fn streaming_modeled_composition_matches_reference_layer_equations() {
        let layers = modeled_linear_layers();
        let count = layers[0].len();
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let expected = (0..count)
                .map(|index| match model {
                    HalftoneChannelModel::Rgb => {
                        let mut pixel = background_pixel(RasterBackground::Transparent);
                        for layer in &layers {
                            let source = layer[index];
                            pixel.red = (pixel.red + source.red).clamp(0.0, 1.0);
                            pixel.green = (pixel.green + source.green).clamp(0.0, 1.0);
                            pixel.blue = (pixel.blue + source.blue).clamp(0.0, 1.0);
                            pixel.alpha = (pixel.alpha + source.alpha).clamp(0.0, 1.0);
                        }
                        pixel
                    }
                    HalftoneChannelModel::Cmyk => {
                        let mut transmittance = [1.0; 3];
                        let mut uncovered = 1.0;
                        for layer in &layers {
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
                    }
                    HalftoneChannelModel::SourceColorAlpha => {
                        let mut pixel = background_pixel(RasterBackground::Transparent);
                        for layer in &layers {
                            source_over(&mut pixel, layer[index]);
                        }
                        pixel
                    }
                })
                .collect::<Vec<_>>();
            let actual = compose_model(model, &layers, &|| false)
                .expect("streaming modeled composition completes");
            let expected_surface = pixels_from_linear(64, 64, expected, &|| false)
                .expect("reference composition quantizes");
            let actual_surface = pixels_from_linear(64, 64, actual, &|| false)
                .expect("streaming composition quantizes");
            assert_eq!(actual_surface.pixels(), expected_surface.pixels());
        }
    }

    /// Proves checked raster allocation helpers retain valid sizes and report impossible dimensions safely.
    ///
    /// # Panics
    ///
    /// Panics when a small valid buffer cannot be allocated through the
    /// fallible path or an overflowing surface bypasses `raster.allocation`.
    #[test]
    fn raster_allocation_helpers_are_checked_and_fallible() {
        assert_eq!(raster_pixel_count(2, 3).expect("small pixel count"), 6);
        assert_eq!(raster_byte_count(2, 3).expect("small byte count"), 24);
        assert_eq!(
            allocate_raster_bytes(2, 3)
                .expect("small byte allocation")
                .len(),
            24
        );
        let error = RasterSurface::new(u32::MAX, u32::MAX, Vec::new())
            .expect_err("overflowing dimensions do not attempt allocation");
        assert_eq!(error.path(), "raster.allocation");
    }

    /// Proves cancellation can be isolated inside composition, background, and quantization workers.
    #[test]
    fn parallel_pixel_stages_cancel_before_publication() {
        let layers = modeled_linear_layers();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();

        let composition_observed = AtomicBool::new(false);
        let composition_error = pool
            .install(|| {
                compose_model(
                    HalftoneChannelModel::SourceColorAlpha,
                    &layers,
                    &worker_cancellation_probe(&composition_observed),
                )
            })
            .expect_err("composition cancellation publishes no pixel buffer");
        assert!(composition_observed.load(Ordering::Relaxed));
        assert_eq!(composition_error.path(), "evaluation.cancelled");

        let background_observed = AtomicBool::new(false);
        let mut background_pixels = layers[0].clone();
        let background_error = pool
            .install(|| {
                apply_background(
                    &mut background_pixels,
                    RasterBackground::OpaqueWhite,
                    &worker_cancellation_probe(&background_observed),
                )
            })
            .expect_err("background cancellation publishes no final buffer");
        assert!(background_observed.load(Ordering::Relaxed));
        assert_eq!(background_error.path(), "evaluation.cancelled");

        let quantization_observed = AtomicBool::new(false);
        let quantization_error = pool
            .install(|| {
                pixels_from_linear(
                    64,
                    64,
                    layers[0].clone(),
                    &worker_cancellation_probe(&quantization_observed),
                )
            })
            .expect_err("quantization cancellation publishes no surface");
        assert!(quantization_observed.load(Ordering::Relaxed));
        assert_eq!(quantization_error.path(), "evaluation.cancelled");
    }

    /// Accepts a tiny cubic at large finite coordinates through the control-polygon tolerance fast path.
    #[test]
    fn flatten_cubic_avoids_numeric_failure_for_tiny_large_coordinate_control_polygon() {
        let a = Point2::new(1.0e12, -1.0e12);
        let b = Point2::new(1.0e12 + 0.001, -1.0e12 + 0.001);
        let c = Point2::new(1.0e12 + 0.002, -1.0e12 + 0.001);
        let d = Point2::new(1.0e12 + 0.003, -1.0e12);
        let mut points = vec![a];
        let mut work = RasterWork::new(
            RasterizationLimits::default(),
            RasterAntialiasing::On,
            &|| false,
        );
        flatten_cubic(a, b, c, d, 0, &mut points, &mut work)
            .expect("tiny control polygon flattens without numeric subdivision");
        assert_eq!(points, vec![a, d]);
    }
}
