//! Fixed canvas-grid preview roots. Prepared geometry is not certified output until a complete
//! sampled-region query succeeds. Tile bytes remain encapsulated behind root/generation checks.
use super::*;
use area_raster::{AreaDebtLedger, PixelRegion, PreparedAreaLayer};
use std::borrow::Cow;

/// Half-open integer root pixels, including every pixel fetched by presentation filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Region {
    /// Validates a nonempty allocated crop against the virtual root without changing grid phase.
    /// # Errors
    /// Rejects overflow, empty or outside regions before allocation.
    fn checked(self, frame: Frame) -> Result<PixelRegion, RenderError> {
        let x1 = self
            .x
            .checked_add(self.width)
            .ok_or(error("region overflows"))?;
        let y1 = self
            .y
            .checked_add(self.height)
            .ok_or(error("region overflows"))?;
        if self.width == 0 || self.height == 0 {
            return Err(error("region is empty"));
        }
        PixelRegion::new(self.x, self.y, x1, y1, frame.width, frame.height)
    }
    /// Tests containment using widened endpoints; a crop never authorizes a larger view.
    fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && u64::from(self.x) + u64::from(self.width)
                >= u64::from(other.x) + u64::from(other.width)
            && u64::from(self.y) + u64::from(self.height)
                >= u64::from(other.y) + u64::from(other.height)
    }
}

/// Immutable physical canvas grid; virtual dimensions do not allocate a full-root surface.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub background: RasterBackground,
}

/// Actual retained capacity/work facts, excluding caller-owned scene and allocator overhead.
#[derive(Clone, Copy, Debug)]
pub struct RootFacts {
    pub prepared_capacity_bytes: usize,
    pub debt_capacity_bytes: usize,
    pub debt_events: usize,
    pub consumed_edges: usize,
    pub consumed_boolean_operations: usize,
    pub generation: u64,
}

struct Layer {
    prepared: Arc<PreparedAreaLayer>,
    ledger: AreaDebtLedger,
    retry_used: bool,
}

/// Worker-owned retained root. A later typed restoration invalidates all previous pixel generations.
/// Global debt remains global; only normalization receives the existing zero-visible-fill waiver.
pub struct PreviewRoot {
    scene: Arc<RenderScene>,
    frame: Frame,
    clip: [f64; 4],
    transform: CanonicalRasterTransform,
    layers: Vec<Layer>,
    identity: Arc<()>,
    generation: u64,
    limits: RasterizationLimits,
    remaining: RasterizationLimits,
}

/// Permission for one complete sampled region at one root generation; not a per-tile budget.
pub struct RegionCertificate {
    identity: Arc<()>,
    generation: u64,
    region: Region,
    bounds: Vec<f64>,
}
impl RegionCertificate {
    /// Tests whole-region containment without aggregating certificates from separate tiles.
    pub fn covers(&self, region: Region) -> bool {
        self.region.contains(region)
    }
    /// Returns the exact ordered-replay bounds for every layer, including empty paints.
    pub fn layer_bounds(&self) -> &[f64] {
        &self.bounds
    }
    /// Returns the complete sampled region certified by this query.
    pub fn region(&self) -> Region {
        self.region
    }
    /// Returns the pixel namespace invalidated by any prepared-geometry replacement.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Immutable painting snapshot. No method here performs Boolean work or grants display permission.
#[derive(Clone)]
pub struct RootSnapshot {
    frame: Frame,
    model: Option<HalftoneChannelModel>,
    layers: Vec<Arc<PreparedAreaLayer>>,
    identity: Arc<()>,
    generation: u64,
}

/// Encapsulated unchanged straight-sRGBA crop. Access requires a current complete-view certificate.
pub struct RawTilePixels {
    surface: RasterSurface,
    region: Region,
    identity: Arc<()>,
    generation: u64,
}
impl RawTilePixels {
    /// Transfers unchanged pixels into private upload storage after root-generation validation.
    /// This grants storage/upload validity only; display still requires a current whole-view certificate.
    /// # Errors
    /// Rejects replaced roots/generations without exposing stale pixel bytes.
    pub fn into_upload(self, root: &PreviewRoot) -> Result<RasterSurface, RenderError> {
        if !Arc::ptr_eq(&self.identity, &root.identity) || self.generation != root.generation {
            return Err(error("stale upload pixels"));
        }
        Ok(self.surface)
    }
    /// Exposes bytes only while the current root and complete-region certificate authorize this crop.
    /// # Errors
    /// Rejects stale/wrong-root generations and crops outside the certified whole sampled region.
    pub fn surface<'a>(
        &'a self,
        root: &PreviewRoot,
        certificate: &RegionCertificate,
    ) -> Result<&'a RasterSurface, RenderError> {
        root.validate_certificate(certificate)?;
        if !Arc::ptr_eq(&self.identity, &root.identity)
            || self.generation != root.generation
            || !certificate.region.contains(self.region)
        {
            return Err(error(
                "tile is outside the current certified pixel namespace",
            ));
        }
        Ok(&self.surface)
    }
}

/// Returns the stable headless preview-root admission/certification diagnostic.
fn error(message: &'static str) -> RenderError {
    RenderError::new("preview.root", message)
}

impl PreviewRoot {
    /// Prepares complete canvas geometry once without allocating full-root pixels or publishing it.
    /// Ordered cumulative debt is checked later per whole sampled region; local errors can therefore
    /// precede a cumulative debt rejection on this opt-in route. Ordinary render paths stay eager.
    /// # Errors
    /// Returns frame, materialization, allocation, local precision, cancellation or consumed-work errors.
    pub fn new(
        scene: Arc<RenderScene>,
        scale: f64,
        background: RasterBackground,
        limits: RasterizationLimits,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, RenderError> {
        if !scale.is_finite() || scale <= 0. {
            return Err(error("scale must be finite and positive"));
        }
        let clip = [
            0.,
            0.,
            scene.canvas.width * scale,
            scene.canvas.height * scale,
        ];
        if clip[2..]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || v.ceil() > f64::from(u32::MAX))
        {
            return Err(error("virtual extent is not representable"));
        }
        let frame = Frame {
            width: clip[2].ceil() as u32,
            height: clip[3].ceil() as u32,
            scale,
            background,
        };
        usize::try_from(u64::from(frame.width) * u64::from(frame.height))
            .map_err(|_| error("virtual pixel indices overflow"))?;
        let transform = CanonicalRasterTransform {
            scale_x: scale,
            scale_y: scale,
            offset_x: 0.,
            offset_y: 0.,
        };
        transform.validate()?;
        let scene = match materialization::prepare_scene(&scene, cancelled)? {
            Cow::Borrowed(_) => scene.clone(),
            Cow::Owned(value) => Arc::new(value),
        };
        let mut work = RasterWork::with_progress(
            limits,
            RasterAntialiasing::On,
            cancelled,
            0,
            &ignore_raster_progress,
        );
        work.check()?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(scene.layers.len())
            .map_err(|_| error("layer allocation failed"))?;
        for layer in &scene.layers {
            work.check()?;
            let (prepared, ledger) = area_raster::prepare_retained_area_layer(
                layer,
                frame.width,
                frame.height,
                transform,
                clip,
                &mut work,
            )?;
            layers.push(Layer {
                prepared: Arc::new(prepared),
                ledger,
                retry_used: false,
            });
        }
        let remaining = RasterizationLimits {
            max_flattened_edges: work.remaining_edges,
            max_boolean_operations: work.remaining_boolean_operations,
        };
        Ok(Self {
            scene,
            frame,
            clip,
            transform,
            layers,
            identity: Arc::new(()),
            generation: 0,
            limits,
            remaining,
        })
    }
    /// Shares the actually retained canonical scene for derived-storage accounting without cloning it.
    /// A materialized owned scene is distinct from the accepted upstream scene and must be charged.
    pub fn shared_scene(&self) -> Arc<RenderScene> {
        self.scene.clone()
    }
    /// Returns the current pixel namespace without scanning retained geometry.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Replays a whole region without restoration or mutation, for bounded resident-hit planning.
    /// Failed queries grant no permission; the owning worker may call `certify` for the typed retry.
    /// # Errors
    /// Returns region, cancellation or original cumulative debt diagnostics, never performing prep.
    pub fn certify_without_retry(
        &self,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RegionCertificate, RenderError> {
        let pixel = region.checked(self.frame)?;
        let visible = [
            f64::from(pixel.x0),
            f64::from(pixel.y0),
            f64::from(pixel.x1),
            f64::from(pixel.y1),
        ];
        let work = RasterWork::with_progress(
            self.remaining,
            RasterAntialiasing::On,
            cancelled,
            0,
            &ignore_raster_progress,
        );
        let mut bounds = Vec::new();
        bounds
            .try_reserve_exact(self.layers.len())
            .map_err(|_| error("certificate allocation failed"))?;
        for layer in &self.layers {
            bounds.push(layer.ledger.certify(visible, &work).map_err(|f| f.error)?);
        }
        Ok(RegionCertificate {
            identity: self.identity.clone(),
            generation: self.generation,
            region,
            bounds,
        })
    }

    /// Returns the fixed root frame independently of any viewport or allocated tile.
    pub fn frame(&self) -> Frame {
        self.frame
    }
    /// Encloses all root pixels fetched by bilinear fractional translation; clips to root coverage.
    /// Coordinates are physical root pixels, not canonical positions. Outside holes use viewer backing.
    /// # Errors
    /// Rejects nonfinite/reversed/fully outside views; uncertainty never narrows certification.
    pub fn sampled_region(
        &self,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    ) -> Result<Region, RenderError> {
        let right = x + f64::from(width);
        let bottom = y + f64::from(height);
        if ![x, y, right, bottom].iter().all(|v| v.is_finite()) || width == 0 || height == 0 {
            return Err(error("invalid sampled view"));
        }
        let x0 = x.floor().max(0.).min(f64::from(self.frame.width)) as u32;
        let y0 = y.floor().max(0.).min(f64::from(self.frame.height)) as u32;
        let x1 = right.ceil().max(0.).min(f64::from(self.frame.width)) as u32;
        let y1 = bottom.ceil().max(0.).min(f64::from(self.frame.height)) as u32;
        let region = Region {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        region.checked(self.frame)?;
        Ok(region)
    }
    /// Certifies one entire sampled view, replaying every layer's debt exactly once in original order.
    /// At most one remaining typed complete-stroke restoration per layer shares consumed root work;
    /// any replacement increments generation even if its subsequent region certification fails.
    /// # Errors
    /// Returns original debt/local/resource/cancellation errors. Failed work is never refunded.
    pub fn certify(
        &mut self,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RegionCertificate, RenderError> {
        let pixel = region.checked(self.frame)?;
        let visible = [
            f64::from(pixel.x0),
            f64::from(pixel.y0),
            f64::from(pixel.x1),
            f64::from(pixel.y1),
        ];
        let mut work = RasterWork::with_progress(
            self.remaining,
            RasterAntialiasing::On,
            cancelled,
            0,
            &ignore_raster_progress,
        );
        let result = (|| {
            work.check()?;
            let mut bounds = Vec::new();
            bounds
                .try_reserve_exact(self.layers.len())
                .map_err(|_| error("certificate allocation failed"))?;
            for (index, layer) in self.layers.iter_mut().enumerate() {
                let bound = match layer.ledger.certify(visible, &work) {
                    Ok(value) => value,
                    Err(failure) if failure.split_budget && !layer.retry_used => {
                        work.check()?;
                        let Some(restored) = materialization::complete_restricted_stroke_layer(
                            &self.scene.layers[index],
                            cancelled,
                        )?
                        else {
                            return Err(failure.error);
                        };
                        // Cancellation before an actual preparation attempt does not consume
                        // the root-lifetime retry. Restoration has not published any geometry.
                        work.check()?;
                        layer.retry_used = true;
                        let (prepared, ledger) = area_raster::prepare_retained_area_layer(
                            &restored,
                            self.frame.width,
                            self.frame.height,
                            self.transform,
                            self.clip,
                            &mut work,
                        )?;
                        let generation = self
                            .generation
                            .checked_add(1)
                            .ok_or(error("root generation overflows"))?;
                        layer.prepared = Arc::new(prepared);
                        layer.ledger = ledger;
                        self.generation = generation;
                        layer.ledger.certify(visible, &work).map_err(|f| f.error)?
                    }
                    Err(failure) => return Err(failure.error),
                };
                bounds.push(bound);
            }
            work.check()?;
            Ok(RegionCertificate {
                identity: self.identity.clone(),
                generation: self.generation,
                region,
                bounds,
            })
        })();
        self.remaining = RasterizationLimits {
            max_flattened_edges: work.remaining_edges,
            max_boolean_operations: work.remaining_boolean_operations,
        };
        result
    }
    /// Checks that complete-view permission belongs to the current immutable pixel namespace.
    /// # Errors
    /// Rejects certificates from another root or a prior geometry generation.
    fn validate_certificate(&self, certificate: &RegionCertificate) -> Result<(), RenderError> {
        if !Arc::ptr_eq(&self.identity, &certificate.identity)
            || self.generation != certificate.generation
        {
            return Err(error("stale region certificate"));
        }
        Ok(())
    }
    /// Shares prepared geometry for worker painting after complete-view certification.
    /// # Errors
    /// Rejects stale permission or fallible snapshot vector allocation.
    pub fn snapshot(&self, certificate: &RegionCertificate) -> Result<RootSnapshot, RenderError> {
        self.validate_certificate(certificate)?;
        self.storage_snapshot()
    }
    /// Shares immutable prepared geometry for private background pixel storage, not presentation.
    /// Every displayed fetched footprint still requires one current whole-region certificate.
    /// # Errors
    /// Returns fallible snapshot metadata allocation errors; geometry and debt remain unchanged.
    pub fn storage_snapshot(&self) -> Result<RootSnapshot, RenderError> {
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(self.layers.len())
            .map_err(|_| error("snapshot allocation failed"))?;
        layers.extend(self.layers.iter().map(|l| l.prepared.clone()));
        Ok(RootSnapshot {
            frame: self.frame,
            model: self.scene.model,
            layers,
            identity: self.identity.clone(),
            generation: self.generation,
        })
    }
    /// Measures actual vector capacities and cumulative work; excludes scene/allocator/GPU payload.
    /// # Errors
    /// Rejects checked capacity arithmetic overflow rather than saturating observations.
    pub fn facts(&self) -> Result<RootFacts, RenderError> {
        let mut bytes = 0usize;
        let mut debts = 0usize;
        let mut events = 0usize;
        for layer in &self.layers {
            bytes = bytes
                .checked_add(layer.prepared.pilot_storage()?.0)
                .ok_or(error("capacity overflows"))?;
            debts = debts
                .checked_add(
                    layer
                        .ledger
                        .events
                        .capacity()
                        .checked_mul(std::mem::size_of::<area_raster::AreaDebt>())
                        .ok_or(error("capacity overflows"))?,
                )
                .ok_or(error("capacity overflows"))?;
            events = events
                .checked_add(layer.ledger.events.len())
                .ok_or(error("event count overflows"))?;
        }
        Ok(RootFacts {
            prepared_capacity_bytes: bytes,
            debt_capacity_bytes: debts,
            debt_events: events,
            consumed_edges: self.limits.max_flattened_edges - self.remaining.max_flattened_edges,
            consumed_boolean_operations: self.limits.max_boolean_operations
                - self.remaining.max_boolean_operations,
            generation: self.generation,
        })
    }
}
impl RootSnapshot {
    /// Returns the immutable generation carried by privately stored background pixels.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Estimates this crop's pixel buffers plus the unchanged worst-layer row/global-X scratch.
    /// Current-toolchain row-vector growth is estimated, not a portable allocator/RSS guarantee.
    /// # Errors
    /// Returns cancellation, region or checked-payload overflow before any painting allocation.
    pub fn estimated_paint_payload(
        &self,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<usize, RenderError> {
        let pixel = region.checked(self.frame)?;
        let work = RasterWork::with_progress(
            RasterizationLimits {
                max_flattened_edges: 0,
                max_boolean_operations: 0,
            },
            RasterAntialiasing::On,
            cancelled,
            0,
            &ignore_raster_progress,
        );
        let mut scratch = 0;
        for layer in &self.layers {
            scratch = scratch.max(layer.pilot_scratch(
                self.frame.width,
                self.frame.height,
                pixel,
                &work,
            )?);
        }
        (region.width as usize)
            .checked_mul(region.height as usize)
            .and_then(|n| n.checked_mul(72))
            .and_then(|n| n.checked_add(scratch))
            .ok_or(error("paint payload overflows"))
    }
    /// Paints an authorized crop using unchanged global-X integration and zero preparation budgets.
    /// No returned bytes can be read without a current root/whole-view certificate check.
    /// # Errors
    /// Rejects stale/outside permission, target allocation, cancellation or unchanged paint diagnostics.
    pub fn paint(
        &self,
        region: Region,
        certificate: &RegionCertificate,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawTilePixels, RenderError> {
        if !Arc::ptr_eq(&self.identity, &certificate.identity)
            || self.generation != certificate.generation
            || !certificate.region.contains(region)
        {
            return Err(error("crop is not covered by region certificate"));
        }
        self.paint_for_storage(region, cancelled)
    }
    /// Paints unchanged fixed-root pixels into private storage without granting display permission.
    /// Root/generation validation remains required at storage acceptance and whole-view certification
    /// remains required at presentation. No Boolean work, debt reset or new accuracy allowance occurs.
    /// # Errors
    /// Returns bounds, cancellation, allocation and original integration errors atomically.
    pub fn paint_for_storage(
        &self,
        region: Region,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<RawTilePixels, RenderError> {
        let pixel = region.checked(self.frame)?;
        let mut work = RasterWork::with_progress(
            RasterizationLimits {
                max_flattened_edges: 0,
                max_boolean_operations: 0,
            },
            RasterAntialiasing::On,
            cancelled,
            0,
            &ignore_raster_progress,
        );
        work.check()?;
        let mut pixels = match self.model {
            Some(m) => begin_model_composition(m, region.width, region.height)?,
            None => begin_source_over_composition(region.width, region.height)?,
        };
        for layer in &self.layers {
            work.check()?;
            let mut transient = allocate_linear_pixels(
                region.width,
                region.height,
                background_pixel(RasterBackground::Transparent),
            )?;
            layer.paint_region(
                &mut transient,
                self.frame.width,
                self.frame.height,
                pixel,
                &mut work,
            )?;
            match self.model {
                Some(m) => compose_model_layer(m, &mut pixels, &transient, cancelled)?,
                None => compose_source_over_layer(&mut pixels, &transient, cancelled)?,
            };
        }
        if let Some(m) = self.model {
            finish_model_composition(m, &mut pixels, cancelled)?;
        }
        apply_background(&mut pixels, self.frame.background, cancelled)?;
        let surface = pixels_from_linear(region.width, region.height, pixels, cancelled)?;
        Ok(RawTilePixels {
            surface,
            region,
            identity: self.identity.clone(),
            generation: self.generation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Exercises cancellation/resume and actual restoration after an ordered synthetic debt rejection.
    /// # Panics
    /// Panics on stale tile admission, repeated restoration, work refund or consumed pre-attempt eligibility.
    #[test]
    fn later_typed_retry_invalidates_old_namespace_and_is_not_replenished() {
        let mut layer = area_raster::restricted_retry_tests::layer();
        layer.outputs[0].primitive_paints = None;
        let scene = Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: 32.,
                    height: 24.,
                },
                "retry".into(),
                "restricted".into(),
                vec![layer],
            )
            .unwrap(),
        );
        let mut root = PreviewRoot::new(
            scene,
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| false,
        )
        .unwrap();
        let region = Region {
            x: 0,
            y: 0,
            width: 32,
            height: 24,
        };
        let certificate = root.certify(region, &|| false).unwrap();
        let old_snapshot = root.snapshot(&certificate).unwrap();
        let tile = old_snapshot.paint(region, &certificate, &|| false).unwrap();
        let before = root.facts().unwrap();
        root.layers[0]
            .ledger
            .events
            .push(area_raster::AreaDebt::Split(2e-6));
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let cancellation = root
            .certify(region, &|| {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 2
            })
            .err()
            .unwrap();
        assert_eq!(cancellation.path(), "evaluation.cancelled");
        assert!(!root.layers[0].retry_used);
        assert_eq!(root.generation, certificate.generation());
        let replacement = root.certify(region, &|| false).unwrap();
        assert_eq!(replacement.generation(), certificate.generation() + 1);
        assert!(tile.surface(&root, &certificate).is_err());
        assert!(tile.surface(&root, &replacement).is_err());
        assert!(root.facts().unwrap().consumed_edges > before.consumed_edges);
        assert!(
            root.facts().unwrap().consumed_boolean_operations > before.consumed_boolean_operations
        );
        root.layers[0]
            .ledger
            .events
            .push(area_raster::AreaDebt::Split(2e-6));
        let before = root.facts().unwrap();
        assert!(root.certify(region, &|| false).is_err());
        assert_eq!(root.generation, before.generation);
        assert_eq!(root.facts().unwrap().consumed_edges, before.consumed_edges);
        assert_eq!(
            root.facts().unwrap().consumed_boolean_operations,
            before.consumed_boolean_operations
        );
    }
    /// Rejects speculative wider debt without consuming actual-view restoration or numerical work.
    /// # Panics
    /// Panics if wider refusal/cancellation changes generation, retry eligibility or work authority.
    #[test]
    fn speculative_envelope_refusal_preserves_required_region_authority() {
        let mut layer = area_raster::restricted_retry_tests::layer();
        layer.outputs[0].primitive_paints = None;
        let scene = Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: 32.,
                    height: 24.,
                },
                "envelope".into(),
                "envelope".into(),
                vec![layer],
            )
            .unwrap(),
        );
        let mut root = PreviewRoot::new(
            scene,
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| false,
        )
        .unwrap();
        root.layers[0]
            .ledger
            .events
            .push(area_raster::AreaDebt::Normalization {
                bound: 2e-6,
                boxes: Some([[12., 12., 20., 20.]; 2]),
            });
        let required = Region {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        let envelope = Region {
            x: 0,
            y: 0,
            width: 24,
            height: 24,
        };
        let before = root.facts().unwrap();
        assert!(root.certify_without_retry(envelope, &|| false).is_err());
        assert!(root.certify_without_retry(required, &|| false).is_ok());
        assert_eq!(
            root.certify_without_retry(envelope, &|| true)
                .err()
                .unwrap()
                .path(),
            "evaluation.cancelled"
        );
        assert!(root.certify_without_retry(required, &|| false).is_ok());
        let after = root.facts().unwrap();
        assert_eq!(
            (
                before.generation,
                before.consumed_edges,
                before.consumed_boolean_operations
            ),
            (
                after.generation,
                after.consumed_edges,
                after.consumed_boolean_operations
            )
        );
        assert!(!root.layers[0].retry_used);
    }

    /// Stores full-root pixels without treating them as numerical permission, then rejects global debt.
    /// # Panics
    /// Panics if private storage changes work/debt or can bypass the ordinary whole-view budget.
    #[test]
    fn background_storage_does_not_grant_whole_root_permission() {
        let mut layer = area_raster::restricted_retry_tests::layer();
        layer.outputs[0].primitive_paints = None;
        let scene = Arc::new(
            RenderScene::new(
                CanvasSpec {
                    width: 32.,
                    height: 24.,
                },
                "storage".into(),
                "storage".into(),
                vec![layer],
            )
            .unwrap(),
        );
        let mut root = PreviewRoot::new(
            scene,
            1.,
            RasterBackground::Transparent,
            RasterizationLimits::default(),
            &|| false,
        )
        .unwrap();
        let region = Region {
            x: 0,
            y: 0,
            width: 32,
            height: 24,
        };
        root.layers[0]
            .ledger
            .events
            .push(area_raster::AreaDebt::Split(2e-6));
        let before = root.facts().unwrap();
        let snapshot = root.storage_snapshot().unwrap();
        let pixels = snapshot.paint_for_storage(region, &|| false).unwrap();
        assert!(root.certify_without_retry(region, &|| false).is_err());
        assert!(pixels.into_upload(&root).is_ok());
        let after = root.facts().unwrap();
        assert_eq!(before.consumed_edges, after.consumed_edges);
        assert_eq!(
            before.consumed_boolean_operations,
            after.consumed_boolean_operations
        );
        assert_eq!(before.debt_events, after.debt_events);
    }
}
