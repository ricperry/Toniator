use std::collections::BTreeSet;
use toniator_domain::{AuthoredStructureDraft, AuthoredStructureId, AuthoredStructureKind};
use toniator_geometry::{CurveError, CurvePath, CurveSegment, PathClosure, PathLocation, Point2};

/// Selects the geometry policy projected by one reusable authored-path editor.
///
/// This is app-local interaction policy only. It never adds a field to an authored resource: the
/// canonical `CurvePath` remains the only persisted geometry authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthoredPathEditorMode {
    /// Authors an open guide against the target workspace's aspect-preserving frame.
    Guide,
    /// Authors a canonical closed shape without a special endpoint rule.
    Shape,
    /// Authors one tile-local Curve Motif with immutable terminal anchors.
    Motif,
}

/// Stores the finite document-space guide frame and its uniform screen projection.
///
/// The frame is a visual authoring aid only. Coordinates outside it remain valid path geometry and
/// no clipping or generated-geometry termination is implied by this presentation transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GuideAuthoringFrame {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) canvas_width: f64,
    pub(crate) canvas_height: f64,
    pub(crate) padding: f64,
}

impl GuideAuthoringFrame {
    /// Creates a finite workspace-sized guide frame before a canvas allocation is known.
    pub(crate) fn new(width: f64, height: f64) -> Option<Self> {
        (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0).then_some(Self {
            width,
            height,
            canvas_width: width,
            canvas_height: height,
            padding: 0.0,
        })
    }

    /// Returns the frame's one scale factor, never independently stretching X and Y.
    pub(crate) fn uniform_scale(self) -> f64 {
        let usable_width = (self.canvas_width - self.padding * 2.0).max(1.0);
        let usable_height = (self.canvas_height - self.padding * 2.0).max(1.0);
        (usable_width / self.width).min(usable_height / self.height)
    }

    /// Returns the screen-space offset that centers the uniformly scaled frame in its canvas.
    pub(crate) fn offset(self) -> Point2 {
        let scale = self.uniform_scale();
        Point2::new(
            (self.canvas_width - self.width * scale) * 0.5,
            (self.canvas_height - self.height * scale) * 0.5,
        )
    }

    /// Maps one document-space point to the guide frame's screen presentation without clipping.
    pub(crate) fn to_canvas(self, point: Point2) -> Point2 {
        let scale = self.uniform_scale();
        let offset = self.offset();
        Point2::new(offset.x + point.x * scale, offset.y + point.y * scale)
    }

    /// Maps one canvas-space point through the guide frame's exact uniform inverse.
    pub(crate) fn canvas_to_document(self, point: Point2) -> Option<Point2> {
        if !point.is_finite() {
            return None;
        }
        let scale = self.uniform_scale();
        (scale.is_finite() && scale > 0.0).then(|| {
            let offset = self.offset();
            Point2::new((point.x - offset.x) / scale, (point.y - offset.y) / scale)
        })
    }

    /// Clamps a centered-local guide endpoint away from frame corners using the visible hit radius.
    pub(crate) fn clamp_endpoint_y(self, y: f64, visible_hit_radius: f64, zoom: f64) -> f64 {
        let scale = self.uniform_scale() * zoom;
        let inset = if scale.is_finite() && scale > 0.0 {
            visible_hit_radius / scale
        } else {
            0.0
        };
        let half_height = self.height * 0.5;
        y.clamp(
            (-half_height + inset).min(0.0),
            (half_height - inset).max(0.0),
        )
    }
}

/// Represents the non-persisted terminal-handle presentation inferred for a Curve Motif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotifTerminalDirection {
    /// Terminal handle directions are linked across the tile seam.
    Smooth,
    /// Terminal handle directions are intentionally independent.
    Corner,
}

/// Identifies one fixed Curve Motif terminal control handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotifTerminalHandle {
    /// Selects the first segment's outgoing cubic control.
    Left,
    /// Selects the final segment's incoming cubic control.
    Right,
}

/// Widget-independent construction state for the Stage 20F private Pattern Editor.
///
/// This state owns only selection, viewport, and incomplete input. A completed path becomes an
/// ID-free draft consumed by the app's existing typed document-history command boundary.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Selection fields are consumed by the subsequent GTK projection.
pub(crate) struct Stage20fEditorState {
    mode: Option<AuthoredPathEditorMode>,
    guide_frame: Option<GuideAuthoringFrame>,
    motif_terminal_direction: Option<MotifTerminalDirection>,
    pub(crate) selected_structure: Option<AuthoredStructureId>,
    pub(crate) selected_node: Option<usize>,
    pub(crate) selected_segment: Option<usize>,
    pub(crate) selected_segment_parameter: Option<f64>,
    pub(crate) selected_target: Option<NumericTarget>,
    shared_armed: BTreeSet<AuthoredStructureId>,
    pub(crate) pan: Point2,
    pub(crate) zoom: f64,
    construction: Option<Construction>,
    drag: Option<(CurvePath, NumericTarget)>,
    local_preview: Option<CurvePath>,
    drag_screen_origin: Option<Point2>,
    pan_drag: Option<(Point2, Point2)>,
    pub(crate) local_invalid: bool,
}

impl Default for Stage20fEditorState {
    /// Creates a neutral unit-zoom viewport with no selected or incomplete construction state.
    fn default() -> Self {
        Self {
            mode: None,
            guide_frame: None,
            motif_terminal_direction: None,
            selected_structure: None,
            selected_node: None,
            selected_segment: None,
            selected_segment_parameter: None,
            selected_target: None,
            shared_armed: BTreeSet::new(),
            pan: Point2::new(0.0, 0.0),
            zoom: 1.0,
            construction: None,
            drag: None,
            local_preview: None,
            drag_screen_origin: None,
            pan_drag: None,
            local_invalid: false,
        }
    }
}

/// One locally incomplete open-guide or closed-mark construction sequence.
#[derive(Clone, Debug)]
struct Construction {
    kind: AuthoredStructureKind,
    points: Vec<Point2>,
}
/// One editable construction point addressed without GTK widget identity.
#[allow(dead_code)] // GTK numeric controls consume every target variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericTarget {
    Anchor(usize),
    Control1(usize),
    Control2(usize),
}

/// One explicit edit applied to the currently selected segment or anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PathEdit {
    /// Converts a selected line segment to its explicit cubic representation.
    MakeCurve,
    /// Converts a selected cubic segment to its direct line chord.
    MakeLine,
    /// Splits a selected segment at its stable segment-local parameter.
    InsertNode { parameter: f64 },
    /// Removes the selected anchor while the geometry boundary retains its node minimum.
    DeleteNode { node: usize },
}

/// Stable local failure for a requested action that has no compatible current selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PathEditError {
    /// The caller requested a segment action while no segment is selected.
    NoSegmentSelection,
    /// Canonical geometry rejected an otherwise selected path edit.
    Geometry(CurveError),
    /// The active Motif policy keeps its tile terminal anchors immutable.
    EndpointLocked,
}

impl From<CurveError> for PathEditError {
    /// Preserves the canonical geometry diagnostic at the widget-independent editor boundary.
    fn from(value: CurveError) -> Self {
        Self::Geometry(value)
    }
}

/// Infers whether terminal Motif handles share a finite seam direction without changing geometry.
fn motif_terminal_direction(path: &CurvePath) -> MotifTerminalDirection {
    let (Some(CurveSegment::CubicBezier(first)), Some(CurveSegment::CubicBezier(last))) =
        (path.segments().first(), path.segments().last())
    else {
        return MotifTerminalDirection::Corner;
    };
    let outgoing = Point2::new(
        first.control_1().x - first.start().x,
        first.control_1().y - first.start().y,
    );
    let incoming = Point2::new(
        last.end().x - last.control_2().x,
        last.end().y - last.control_2().y,
    );
    let outgoing_length = outgoing.x.hypot(outgoing.y);
    let incoming_length = incoming.x.hypot(incoming.y);
    if outgoing_length <= 1.0e-12 || incoming_length <= 1.0e-12 {
        return MotifTerminalDirection::Corner;
    }
    let cross = outgoing.x * incoming.y - outgoing.y * incoming.x;
    let dot = outgoing.x * incoming.x + outgoing.y * incoming.y;
    if cross.abs() <= 1.0e-9 * outgoing_length * incoming_length && dot > 0.0 {
        MotifTerminalDirection::Smooth
    } else {
        MotifTerminalDirection::Corner
    }
}

#[allow(dead_code)] // GTK projection is introduced incrementally; state remains directly unit-testable.
impl Stage20fEditorState {
    /// Configures the editor-local policy and workspace frame for one modal open.
    ///
    /// The workspace dimensions influence only Guide authoring presentation and its endpoint guard;
    /// Shape and Motif geometry keep their canonical local coordinates. Invalid workspace values
    /// disable the frame rather than manufacturing a substitute scale.
    pub(crate) fn configure_mode(
        &mut self,
        mode: AuthoredPathEditorMode,
        workspace_width: f64,
        workspace_height: f64,
    ) {
        self.mode = Some(mode);
        self.guide_frame = match mode {
            AuthoredPathEditorMode::Guide => {
                GuideAuthoringFrame::new(workspace_width, workspace_height)
            }
            AuthoredPathEditorMode::Motif => GuideAuthoringFrame::new(1.0, 1.0),
            AuthoredPathEditorMode::Shape => None,
        };
        self.motif_terminal_direction = None;
    }

    /// Updates the Guide frame's canvas allocation while retaining the same uniform mapping.
    pub(crate) fn set_guide_canvas_allocation(
        &mut self,
        canvas_width: f64,
        canvas_height: f64,
        padding: f64,
    ) {
        let Some(frame) = self.guide_frame.as_mut() else {
            return;
        };
        if canvas_width.is_finite()
            && canvas_height.is_finite()
            && canvas_width > 0.0
            && canvas_height > 0.0
            && padding.is_finite()
            && padding >= 0.0
        {
            frame.canvas_width = canvas_width;
            frame.canvas_height = canvas_height;
            frame.padding = padding.min(canvas_width.min(canvas_height) * 0.45);
        }
    }

    /// Returns the current Guide frame for canvas drawing and focused policy tests.
    pub(crate) const fn guide_frame(&self) -> Option<GuideAuthoringFrame> {
        self.guide_frame
    }

    /// Inspects an opened Motif path to derive its current non-persisted terminal-direction presentation.
    ///
    /// The first inspection after selection records only a local UI mode. It never changes the
    /// source path, and a subsequent explicit Smooth or Corner choice survives ordinary rebuilds.
    pub(crate) fn inspect_motif_terminal_direction(&mut self, path: &CurvePath) {
        if self.mode == Some(AuthoredPathEditorMode::Motif)
            && self.motif_terminal_direction.is_none()
        {
            self.motif_terminal_direction = Some(motif_terminal_direction(path));
        }
    }

    /// Returns the effective local Motif terminal-direction mode after inspecting the current path.
    pub(crate) fn motif_terminal_direction(&self, path: &CurvePath) -> MotifTerminalDirection {
        self.motif_terminal_direction
            .unwrap_or_else(|| motif_terminal_direction(path))
    }

    /// Selects the local next-edit terminal-direction policy without persisting a seam-mode field.
    pub(crate) fn set_motif_terminal_direction(&mut self, direction: MotifTerminalDirection) {
        if self.mode == Some(AuthoredPathEditorMode::Motif) {
            self.motif_terminal_direction = Some(direction);
        }
    }

    /// Reports whether this selected Motif exposes both editable terminal handles.
    pub(crate) fn motif_terminal_controls_editable(&self, path: &CurvePath) -> bool {
        self.mode == Some(AuthoredPathEditorMode::Motif)
            && path.closure() == PathClosure::Open
            && path
                .segments()
                .first()
                .is_some_and(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
            && path
                .segments()
                .last()
                .is_some_and(|segment| matches!(segment, CurveSegment::CubicBezier(_)))
    }

    /// Resolves the selected construction coordinate from immutable geometry.
    pub(crate) fn coordinate(&self, path: &CurvePath, target: NumericTarget) -> Option<Point2> {
        match target {
            NumericTarget::Anchor(index) => (index < path.segments().len())
                .then(|| path.segments()[index].start())
                .or_else(|| {
                    (path.closure() == PathClosure::Open && index == path.segments().len())
                        .then(|| path.end())
                }),
            NumericTarget::Control1(index) => match path.segments().get(index)? {
                toniator_geometry::CurveSegment::CubicBezier(value) => Some(value.control_1()),
                _ => None,
            },
            NumericTarget::Control2(index) => match path.segments().get(index)? {
                toniator_geometry::CurveSegment::CubicBezier(value) => Some(value.control_2()),
                _ => None,
            },
        }
    }

    /// Constrains a selected point through the active Guide or Motif policy before canonical mutation.
    fn constrain_target_point(
        &self,
        path: &CurvePath,
        target: NumericTarget,
        point: Point2,
    ) -> Option<Point2> {
        if !point.is_finite() {
            return None;
        }
        match self.mode {
            Some(AuthoredPathEditorMode::Guide) => {
                let frame = self.guide_frame?;
                let end_index = path.segments().len();
                match target {
                    NumericTarget::Anchor(0) => Some(Point2::new(
                        frame.width * -0.5,
                        frame.clamp_endpoint_y(point.y, 8.0, self.zoom),
                    )),
                    NumericTarget::Anchor(index)
                        if path.closure() == PathClosure::Open && index == end_index =>
                    {
                        Some(Point2::new(
                            frame.width * 0.5,
                            frame.clamp_endpoint_y(point.y, 8.0, self.zoom),
                        ))
                    }
                    _ => Some(point),
                }
            }
            Some(AuthoredPathEditorMode::Motif) => {
                let end_index = path.segments().len();
                match target {
                    NumericTarget::Anchor(0) => self.coordinate(path, target),
                    NumericTarget::Anchor(index)
                        if path.closure() == PathClosure::Open && index == end_index =>
                    {
                        self.coordinate(path, target)
                    }
                    _ => Some(point),
                }
            }
            Some(AuthoredPathEditorMode::Shape) | None => Some(point),
        }
    }

    /// Applies one point mutation through the active mode policy and returns the full immutable replacement.
    fn move_target(
        &self,
        path: &CurvePath,
        target: NumericTarget,
        point: Point2,
    ) -> Result<CurvePath, CurveError> {
        let point = self
            .constrain_target_point(path, target, point)
            .unwrap_or(point);
        let moved = match target {
            NumericTarget::Anchor(index) => path.move_anchor(index, point)?,
            NumericTarget::Control1(index) => path.move_cubic_control(index, true, point)?,
            NumericTarget::Control2(index) => path.move_cubic_control(index, false, point)?,
        };
        if self.mode != Some(AuthoredPathEditorMode::Motif)
            || self.motif_terminal_direction(path) != MotifTerminalDirection::Smooth
        {
            return Ok(moved);
        }
        let last_index = path.segments().len().saturating_sub(1);
        match target {
            NumericTarget::Control1(0)
                if last_index > 0
                    || matches!(path.segments().first(), Some(CurveSegment::CubicBezier(_))) =>
            {
                let Some(CurveSegment::CubicBezier(last)) = path.segments().last() else {
                    return Ok(moved);
                };
                let start = path.segments()[0].start();
                let direction = Point2::new(point.x - start.x, point.y - start.y);
                let direction_length = direction.x.hypot(direction.y);
                let incoming = Point2::new(
                    last.end().x - last.control_2().x,
                    last.end().y - last.control_2().y,
                );
                let incoming_length = incoming.x.hypot(incoming.y);
                if direction_length <= 1.0e-12 || incoming_length <= 1.0e-12 {
                    return Ok(moved);
                }
                let opposite = Point2::new(
                    last.end().x - direction.x / direction_length * incoming_length,
                    last.end().y - direction.y / direction_length * incoming_length,
                );
                moved.move_cubic_control(last_index, false, opposite)
            }
            NumericTarget::Control2(index) if index == last_index => {
                let Some(CurveSegment::CubicBezier(first)) = path.segments().first() else {
                    return Ok(moved);
                };
                let end = path.end();
                let direction = Point2::new(end.x - point.x, end.y - point.y);
                let direction_length = direction.x.hypot(direction.y);
                let outgoing = Point2::new(
                    first.control_1().x - first.start().x,
                    first.control_1().y - first.start().y,
                );
                let outgoing_length = outgoing.x.hypot(outgoing.y);
                if direction_length <= 1.0e-12 || outgoing_length <= 1.0e-12 {
                    return Ok(moved);
                }
                let opposite = Point2::new(
                    first.start().x + direction.x / direction_length * outgoing_length,
                    first.start().y + direction.y / direction_length * outgoing_length,
                );
                moved.move_cubic_control(0, true, opposite)
            }
            _ => Ok(moved),
        }
    }
    /// Commits finite changed numeric coordinates as one replacement payload and treats equality as no-op.
    pub(crate) fn commit_numeric(
        &self,
        path: &CurvePath,
        target: NumericTarget,
        x: &str,
        y: &str,
    ) -> Option<AuthoredStructureDraft> {
        let point = Point2::new(x.parse().ok()?, y.parse().ok()?);
        if !point.is_finite() {
            return None;
        }
        let point = self.constrain_target_point(path, target, point)?;
        let current = self.coordinate(path, target)?;
        if point == current {
            return None;
        }
        let path = self.move_target(path, target, point).ok()?;
        path.to_authored_structure_draft().ok()
    }
    /// Returns the deterministic document-unit nudge step; Control precision takes precedence over Shift.
    pub(crate) fn nudge_step(shift: bool, control: bool) -> f64 {
        if control {
            0.1
        } else if shift {
            10.0
        } else {
            1.0
        }
    }
    /// Produces one replacement payload for a selected anchor or control-point nudge.
    pub(crate) fn nudge_selected(
        &self,
        path: &CurvePath,
        target: NumericTarget,
        dx: f64,
        dy: f64,
    ) -> Option<AuthoredStructureDraft> {
        let point = self.coordinate(path, target)?;
        self.commit_numeric(
            path,
            target,
            &(point.x + dx).to_string(),
            &(point.y + dy).to_string(),
        )
    }
    /// Produces one replacement draft for an explicit path action, preserving no-op segment kinds.
    ///
    /// # Errors
    ///
    /// Returns the canonical geometry failure for an invalid selection or a deletion that would
    /// violate the path's two-node lower bound. No mutable editor or document state changes here.
    pub(crate) fn edit_path(
        &self,
        path: &CurvePath,
        edit: PathEdit,
    ) -> Result<Option<AuthoredStructureDraft>, PathEditError> {
        let edited = match edit {
            PathEdit::MakeCurve => {
                let index = self
                    .selected_segment
                    .ok_or(PathEditError::NoSegmentSelection)?;
                if matches!(
                    path.segments().get(index),
                    Some(CurveSegment::CubicBezier(_))
                ) {
                    return Ok(None);
                }
                path.toggle_segment_kind(index)?
            }
            PathEdit::MakeLine => {
                let index = self
                    .selected_segment
                    .ok_or(PathEditError::NoSegmentSelection)?;
                if matches!(path.segments().get(index), Some(CurveSegment::Line(_))) {
                    return Ok(None);
                }
                path.toggle_segment_kind(index)?
            }
            PathEdit::InsertNode { parameter } => {
                let index = self
                    .selected_segment
                    .ok_or(PathEditError::NoSegmentSelection)?;
                path.insert_node(PathLocation::new(index, parameter.clamp(0.05, 0.95))?)?
            }
            PathEdit::DeleteNode { node } => {
                if self.mode == Some(AuthoredPathEditorMode::Motif)
                    && path.closure() == PathClosure::Open
                    && (node == 0 || node == path.segments().len())
                {
                    return Err(PathEditError::EndpointLocked);
                }
                path.delete_node(node)?
            }
        };
        Ok(Some(edited.to_authored_structure_draft()?))
    }

    /// Selects a fixed Motif terminal handle and explicitly converts only its line segment if needed.
    ///
    /// # Errors
    ///
    /// Returns the canonical path editing diagnostic for a missing terminal segment or invalid
    /// conversion. Cubic selection changes only local editor state; a returned draft is one
    /// explicit line-to-cubic history payload and never occurs merely by opening the editor.
    pub(crate) fn select_motif_terminal_handle(
        &mut self,
        path: &CurvePath,
        handle: MotifTerminalHandle,
    ) -> Result<Option<AuthoredStructureDraft>, PathEditError> {
        if self.mode != Some(AuthoredPathEditorMode::Motif) || path.closure() != PathClosure::Open {
            return Err(PathEditError::NoSegmentSelection);
        }
        let last = path
            .segments()
            .len()
            .checked_sub(1)
            .ok_or(PathEditError::NoSegmentSelection)?;
        let (segment, target) = match handle {
            MotifTerminalHandle::Left => (0, NumericTarget::Control1(0)),
            MotifTerminalHandle::Right => (last, NumericTarget::Control2(last)),
        };
        self.selected_node = None;
        self.selected_segment = Some(segment);
        self.selected_segment_parameter = None;
        self.selected_target = Some(target);
        match path.segments().get(segment) {
            Some(CurveSegment::CubicBezier(_)) => Ok(None),
            Some(CurveSegment::Line(_)) => self.edit_path(path, PathEdit::MakeCurve),
            None => Err(PathEditError::NoSegmentSelection),
        }
    }
    /// Starts local construction without publishing a draft command.
    pub(crate) fn begin(&mut self, kind: AuthoredStructureKind) {
        self.local_invalid = false;
        let points = if self.mode == Some(AuthoredPathEditorMode::Motif)
            && kind == AuthoredStructureKind::OpenPath
        {
            self.motif_terminal_direction = Some(MotifTerminalDirection::Smooth);
            vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)]
        } else {
            Vec::new()
        };
        self.construction = Some(Construction { kind, points });
    }
    /// Adds one finite document-space node without publishing a draft command.
    pub(crate) fn add_node(&mut self, point: Point2) -> Option<AuthoredStructureDraft> {
        let construction = self.construction.as_mut()?;
        if !point.is_finite() {
            return None;
        }
        let point = if self.mode == Some(AuthoredPathEditorMode::Guide)
            && construction.kind == AuthoredStructureKind::OpenPath
        {
            let frame = self.guide_frame?;
            match construction.points.len() {
                0 => Point2::new(
                    frame.width * -0.5,
                    frame.clamp_endpoint_y(point.y, 8.0, self.zoom),
                ),
                _ => point,
            }
        } else {
            point
        };
        if self.mode == Some(AuthoredPathEditorMode::Motif)
            && construction.kind == AuthoredStructureKind::OpenPath
            && construction.points.len() >= 2
        {
            let insertion_index = construction.points.len() - 1;
            construction.points.insert(insertion_index, point);
        } else {
            construction.points.push(point);
        }
        None
    }
    /// Adds one screen-space construction click after inverse projection and closes a mark at its first anchor hit.
    pub(crate) fn add_node_screen(
        &mut self,
        screen: Point2,
        radius: f64,
    ) -> Option<AuthoredStructureDraft> {
        let document = self.to_document(screen)?;
        let construction = self.construction.as_ref()?;
        if construction.kind == AuthoredStructureKind::ClosedShape && construction.points.len() >= 2
        {
            let first = self.to_screen(construction.points[0]);
            if (first.x - screen.x).hypot(first.y - screen.y) <= radius {
                return self.complete();
            }
        }
        self.add_node(document)
    }
    /// Builds a completion payload only when local construction has at least two nodes.
    ///
    /// The local construction remains present until the caller's typed attachment succeeds, so a
    /// target-validation or history failure never discards the artist's unfinished geometry.
    pub(crate) fn complete(&mut self) -> Option<AuthoredStructureDraft> {
        let construction = self.construction.as_ref()?;
        if construction.points.len() < 2 {
            return None;
        }
        let closure = if construction.kind == AuthoredStructureKind::OpenPath {
            PathClosure::Open
        } else {
            PathClosure::Closed
        };
        if self.mode == Some(AuthoredPathEditorMode::Motif)
            && construction.kind == AuthoredStructureKind::OpenPath
            && construction.points.len() == 2
        {
            let start = Point2::new(0.0, 0.0);
            let end = Point2::new(1.0, 0.0);
            return CurvePath::new(
                vec![CurveSegment::CubicBezier(
                    toniator_geometry::CubicBezierSegment::new(
                        start,
                        Point2::new(1.0 / 3.0, 0.0),
                        Point2::new(2.0 / 3.0, 0.0),
                        end,
                    )
                    .ok()?,
                )],
                PathClosure::Open,
            )
            .ok()?
            .to_authored_structure_draft()
            .ok();
        }
        let mut points = construction.points.clone();
        if self.mode == Some(AuthoredPathEditorMode::Guide)
            && construction.kind == AuthoredStructureKind::OpenPath
        {
            let frame = self.guide_frame?;
            if let Some(first) = points.first_mut() {
                *first = Point2::new(
                    frame.width * -0.5,
                    frame.clamp_endpoint_y(first.y, 8.0, self.zoom),
                );
            }
            if let Some(last) = points.last_mut() {
                *last = Point2::new(
                    frame.width * 0.5,
                    frame.clamp_endpoint_y(last.y, 8.0, self.zoom),
                );
            }
        }
        CurvePath::polyline(points, closure)
            .ok()?
            .to_authored_structure_draft()
            .ok()
    }
    /// Cancels incomplete local construction without affecting document history.
    pub(crate) fn cancel(&mut self) {
        self.construction = None;
        self.local_invalid = false;
    }
    /// Reports whether incomplete construction blocks Apply.
    pub(crate) fn incomplete(&self) -> bool {
        self.construction.is_some()
    }
    /// Returns the current local construction points for canvas-only presentation.
    ///
    /// The returned points have no document or history authority and remain absent after
    /// completion or cancellation. GTK must not treat this projection as a persisted path.
    pub(crate) fn construction_points(&self) -> Option<&[Point2]> {
        self.construction
            .as_ref()
            .map(|construction| construction.points.as_slice())
    }
    /// Sets local editor validity without mutating document history.
    pub(crate) fn set_local_invalid(&mut self, value: bool) {
        self.local_invalid = value;
    }
    /// Reports whether Apply may publish the draft under local editor gates.
    pub(crate) fn apply_ready(&self, dirty: bool) -> bool {
        dirty && !self.incomplete() && !self.local_invalid
    }
    /// Reports whether this draft already received a shared-resource choice for one original ID.
    pub(crate) fn is_shared_armed(&self, structure_id: AuthoredStructureId) -> bool {
        self.shared_armed.contains(&structure_id)
    }
    /// Records the chosen shared-resource policy for this private draft only.
    pub(crate) fn arm_shared_resource(&mut self, structure_id: AuthoredStructureId) {
        self.shared_armed.insert(structure_id);
    }
    /// Selects one explicit authored resource and clears only local point and validation presentation state.
    pub(crate) fn select_structure(&mut self, structure_id: AuthoredStructureId) {
        self.selected_structure = Some(structure_id);
        self.selected_node = None;
        self.selected_segment = None;
        self.selected_segment_parameter = None;
        self.selected_target = None;
        self.motif_terminal_direction = None;
        self.local_invalid = false;
    }
    /// Clears the current persisted-resource selection without changing construction or history.
    ///
    /// Purpose-specific GTK presentations use this when a different resource kind is no longer
    /// visible, so numeric and segment operations cannot silently target a hidden structure.
    pub(crate) fn clear_structure_selection(&mut self) {
        self.selected_structure = None;
        self.selected_node = None;
        self.selected_segment = None;
        self.selected_segment_parameter = None;
        self.selected_target = None;
        self.local_invalid = false;
    }
    /// Converts document coordinates into screen coordinates under the finite editor viewport.
    pub(crate) fn to_screen(&self, point: Point2) -> Point2 {
        let point = self
            .guide_frame
            .map(|frame| frame.to_canvas(self.frame_presentation_point(point)))
            .unwrap_or(point);
        Point2::new(
            point.x * self.zoom + self.pan.x,
            point.y * self.zoom + self.pan.y,
        )
    }

    /// Returns the active authoring frame's exact screen-space bounds.
    ///
    /// Guide frames expose centered-local workspace coordinates so baseline rotation retains the
    /// guide across the target canvas. Motif frames expose the canonical tile interval from
    /// `y = -0.5` through `y = 0.5`, so the immutable `(0, 0)` and `(1, 0)` endpoints remain
    /// centered rather than appearing on the top edge. Shape mode has no authoring frame.
    pub(crate) fn frame_screen_bounds(&self) -> Option<(Point2, Point2)> {
        let frame = self.guide_frame?;
        let (top_left, bottom_right) = if self.mode == Some(AuthoredPathEditorMode::Motif) {
            (Point2::new(0.0, -0.5), Point2::new(1.0, 0.5))
        } else {
            (
                Point2::new(frame.width * -0.5, frame.height * -0.5),
                Point2::new(frame.width * 0.5, frame.height * 0.5),
            )
        };
        Some((self.to_screen(top_left), self.to_screen(bottom_right)))
    }

    /// Converts finite screen coordinates to document coordinates through the current finite viewport inverse.
    pub(crate) fn to_document(&self, point: Point2) -> Option<Point2> {
        if !point.is_finite() || !self.pan.is_finite() || !self.zoom.is_finite() || self.zoom <= 0.0
        {
            return None;
        }
        let projected = Point2::new(
            (point.x - self.pan.x) / self.zoom,
            (point.y - self.pan.y) / self.zoom,
        );
        let document = self
            .guide_frame
            .map(|frame| frame.canvas_to_document(projected))
            .unwrap_or(Some(projected))?;
        let document = self.document_point_from_frame_presentation(document);
        document.is_finite().then_some(document)
    }

    /// Maps one canonical point into the active frame's presentational coordinate space.
    ///
    /// Motif geometry remains tile-local with canonical endpoints `(0, 0)` and `(1, 0)`, while
    /// Guide geometry uses a centered local origin for rotation-safe placement. Shape coordinates
    /// pass through unchanged.
    fn frame_presentation_point(&self, point: Point2) -> Point2 {
        match self.mode {
            Some(AuthoredPathEditorMode::Guide) => self.guide_frame.map_or(point, |frame| {
                Point2::new(point.x + frame.width * 0.5, point.y + frame.height * 0.5)
            }),
            Some(AuthoredPathEditorMode::Motif) => Point2::new(point.x, point.y + 0.5),
            _ => point,
        }
    }

    /// Inverts the active frame's presentation-only coordinate adjustment.
    ///
    /// The result restores canonical Motif coordinates before every pointer-derived construction,
    /// selection, drag, or numeric edit path reaches the shared geometry policy.
    fn document_point_from_frame_presentation(&self, point: Point2) -> Point2 {
        match self.mode {
            Some(AuthoredPathEditorMode::Guide) => self.guide_frame.map_or(point, |frame| {
                Point2::new(point.x - frame.width * 0.5, point.y - frame.height * 0.5)
            }),
            Some(AuthoredPathEditorMode::Motif) => Point2::new(point.x, point.y - 0.5),
            _ => point,
        }
    }
    /// Resolves the first anchor inside the screen-space hit radius.
    pub(crate) fn hit_anchor(
        &self,
        path: &CurvePath,
        screen: Point2,
        radius: f64,
    ) -> Option<usize> {
        let nodes = if path.closure() == PathClosure::Closed {
            path.segments().len()
        } else {
            path.segments().len() + 1
        };
        (0..nodes).find(|index| {
            let point = if *index < path.segments().len() {
                path.segments()[*index].start()
            } else {
                path.end()
            };
            let point = self.to_screen(point);
            (point.x - screen.x).hypot(point.y - screen.y) <= radius
        })
    }
    /// Resolves the first cubic control point inside the screen-space hit radius.
    pub(crate) fn hit_control(
        &self,
        path: &CurvePath,
        screen: Point2,
        radius: f64,
    ) -> Option<NumericTarget> {
        path.segments()
            .iter()
            .enumerate()
            .find_map(|(index, segment)| {
                let CurveSegment::CubicBezier(cubic) = segment else {
                    return None;
                };
                [
                    (NumericTarget::Control1(index), cubic.control_1()),
                    (NumericTarget::Control2(index), cubic.control_2()),
                ]
                .into_iter()
                .find_map(|(target, point)| {
                    let point = self.to_screen(point);
                    ((point.x - screen.x).hypot(point.y - screen.y) <= radius).then_some(target)
                })
            })
    }
    /// Returns the nearest bounded screen-space parameter on one segment inside the hit radius.
    ///
    /// Lines use their exact projected chord. Cubics use 64 fixed screen-space chords so hit
    /// testing remains bounded, zoom-stable, and sufficiently dense for normal pointer radii.
    fn hit_segment_parameter(
        &self,
        segment: &CurveSegment,
        screen: Point2,
        radius: f64,
    ) -> Option<(f64, f64)> {
        const CUBIC_STEPS: usize = 64;
        let steps = if matches!(segment, CurveSegment::Line(_)) {
            1
        } else {
            CUBIC_STEPS
        };
        let mut closest = None::<(f64, f64)>;
        for index in 0..steps {
            let parameter_start = index as f64 / steps as f64;
            let parameter_end = (index + 1) as f64 / steps as f64;
            let start = segment
                .point_at(parameter_start)
                .ok()
                .map(|point| self.to_screen(point))?;
            let end = segment
                .point_at(parameter_end)
                .ok()
                .map(|point| self.to_screen(point))?;
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let length_squared = dx.mul_add(dx, dy * dy);
            let local = if length_squared > 1.0e-12 {
                (((screen.x - start.x) * dx + (screen.y - start.y) * dy) / length_squared)
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };
            let nearest_x = start.x + dx * local;
            let nearest_y = start.y + dy * local;
            let distance_squared = (screen.x - nearest_x).mul_add(
                screen.x - nearest_x,
                (screen.y - nearest_y) * (screen.y - nearest_y),
            );
            if distance_squared.is_finite()
                && closest.is_none_or(|(best, _)| distance_squared < best)
            {
                closest = Some((
                    distance_squared,
                    parameter_start + (parameter_end - parameter_start) * local,
                ));
            }
        }
        closest.filter(|(distance_squared, _)| *distance_squared <= radius * radius)
    }
    /// Selects anchors and controls first, then the nearest bounded screen-space segment projection.
    pub(crate) fn select_at(&mut self, path: &CurvePath, screen: Point2, radius: f64) {
        self.local_invalid = false;
        self.selected_node = self.hit_anchor(path, screen, radius);
        if self.selected_node.is_some() {
            self.selected_segment = None;
            self.selected_segment_parameter = None;
            self.selected_target = self.selected_node.map(NumericTarget::Anchor);
            return;
        }
        if let Some(target) = self.hit_control(path, screen, radius) {
            self.selected_node = None;
            self.selected_segment = match target {
                NumericTarget::Control1(index) | NumericTarget::Control2(index) => Some(index),
                NumericTarget::Anchor(_) => None,
            };
            self.selected_segment_parameter = None;
            self.selected_target = Some(target);
            return;
        }
        let hit = path
            .segments()
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| {
                self.hit_segment_parameter(segment, screen, radius)
                    .map(|(distance_squared, parameter)| (distance_squared, index, parameter))
            });
        let hit = hit.min_by(|left, right| left.0.total_cmp(&right.0));
        self.selected_segment = hit.map(|(_, index, _)| index);
        self.selected_segment_parameter = hit.map(|(_, _, parameter)| parameter);
        self.selected_target = None;
    }
    /// Updates finite pan/zoom presentation state without modifying geometry or history.
    pub(crate) fn set_viewport(&mut self, pan: Point2, zoom: f64) {
        if pan.is_finite() && zoom.is_finite() && zoom > 0.0 {
            self.pan = pan;
            self.zoom = zoom;
        }
    }
    /// Starts an anchor or cubic-control drag with an immutable path base and no command payload.
    pub(crate) fn begin_target_drag(&mut self, path: CurvePath, target: NumericTarget) {
        self.drag = Some((path, target));
        self.local_preview = None;
        self.drag_screen_origin = None;
    }
    /// Starts a target drag at one screen point so GTK offset callbacks retain an exact viewport inverse.
    pub(crate) fn begin_target_drag_at(
        &mut self,
        path: CurvePath,
        target: NumericTarget,
        screen: Point2,
    ) {
        self.begin_target_drag(path, target);
        self.drag_screen_origin = screen.is_finite().then_some(screen);
    }
    /// Reports whether one primary-pointer sequence may become an editable target drag.
    ///
    /// Incomplete construction owns primary clicks until explicit completion or cancellation, and
    /// a drag without an anchor/control target must leave the sequence available to click handling.
    pub(crate) fn may_claim_target_drag(&self, target: Option<NumericTarget>) -> bool {
        !self.incomplete() && target.is_some()
    }
    /// Produces a local immutable preview path during pointer motion without publishing history input.
    pub(crate) fn drag_preview(&self, point: Point2) -> Option<CurvePath> {
        let (path, target) = self.drag.as_ref()?;
        self.move_target(path, *target, point).ok()
    }
    /// Updates only the local drag preview used by canvas drawing and publishes no history payload.
    pub(crate) fn update_drag_preview(&mut self, point: Point2) -> bool {
        self.local_preview = self.drag_preview(point);
        self.local_preview.is_some()
    }
    /// Updates a GTK drag offset through its retained screen origin and current inverse viewport.
    pub(crate) fn update_drag_offset(&mut self, offset_x: f64, offset_y: f64) -> bool {
        let Some(origin) = self.drag_screen_origin else {
            return false;
        };
        self.to_document(Point2::new(origin.x + offset_x, origin.y + offset_y))
            .is_some_and(|point| self.update_drag_preview(point))
    }
    /// Returns the local drag path, if any, for presentation-only canvas drawing.
    pub(crate) fn local_preview_path(&self) -> Option<&CurvePath> {
        self.local_preview.as_ref()
    }
    /// Ends a drag and returns one payload only when the target geometry changed.
    ///
    /// A zero-distance press/release still clears local drag presentation but returns no payload,
    /// so selection clicks cannot reach the typed replacement command or overwrite status with a
    /// semantic no-op diagnostic.
    pub(crate) fn end_drag(&mut self, point: Point2) -> Option<AuthoredStructureDraft> {
        let source = self.drag.as_ref()?.0.clone();
        let preview = self.drag_preview(point)?;
        self.drag = None;
        self.local_preview = None;
        self.drag_screen_origin = None;
        (preview != source)
            .then(|| preview.to_authored_structure_draft().ok())
            .flatten()
    }
    /// Ends a GTK drag offset through its retained screen origin and publishes one replacement payload.
    pub(crate) fn end_drag_offset(
        &mut self,
        offset_x: f64,
        offset_y: f64,
    ) -> Option<AuthoredStructureDraft> {
        let origin = self.drag_screen_origin?;
        let point = self.to_document(Point2::new(origin.x + offset_x, origin.y + offset_y))?;
        self.end_drag(point)
    }
    /// Starts a middle-button pan gesture without changing construction, selection, or history.
    pub(crate) fn begin_pan(&mut self, screen: Point2) {
        self.pan_drag = screen.is_finite().then_some((screen, self.pan));
    }
    /// Updates pan by the screen-space gesture delta while retaining the current zoom and document geometry.
    pub(crate) fn update_pan(&mut self, screen: Point2) -> bool {
        let Some((start, initial)) = self.pan_drag else {
            return false;
        };
        if !screen.is_finite() {
            return false;
        }
        self.pan = Point2::new(
            initial.x + screen.x - start.x,
            initial.y + screen.y - start.y,
        );
        self.pan.is_finite()
    }
    /// Updates pan from GTK drag offsets while preserving the initial pan without document mutation.
    pub(crate) fn update_pan_offset(&mut self, offset_x: f64, offset_y: f64) -> bool {
        let Some((_, initial)) = self.pan_drag else {
            return false;
        };
        let pan = Point2::new(initial.x + offset_x, initial.y + offset_y);
        if !pan.is_finite() {
            return false;
        }
        self.pan = pan;
        true
    }
    /// Ends a presentation-only pan gesture without producing a history action.
    pub(crate) fn end_pan(&mut self) {
        self.pan_drag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Proves screen-space selection prefers anchors over overlapping segment samples.
    #[test]
    fn selection_prefers_anchor_then_segment() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.select_at(&path, Point2::new(0.0, 0.0), 2.0);
        assert_eq!(state.selected_node, Some(0));
        state.select_at(&path, Point2::new(5.0, 0.0), 1.0);
        assert_eq!(state.selected_node, None);
        assert_eq!(state.selected_segment, Some(0));
    }
    /// Proves a selection click yields no payload while a moved drag exposes exactly one replacement payload.
    #[test]
    fn drag_motion_is_local_until_release() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.begin_target_drag(path.clone(), NumericTarget::Anchor(0));
        assert!(state.end_drag(Point2::new(0.0, 0.0)).is_none());
        assert!(state.drag_preview(Point2::new(3.0, 3.0)).is_none());
        state.begin_target_drag(path, NumericTarget::Anchor(0));
        assert_eq!(
            state.drag_preview(Point2::new(2.0, 3.0)).unwrap().start(),
            Point2::new(2.0, 3.0)
        );
        assert!(state.end_drag(Point2::new(2.0, 3.0)).is_some());
        assert!(state.drag_preview(Point2::new(3.0, 3.0)).is_none());
    }
    /// Proves numeric point edits resolve anchors and controls, reject invalid text, and avoid equal-value commands.
    #[test]
    fn numeric_coordinates_commit_once_and_reject_invalid_input() {
        let path = CurvePath::new(
            vec![toniator_geometry::CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 2.0),
                    Point2::new(3.0, 2.0),
                    Point2::new(4.0, 0.0),
                )
                .unwrap(),
            )],
            PathClosure::Open,
        )
        .unwrap();
        let state = Stage20fEditorState::default();
        assert_eq!(
            state.coordinate(&path, NumericTarget::Anchor(0)),
            Some(Point2::new(0.0, 0.0))
        );
        assert_eq!(
            state.coordinate(&path, NumericTarget::Control1(0)),
            Some(Point2::new(1.0, 2.0))
        );
        assert!(
            state
                .commit_numeric(&path, NumericTarget::Anchor(0), "0", "0")
                .is_none()
        );
        assert!(
            state
                .commit_numeric(&path, NumericTarget::Anchor(0), "bad", "2")
                .is_none()
        );
        let moved = state
            .commit_numeric(&path, NumericTarget::Control2(0), "5", "6")
            .unwrap();
        assert_eq!(moved.segments()[0].end().x, 4.0);
    }
    /// Proves local invalid state blocks Apply without changing draft dirty state.
    #[test]
    fn apply_gate_requires_dirty_complete_and_valid_editor_state() {
        let mut state = Stage20fEditorState::default();
        assert!(!state.apply_ready(false));
        assert!(state.apply_ready(true));
        state.set_local_invalid(true);
        assert!(!state.apply_ready(true));
    }
    /// Proves normal, shifted, and precision nudge steps remain deterministic.
    #[test]
    fn nudge_steps_use_control_precision_and_shift_coarse_units() {
        assert_eq!(Stage20fEditorState::nudge_step(false, false), 1.0);
        assert_eq!(Stage20fEditorState::nudge_step(true, false), 10.0);
        assert_eq!(Stage20fEditorState::nudge_step(true, true), 0.1);
    }
    /// Proves explicit segment conversion preserves already-correct kinds as history-free no-ops.
    #[test]
    fn segment_kind_actions_convert_only_when_needed() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(9.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let state = Stage20fEditorState {
            selected_segment: Some(0),
            ..Default::default()
        };
        let curve = state
            .edit_path(&path, PathEdit::MakeCurve)
            .unwrap()
            .unwrap();
        assert!(matches!(
            curve.segments()[0],
            toniator_domain::AuthoredCurveSegment::CubicBezier { .. }
        ));
        let cubic_path = path.toggle_segment_kind(0).unwrap();
        assert_eq!(
            state.edit_path(&cubic_path, PathEdit::MakeCurve).unwrap(),
            None
        );
        let line = state
            .edit_path(&cubic_path, PathEdit::MakeLine)
            .unwrap()
            .unwrap();
        assert!(matches!(
            line.segments()[0],
            toniator_domain::AuthoredCurveSegment::Line { .. }
        ));
    }
    /// Proves insertion clamps pointer parameters and deletion retains the canonical node minimum.
    #[test]
    fn insert_and_delete_actions_respect_path_bounds() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let state = Stage20fEditorState {
            selected_segment: Some(0),
            ..Default::default()
        };
        let inserted = state
            .edit_path(&path, PathEdit::InsertNode { parameter: 0.0 })
            .unwrap()
            .unwrap();
        assert_eq!(inserted.segments().len(), 2);
        let error = state
            .edit_path(&path, PathEdit::DeleteNode { node: 0 })
            .unwrap_err();
        assert!(
            matches!(error, PathEditError::Geometry(error) if error.path() == "curve.path.edit.node_minimum")
        );
    }
    /// Proves segment hit-testing records its stable local parameter for pointer insertion.
    #[test]
    fn segment_selection_retains_hit_parameter() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.select_at(&path, Point2::new(5.0, 0.0), 0.1);
        assert_eq!(state.selected_segment, Some(0));
        assert_eq!(state.selected_segment_parameter, Some(0.5));
    }
    /// Proves exact line projection selects long segments between the old sparse sample locations.
    #[test]
    fn segment_selection_projects_between_sparse_samples() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(1_000.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.select_at(&path, Point2::new(137.0, 5.0), 8.0);
        assert_eq!(state.selected_segment, Some(0));
        assert!((state.selected_segment_parameter.unwrap() - 0.137).abs() < 1.0e-12);
    }
    /// Proves cubic-control hit testing selects the control target before sampled path segments.
    #[test]
    fn control_hit_testing_selects_the_exact_numeric_target() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 3.0),
                    Point2::new(3.0, 3.0),
                    Point2::new(4.0, 0.0),
                )
                .unwrap(),
            )],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.select_at(&path, Point2::new(1.0, 3.0), 0.1);
        assert_eq!(state.selected_target, Some(NumericTarget::Control1(0)));
        assert_eq!(state.selected_node, None);
        assert_eq!(state.selected_segment, Some(0));
    }

    /// Proves explicit terminal actions select cubic controls and convert only an addressed line.
    ///
    /// # Panics
    ///
    /// Panics when either fixed Motif terminal maps to the wrong numeric target, a cubic selection
    /// creates history payload, or an explicit line action fails to return exactly one replacement.
    #[test]
    fn motif_terminal_actions_select_controls_or_explicitly_convert_the_terminal_line() {
        let cubic = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(0.2, 0.3),
                    Point2::new(0.8, 0.3),
                    Point2::new(1.0, 0.0),
                )
                .expect("cubic fixture validates"),
            )],
            PathClosure::Open,
        )
        .expect("open cubic fixture validates");
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Motif, 1.0, 1.0);
        assert_eq!(
            state
                .select_motif_terminal_handle(&cubic, MotifTerminalHandle::Left)
                .expect("left cubic selection validates"),
            None
        );
        assert_eq!(state.selected_target, Some(NumericTarget::Control1(0)));
        assert_eq!(
            state
                .select_motif_terminal_handle(&cubic, MotifTerminalHandle::Right)
                .expect("right cubic selection validates"),
            None
        );
        assert_eq!(state.selected_target, Some(NumericTarget::Control2(0)));

        let line = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)],
            PathClosure::Open,
        )
        .expect("open line fixture validates");
        assert!(
            state
                .select_motif_terminal_handle(&line, MotifTerminalHandle::Left)
                .expect("explicit line conversion validates")
                .is_some()
        );
        assert_eq!(state.selected_target, Some(NumericTarget::Control1(0)));
    }
    /// Proves a control drag remains local until release and control nudges preserve its target.
    #[test]
    fn control_drag_and_nudge_edit_only_the_selected_handle() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 2.0),
                    Point2::new(3.0, 2.0),
                    Point2::new(4.0, 0.0),
                )
                .unwrap(),
            )],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.begin_target_drag(path.clone(), NumericTarget::Control2(0));
        let preview = state.drag_preview(Point2::new(5.0, 6.0)).unwrap();
        let CurveSegment::CubicBezier(preview) = preview.segments()[0] else {
            panic!("test path remains cubic");
        };
        assert_eq!(preview.control_2(), Point2::new(5.0, 6.0));
        let released = state.end_drag(Point2::new(5.0, 6.0)).unwrap();
        assert!(matches!(
            released.segments()[0],
            toniator_domain::AuthoredCurveSegment::CubicBezier { control_2, .. }
                if control_2.x == 5.0 && control_2.y == 6.0
        ));
        let nudged = state
            .nudge_selected(&path, NumericTarget::Control1(0), 0.1, -0.1)
            .unwrap();
        assert!(matches!(
            nudged.segments()[0],
            toniator_domain::AuthoredCurveSegment::CubicBezier { control_1, .. }
                if control_1.x == 1.1 && control_1.y == 1.9
        ));
    }
    /// Proves one private draft asks for a shared-resource policy only before its first mutation.
    #[test]
    fn shared_resource_gate_arms_only_after_an_explicit_choice() {
        let mut state = Stage20fEditorState::default();
        let resource = AuthoredStructureId(42);
        assert!(!state.is_shared_armed(resource));
        state.arm_shared_resource(resource);
        assert!(state.is_shared_armed(resource));
    }
    /// Proves open Enter completion, closed first-node seam completion, and Escape remain local.
    #[test]
    fn construction_preserves_open_closed_and_cancel_boundaries() {
        let mut state = Stage20fEditorState::default();
        state.begin(AuthoredStructureKind::OpenPath);
        assert!(state.add_node(Point2::new(0.0, 0.0)).is_none());
        assert!(state.add_node(Point2::new(4.0, 0.0)).is_none());
        assert_eq!(
            state.construction_points(),
            Some([Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)].as_slice())
        );
        assert_eq!(
            state.complete().unwrap().kind(),
            AuthoredStructureKind::OpenPath
        );
        assert!(state.incomplete());
        state.cancel();
        state.begin(AuthoredStructureKind::ClosedShape);
        assert!(state.add_node_screen(Point2::new(0.0, 0.0), 8.0).is_none());
        assert!(state.add_node_screen(Point2::new(4.0, 0.0), 8.0).is_none());
        assert_eq!(
            state
                .add_node_screen(Point2::new(7.0, 0.0), 8.0)
                .unwrap()
                .kind(),
            AuthoredStructureKind::ClosedShape
        );
        state.begin(AuthoredStructureKind::OpenPath);
        state.add_node(Point2::new(2.0, 2.0));
        state.cancel();
        assert!(!state.incomplete());
        assert_eq!(state.construction_points(), None);
    }
    /// Proves inactive canvas input cannot silently start a new guide construction.
    #[test]
    fn inactive_canvas_click_is_ignored_until_an_explicit_construction_action() {
        let mut state = Stage20fEditorState::default();
        assert!(state.add_node_screen(Point2::new(3.0, 4.0), 8.0).is_none());
        assert!(!state.incomplete());
        assert_eq!(state.construction_points(), None);
    }
    /// Proves construction and targetless pointer sequences stay available to the click controller.
    #[test]
    fn construction_prevents_drag_gesture_claiming() {
        let mut state = Stage20fEditorState::default();
        assert!(!state.may_claim_target_drag(None));
        assert!(state.may_claim_target_drag(Some(NumericTarget::Anchor(0))));
        state.begin(AuthoredStructureKind::ClosedShape);
        assert!(!state.may_claim_target_drag(Some(NumericTarget::Anchor(0))));
    }
    /// Proves inverse projection keeps zoomed construction and target dragging in document coordinates.
    #[test]
    fn inverse_viewport_preserves_zoomed_construction_and_drag() {
        let mut state = Stage20fEditorState::default();
        state.set_viewport(Point2::new(100.0, 50.0), 2.0);
        assert_eq!(
            state.to_document(Point2::new(104.0, 56.0)),
            Some(Point2::new(2.0, 3.0))
        );
        state.begin(AuthoredStructureKind::OpenPath);
        state.add_node_screen(Point2::new(100.0, 50.0), 8.0);
        state.add_node_screen(Point2::new(120.0, 50.0), 8.0);
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        state.begin_target_drag_at(path, NumericTarget::Anchor(0), Point2::new(100.0, 50.0));
        assert!(state.update_drag_offset(4.0, 6.0));
        assert_eq!(
            state.local_preview_path().unwrap().start(),
            Point2::new(2.0, 3.0)
        );
    }
    /// Proves middle-button pan and screen-space zoom leave document geometry untouched.
    #[test]
    fn pan_updates_only_viewport_translation() {
        let mut state = Stage20fEditorState::default();
        state.begin(AuthoredStructureKind::OpenPath);
        state.add_node(Point2::new(2.0, 3.0));
        let construction_before = state.construction_points().unwrap().to_vec();
        state.begin_pan(Point2::new(20.0, 30.0));
        assert!(state.update_pan_offset(6.0, -4.0));
        assert_eq!(state.pan, Point2::new(6.0, -4.0));
        assert_eq!(
            state.construction_points(),
            Some(construction_before.as_slice())
        );
        state.end_pan();
        assert!(!state.update_pan_offset(1.0, 1.0));
    }
    /// Proves resource-row selection changes only the explicit resource target and clears local point state.
    #[test]
    fn explicit_resource_selection_does_not_fall_back_to_the_first_resource() {
        let mut state = Stage20fEditorState {
            selected_structure: Some(AuthoredStructureId(1)),
            selected_node: Some(0),
            local_invalid: true,
            ..Default::default()
        };
        state.select_structure(AuthoredStructureId(9));
        assert_eq!(state.selected_structure, Some(AuthoredStructureId(9)));
        assert_eq!(state.selected_node, None);
        assert!(!state.local_invalid);
    }

    /// Proves Guide authoring uses one aspect-preserving frame transform and never clips outside geometry.
    #[test]
    fn guide_frame_uses_uniform_workspace_mapping_without_geometry_clipping() {
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Guide, 200.0, 100.0);
        state.set_guide_canvas_allocation(400.0, 400.0, 20.0);
        let frame = state.guide_frame().expect("Guide owns a workspace frame");
        assert!((frame.uniform_scale() - 1.8).abs() < 1.0e-12);
        assert_eq!(
            state.to_screen(Point2::new(-100.0, -50.0)),
            Point2::new(20.0, 110.0)
        );
        assert_eq!(
            state.to_document(Point2::new(200.0, 200.0)),
            Some(Point2::new(0.0, 0.0))
        );
        assert!(state.to_screen(Point2::new(140.0, 0.0)).x > 400.0);
    }

    /// Proves Guide endpoint guards share one pointer, numeric, and keyboard-nudge constraint.
    #[test]
    fn guide_endpoint_corner_guard_is_identical_for_numeric_nudge_and_drag() {
        let path = CurvePath::polyline(
            vec![Point2::new(-100.0, 0.0), Point2::new(100.0, 0.0)],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Guide, 200.0, 100.0);
        state.set_guide_canvas_allocation(400.0, 400.0, 20.0);
        let expected_y = state
            .guide_frame()
            .unwrap()
            .clamp_endpoint_y(-200.0, 8.0, 1.0);
        let numeric = state
            .commit_numeric(&path, NumericTarget::Anchor(0), "999", "-200")
            .unwrap();
        assert_eq!(numeric.segments()[0].start().x, -100.0);
        assert_eq!(numeric.segments()[0].start().y, expected_y);
        let nudge = state
            .nudge_selected(&path, NumericTarget::Anchor(0), 999.0, -250.0)
            .unwrap();
        assert_eq!(nudge.segments()[0].start().x, -100.0);
        assert_eq!(nudge.segments()[0].start().y, expected_y);
        state.begin_target_drag(path, NumericTarget::Anchor(0));
        let drag = state.end_drag(Point2::new(999.0, -200.0)).unwrap();
        assert_eq!(drag.segments()[0].start().x, -100.0);
        assert_eq!(drag.segments()[0].start().y, expected_y);
    }

    /// Proves Shape editing keeps closed seams, insertion, and the canonical two-node deletion floor.
    #[test]
    fn shape_policy_retains_closed_seam_and_canonical_minimum() {
        let path = CurvePath::polyline(
            vec![Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)],
            PathClosure::Closed,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Shape, 400.0, 300.0);
        state.selected_segment = Some(0);
        assert_eq!(
            state
                .edit_path(&path, PathEdit::InsertNode { parameter: 0.5 })
                .unwrap()
                .unwrap()
                .segments()
                .len(),
            3
        );
        assert!(matches!(
            state.edit_path(&path, PathEdit::DeleteNode { node: 0 }),
            Err(PathEditError::Geometry(error)) if error.path() == "curve.path.edit.node_minimum"
        ));
    }

    /// Proves a fresh Motif starts as an unmodified straight cubic in a centered tile-local frame.
    #[test]
    fn motif_begin_produces_straight_cubic_with_usable_tile_projection() {
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Motif, 500.0, 300.0);
        state.set_guide_canvas_allocation(220.0, 180.0, 12.0);
        state.begin(AuthoredStructureKind::OpenPath);
        let draft = state.complete().unwrap();
        assert!(matches!(
            draft.segments()[0],
            toniator_domain::AuthoredCurveSegment::CubicBezier { start, control_1, control_2, end }
                if start.x == 0.0 && start.y == 0.0
                    && end.x == 1.0 && end.y == 0.0
                    && control_1.x == 1.0 / 3.0 && control_1.y == 0.0
                    && control_2.x == 2.0 / 3.0 && control_2.y == 0.0
        ));
        assert!(
            state.to_screen(Point2::new(1.0, 0.0)).x - state.to_screen(Point2::new(0.0, 0.0)).x
                > 100.0
        );
        let start = state.to_screen(Point2::new(0.0, 0.0));
        let end = state.to_screen(Point2::new(1.0, 0.0));
        assert_eq!(start.y, end.y);
        assert!(
            (start.y - 90.0).abs() < 1.0e-12,
            "canonical tile endpoints present at the frame's vertical center"
        );
        assert_eq!(
            state.to_document(start),
            Some(Point2::new(0.0, 0.0)),
            "pointer inversion restores the canonical locked start endpoint"
        );
        assert_eq!(
            state.to_document(end),
            Some(Point2::new(1.0, 0.0)),
            "pointer inversion restores the canonical locked end endpoint"
        );
        let raised = state.to_screen(Point2::new(0.32, -0.27));
        let lowered = state.to_screen(Point2::new(0.70, 0.18));
        assert!(raised.y < start.y);
        assert!(lowered.y > start.y);
        assert_eq!(
            state.frame_screen_bounds(),
            Some((Point2::new(32.0, 12.0), Point2::new(188.0, 168.0))),
            "the tile frame surrounds rather than starts at the centered terminal line"
        );
    }

    /// Proves Motif terminals reject endpoint moves and deletion with exactly the same no-op payload rule.
    #[test]
    fn motif_endpoints_are_locked_for_numeric_nudge_drag_and_delete() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0 / 3.0, 0.0),
                    Point2::new(2.0 / 3.0, 0.0),
                    Point2::new(1.0, 0.0),
                )
                .unwrap(),
            )],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Motif, 1.0, 1.0);
        assert!(
            state
                .commit_numeric(&path, NumericTarget::Anchor(0), "9", "9")
                .is_none()
        );
        assert!(
            state
                .nudge_selected(&path, NumericTarget::Anchor(1), 2.0, 3.0)
                .is_none()
        );
        state.begin_target_drag(path.clone(), NumericTarget::Anchor(0));
        assert!(state.end_drag(Point2::new(5.0, 5.0)).is_none());
        assert_eq!(
            state.edit_path(&path, PathEdit::DeleteNode { node: 1 }),
            Err(PathEditError::EndpointLocked)
        );
    }

    /// Proves Smooth links terminal directions while preserving the opposite handle's independent length.
    #[test]
    fn motif_smooth_links_terminal_handles_without_persisting_or_open_mutation() {
        let path = CurvePath::new(
            vec![CurveSegment::CubicBezier(
                toniator_geometry::CubicBezierSegment::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(0.2, 0.0),
                    Point2::new(0.7, 0.0),
                    Point2::new(1.0, 0.0),
                )
                .unwrap(),
            )],
            PathClosure::Open,
        )
        .unwrap();
        let mut state = Stage20fEditorState::default();
        state.configure_mode(AuthoredPathEditorMode::Motif, 1.0, 1.0);
        state.inspect_motif_terminal_direction(&path);
        assert_eq!(
            state.motif_terminal_direction(&path),
            MotifTerminalDirection::Smooth
        );
        assert_eq!(path.segments()[0].start(), Point2::new(0.0, 0.0));
        let moved = state
            .commit_numeric(&path, NumericTarget::Control1(0), "0", "0.4")
            .unwrap();
        let toniator_domain::AuthoredCurveSegment::CubicBezier { control_2, .. } =
            moved.segments()[0]
        else {
            panic!("Motif remains cubic");
        };
        assert!(
            (Point2::new(1.0 - control_2.x, -control_2.y)
                .x
                .hypot(Point2::new(1.0 - control_2.x, -control_2.y).y)
                - 0.3)
                .abs()
                < 1.0e-12
        );
        assert!(control_2.y < 0.0);
        state.set_motif_terminal_direction(MotifTerminalDirection::Corner);
        let corner = state
            .commit_numeric(&path, NumericTarget::Control1(0), "0", "0.4")
            .unwrap();
        let toniator_domain::AuthoredCurveSegment::CubicBezier { control_2, .. } =
            corner.segments()[0]
        else {
            panic!("Motif remains cubic");
        };
        assert_eq!(control_2.x, 0.7);
        assert_eq!(control_2.y, 0.0);
    }
}
