//! Preview-only drawing of the evaluator's complete guide and parametric geometry.

use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};
use toniator_domain::{CanvasSpec, ChannelId};
use toniator_engine::EvaluationResult;
use toniator_geometry::{CurvePath, CurveSegment, Point2, StructuralPathSourceId};

/// Owns noninteractive guide presentation without altering any document or render scene.
pub(super) struct Overlay {
    area: gtk::DrawingArea,
    legend: gtk::Label,
    data: Rc<RefCell<Option<GuideDrawing>>>,
}

/// Retains exact evaluator paths and the canvas used by their matching accepted preview.
struct GuideDrawing {
    canvas: CanvasSpec,
    paths: Vec<CurvePath>,
    sites: Vec<Point2>,
}

impl Overlay {
    /// Binds a transparent GTK layer; it adds no keyboard stop or pointer target.
    pub(super) fn new(area: gtk::DrawingArea, legend: gtk::Label) -> Self {
        let data = Rc::new(RefCell::new(None::<GuideDrawing>));
        let drawing = Rc::clone(&data);
        area.set_draw_func(move |_, context, width, height| {
            if let Some(drawing) = drawing.borrow().as_ref() {
                draw(context, width, height, drawing);
            }
        });
        Self { area, legend, data }
    }

    /// Installs exact paths and sites from the accepted result without adding document geometry.
    /// Empty families clear the layer. Failed/stale results never reach this point.
    pub(super) fn install(
        &self,
        result: &EvaluationResult,
        channel: ChannelId,
        canvas: CanvasSpec,
    ) {
        let path_set = result
            .family_output(channel)
            .and_then(|family| family.structural_path_set());
        let parametric = path_set.is_some_and(|set| {
            set.paths().iter().any(|instance| {
                matches!(
                    instance.id.source,
                    StructuralPathSourceId::ParametricCurve(_)
                )
            })
        });
        let paths = path_set
            .map(|set| {
                set.paths()
                    .iter()
                    .filter(|instance| {
                        matches!(
                            instance.id.source,
                            StructuralPathSourceId::GuideDimension(_)
                                | StructuralPathSourceId::ParametricCurve(_)
                        )
                    })
                    .map(|instance| instance.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let sites = result
            .family_output(channel)
            .map(|family| {
                family
                    .site_set()
                    .sites()
                    .iter()
                    .map(|site| site.position)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let has_paths = !paths.is_empty();
        let has_sites = !sites.is_empty();
        let visible = has_paths || has_sites;
        self.legend
            .set_label(match (has_paths, has_sites, parametric) {
                (true, true, true) => "Blue: curve · Orange: sites · preview only",
                (true, true, false) => "Blue: guides · Orange: sites · preview only",
                (true, false, true) => "Blue: parametric curve · preview only",
                (true, false, false) => "Blue: guide shape · preview only",
                _ => "Orange: point sites · preview only",
            });
        *self.data.borrow_mut() = visible.then_some(GuideDrawing {
            canvas,
            paths,
            sites,
        });
        self.legend.set_visible(visible);
        self.area.queue_draw();
    }
}

/// Paints exact paths at 1pt and actual sites as crisp 1px squares in the centered preview.
/// GTK logical pixels use 96px/inch, so 1pt is 96/72 logical pixels independent of preview scale.
/// Canvas clipping and presentation color affect only this GTK layer, never canonical geometry.
fn draw(context: &gtk::cairo::Context, width: i32, height: i32, drawing: &GuideDrawing) {
    let scale =
        (f64::from(width) / drawing.canvas.width).min(f64::from(height) / drawing.canvas.height);
    if !scale.is_finite() || scale <= 0.0 || context.save().is_err() {
        return;
    }
    context.translate(
        (f64::from(width) - drawing.canvas.width * scale) * 0.5,
        (f64::from(height) - drawing.canvas.height * scale) * 0.5,
    );
    context.scale(scale, scale);
    context.rectangle(0.0, 0.0, drawing.canvas.width, drawing.canvas.height);
    context.clip();
    context.set_source_rgb(0.12, 0.48, 0.86);
    context.set_line_width((96.0 / 72.0) / scale);
    for path in &drawing.paths {
        for segment in path.segments() {
            let start = segment.start();
            context.move_to(start.x, start.y);
            match segment {
                CurveSegment::Line(line) => context.line_to(line.end().x, line.end().y),
                CurveSegment::CubicBezier(curve) => context.curve_to(
                    curve.control_1().x,
                    curve.control_1().y,
                    curve.control_2().x,
                    curve.control_2().y,
                    curve.end().x,
                    curve.end().y,
                ),
            }
        }
    }
    let _ = context.stroke();
    let _ = context.restore();
    if context.save().is_err() {
        return;
    }
    let offset_x = (f64::from(width) - drawing.canvas.width * scale) * 0.5;
    let offset_y = (f64::from(height) - drawing.canvas.height * scale) * 0.5;
    context.rectangle(
        offset_x,
        offset_y,
        drawing.canvas.width * scale,
        drawing.canvas.height * scale,
    );
    context.clip();
    context.set_antialias(gtk::cairo::Antialias::None);
    context.set_source_rgb(0.95, 0.32, 0.05);
    for site in &drawing.sites {
        if site.x >= 0.0
            && site.y >= 0.0
            && site.x < drawing.canvas.width
            && site.y < drawing.canvas.height
        {
            context.rectangle(
                (offset_x + site.x * scale).floor(),
                (offset_y + site.y * scale).floor(),
                1.0,
                1.0,
            );
        }
    }
    let _ = context.fill();
    let _ = context.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_geometry::Point2;

    /// Keeps each repeated guide at one screen point when the preview doubles in size.
    ///
    /// # Panics
    /// Panics if Cairo fails or scaled guides become wider, disappear, or leak beyond canvas clipping.
    #[test]
    fn guide_width_stays_constant_and_paths_clip_to_the_canvas() {
        let drawing = GuideDrawing {
            sites: Vec::new(),
            canvas: CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            paths: [-10.0, 25.0, 75.0]
                .into_iter()
                .map(|x| CurvePath::line(Point2 { x, y: -20.0 }, Point2 { x, y: 120.0 }).unwrap())
                .collect(),
        };
        for size in [100, 200] {
            let mut surface =
                gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, size, size).unwrap();
            let context = gtk::cairo::Context::new(&surface).unwrap();
            draw(&context, size, size, &drawing);
            drop(context);
            let stride = surface.stride() as usize;
            let bytes = surface.data().unwrap();
            let row = &bytes[size as usize / 2 * stride..][..size as usize * 4];
            let coverage: u32 = row
                .chunks_exact(4)
                .map(|pixel| u32::from_ne_bytes(pixel.try_into().unwrap()) >> 24)
                .sum();
            assert!(
                (650..=710).contains(&coverage),
                "two 1pt guides at {size}px: {coverage}"
            );
            assert_eq!(u32::from_ne_bytes(row[..4].try_into().unwrap()), 0);
        }
    }

    /// Keeps site indicators one logical pixel at different scales and excludes off-canvas guards.
    ///
    /// # Panics
    /// Panics if Cairo fails or markers change size, lose alignment, or escape canvas bounds.
    #[test]
    fn site_pixels_stay_constant_and_follow_centered_canvas() {
        let drawing = GuideDrawing {
            canvas: CanvasSpec {
                width: 100.0,
                height: 100.0,
            },
            paths: Vec::new(),
            sites: vec![
                Point2 { x: 25.25, y: 35.25 },
                Point2 { x: 75.25, y: 65.25 },
                Point2 { x: -0.1, y: 50.0 },
            ],
        };
        for (width, height) in [(100, 100), (200, 200), (200, 100)] {
            let mut surface =
                gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, width, height)
                    .unwrap();
            let context = gtk::cairo::Context::new(&surface).unwrap();
            draw(&context, width, height, &drawing);
            drop(context);
            let stride = surface.stride() as usize;
            let bytes = surface.data().unwrap();
            let painted = bytes
                .chunks_exact(4)
                .filter(|pixel| u32::from_ne_bytes((*pixel).try_into().unwrap()) >> 24 != 0)
                .count();
            assert_eq!(painted, 2, "{width}x{height}");
            let scale = f64::from(height) / 100.0;
            let offset = (f64::from(width) - 100.0 * scale) * 0.5;
            for site in &drawing.sites[..2] {
                let index = (site.y * scale).floor() as usize * stride
                    + (offset + site.x * scale).floor() as usize * 4;
                assert_eq!(
                    u32::from_ne_bytes(bytes[index..index + 4].try_into().unwrap()) >> 24,
                    255
                );
            }
        }
    }
}
