//! Current consumer entry preparation; fallback uses immutable resolved geometry, never artwork.
use super::*;
use std::borrow::Cow;
use toniator_geometry::{
    Bounds, CANVAS_MATERIALIZATION_CONTRACT, MaterializationCoverage, VariableWidthOutlineLimits,
    materialize_prepared_outline_cancellable,
};
/// Restores incompatible marks/outlines before a current consumer observes canonical geometry.
/// A compatible canvas certificate applies to all current resolutions; optical outside-canvas filters are not claimed.
/// # Errors
/// Returns canvas, preparation, cancellation or actual allocation diagnostics without partial output.
pub fn prepare_geometry_for_canvas<'a>(
    geometry: &'a GeometryOutput,
    canvas: &CanvasSpec,
    contract: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Cow<'a, GeometryOutput>, RenderError> {
    prepare_geometry_for_canvas_contracts(geometry, canvas, [contract, contract], cancelled)
}
/// Applies consumer-specific numeric requirements independently to whole marks and stroke contours.
/// # Errors
/// Returns checked preparation or cancellation diagnostics without partial output.
fn prepare_geometry_for_canvas_contracts<'a>(
    geometry: &'a GeometryOutput,
    canvas: &CanvasSpec,
    contracts: [&str; 2],
    cancelled: &dyn Fn() -> bool,
) -> Result<Cow<'a, GeometryOutput>, RenderError> {
    let [mark_contract, stroke_contract] = contracts;
    let bounds = Bounds::new(
        Point2::new(0., 0.),
        Point2::new(canvas.width, canvas.height),
    )
    .ok_or(RenderError::new(
        "materialization.canvas",
        "materialization canvas must be finite",
    ))?;
    let incompatible = geometry.parts().any(|part| match part {
        GeometryOutputPart::CanonicalMarks(marks) => marks.iter().any(
            |m| matches!(m,CanonicalMark::DeferredPath(v) if !v.coverage.supports(bounds,mark_contract)),
        ),
        GeometryOutputPart::CanonicalStrokes(strokes) => strokes
            .iter()
            .any(|v| !v.outline.coverage.supports(bounds, stroke_contract)),
        _ => false,
    });
    if !incompatible {
        return Ok(Cow::Borrowed(geometry));
    }
    let restored = match geometry {
        GeometryOutput::CanonicalMarks(marks) => {
            GeometryOutput::CanonicalMarks(copy_marks(marks, bounds, mark_contract, cancelled)?)
        }
        GeometryOutput::CanonicalStrokes(strokes) => GeometryOutput::CanonicalStrokes(
            copy_strokes(strokes, bounds, stroke_contract, cancelled)?,
        ),
        GeometryOutput::CanonicalConnections(value) => {
            GeometryOutput::CanonicalConnections(toniator_geometry::CanonicalConnections {
                strokes: copy_strokes(&value.strokes, bounds, stroke_contract, cancelled)?,
                isolated_points: copy_marks(
                    &value.isolated_points,
                    bounds,
                    mark_contract,
                    cancelled,
                )?,
            })
        }
        _ => return Ok(Cow::Borrowed(geometry)),
    };
    Ok(Cow::Owned(restored))
}
/// Reserves exact new fallback storage before copying any variable-size canonical payload.
/// # Errors
/// Returns cancellation, addressability or allocator refusal without publishing partial geometry.
fn copy_slice<T, U>(
    values: &[T],
    cancelled: &dyn Fn() -> bool,
    mut copy: impl FnMut(&T) -> Result<U, RenderError>,
) -> Result<Vec<U>, RenderError> {
    let mut result = Vec::new();
    result.try_reserve_exact(values.len()).map_err(|_| {
        RenderError::new(
            "materialization.allocation",
            "complete fallback storage allocation failed",
        )
    })?;
    for value in values {
        if cancelled() {
            return Err(RenderError::new(
                "evaluation.cancelled",
                "materialization was cancelled",
            ));
        }
        result.push(copy(value)?);
    }
    Ok(result)
}
/// Copies an already validated original path with checked storage and its unchanged closure.
/// # Errors
/// Returns allocation, original path validation or cancellation diagnostics atomically.
fn copy_path(
    path: &toniator_geometry::CurvePath,
    cancelled: &dyn Fn() -> bool,
) -> Result<toniator_geometry::CurvePath, RenderError> {
    path.try_clone_cancellable(cancelled).map_err(curve_error)
}
/// Maps a geometry preparation diagnostic without changing its stable authority path.
fn curve_error(error: toniator_geometry::CurveError) -> RenderError {
    RenderError::new(error.path(), error.message())
}
/// Restores incompatible decisions and checked-copies every other semantic/paint slot in order.
/// # Errors
/// Returns cancellation, allocation or original exact-transform diagnostics before publication.
fn copy_marks(
    marks: &[CanonicalMark],
    bounds: Bounds,
    contract: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalMark>, RenderError> {
    copy_marks_selected(
        marks,
        |value| !value.coverage.supports(bounds, contract),
        cancelled,
    )
}
/// Copies semantic mark slots while applying only the caller-selected deferred restoration.
/// # Errors
/// Returns checked copying, cancellation, or exact immutable transform diagnostics.
fn copy_marks_selected(
    marks: &[CanonicalMark],
    restore: impl Fn(&toniator_geometry::DeferredPathMark) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalMark>, RenderError> {
    copy_slice(marks, cancelled, |mark| {
        Ok(match mark {
            CanonicalMark::Circle {
                source_site_id,
                center,
                radius,
                scope,
                provenance,
                fill_rule,
            } => CanonicalMark::Circle {
                source_site_id: *source_site_id,
                center: *center,
                radius: *radius,
                scope: *scope,
                provenance: provenance
                    .try_clone_cancellable(cancelled)
                    .map_err(curve_error)?,
                fill_rule: *fill_rule,
            },
            CanonicalMark::ClosedPath(value) => {
                CanonicalMark::ClosedPath(toniator_geometry::CanonicalPathMark {
                    path: copy_path(&value.path, cancelled)?,
                    provenance: value
                        .provenance
                        .try_clone_cancellable(cancelled)
                        .map_err(curve_error)?,
                    source_site_id: value.source_site_id,
                    bounds: value.bounds,
                    scope: value.scope,
                    fill_rule: value.fill_rule,
                })
            }
            CanonicalMark::DeferredPath(value) if restore(value) => {
                CanonicalMark::ClosedPath(value.materialize(cancelled).map_err(curve_error)?)
            }
            CanonicalMark::DeferredPath(value) => CanonicalMark::DeferredPath(Arc::clone(value)),
        })
    })
}
/// Re-emits incompatible outlines and checked-copies the complete logical path/profile once.
/// # Errors
/// Rejects missing preparation and propagates cancellation or checked storage diagnostics.
fn copy_strokes(
    strokes: &[CanonicalStroke],
    bounds: Bounds,
    contract: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalStroke>, RenderError> {
    copy_strokes_selected(
        strokes,
        |outline| !outline.coverage.supports(bounds, contract),
        cancelled,
    )
}
/// Restores selected frozen outlines and fallibly copies all complete logical paths/profiles.
/// # Errors
/// Returns missing preparation, allocation or cancellation diagnostics atomically.
fn copy_strokes_selected(
    strokes: &[CanonicalStroke],
    restore: impl Fn(&toniator_geometry::CanonicalFilledOutline) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CanonicalStroke>, RenderError> {
    copy_slice(strokes, cancelled, |stroke| {
        let outline = if restore(&stroke.outline) {
            let prepared = stroke.outline.prepared.as_ref().ok_or(RenderError::new(
                "materialization.preparation",
                "restricted outline lacks complete preparation",
            ))?;
            toniator_geometry::ConstructionContext::current().count(
                toniator_geometry::ConstructionCounter::FullRematerializations,
                1,
            );
            materialize_prepared_outline_cancellable(
                Arc::clone(prepared),
                MaterializationCoverage::Complete,
                VariableWidthOutlineLimits::new(usize::MAX).map_err(curve_error)?,
                cancelled,
            )
            .map_err(curve_error)?
        } else {
            toniator_geometry::CanonicalFilledOutline {
                full_segment_count: stroke.outline.full_segment_count,
                prepared: stroke.outline.prepared.as_ref().map(Arc::clone),
                coverage: stroke.outline.coverage,
                contours: copy_slice(&stroke.outline.contours, cancelled, |contour| {
                    Ok(toniator_geometry::CanonicalOutlineContour {
                        segments: copy_slice(&contour.segments, cancelled, |value| Ok(*value))?,
                    })
                })?,
                fill_rule: stroke.outline.fill_rule,
                bounds: stroke.outline.bounds,
            }
        };
        Ok(CanonicalStroke {
            source_id: stroke.source_id.clone(),
            source_structure_id: stroke.source_structure_id,
            path: copy_path(&stroke.path, cancelled)?,
            nominal_basis: stroke.nominal_basis,
            style: stroke.style,
            profile: copy_slice(&stroke.profile, cancelled, |value| Ok(*value))?,
            outline,
        })
    })
}
/// Copies only layer vectors; immutable geometry/paint handles remain shared until replaced.
/// # Errors
/// Returns cancellation or checked layer/output vector allocation failure atomically.
fn copy_layers(
    layers: &[RenderLayer],
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RenderLayer>, RenderError> {
    copy_slice(layers, cancelled, |layer| {
        Ok(RenderLayer {
            channel_id: layer.channel_id,
            visible: layer.visible,
            color: layer.color.clone(),
            opacity: layer.opacity,
            outputs: copy_slice(&layer.outputs, cancelled, |output| Ok(output.clone()))?,
        })
    })
}
/// Shares compatible scenes and rebuilds identity only when actual complete fallback replaces geometry.
/// # Errors
/// Propagates preparation, canonical scene or cancellation diagnostics before rendering starts.
pub(super) fn prepare_scene<'a>(
    scene: &'a RenderScene,
    cancelled: &dyn Fn() -> bool,
) -> Result<Cow<'a, RenderScene>, RenderError> {
    prepare_scene_with_contract(scene, CANVAS_MATERIALIZATION_CONTRACT, cancelled)
}
/// Restores full frozen geometry for a current consumer whose numeric contract needs complete contours.
/// # Errors
/// Returns checked reconstruction, original canonical validation or cancellation diagnostics atomically.
pub(super) fn prepare_scene_with_contract<'a>(
    scene: &'a RenderScene,
    contract: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Cow<'a, RenderScene>, RenderError> {
    let mut layers = None;
    for (li, layer) in scene.layers.iter().enumerate() {
        for (oi, output) in layer.outputs.iter().enumerate() {
            let geometry = prepare_geometry_for_canvas_contracts(
                output.geometry(),
                &scene.canvas,
                [CANVAS_MATERIALIZATION_CONTRACT, contract],
                cancelled,
            )?;
            if let Cow::Owned(geometry) = geometry {
                if layers.is_none() {
                    layers = Some(copy_layers(&scene.layers, cancelled)?);
                }
                layers.as_mut().expect("fallback layers exist")[li].outputs[oi].geometry =
                    Arc::new(geometry);
            }
        }
    }
    match layers {
        None => Ok(Cow::Borrowed(scene)),
        Some(layers) => Ok(Cow::Owned(RenderScene::build(
            scene.canvas.clone(),
            scene.identity.family_fingerprint.clone(),
            scene.identity.realization_fingerprint.clone(),
            scene.model,
            layers,
        )?)),
    }
}

/// Finds a canvas-restricted stroke that retains immutable complete procedural preparation.
fn has_restricted_prepared_stroke(geometry: &GeometryOutput) -> bool {
    geometry.parts().any(|part| match part {
        GeometryOutputPart::CanonicalStrokes(strokes) => strokes.iter().any(|stroke| {
            matches!(
                stroke.outline.coverage,
                MaterializationCoverage::Canvas { .. }
            ) && stroke.outline.prepared.is_some()
        }),
        _ => false,
    })
}

/// Restores only restricted prepared strokes after a renderer's typed certificate rejection.
/// This complete-only service needs no target-derived canvas. Mark-only geometry and paints keep
/// their Arc handles; mixed circle/deferred slots preserve their semantic indices and decisions.
/// # Errors
/// Returns checked allocation/copying, preparation or cancellation errors without a partial layer.
pub(super) fn complete_restricted_stroke_layer(
    layer: &RenderLayer,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<RenderLayer>, RenderError> {
    if !layer
        .outputs
        .iter()
        .any(|output| has_restricted_prepared_stroke(output.geometry()))
    {
        return Ok(None);
    }
    let outputs = copy_slice(&layer.outputs, cancelled, |output| {
        if !has_restricted_prepared_stroke(output.geometry()) {
            return Ok(output.clone());
        }
        let restore = |outline: &toniator_geometry::CanonicalFilledOutline| {
            matches!(outline.coverage, MaterializationCoverage::Canvas { .. })
                && outline.prepared.is_some()
        };
        let geometry = match output.geometry() {
            GeometryOutput::CanonicalStrokes(strokes) => GeometryOutput::CanonicalStrokes(
                copy_strokes_selected(strokes, restore, cancelled)?,
            ),
            GeometryOutput::CanonicalConnections(value) => {
                GeometryOutput::CanonicalConnections(toniator_geometry::CanonicalConnections {
                    strokes: copy_strokes_selected(&value.strokes, restore, cancelled)?,
                    isolated_points: copy_marks_selected(
                        &value.isolated_points,
                        |_| false,
                        cancelled,
                    )?,
                })
            }
            _ => unreachable!("restricted strokes belong to stroke or mixed connection outputs"),
        };
        Ok(RenderOutputLayer::from_shared(
            output.output_layer_id,
            Arc::new(geometry),
            output.primitive_paints.as_ref().map(Arc::clone),
        ))
    })?;
    Ok(Some(RenderLayer {
        channel_id: layer.channel_id,
        visible: layer.visible,
        color: layer.color.clone(),
        opacity: layer.opacity,
        outputs,
    }))
}
