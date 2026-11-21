//! Shared mark silhouettes projected from typed document outputs and the personal-resource store.

use super::*;
use toniator_domain::{DocumentConfiguration, PersonalAuthoredResource, PersonalResourceId};
use toniator_io::personal_library::PersonalResourceSnapshot;

/// Retains scanned resources and explicit thumbnail identity within one Wizard lifetime.
/// Geometry and history remain authoritative; a selection survives only while its geometry matches.
#[derive(Default)]
pub(super) struct State {
    generation: Option<u64>,
    personal: Rc<Vec<PersonalResourceSnapshot>>,
    root: Option<PathBuf>,
    selections: BTreeMap<usize, Choice>,
    focus: Option<(usize, String)>,
    page_identity: Rc<()>,
}

/// Names one visible library tile independently from its possibly duplicated geometry.
#[derive(Clone)]
struct Choice {
    id: String,
    name: String,
    shape: Option<AuthoredStructureDraft>,
    adaptive: bool,
}

/// Captures the exact private draft and output addressed by a Shapes control.
#[derive(Clone)]
struct Target {
    epoch: u64,
    page_identity: Rc<()>,
    target: InspectorTarget,
    definition: Rc<toniator_domain::PatternDefinition>,
    output: PatternOutputLayerId,
    document: Rc<Document>,
}

impl Target {
    /// Admits current-page controls across scalar commits while rejecting replaced-page callbacks.
    /// # Errors
    /// Returns a stale page/scope diagnostic without document or filesystem mutation.
    fn validate_live(&self, state: &Rc<RefCell<AppState>>) -> Result<(), String> {
        let app = state.borrow();
        let wizard = app
            .pattern_wizard
            .as_ref()
            .filter(|wizard| {
                wizard.epoch == self.epoch
                    && wizard.target == self.target
                    && Rc::ptr_eq(&wizard.shapes.page_identity, &self.page_identity)
            })
            .ok_or_else(|| "The Shapes page changed. Use its current controls.".to_owned())?;
        if wizard.replacing_page {
            return Err("The Shapes page is being replaced.".into());
        }
        Ok(())
    }

    /// Rejects callbacks from replaced pages, closed wizards, and changed private drafts.
    ///
    /// # Errors
    /// Returns a visible stale-target diagnostic before document or filesystem mutation.
    fn validate(&self, state: &Rc<RefCell<AppState>>) -> Result<(), String> {
        let app = state.borrow();
        let wizard = app
            .pattern_wizard
            .as_ref()
            .filter(|wizard| wizard.epoch == self.epoch)
            .ok_or_else(|| "The Pattern Wizard is no longer open.".to_owned())?;
        if wizard.target != self.target
            || wizard.draft.borrow().document() != self.document.as_ref()
        {
            return Err("The Pattern changed. Reopen Shapes before trying again.".into());
        }
        Ok(())
    }

    /// Validates the invoking page before committing text, then recaptures its current typed slot.
    /// Numeric commits may allocate a named-channel definition copy; painter order resolves that
    /// documented ownership transition without accepting callbacks from an already stale page.
    ///
    /// # Errors
    /// Rejects stale origins, invalid pending text and incompatible post-commit output structure.
    fn after_pending(&self, state: &Rc<RefCell<AppState>>) -> Result<Self, String> {
        self.validate_live(state)?;
        if !commit_wizard_pending_text_inputs(state, self.epoch) {
            return Err("Complete the invalid Pattern setting before choosing a shape.".into());
        }
        let app = state.borrow();
        let wizard = app
            .pattern_wizard
            .as_ref()
            .filter(|wizard| wizard.epoch == self.epoch && wizard.target == self.target)
            .ok_or_else(|| "The Pattern target changed.".to_owned())?;
        let document = wizard.draft.borrow().document().clone();
        let definition_id = match self.target {
            InspectorTarget::DocumentAll => document.pattern_settings().definition_id,
            InspectorTarget::Channel(channel) => {
                document
                    .effective_channel_pattern(channel)
                    .map_err(|error| error.to_string())?
                    .definition_id
            }
        };
        let definition = document
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == definition_id)
            .map(|bundle| bundle.definition.clone())
            .ok_or_else(|| "The Pattern definition is unavailable.".to_owned())?;
        let ordinal = self
            .definition
            .output_layers
            .iter()
            .position(|output| output.id == self.output)
            .ok_or_else(|| "The mark target is unavailable.".to_owned())?;
        let output = definition
            .output_layers
            .get(ordinal)
            .filter(|output| {
                matches!(
                    output.realization,
                    PatternOutputRealization::CircularMarks { .. }
                        | PatternOutputRealization::MarkPrototype { .. }
                )
            })
            .ok_or_else(|| "The mark target changed.".to_owned())?
            .id;
        Ok(Self {
            epoch: self.epoch,
            page_identity: Rc::clone(&wizard.shapes.page_identity),
            target: self.target,
            definition: Rc::new(definition),
            output,
            document: Rc::new(document),
        })
    }

    /// Builds one domain-owned silhouette transition without changing the captured document.
    ///
    /// # Errors
    /// Returns stale output, topology, ownership or complete validation diagnostics.
    fn configuration(
        &self,
        shape: Option<&AuthoredStructureDraft>,
    ) -> Result<DocumentConfiguration, String> {
        self.document
            .edit_pattern_mark_shape_configuration(
                &self.definition,
                self.output,
                self.target.channel_id(),
                shape,
            )
            .map_err(|error| error.to_string())
    }
}

/// Lists immutable built-in names; their geometry remains ordinary authored data, not renderer keys.
const BUILT_INS: [&str; 5] = ["Circle", "Oval", "Square", "Triangle", "Star"];

/// Constructs a normalized editable silhouette; Circle selection itself retains analytic geometry.
///
/// Four cubic sections approximate an editable ellipse using the standard tangent construction.
/// Polygon coordinates and cubic controls remain explicit authored geometry in projects/Patterns.
///
/// # Panics
/// Panics only when the fixed built-in geometry violates the domain's closed-path invariant.
fn built_in(name: &str) -> AuthoredStructureDraft {
    let point = |x, y| AuthoredPoint2 { x, y };
    let segments = if matches!(name, "Circle" | "Oval") {
        let y = if name == "Oval" { 0.62 } else { 1.0 };
        let k = 4.0 * (2.0_f64.sqrt() - 1.0) / 3.0;
        vec![
            AuthoredCurveSegment::CubicBezier {
                start: point(1.0, 0.0),
                control_1: point(1.0, k * y),
                control_2: point(k, y),
                end: point(0.0, y),
            },
            AuthoredCurveSegment::CubicBezier {
                start: point(0.0, y),
                control_1: point(-k, y),
                control_2: point(-1.0, k * y),
                end: point(-1.0, 0.0),
            },
            AuthoredCurveSegment::CubicBezier {
                start: point(-1.0, 0.0),
                control_1: point(-1.0, -k * y),
                control_2: point(-k, -y),
                end: point(0.0, -y),
            },
            AuthoredCurveSegment::CubicBezier {
                start: point(0.0, -y),
                control_1: point(k, -y),
                control_2: point(1.0, -k * y),
                end: point(1.0, 0.0),
            },
        ]
    } else {
        let points = match name {
            "Square" => vec![
                point(-1.0, -1.0),
                point(1.0, -1.0),
                point(1.0, 1.0),
                point(-1.0, 1.0),
            ],
            "Triangle" => vec![
                point(0.0, -1.0),
                point(0.8660254037844386, 0.5),
                point(-0.8660254037844386, 0.5),
            ],
            _ => (0..10)
                .map(|index| {
                    let angle =
                        -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::PI / 5.0;
                    let radius = if index % 2 == 0 { 1.0 } else { 0.42 };
                    point(radius * angle.cos(), radius * angle.sin())
                })
                .collect(),
        };
        (0..points.len())
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % points.len()],
            })
            .collect()
    };
    AuthoredStructureDraft::new(AuthoredStructureKind::ClosedShape, segments)
        .expect("fixed built-in closed geometry is valid")
}

/// Resolves the current silhouette, using `None` for analytic Circle or either adaptive policy.
///
/// # Errors
/// Returns a missing-resource or non-mark diagnostic instead of inventing replacement geometry.
fn current_shape(target: &Target) -> Result<Option<AuthoredStructureDraft>, String> {
    let output = target
        .definition
        .output_layers
        .iter()
        .find(|output| output.id == target.output)
        .ok_or_else(|| "This mark output is no longer available.".to_owned())?;
    match output.realization {
        PatternOutputRealization::MarkPrototype { prototype: MarkPrototype::AdaptiveSquare | MarkPrototype::AdaptiveSquareBlueNoise, .. } => Ok(None),
        PatternOutputRealization::CircularMarks { .. }
        | PatternOutputRealization::MarkPrototype {
            prototype: MarkPrototype::Circle,
            ..
        } => Ok(None),
        PatternOutputRealization::MarkPrototype {
            prototype: MarkPrototype::AuthoredClosedShape { structure_id },
            ..
        } => {
            let structure = target
                .document
                .authored_structure(structure_id)
                .ok_or_else(|| "The current shape is missing.".to_owned())?;
            AuthoredStructureDraft::new(structure.kind(), structure.segments().to_vec())
                .map(Some)
                .map_err(|error| error.to_string())
        }
        _ => Err("This output does not draw shapes.".into()),
    }
}

/// Reports differing effective silhouettes for ALL using the same compatible mark ordinal as edits.
/// Independent arrangement differences alone do not make a shared silhouette Mixed.
fn mixed_shapes(target: &Target) -> bool {
    if target.target != InspectorTarget::DocumentAll {
        return false;
    }
    let Ok(base_shape) = current_shape(target) else {
        return false;
    };
    let ordinal = target
        .definition
        .output_layers
        .iter()
        .take_while(|output| output.id != target.output)
        .filter(|output| {
            matches!(
                output.realization,
                PatternOutputRealization::CircularMarks { .. }
                    | PatternOutputRealization::MarkPrototype { .. }
            )
        })
        .count();
    let mut definitions = BTreeSet::new();
    for channel in authoritative_channel_ids(&target.document) {
        let Ok(effective) = target.document.effective_channel_pattern(channel) else {
            continue;
        };
        if !definitions.insert(effective.definition_id) {
            continue;
        }
        let Some(definition) = target
            .document
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == effective.definition_id)
            .map(|bundle| &bundle.definition)
        else {
            continue;
        };
        let Some(output) = definition
            .output_layers
            .iter()
            .filter(|output| {
                matches!(
                    output.realization,
                    PatternOutputRealization::CircularMarks { .. }
                        | PatternOutputRealization::MarkPrototype { .. }
                )
            })
            .nth(ordinal)
        else {
            continue;
        };
        let other = Target {
            definition: Rc::new(definition.clone()),
            output: output.id,
            ..target.clone()
        };
        if other.definition.uses_adaptive_square_marks() != target.definition.uses_adaptive_square_marks() || current_shape(&other).ok() != Some(base_shape.clone()) {
            return true;
        }
    }
    false
}

/// Draws a neutral thumbnail from exact authored segments, retaining each curve and concavity.
/// The image is decorative; its owning native control supplies name and selected state.
fn thumbnail(shape: Option<AuthoredStructureDraft>, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .can_target(false)
        .build();
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    area.set_draw_func(move |area, context, width, height| {
        let color = area.color();
        context.set_source_rgba(
            color.red().into(),
            color.green().into(),
            color.blue().into(),
            color.alpha().into(),
        );
        if let Some(shape) = shape.as_ref() {
            let points = shape
                .segments()
                .iter()
                .flat_map(|segment| match segment {
                    AuthoredCurveSegment::Line { start, end } => [*start, *start, *end, *end],
                    AuthoredCurveSegment::CubicBezier {
                        start,
                        control_1,
                        control_2,
                        end,
                    } => [*start, *control_1, *control_2, *end],
                })
                .collect::<Vec<_>>();
            let min_x = points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min);
            let max_x = points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_y = points
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min);
            let max_y = points
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max);
            let extent = (max_x - min_x).max(max_y - min_y).max(f64::EPSILON);
            let scale = f64::from(width.min(height) - 12).max(1.0) / extent;
            context.translate(f64::from(width) / 2.0, f64::from(height) / 2.0);
            context.scale(scale, scale);
            context.translate(-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0);
            for (index, segment) in shape.segments().iter().enumerate() {
                let start = match segment {
                    AuthoredCurveSegment::Line { start, .. }
                    | AuthoredCurveSegment::CubicBezier { start, .. } => start,
                };
                if index == 0 {
                    context.move_to(start.x, start.y);
                }
                match segment {
                    AuthoredCurveSegment::Line { end, .. } => context.line_to(end.x, end.y),
                    AuthoredCurveSegment::CubicBezier {
                        control_1,
                        control_2,
                        end,
                        ..
                    } => context.curve_to(
                        control_1.x,
                        control_1.y,
                        control_2.x,
                        control_2.y,
                        end.x,
                        end.y,
                    ),
                }
            }
            context.close_path();
        } else {
            context.arc(
                f64::from(width) / 2.0,
                f64::from(height) / 2.0,
                f64::from(width.min(height) - 12).max(1.0) / 2.0,
                0.0,
                std::f64::consts::TAU,
            );
        }
        context.set_fill_rule(gtk::cairo::FillRule::EvenOdd);
        let _ = context.fill();
    });
    area
}

/// Draws the strategy's five full-cell silhouettes using native Cairo paths and exact quarter arcs.
/// This decorative preview shares product vocabulary with its enclosing native toggle button.
fn adaptive_thumbnail(size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .can_target(false)
        .build();
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    area.set_draw_func(|area, context, width, height| {
        let color = area.color();
        context.set_source_rgba(
            color.red().into(),
            color.green().into(),
            color.blue().into(),
            color.alpha().into(),
        );
        let cell = f64::from(width.min(height) - 8) / 3.0;
        context.translate(4.0, 4.0);
        context.scale(cell, cell);
        // Segment, right triangle, sector, disk and square retain their common unit cell.
        context.arc(0.0, 1.0, 1.0, -std::f64::consts::FRAC_PI_2, 0.0);
        context.close_path();
        let _ = context.fill();
        context.move_to(1.0, 0.0);
        context.line_to(1.0, 1.0);
        context.line_to(2.0, 1.0);
        context.close_path();
        let _ = context.fill();
        context.move_to(2.0, 1.0);
        context.arc(2.0, 1.0, 1.0, -std::f64::consts::FRAC_PI_2, 0.0);
        context.close_path();
        let _ = context.fill();
        context.arc(0.5, 1.7, 0.5, 0.0, std::f64::consts::TAU);
        let _ = context.fill();
        context.rectangle(1.2, 1.2, 1.0, 1.0);
        let _ = context.fill();
    });
    area
}

/// Appends native thumbnail selectors and explicit copy/edit/save actions for every mark output.
/// Domain output identities supply scope; labels and thumbnails never become persistence authority.
pub(super) fn append(
    state: &Rc<RefCell<AppState>>,
    epoch: u64,
    page: &gtk::Box,
    document: &Document,
    scope: InspectorTarget,
) -> usize {
    let definition_id = match scope {
        InspectorTarget::DocumentAll => document.pattern_settings().definition_id,
        InspectorTarget::Channel(channel) => match document.effective_channel_pattern(channel) {
            Ok(value) => value.definition_id,
            Err(_) => return 0,
        },
    };
    let Some(definition) = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .map(|bundle| bundle.definition.clone())
    else {
        return 0;
    };
    let (generation, library_root) = {
        let app = state.borrow();
        (
            app.personal_catalog_generation,
            app.personal_library
                .as_ref()
                .map(|library| library.root().to_path_buf()),
        )
    };
    let needs_scan = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .is_some_and(|wizard| wizard.shapes.generation != Some(generation));
    if needs_scan {
        let entries = active_personal_library(state)
            .and_then(|library| library.scan().map_err(|error| error.to_string()))
            .map(|snapshot| snapshot.shapes)
            .unwrap_or_default();
        if let Some(wizard) = state
            .borrow_mut()
            .pattern_wizard
            .as_mut()
            .filter(|wizard| wizard.epoch == epoch)
        {
            wizard.shapes.personal = Rc::new(entries);
            wizard.shapes.root = library_root;
            wizard.shapes.generation = Some(generation);
        }
    }
    let (personal, root, selections, focus, page_identity) = {
        let mut app = state.borrow_mut();
        let Some(wizard) = app
            .pattern_wizard
            .as_mut()
            .filter(|wizard| wizard.epoch == epoch)
        else {
            return 0;
        };
        wizard.shapes.page_identity = Rc::new(());
        (
            Rc::clone(&wizard.shapes.personal),
            wizard.shapes.root.clone(),
            wizard.shapes.selections.clone(),
            wizard.shapes.focus.take(),
            Rc::clone(&wizard.shapes.page_identity),
        )
    };
    let definition = Rc::new(definition);
    let document = Rc::new(document.clone());
    let mut count = 0;
    for (index, output) in definition.output_layers.iter().enumerate() {
        if !matches!(
            output.realization,
            PatternOutputRealization::CircularMarks { .. }
                | PatternOutputRealization::MarkPrototype { .. }
        ) {
            continue;
        }
        let target = Target {
            epoch,
            page_identity: Rc::clone(&page_identity),
            target: scope,
            definition: Rc::clone(&definition),
            output: output.id,
            document: Rc::clone(&document),
        };
        let Ok(current) = current_shape(&target) else {
            continue;
        };
        let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
        group.set_accessible_role(gtk::AccessibleRole::Group);
        let title = if definition.output_layers.len() == 1 {
            "Shapes".to_owned()
        } else {
            format!("Drawing layer {} — Shapes", index + 1)
        };
        group.update_property(&[gtk::accessible::Property::Label(&title)]);
        let heading = gtk::Label::new(Some(&title));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");
        group.append(&heading);
        let adaptive = definition.uses_adaptive_square_marks();
        let mut builtins = BUILT_INS
            .iter()
            .map(|name| Choice {
                id: format!("built-in:{name}"),
                name: (*name).to_owned(),
                shape: (*name != "Circle").then(|| built_in(name)),
                adaptive: false,
            })
            .collect::<Vec<_>>();
        builtins.push(Choice {
            id: "built-in:adaptive-square".into(),
            name: "Adaptive Shape Marks".into(),
            shape: None,
            adaptive: true,
        });
        let my_shapes = personal
            .iter()
            .map(|entry| Choice {
                id: entry.resource.id().as_str().to_owned(),
                name: entry.resource.name().to_owned(),
                shape: Some(entry.resource.draft().clone()),
                adaptive: false,
            })
            .collect::<Vec<_>>();
        let retained = selections
            .get(&index)
            .filter(|choice| choice.shape == current && choice.adaptive == adaptive);
        let selected = retained
            .and_then(|retained| {
                builtins.iter().chain(&my_shapes).find(|choice| {
                    choice.id == retained.id
                        && choice.shape == current
                        && choice.adaptive == adaptive
                })
            })
            .or_else(|| {
                builtins
                    .iter()
                    .chain(&my_shapes)
                    .find(|choice| choice.shape == current && choice.adaptive == adaptive)
            });
        let mixed = mixed_shapes(&target);
        let selected_id = (!mixed)
            .then(|| selected.map(|choice| choice.id.as_str()))
            .flatten();
        let selected_name = selected.map_or("Document shape", |choice| choice.name.as_str());
        let preview = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        preview.append(&if adaptive {
            adaptive_thumbnail(64)
        } else {
            thumbnail(current.clone(), 64)
        });
        let readback = gtk::Label::new(Some(&if mixed {
            format!("Current shape: Mixed · base shape: {selected_name}")
        } else {
            format!("Current shape: {selected_name}")
        }));
        readback.set_xalign(0.0);
        readback.set_hexpand(true);
        readback.set_wrap(true);
        preview.append(&readback);
        group.append(&preview);
        for (heading, entries) in [("Built-in", &builtins), ("My Shapes", &my_shapes)] {
            let label = gtk::Label::new(Some(heading));
            label.set_xalign(0.0);
            group.append(&label);
            let flow = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .focusable(false)
                .max_children_per_line(5)
                .min_children_per_line(1)
                .column_spacing(6)
                .row_spacing(6)
                .homogeneous(true)
                .build();
            for choice in entries {
                let Choice { name, shape, .. } = choice;
                let button = gtk::ToggleButton::new();
                button.set_active(selected_id == Some(choice.id.as_str()));
                let content = gtk::Box::new(gtk::Orientation::Vertical, 3);
                content.append(&if choice.adaptive {
                    adaptive_thumbnail(48)
                } else {
                    thumbnail(shape.clone(), 48)
                });
                if choice.adaptive {
                    let eligible = target
                        .document
                        .edit_pattern_adaptive_square_configuration(
                            &target.definition,
                            target.output,
                            target.target.channel_id(),
                        )
                        .is_ok();
                    button.set_sensitive(eligible);
                    button.set_tooltip_text(Some("Selects full-cell silhouettes by nearest ink area and overlap. Requires two perpendicular straight Guides at equal pitch."));
                }
                let label = gtk::Label::new(Some(name));
                label.set_wrap(true);
                label.set_max_width_chars(14);
                content.append(&label);
                button.set_child(Some(&content));
                button.update_property(&[gtk::accessible::Property::Label(name), gtk::accessible::Property::Description(if choice.adaptive { "Choose a fixed library of full-cell shapes by nearest channel ink area and overlap. Requires square Guides; the strategy controls pose and size." } else { "Choose this silhouette for the Pattern draft. The library original remains unchanged." })]);
                let state = Rc::clone(state);
                let target = target.clone();
                let choice_for_click = choice.clone();
                let projected_pressed = selected_id == Some(choice.id.as_str());
                button.connect_clicked(move |button| {
                    button.set_active(projected_pressed);
                    select(&state, &target, index, &choice_for_click);
                });
                flow.insert(&button, -1);
                if let Some(child) = button.parent().and_downcast::<gtk::FlowBoxChild>() {
                    child.set_focusable(false);
                }
                if focus
                    .as_ref()
                    .is_some_and(|(slot, id)| *slot == index && id == &choice.id)
                {
                    let button = button.clone();
                    glib::idle_add_local_once(move || {
                        if button.root().is_some() {
                            button.grab_focus();
                        }
                    });
                }
            }
            group.append(&flow);
            if heading == "My Shapes" && personal.is_empty() {
                let empty = gtk::Label::new(Some(
                    "Save a named copy here to reuse it in other Patterns.",
                ));
                empty.set_xalign(0.0);
                empty.set_wrap(true);
                group.append(&empty);
            }
        }
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        for (label, new) in [("New Shape…", true), ("Edit Shape…", false)] {
            let button = gtk::Button::with_label(label);
            if !new && adaptive {
                button.set_sensitive(false);
                button.set_tooltip_text(Some("Adaptive Shape Marks use fixed cell silhouettes. Choose an individual shape to edit its outline."));
            }
            let state_for_editor = Rc::clone(state);
            let target = target.clone();
            button.connect_clicked(move |button| {
                open_editor(&state_for_editor, &target, new, button.clone().upcast())
            });
            if !new {
                register_wizard_nested_focus_control(
                    state,
                    epoch,
                    NestedEditorFocusToken {
                        wizard_epoch: epoch,
                        target: NestedEditorFocusTarget::Shape {
                            output_layer_id: output.id,
                        },
                    },
                    button.clone().upcast(),
                );
            }
            actions.append(&button);
        }
        for (label, update) in [("Save to My Shapes…", false), ("Update My Shape…", true)] {
            let button = gtk::Button::with_label(label);
            button.set_sensitive(!adaptive && root.is_some() && (!update || !personal.is_empty()));
            button.set_tooltip_text(Some(if update { "Explicitly replace a chosen personal master with this shape. Projects keep their embedded copies." } else { "Save a separate named library copy. This does not apply the Pattern." }));
            let state = Rc::clone(state);
            let target = target.clone();
            let personal = Rc::clone(&personal);
            let root = root.clone();
            button.connect_clicked(move |_| {
                save_dialog(&state, &target, update, &personal, root.as_deref())
            });
            actions.append(&button);
        }
        let refresh = gtk::Button::with_label("Refresh My Shapes");
        let state_for_refresh = Rc::clone(state);
        let target_for_refresh = target.clone();
        refresh.connect_clicked(move |_| {
            let result = target_for_refresh
                .validate_live(&state_for_refresh)
                .and_then(|()| refresh_personal_catalog(&state_for_refresh));
            match result {
                Ok(()) => show_wizard_current_route_page(&state_for_refresh, epoch),
                Err(error) => set_wizard_input_error(&state_for_refresh, epoch, &error),
            }
        });
        actions.append(&refresh);
        group.append(&actions);
        page.append(&group);
        count += 1;
    }
    count
}

/// Publishes a chosen silhouette as one private history entry and refreshes ordinary preview routing.
fn select(state: &Rc<RefCell<AppState>>, target: &Target, index: usize, choice: &Choice) {
    let refreshed = match target.after_pending(state) {
        Ok(target) => target,
        Err(error) => {
            set_wizard_input_error(state, target.epoch, &error);
            return;
        }
    };
    let target = &refreshed;
    let result = (|| -> Result<(), String> {
        target.validate(state)?;
        let configuration = if choice.adaptive {
            target
                .document
                .edit_pattern_adaptive_square_configuration(
                    &target.definition,
                    target.output,
                    target.target.channel_id(),
                )
                .map_err(|error| error.to_string())?
        } else {
            target.configuration(choice.shape.as_ref())?
        };
        let app = state.borrow();
        let wizard = app
            .pattern_wizard
            .as_ref()
            .ok_or_else(|| "The Pattern Wizard closed.".to_owned())?;
        let mut draft = wizard.draft.borrow_mut();
        let revision = draft.revision();
        draft
            .apply_document_configuration(&target.document, revision, &configuration)
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            if let Some(wizard) = state
                .borrow_mut()
                .pattern_wizard
                .as_mut()
                .filter(|wizard| wizard.epoch == target.epoch)
            {
                wizard.shapes.selections.insert(index, choice.clone());
                wizard.shapes.focus = Some((index, choice.id.clone()));
            }
            let _ = refresh_wizard_route(state, target.epoch);
            show_wizard_current_route_page(state, target.epoch);
            submit_wizard_preview(state, target.epoch);
        }
        Err(error) => set_wizard_input_error(state, target.epoch, &error),
    }
}

/// Discloses deliberate Shape child edits while treating its untouched preparation as disposable.
/// The parent baseline still controls Apply readiness and history; this predicate affects only
/// discard confirmation. Undo to either the prepared or exact parent snapshot closes directly.
pub(super) fn discard_requires_confirmation(
    current: &Document,
    parent: &Document,
    prepared: &Document,
) -> bool {
    current != parent && current != prepared
}

/// Opens the existing closed-path editor with conversion confined to its cancelable child history.
/// New starts from a fresh editable triangle; editing Circle materializes only a child-local cubic.
fn open_editor(
    state: &Rc<RefCell<AppState>>,
    target: &Target,
    new: bool,
    invoking_control: gtk::Widget,
) {
    let refreshed = match target.after_pending(state) {
        Ok(target) => target,
        Err(error) => {
            set_wizard_input_error(state, target.epoch, &error);
            return;
        }
    };
    let target = &refreshed;
    let result = (|| -> Result<PatternEditorLaunch, String> {
        target.validate(state)?;
        let (configuration, use_value) = editor_preparation(target, new)?;
        let structure_id = use_value.structure_id();
        Ok(PatternEditorLaunch {
            parent: PatternEditorParent::Wizard {
                epoch: target.epoch,
                focus_token: nested_editor_focus_token(target.epoch, use_value),
                invoking_control,
            },
            selected_channel: authored_use_channel(&use_value),
            selected_structure: Some(structure_id),
            invoking_use: Some(use_value),
            preparation_command: None,
            preparation_configuration: configuration,
        })
    })();
    match result {
        Ok(launch) => open_pattern_editor_with_launch(state, PatternEditorPurpose::Shape, launch),
        Err(error) => set_wizard_input_error(state, target.epoch, &error),
    }
}

/// Resolves the post-allocation output by painter slot, preserving named-channel copy ownership.
/// Preparation remains disposable until child Apply; analytic Circle conversion never edits a parent.
///
/// # Errors
/// Returns missing output/resource, domain configuration or stale typed-use diagnostics.
fn editor_preparation(
    target: &Target,
    new: bool,
) -> Result<(Option<DocumentConfiguration>, AuthoredStructureUse), String> {
    let current = current_shape(target)?;
    let (configuration, document) = if new || current.is_none() {
        let shape = if new {
            built_in("Triangle")
        } else {
            built_in("Circle")
        };
        let configuration = target.configuration(Some(&shape))?;
        let document = configuration
            .bind(&target.document)
            .map_err(|error| error.to_string())?;
        (Some(configuration), document)
    } else {
        (None, target.document.as_ref().clone())
    };
    let definition_id = match target.target {
        InspectorTarget::DocumentAll => document.pattern_settings().definition_id,
        InspectorTarget::Channel(channel) => {
            document
                .effective_channel_pattern(channel)
                .map_err(|error| error.to_string())?
                .definition_id
        }
    };
    let ordinal = target
        .definition
        .output_layers
        .iter()
        .position(|output| output.id == target.output)
        .ok_or_else(|| "The mark output is unavailable.".to_owned())?;
    let output = document
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == definition_id)
        .and_then(|bundle| bundle.definition.output_layers.get(ordinal))
        .ok_or_else(|| "The prepared shape is unavailable.".to_owned())?;
    let PatternOutputRealization::MarkPrototype {
        prototype: MarkPrototype::AuthoredClosedShape { structure_id },
        ..
    } = output.realization
    else {
        return Err("The prepared mark has no editable shape.".into());
    };
    let focus = NestedEditorFocusTarget::Shape {
        output_layer_id: output.id,
    };
    let use_value = wizard_authored_use(&document, target.target, focus, structure_id)
        .ok_or_else(|| "The shape target changed.".to_owned())?;
    Ok((configuration, use_value))
}

/// Opens an explicit named-copy or chosen-master update dialog with captured draft/root/fingerprints.
/// Cancel has no filesystem or document effect; successful writes rescan the active library.
///
/// # Panics
/// Panics only if the existing frontend ID allocator violates canonical personal-ID spelling.
fn save_dialog(
    state: &Rc<RefCell<AppState>>,
    target: &Target,
    update: bool,
    personal: &[PersonalResourceSnapshot],
    root: Option<&Path>,
) {
    let refreshed = match target.after_pending(state) {
        Ok(target) => target,
        Err(error) => {
            set_wizard_input_error(state, target.epoch, &error);
            return;
        }
    };
    let target = &refreshed;
    let Ok(shape) = current_shape(target).map(|shape| shape.unwrap_or_else(|| built_in("Circle")))
    else {
        return;
    };
    let parent = state
        .borrow()
        .pattern_wizard
        .as_ref()
        .map(|wizard| wizard.window.clone());
    let Some(parent) = parent else {
        return;
    };
    let dialog = gtk::Window::builder()
        .title(if update {
            "Update My Shape"
        } else {
            "Save Shape to My Shapes"
        })
        .transient_for(&parent)
        .destroy_with_parent(true)
        .modal(true)
        .accessible_role(gtk::AccessibleRole::Dialog)
        .default_width(380)
        .build();
    let content = components::ToniatorConfirmationContent::new();
    content.set_detail(if update { "Replace the chosen personal master with the current shape. Existing projects and Patterns keep their embedded geometry. External changes refuse the update; refresh or save a copy." } else { "Save a named, reusable copy of the current shape. Applying the Pattern remains a separate action." });
    let name = gtk::Entry::new();
    name.update_property(&[gtk::accessible::Property::Label("Shape name")]);
    let names = personal
        .iter()
        .map(|entry| entry.resource.name())
        .collect::<Vec<_>>();
    let choice = accessible_string_dropdown(&names);
    choice.update_property(&[gtk::accessible::Property::Label("My Shape to update")]);
    let label = gtk::Label::new(Some(if update {
        "My Shape to update"
    } else {
        "Shape name"
    }));
    label.set_xalign(0.0);
    label.set_mnemonic_widget(Some(if update {
        choice.upcast_ref::<gtk::Widget>()
    } else {
        name.upcast_ref::<gtk::Widget>()
    }));
    content.append(&label);
    if update {
        content.append(&choice);
    } else {
        content.append(&name);
    }
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    content.append(&status);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label(if update { "Update Shape" } else { "Save Shape" });
    save.set_sensitive(update && !personal.is_empty());
    if !update {
        let save = save.clone();
        name.connect_changed(move |name| save.set_sensitive(!name.text().trim().is_empty()));
    }
    if update {
        save.add_css_class("destructive-action");
    } else {
        save.add_css_class("suggested-action");
    }
    actions.append(&cancel);
    actions.append(&save);
    content.append(&actions);
    dialog.set_child(Some(&content));
    let cancel_dialog = dialog.clone();
    cancel.connect_clicked(move |_| cancel_dialog.close());
    let state = Rc::clone(state);
    let target = target.clone();
    let root = root.map(Path::to_path_buf);
    let personal = personal.to_vec();
    let saved_dialog = dialog.clone();
    let name_for_save = name.clone();
    let choice_for_save = choice.clone();
    save.connect_clicked(move |button| {
        let result = (|| -> Result<(), String> {
            target.validate(&state)?;
            let library = active_personal_library(&state)?;
            if root.as_deref() != Some(library.root()) {
                return Err(
                    "The library folder changed. Close this dialog and refresh My Shapes.".into(),
                );
            }
            let selected = if update {
                Some(
                    personal
                        .get(choice_for_save.selected() as usize)
                        .ok_or_else(|| "Choose a personal shape to update.".to_owned())?,
                )
            } else {
                None
            };
            let id = selected
                .map(|entry| entry.resource.id().clone())
                .unwrap_or_else(|| {
                    PersonalResourceId::new(fresh_personal_pattern_id(&state.borrow().catalog))
                        .expect("fresh canonical ID")
                });
            let name = selected
                .map(|entry| entry.resource.name().to_owned())
                .unwrap_or_else(|| name_for_save.text().trim().to_owned());
            if BUILT_INS
                .iter()
                .any(|built_in| built_in.eq_ignore_ascii_case(&name))
            {
                return Err("Choose a name different from the built-in shapes.".into());
            }
            let resource = PersonalAuthoredResource::new(
                id,
                name,
                PersonalAuthoredResourceKind::Shape,
                shape.clone(),
            )
            .map_err(|error| error.to_string())?;
            library
                .write_resource(&resource, selected.map(|entry| &entry.fingerprint))
                .map_err(|error| format!("{error}. Refresh My Shapes or save a named copy."))?;
            button.set_sensitive(false);
            refresh_personal_catalog(&state)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                saved_dialog.close();
                show_wizard_current_route_page(&state, target.epoch);
            }
            Err(error) => status.set_label(&error),
        }
    });
    dialog.present();
    if update {
        choice.grab_focus();
    } else {
        name.grab_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies shared RGB named-channel conversion resolves fresh IDs and never changes its parent.
    ///
    /// # Panics
    /// Panics on lost typed target, incorrect source ownership or non-isolated preparation.
    #[test]
    fn shape_library_named_shared_circle_editor_preparation_is_cancelable_and_targeted() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let channel = authoritative_channel_ids(&document)[0];
        let definition = document.pattern_definition_for(channel).unwrap().clone();
        let target = Target {
            epoch: 1,
            page_identity: Rc::new(()),
            target: InspectorTarget::Channel(channel),
            output: definition.output_layers[0].id,
            definition: Rc::new(definition),
            document: Rc::new(document.clone()),
        };
        for new in [false, true] {
            let (configuration, use_value) = editor_preparation(&target, new).unwrap();
            let prepared = configuration.unwrap().bind(&document).unwrap();
            assert_eq!(authored_use_channel(&use_value), channel);
            assert!(
                prepared
                    .authored_structure(use_value.structure_id())
                    .is_some()
            );
            assert_eq!(
                target.document.as_ref(),
                &document,
                "Cancel drops preparation and preserves analytic parent"
            );
            assert!(matches!(
                document
                    .pattern_definition_for(channel)
                    .unwrap()
                    .output_layers[0]
                    .realization,
                PatternOutputRealization::CircularMarks { .. }
                    | PatternOutputRealization::MarkPrototype {
                        prototype: MarkPrototype::Circle,
                        ..
                    }
            ));
            for other in authoritative_channel_ids(&document)
                .into_iter()
                .filter(|other| *other != channel)
            {
                assert_eq!(
                    prepared.pattern_definition_for(other),
                    document.pattern_definition_for(other)
                );
            }
        }
    }

    /// Distinguishes untouched preparation, actual geometry edits and undone changes at Cancel.
    /// Explicit Apply still publishes untouched preparation as one parent step; Cancel does not.
    ///
    /// # Panics
    /// Panics on failed canonical preparation, local curve edit or history isolation regression.
    #[test]
    fn shape_library_cancel_ignores_preparation_but_confirms_deliberate_edits() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let channel = authoritative_channel_ids(&document)[0];
        for scope in [
            InspectorTarget::DocumentAll,
            InspectorTarget::Channel(channel),
        ] {
            let definition = document.pattern_definition_for(channel).unwrap().clone();
            let target = Target {
                epoch: 1,
                page_identity: Rc::new(()),
                target: scope,
                output: definition.output_layers[0].id,
                definition: Rc::new(definition),
                document: Rc::new(document.clone()),
            };
            for new in [false, true] {
                let (configuration, use_value) = editor_preparation(&target, new).unwrap();
                let parent = DocumentHistory::new(DocumentSession::new(document.clone()).unwrap());
                let mut child = DocumentHistory::new_draft(&parent);
                child
                    .apply_document_configuration(
                        &document,
                        child.revision(),
                        &configuration.unwrap(),
                    )
                    .unwrap();
                let prepared = child.document().clone();
                assert!(!discard_requires_confirmation(
                    child.document(),
                    &document,
                    &prepared
                ));
                assert!(
                    wizard_apply_enabled(child.document(), &document),
                    "untouched preparation stays applyable"
                );
                assert_eq!(
                    parent.document(),
                    &document,
                    "Cancel never publishes preparation"
                );
                let mut apply_parent = DocumentHistory::new_draft(&parent);
                assert!(!apply_parent.squash_draft(&child).unwrap().unchanged);
                assert_eq!(apply_parent.document(), &prepared);
                let structure = child
                    .document()
                    .authored_structure(use_value.structure_id())
                    .unwrap()
                    .clone();
                child
                    .apply(&DocumentCommand::ReplaceAuthoredStructure {
                        base_structure: structure,
                        replacement: built_in("Square"),
                    })
                    .unwrap();
                assert!(discard_requires_confirmation(
                    child.document(),
                    &document,
                    &prepared
                ));
                child.undo().unwrap();
                assert!(!discard_requires_confirmation(
                    child.document(),
                    &document,
                    &prepared
                ));
                child.undo().unwrap();
                assert_eq!(child.document(), &document);
                assert!(!discard_requires_confirmation(
                    child.document(),
                    &document,
                    &prepared
                ));
                assert!(!parent.can_undo());
            }
        }
    }

    /// Reports differing named-channel silhouettes as Mixed and clears it after ALL equalization.
    ///
    /// # Panics
    /// Panics on lost base identity, incorrect compatible-slot mapping or nonportable authored data.
    #[test]
    fn shape_library_all_readback_tracks_mixed_channel_silhouettes() {
        let document = Document::new_default_document(
            CanvasSpec {
                width: 120.0,
                height: 80.0,
            },
            SourceReference::Unassigned,
        )
        .unwrap();
        let channel = authoritative_channel_ids(&document)[0];
        let definition = document.pattern_definition_for(channel).unwrap().clone();
        let mut target = Target {
            epoch: 1,
            page_identity: Rc::new(()),
            target: InspectorTarget::Channel(channel),
            output: definition.output_layers[0].id,
            definition: Rc::new(definition),
            document: Rc::new(document.clone()),
        };
        let changed = target
            .configuration(Some(&built_in("Star")))
            .unwrap()
            .bind(&document)
            .unwrap();
        target.target = InspectorTarget::DocumentAll;
        target.document = Rc::new(changed);
        assert!(mixed_shapes(&target));
        let equalized = target
            .configuration(None)
            .unwrap()
            .bind(&target.document)
            .unwrap();
        target.document = Rc::new(equalized);
        assert!(!mixed_shapes(&target));
    }

    /// Verifies built-ins retain explicit closed geometry and editing Circle yields smooth cubics.
    ///
    /// # Panics
    /// Panics if fixed silhouette topology or the explicit cubic Circle conversion changes.
    #[test]
    fn shape_library_built_ins_are_closed_and_circle_conversion_is_explicit() {
        for name in BUILT_INS {
            let shape = built_in(name);
            assert_eq!(shape.kind(), AuthoredStructureKind::ClosedShape);
            assert_eq!(
                shape.segments().len(),
                match name {
                    "Circle" | "Oval" | "Square" => 4,
                    "Triangle" => 3,
                    _ => 10,
                }
            );
        }
        assert!(
            built_in("Circle")
                .segments()
                .iter()
                .all(|segment| matches!(segment, AuthoredCurveSegment::CubicBezier { .. }))
        );
        assert_ne!(built_in("Circle"), built_in("Oval"));
    }
}
