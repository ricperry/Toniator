use std::collections::BTreeSet;
use toniator_domain::{AuthoredStructureDraft, AuthoredStructureId, AuthoredStructureKind};
use toniator_geometry::{CurveError, CurvePath, CurveSegment, PathClosure, PathLocation, Point2};

/// Widget-independent construction state for the Stage 20F private Pattern Editor.
///
/// This state owns only selection, viewport, and incomplete input. A completed path becomes an
/// ID-free draft consumed by the app's existing typed document-history command boundary.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Selection fields are consumed by the subsequent GTK projection.
pub(crate) struct Stage20fEditorState {
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
}

impl From<CurveError> for PathEditError {
    /// Preserves the canonical geometry diagnostic at the widget-independent editor boundary.
    fn from(value: CurveError) -> Self {
        Self::Geometry(value)
    }
}

#[allow(dead_code)] // GTK projection is introduced incrementally; state remains directly unit-testable.
impl Stage20fEditorState {
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
    /// Commits finite changed numeric coordinates as one replacement payload and treats equality as no-op.
    pub(crate) fn commit_numeric(
        &self,
        path: &CurvePath,
        target: NumericTarget,
        x: &str,
        y: &str,
    ) -> Option<AuthoredStructureDraft> {
        let point = Point2::new(x.parse().ok()?, y.parse().ok()?);
        let current = self.coordinate(path, target)?;
        if point == current {
            return None;
        }
        let path = match target {
            NumericTarget::Anchor(index) => path.move_anchor(index, point).ok()?,
            NumericTarget::Control1(index) => path.move_cubic_control(index, true, point).ok()?,
            NumericTarget::Control2(index) => path.move_cubic_control(index, false, point).ok()?,
        };
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
            PathEdit::DeleteNode { node } => path.delete_node(node)?,
        };
        Ok(Some(edited.to_authored_structure_draft()?))
    }
    /// Starts local construction without publishing a draft command.
    pub(crate) fn begin(&mut self, kind: AuthoredStructureKind) {
        self.local_invalid = false;
        self.construction = Some(Construction {
            kind,
            points: Vec::new(),
        });
    }
    /// Adds one finite document-space node without publishing a draft command.
    pub(crate) fn add_node(&mut self, point: Point2) -> Option<AuthoredStructureDraft> {
        let construction = self.construction.as_mut()?;
        if !point.is_finite() {
            return None;
        }
        construction.points.push(point);
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
        CurvePath::polyline(construction.points.clone(), closure)
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
        Point2::new(
            point.x * self.zoom + self.pan.x,
            point.y * self.zoom + self.pan.y,
        )
    }
    /// Converts finite screen coordinates to document coordinates through the current finite viewport inverse.
    pub(crate) fn to_document(&self, point: Point2) -> Option<Point2> {
        if !point.is_finite() || !self.pan.is_finite() || !self.zoom.is_finite() || self.zoom <= 0.0
        {
            return None;
        }
        let document = Point2::new(
            (point.x - self.pan.x) / self.zoom,
            (point.y - self.pan.y) / self.zoom,
        );
        document.is_finite().then_some(document)
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
        match target {
            NumericTarget::Anchor(index) => path.move_anchor(*index, point).ok(),
            NumericTarget::Control1(index) => path.move_cubic_control(*index, true, point).ok(),
            NumericTarget::Control2(index) => path.move_cubic_control(*index, false, point).ok(),
        }
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
}
