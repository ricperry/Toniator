//! GTK-facing print-preparation projections and bounded review-list policy.
//!
//! Document settings remain owned by `DocumentHistory`, and report geometry remains owned by
//! the engine pair. This module converts those authorities into runtime-only form and view text.

use std::{
    cell::Cell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

use gtk::prelude::*;
use toniator_domain::{PhysicalPrintSizeMm, PrintPreparationSettings};
use toniator_engine::{
    OutputRasterTarget, RasterBackground, RasterSurface,
    print_preflight::{
        CORE_ALPHA, PixelBounds, PixelRun, PreflightIdentity, PreflightRasterReport,
        PreflightReport, ProbeCandidate, SupportComponent,
    },
};

use crate::{components, print_preparation};

/// Caps each retained GTK review raster independently of the engine pair budget.
pub(crate) const MAX_REVIEW_DISPLAY_BYTES: u64 = 32 * 1024 * 1024;
/// Checked exact-preview raster limit; permits a 3600×4800 print target without downsampling.
///
/// A rendered surface and GTK texture can retain roughly twice this payload,
/// with a transient additional copy during the worker-to-GTK handoff.
pub(crate) const MAX_PREPARED_PREVIEW_BYTES: u64 = 256 * 1024 * 1024;
/// Caps the pair, image texture, and selected-run overlay retained by one review surface.
pub(crate) const MAX_REVIEW_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// Limits the visible finding rows; the rest remain reachable through paging.
pub(crate) const FINDINGS_PER_PAGE: usize = 50;
/// Caps each displayed axis while keeping both preview layers on the raster's common scale.
const MAX_PREVIEW_AXIS: f64 = 32_768.0;

/// Reports whether an explicit zoom can retain its stated output-pixel scale.
pub(crate) fn can_show_exact_zoom(target: OutputRasterTarget, scale: f64) -> bool {
    scale.is_finite()
        && scale > 0.0
        && f64::from(target.width()) * scale <= MAX_PREVIEW_AXIS
        && f64::from(target.height()) * scale <= MAX_PREVIEW_AXIS
}

/// Owns the singleton modeless PNG options and review widgets for one window epoch.
pub(crate) struct Controls {
    /// Shared native PNG options widget used by both menu and inspector entry paths.
    pub(crate) options: components::ToniatorPngExportOptions,
    /// Accessible native split pane for settings and the expanding final preview.
    pub(crate) options_paned: gtk::Paned,
    /// Shared options/review window.
    pub(crate) window: gtk::Window,
    /// Action that expands the in-place review area.
    pub(crate) review_button: gtk::Button,
    /// Collapsible review region expanded by the inspector Review action.
    pub(crate) review_expander: gtk::Expander,
    /// Explicit switch controlling PNG-only binary alpha preparation.
    pub(crate) prepare_toggle: gtk::CheckButton,
    /// Export-local maximum physical box and density, active only for preparation.
    pub(crate) box_settings: gtk::Grid,
    /// Visible maximum-width label projected from the selected print-box unit.
    pub(crate) box_width_label: gtk::Label,
    pub(crate) box_width: gtk::Entry,
    /// Visible maximum-height label projected from the selected print-box unit.
    pub(crate) box_height_label: gtk::Label,
    pub(crate) box_height: gtk::Entry,
    /// Runtime-only display-unit selector for the export-local maximum print box.
    pub(crate) box_unit: gtk::DropDown,
    pub(crate) target_dpi: gtk::Entry,
    pub(crate) box_message: gtk::Label,
    pub(crate) correction_settings: gtk::Grid,
    pub(crate) min_feature: gtk::Entry,
    pub(crate) remove_below: gtk::Entry,
    pub(crate) min_gap: gtk::Entry,
    pub(crate) gap_strategy: gtk::DropDown,
    pub(crate) gap_colors: gtk::Box,
    pub(crate) background_color_row: gtk::Box,
    pub(crate) custom_color_row: gtk::Box,
    pub(crate) background_fill: gtk::Entry,
    pub(crate) background_swatch: gtk::DrawingArea,
    pub(crate) use_garment_color: gtk::Button,
    pub(crate) custom_fill: gtk::Entry,
    /// Copies the current garment preview color once into the custom fill entry.
    pub(crate) use_custom_garment_color: gtk::Button,
    pub(crate) custom_swatch: gtk::DrawingArea,
    /// Unit selector for the complete runtime physical-intent form.
    pub(crate) unit: gtk::DropDown,
    /// Full-canvas physical width entry.
    pub(crate) physical_width: gtk::Entry,
    /// Full-canvas physical height entry.
    pub(crate) physical_height: gtk::Entry,
    /// Positive-feature threshold entry.
    pub(crate) positive_width: gtk::Entry,
    /// Negative-gap threshold entry.
    pub(crate) negative_gap: gtk::Entry,
    /// Applies validated project-owned settings through existing history authority.
    pub(crate) apply: gtk::Button,
    /// Physical-intent validation and Apply result.
    pub(crate) intent_message: gtk::Label,
    /// Applied settings projection inside the review surface.
    pub(crate) applied_summary: gtk::Label,
    /// Current review lifecycle state.
    pub(crate) status: gtk::Label,
    /// Exact selected final output target and two-axis physical projection.
    pub(crate) target_summary: gtk::Label,
    /// Independent positive and gap width-check availability.
    pub(crate) width_status: gtk::Label,
    /// Selects the advisory category shown in the bounded page.
    pub(crate) category: gtk::DropDown,
    /// Holds at most one page of accessible finding rows.
    pub(crate) findings: gtk::ListBox,
    /// Selects the prior finding page.
    pub(crate) previous_page: gtk::Button,
    /// Selects the next finding page.
    pub(crate) next_page: gtk::Button,
    /// Describes the current page and total category record count.
    pub(crate) page_summary: gtk::Label,
    /// Toggles viewer-only exact selected-run highlighting.
    pub(crate) highlight: gtk::ToggleButton,
    /// Centers the selected engine run bounds at a readable zoom.
    pub(crate) zoom_to_location: gtk::Button,
    /// Restores the complete final raster inside the current review viewport.
    pub(crate) fit_view: gtk::Button,
    /// Scales the prepared final-raster preview while the scroll window supplies panning.
    pub(crate) preview_zoom_out: gtk::Button,
    pub(crate) preview_actual_size: gtk::Button,
    pub(crate) preview_zoom_in: gtk::Button,
    pub(crate) preview_fit: gtk::Button,
    /// Selects the preview-only backdrop.
    pub(crate) preview_backdrop: gtk::DropDown,
    /// Enters the explicit Garment display color as `#RRGGBB`.
    pub(crate) garment_color: gtk::Entry,
    /// Shows the selected Garment display color.
    pub(crate) garment_swatch: gtk::DrawingArea,
    /// Invalid explicit color-entry feedback.
    pub(crate) garment_message: gtk::Label,
    /// Shows model-sensitive or explicit effective PNG export backing.
    pub(crate) export_backing_summary: gtk::Label,
    /// Scrollable canvas that retains the full final raster dimensions.
    pub(crate) canvas_scroll: gtk::ScrolledWindow,
    /// Shows the exact transparent final raster over the chosen viewer backdrop.
    pub(crate) raster_picture: gtk::Picture,
    /// Shows only the selected exact run overlay; it never changes raster pixels.
    pub(crate) highlight_picture: gtk::Picture,
    /// Painter for viewer-only solid and checkerboard backdrops.
    pub(crate) backdrop_painter: gtk::DrawingArea,
    /// Finalizes PNG settings and either writes to the chosen path or opens a PNG chooser.
    pub(crate) export: gtk::Button,
    /// Closes the shared options/review surface.
    pub(crate) close: gtk::Button,
    /// Suppresses signal callbacks while GTK receives a main-thread projection.
    pub(crate) syncing: Rc<Cell<bool>>,
    /// Runtime PNG target, AA, and backing selected by the real export widgets.
    pub(crate) selection: Cell<print_preparation::OutputSelection>,
    /// Runtime unit selected by the physical-intent form.
    pub(crate) selected_unit: Cell<EntryUnit>,
    /// Runtime viewer backdrop; it never invalidates preflight identity.
    pub(crate) selected_backdrop: Rc<Cell<PreviewBackdrop>>,
    /// Last valid Garment color, independent of transparent output pixels.
    pub(crate) garment_rgb: Rc<Cell<[u8; 3]>>,
    pub(crate) background_fill_rgb: Rc<Cell<[u8; 3]>>,
    pub(crate) custom_fill_rgb: Rc<Cell<[u8; 3]>>,
    /// Current RGB used by the viewer-only backdrop painter.
    pub(crate) current_preview_rgb: Rc<Cell<[u8; 3]>>,
}

/// Tracks one shared surface's identity, unapplied draft, output path, and visible selection.
pub(crate) struct Surface {
    /// Monotonic app-local surface epoch that rejects late callbacks from a closed window.
    pub(crate) epoch: u64,
    /// Workspace that owns this window; replacement closes it before stale actions can run.
    pub(crate) workspace_generation: u64,
    /// Widget projections and shared runtime controls.
    pub(crate) controls: Controls,
    /// Canonical millimetre values backing the export-local width/height unit projection.
    pub(crate) box_dimensions: Cell<PrintBoxDraft>,
    /// Last viewport allocation used while Fit is active.
    pub(crate) fit_allocation: Cell<(i32, i32)>,
    /// Chosen PNG destination, absent for the inspector-first review route.
    pub(crate) path: Option<PathBuf>,
    /// Frozen live capture held only while the deferred native save chooser is open.
    pub(crate) pending_capture: Option<print_preparation::ExportCapture>,
    /// Frozen PNG output intent paired with `pending_capture` across chooser edits.
    pub(crate) pending_selection: Option<print_preparation::OutputSelection>,
    /// Records user field edits without treating display-unit changes as document intent edits.
    pub(crate) draft_dirty: bool,
    /// Canonical values for every untouched draft field across unit-only display conversions.
    pub(crate) draft_base_settings: PrintPreparationSettings,
    /// Identifies exactly which form fields the user edited since the last draft projection.
    pub(crate) draft_changed_fields: [bool; 4],
    /// Category filter retained across stale/closed reports while this window remains open.
    pub(crate) category_filter: FindingCategory,
    /// Current page inside the selected category.
    pub(crate) page: usize,
    /// Identity of the list page currently represented by stable GTK rows.
    #[allow(dead_code)] // Inactive accepted G2a report display state.
    pub(crate) findings_projection_key: Option<(PreflightIdentity, FindingCategory, usize)>,
    /// Selected report-vector offset within `category_filter`.
    pub(crate) selected_record: Option<usize>,
    /// Full transparent-RGBA identity used to avoid reusing stale display textures.
    #[allow(dead_code)] // Inactive accepted G2a report display state.
    pub(crate) display_identity: Option<String>,
    /// Selection key for the one cached exact-run overlay texture.
    pub(crate) highlight_selection_identity: Option<(FindingCategory, usize)>,
    /// Report/category/record key for one exact selected-run location projection.
    pub(crate) selected_bounds_key: Option<(PreflightIdentity, FindingCategory, usize)>,
    /// Cached exact selected-run bounds used by navigation without rescanning on progress updates.
    pub(crate) selected_bounds: Option<PixelBounds>,
    /// Additional texture for the retained transparent pair.
    pub(crate) raster_texture: Option<gtk::gdk::Texture>,
    /// Bounded viewer-only exact-pixel highlight texture.
    pub(crate) highlight_texture: Option<gtk::gdk::Texture>,
    /// Latest captured render request, independently of the G2a advisory detector.
    pub(crate) preview_request_id: u64,
    pub(crate) preview_cancel: Option<Arc<AtomicBool>>,
    pub(crate) preview_authority: Option<print_preparation::LiveAuthority>,
    pub(crate) preview_selection: Option<print_preparation::OutputSelection>,
    pub(crate) preview_status: PreviewStatus,
    /// None fits the viewport; Some is output-pixel scale for spatial scroll/pan.
    pub(crate) preview_zoom: Option<f64>,
}

/// Main-thread lifecycle of one exact prepared PNG preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewStatus {
    NotRendered,
    Rendering,
    Current,
    OutOfDate,
    Failed,
}

impl Controls {
    /// Instantiates the resource-owned PNG/review form and initializes runtime selections and divider semantics.
    ///
    /// The caller supplies one live canvas and model only for defaults; document state remains
    /// owned by the workspace and is recaptured for automatic preview and final Export.
    ///
    /// # Errors
    /// Returns invalid native output dimensions before presenting a partial options window.
    pub(crate) fn new(
        parent: &gtk::Window,
        canvas: &toniator_domain::CanvasSpec,
        model: Option<toniator_domain::HalftoneChannelModel>,
        settings: &PrintPreparationSettings,
    ) -> Result<Self, String> {
        let options = components::ToniatorPngExportOptions::new();
        let template = options.print_preparation_controls();
        let window = gtk::Window::builder()
            .title("PNG export options")
            .default_width(1_200)
            .default_height(820)
            .transient_for(parent)
            .modal(false)
            .build();
        let title = gtk::Label::new(Some("PNG export options"));
        title.add_css_class("title");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&title));
        header.set_show_title_buttons(true);
        window.set_titlebar(Some(&header));
        window.set_child(Some(&options));
        template.options_paned.set_focusable(true);
        template.options_paned.update_property(&[
            gtk::accessible::Property::Label("PNG settings and final preview divider"),
            gtk::accessible::Property::Description(
                "Press F8 to focus or cycle the divider, use Left and Right arrows to resize, and press Tab to leave.",
            ),
        ]);

        let native = OutputRasterTarget::for_canvas(canvas).map_err(|error| error.to_string())?;
        let background = options.background();
        background.set_model(Some(&gtk::StringList::new(&[
            "Automatic",
            "Transparent",
            "Black",
            "White",
        ])));
        background.set_selected(0);
        background.update_property(&[gtk::accessible::Property::Label("PNG export backing")]);
        let antialiasing = options.antialiasing();
        antialiasing.set_model(Some(&gtk::StringList::new(&["On", "Off"])));
        antialiasing.set_selected(0);
        antialiasing.update_property(&[gtk::accessible::Property::Label("Antialiasing")]);
        let scale = options.scale();
        scale.set_model(Some(&gtk::StringList::new(&[
            "1× native",
            "2×",
            "4×",
            "8×",
            "Custom",
        ])));
        scale.set_selected(0);
        scale.update_property(&[gtk::accessible::Property::Label("Output size")]);
        let dimensions = options.dimensions();
        dimensions.set_text(&format!("{}x{}", native.width(), native.height()));
        dimensions.update_property(&[gtk::accessible::Property::Label("Custom dimensions")]);

        template.unit.set_model(Some(&gtk::StringList::new(&[
            "Millimetres (mm)",
            "Inches (in)",
        ])));
        template.unit.set_selected(0);
        template.box_unit.set_model(Some(&gtk::StringList::new(&[
            "Millimetres (mm)",
            "Inches (in)",
        ])));
        template.box_unit.set_selected(0);
        template
            .gap_strategy
            .set_model(Some(&gtk::StringList::new(&[
                "Fill (average surrounding edges)",
                "Fill (background color)",
                "Fill (custom color)",
                "Grow gap to minimum",
            ])));
        template.gap_strategy.set_selected(0);
        if let Some(size) = settings.size_mm() {
            let (width, height) = size.millimetres();
            template.box_width.set_text(&format_decimal(width));
            template.box_height.set_text(&format_decimal(height));
        }
        template.category.set_model(Some(&gtk::StringList::new(&[
            FindingCategory::Support.label(),
            FindingCategory::BoundaryAssociated.label(),
            FindingCategory::Unresolved.label(),
            FindingCategory::PositiveWidth.label(),
            FindingCategory::NegativeGap.label(),
        ])));
        template.category.set_selected(0);
        template
            .page_summary
            .set_label("Findings appear after Check.");
        template
            .preview_backdrop
            .set_model(Some(&gtk::StringList::new(&[
                "Automatic",
                "Checkerboard",
                "Light",
                "Dark",
                "Garment",
            ])));
        template.preview_backdrop.set_selected(0);

        let draft = IntentDraft::from_settings(settings, EntryUnit::Millimetres)?;
        template.physical_width.set_text(&draft.width);
        template.physical_height.set_text(&draft.height);
        template.positive_width.set_text(&draft.positive_width);
        template.negative_gap.set_text(&draft.negative_gap);
        template.intent_message.set_label("");
        template.intent_message.set_visible(false);
        template.garment_color.update_property(&[
            gtk::accessible::Property::Description(
                "Viewer backdrop color only; this is not fabric, underbase, adhesive, or CMYK simulation.",
            ),
        ]);
        template.garment_swatch.set_can_target(false);
        template.raster_picture.set_can_shrink(true);
        template
            .raster_picture
            .set_content_fit(gtk::ContentFit::Contain);
        template.highlight_picture.set_can_shrink(true);
        template
            .highlight_picture
            .set_content_fit(gtk::ContentFit::Contain);
        template.highlight_picture.set_can_target(false);
        template
            .raster_picture
            .set_paintable(None::<&gtk::gdk::Paintable>);
        template
            .highlight_picture
            .set_paintable(None::<&gtk::gdk::Paintable>);
        template.canvas_scroll.set_min_content_width(320);
        template.canvas_scroll.set_min_content_height(280);

        let syncing = Rc::new(Cell::new(false));
        let garment_rgb = Rc::new(Cell::new([112, 64, 160]));
        let background_fill_rgb = Rc::new(Cell::new([255, 255, 255]));
        let custom_fill_rgb = Rc::new(Cell::new([255, 255, 255]));
        let selected_backdrop = Rc::new(Cell::new(PreviewBackdrop::Automatic));
        let current_preview_rgb = Rc::new(Cell::new(automatic_preview_rgb(model)));
        install_backdrop_painter(
            &template.backdrop_painter,
            Rc::clone(&selected_backdrop),
            Rc::clone(&current_preview_rgb),
            Rc::clone(&garment_rgb),
        );
        install_garment_swatch(&template.garment_swatch, Rc::clone(&garment_rgb));
        install_garment_swatch(&template.background_swatch, Rc::clone(&background_fill_rgb));
        install_garment_swatch(&template.custom_swatch, Rc::clone(&custom_fill_rgb));
        template.garment_color.set_sensitive(false);
        template.garment_swatch.set_sensitive(false);
        template.highlight.set_sensitive(false);
        template.zoom_to_location.set_sensitive(false);
        template.fit_view.set_sensitive(false);
        template.findings.set_sensitive(false);
        template.previous_page.set_sensitive(false);
        template.next_page.set_sensitive(false);

        Ok(Self {
            options,
            window,
            options_paned: template.options_paned,
            review_button: template.review_button,
            review_expander: template.review_expander,
            prepare_toggle: template.prepare_toggle,
            box_settings: template.box_settings,
            box_width_label: template.box_width_label,
            box_width: template.box_width,
            box_height_label: template.box_height_label,
            box_height: template.box_height,
            box_unit: template.box_unit,
            target_dpi: template.target_dpi,
            box_message: template.box_message,
            correction_settings: template.correction_settings,
            min_feature: template.min_feature,
            remove_below: template.remove_below,
            min_gap: template.min_gap,
            gap_strategy: template.gap_strategy,
            gap_colors: template.gap_colors,
            background_color_row: template.background_color_row,
            custom_color_row: template.custom_color_row,
            background_fill: template.background_fill,
            background_swatch: template.background_swatch,
            use_garment_color: template.use_garment_color,
            custom_fill: template.custom_fill,
            use_custom_garment_color: template.use_custom_garment_color,
            custom_swatch: template.custom_swatch,
            unit: template.unit,
            physical_width: template.physical_width,
            physical_height: template.physical_height,
            positive_width: template.positive_width,
            negative_gap: template.negative_gap,
            apply: template.apply,
            intent_message: template.intent_message,
            applied_summary: template.applied_summary,
            status: template.status,
            target_summary: template.target_summary,
            width_status: template.width_status,
            category: template.category,
            findings: template.findings,
            previous_page: template.previous_page,
            next_page: template.next_page,
            page_summary: template.page_summary,
            highlight: template.highlight,
            zoom_to_location: template.zoom_to_location,
            fit_view: template.fit_view,
            preview_zoom_out: template.preview_zoom_out,
            preview_actual_size: template.preview_actual_size,
            preview_zoom_in: template.preview_zoom_in,
            preview_fit: template.preview_fit,
            preview_backdrop: template.preview_backdrop,
            garment_color: template.garment_color,
            garment_swatch: template.garment_swatch,
            garment_message: template.garment_message,
            canvas_scroll: template.canvas_scroll,
            raster_picture: template.raster_picture,
            highlight_picture: template.highlight_picture,
            backdrop_painter: template.backdrop_painter,
            export_backing_summary: template.backing_summary,
            close: template.close,
            export: template.export,
            syncing,
            selection: Cell::new(print_preparation::OutputSelection {
                target: native,
                antialiasing: toniator_engine::RasterAntialiasing::On,
                background: print_preparation::BackgroundChoice::Automatic,
                prepare_for_print: false,
                pixels_per_metre: None,
                cleanup: crate::print_cleanup::Settings::default(),
            }),
            selected_unit: Cell::new(EntryUnit::Millimetres),
            selected_backdrop,
            garment_rgb,
            background_fill_rgb,
            custom_fill_rgb,
            current_preview_rgb,
        })
    }

    /// Applies one synchronous GTK projection while suppressing reentrant signal handlers.
    pub(crate) fn project(&self, project: impl FnOnce()) {
        self.syncing.set(true);
        project();
        self.syncing.set(false);
    }
}

impl Surface {
    /// Creates one surface owner, preserving canonical export-box dimensions in runtime state.
    ///
    /// The project settings supply only the initial millimetre values; later print-box edits remain
    /// export-local. Fit allocation starts unknown and is recorded from the realized preview pane.
    pub(crate) fn new(
        epoch: u64,
        workspace_generation: u64,
        controls: Controls,
        path: Option<PathBuf>,
        expanded: bool,
        settings: PrintPreparationSettings,
    ) -> Self {
        controls.review_expander.set_expanded(expanded);
        let box_dimensions =
            PrintBoxDraft::from_mm(settings.size_mm().map(PhysicalPrintSizeMm::millimetres));
        Self {
            epoch,
            workspace_generation,
            controls,
            box_dimensions: Cell::new(box_dimensions),
            fit_allocation: Cell::new((0, 0)),
            path,
            pending_capture: None,
            pending_selection: None,
            draft_dirty: false,
            draft_base_settings: settings,
            draft_changed_fields: [false; 4],
            category_filter: FindingCategory::Support,
            page: 0,
            findings_projection_key: None,
            selected_record: None,
            display_identity: None,
            highlight_selection_identity: None,
            selected_bounds_key: None,
            selected_bounds: None,
            raster_texture: None,
            highlight_texture: None,
            preview_request_id: 0,
            preview_cancel: None,
            preview_authority: None,
            preview_selection: None,
            preview_status: PreviewStatus::NotRendered,
            preview_zoom: None,
        }
    }

    /// Cancels and removes an obsolete preview without touching document or output settings.
    pub(crate) fn invalidate_preview(&mut self) {
        if let Some(cancel) = self.preview_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        let was_active = self.preview_status != PreviewStatus::NotRendered;
        self.preview_status = if was_active {
            PreviewStatus::OutOfDate
        } else {
            PreviewStatus::NotRendered
        };
        self.preview_authority = None;
        self.preview_selection = None;
        self.raster_texture = None;
        self.controls
            .raster_picture
            .set_paintable(None::<&gtk::gdk::Paintable>);
        self.controls.raster_picture.set_size_request(-1, -1);
    }

    /// Discards selected presentation artifacts while preserving the category filter.
    #[allow(dead_code)] // The visible PNG preview no longer presents G2a advisory findings.
    pub(crate) fn clear_report_presentation(&mut self) {
        if self.display_identity.is_none()
            && self.raster_texture.is_none()
            && self.highlight_texture.is_none()
            && self.selected_record.is_none()
        {
            return;
        }
        self.selected_record = None;
        self.page = 0;
        self.display_identity = None;
        self.highlight_selection_identity = None;
        self.selected_bounds_key = None;
        self.selected_bounds = None;
        self.raster_texture = None;
        self.highlight_texture = None;
        self.controls
            .raster_picture
            .set_paintable(None::<&gtk::gdk::Paintable>);
        self.controls
            .highlight_picture
            .set_paintable(None::<&gtk::gdk::Paintable>);
        self.controls.raster_picture.set_size_request(-1, -1);
        self.controls.highlight_picture.set_size_request(-1, -1);
        self.controls.backdrop_painter.set_size_request(-1, -1);
        self.controls.zoom_to_location.set_sensitive(false);
        self.controls.fit_view.set_sensitive(false);
        self.controls.highlight.set_sensitive(false);
    }
}

impl Drop for Surface {
    /// Cancels detached exact-preview work when its owner window retires.
    fn drop(&mut self) {
        if let Some(cancel) = self.preview_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
    }
}

/// Installs a viewer-only solid or checkerboard background painter.
fn install_backdrop_painter(
    area: &gtk::DrawingArea,
    backdrop: Rc<Cell<PreviewBackdrop>>,
    automatic_rgb: Rc<Cell<[u8; 3]>>,
    garment_rgb: Rc<Cell<[u8; 3]>>,
) {
    area.set_draw_func(move |_, context, width, height| {
        draw_backdrop(
            context,
            width,
            height,
            backdrop.get(),
            automatic_rgb.get(),
            garment_rgb.get(),
        );
    });
}

/// Paints the selected viewer background without inspecting or changing raster alpha.
fn draw_backdrop(
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    backdrop: PreviewBackdrop,
    automatic_rgb: [u8; 3],
    garment_rgb: [u8; 3],
) {
    let rgb = match backdrop {
        PreviewBackdrop::Automatic => automatic_rgb,
        PreviewBackdrop::Checkerboard => {
            context.set_source_rgb(0.74, 0.74, 0.74);
            let _ = context.paint();
            let tile = 18;
            context.set_source_rgb(0.91, 0.91, 0.91);
            for y in (0..height).step_by(tile as usize) {
                for x in (0..width).step_by(tile as usize) {
                    if (x / tile + y / tile) % 2 == 0 {
                        context.rectangle(x as f64, y as f64, tile as f64, tile as f64);
                    }
                }
            }
            let _ = context.fill();
            return;
        }
        PreviewBackdrop::Light => [255, 255, 255],
        PreviewBackdrop::Dark => [24, 24, 24],
        PreviewBackdrop::Garment => garment_rgb,
    };
    context.set_source_rgb(
        f64::from(rgb[0]) / 255.0,
        f64::from(rgb[1]) / 255.0,
        f64::from(rgb[2]) / 255.0,
    );
    let _ = context.paint();
}

/// Installs a noninteractive color swatch tied to the most recent valid garment entry.
fn install_garment_swatch(area: &gtk::DrawingArea, garment_rgb: Rc<Cell<[u8; 3]>>) {
    area.set_draw_func(move |_, context, width, height| {
        let [red, green, blue] = garment_rgb.get();
        context.set_source_rgb(
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0,
        );
        context.rectangle(1.0, 1.0, f64::from(width - 2), f64::from(height - 2));
        let _ = context.fill_preserve();
        context.set_source_rgb(0.1, 0.1, 0.1);
        let _ = context.stroke();
    });
}

/// Selects a display unit for the runtime-only physical-intent form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EntryUnit {
    /// Millimetres are the canonical persisted unit.
    #[default]
    Millimetres,
    /// Inches are converted to millimetres before a history command is created.
    Inches,
}

impl EntryUnit {
    /// Resolves one dropdown index without creating a third display-unit state.
    pub(crate) const fn from_position(position: u32) -> Self {
        if position == 1 {
            Self::Inches
        } else {
            Self::Millimetres
        }
    }

    /// Returns the stable dropdown position for this runtime display unit.
    pub(crate) const fn position(self) -> u32 {
        match self {
            Self::Millimetres => 0,
            Self::Inches => 1,
        }
    }

    /// Returns the unit name used by the visible print-box labels and diagnostics.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Millimetres => "millimetres",
            Self::Inches => "inches",
        }
    }

    /// Returns the short unit text used beside a physical measurement.
    pub(crate) const fn abbreviation(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Inches => "in",
        }
    }

    /// Projects one canonical setting into the entry's selected unit.
    pub(crate) fn display_from_mm(self, value_mm: f64) -> Result<f64, String> {
        match self {
            Self::Millimetres => Ok(value_mm),
            Self::Inches => PrintPreparationSettings::threshold_inches_from_mm(value_mm)
                .map_err(|error| error.to_string()),
        }
    }

    /// Converts one finite nonnegative displayed threshold to canonical millimetres.
    fn threshold_to_mm(self, value: f64) -> Result<f64, String> {
        match self {
            Self::Millimetres => Ok(value),
            Self::Inches => PrintPreparationSettings::threshold_mm_from_inches(value)
                .map_err(|error| error.to_string()),
        }
    }
}

/// Holds export-local maximum dimensions in canonical millimetres across unit-only projections.
///
/// The widget strings are presentation. Valid edits update this draft once; subsequent unit
/// changes project from the retained millimetre values, so formatting cannot accumulate drift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PrintBoxDraft {
    unit: EntryUnit,
    width_mm: Option<f64>,
    height_mm: Option<f64>,
    width_valid: bool,
    height_valid: bool,
}

impl PrintBoxDraft {
    /// Creates a millimetre-backed export draft from optional canonical dimensions.
    pub(crate) fn from_mm(dimensions: Option<(f64, f64)>) -> Self {
        let (width_mm, height_mm) =
            dimensions.map_or((None, None), |(width, height)| (Some(width), Some(height)));
        Self {
            unit: EntryUnit::Millimetres,
            width_mm,
            height_mm,
            width_valid: true,
            height_valid: true,
        }
    }

    /// Returns the currently selected display unit without making it document state.
    pub(crate) const fn unit(self) -> EntryUnit {
        self.unit
    }

    /// Parses a changed width field once into canonical millimetres.
    ///
    /// Blank values remain editable while the print preparation option is off; final fitting
    /// requires a positive value for both axes. Invalid text stays visible and marks this axis
    /// unavailable until the user corrects it.
    ///
    /// # Errors
    /// Returns a field-specific diagnostic for nonnumeric, nonpositive, nonfinite, or
    /// unrepresentable dimensions.
    pub(crate) fn update_width(&mut self, value: &str) -> Result<(), String> {
        match parse_box_dimension(value, self.unit, "Print box width") {
            Ok(width_mm) => {
                self.width_mm = width_mm;
                self.width_valid = true;
                Ok(())
            }
            Err(error) => {
                self.width_valid = false;
                Err(error)
            }
        }
    }

    /// Parses a changed height field once into canonical millimetres.
    ///
    /// Blank values remain editable while the print preparation option is off; final fitting
    /// requires a positive value for both axes. Invalid text stays visible and marks this axis
    /// unavailable until the user corrects it.
    ///
    /// # Errors
    /// Returns a field-specific diagnostic for nonnumeric, nonpositive, nonfinite, or
    /// unrepresentable dimensions.
    pub(crate) fn update_height(&mut self, value: &str) -> Result<(), String> {
        match parse_box_dimension(value, self.unit, "Print box height") {
            Ok(height_mm) => {
                self.height_mm = height_mm;
                self.height_valid = true;
                Ok(())
            }
            Err(error) => {
                self.height_valid = false;
                Err(error)
            }
        }
    }

    /// Returns required maximum width and height in canonical millimetres for final fitting.
    ///
    /// # Errors
    /// Returns a unit-aware field diagnostic if either entry is invalid, blank, or absent.
    pub(crate) fn dimensions_for_fit_mm(self) -> Result<(f64, f64), String> {
        if !self.width_valid || self.width_mm.is_none() {
            return Err(format!(
                "Enter a positive print box width in {}.",
                self.unit.name()
            ));
        }
        if !self.height_valid || self.height_mm.is_none() {
            return Err(format!(
                "Enter a positive print box height in {}.",
                self.unit.name()
            ));
        }
        match (self.width_mm, self.height_mm) {
            (Some(width), Some(height)) => Ok((width, height)),
            (None, _) => Err(format!(
                "Enter a positive print box width in {}.",
                self.unit.name()
            )),
            (_, None) => Err(format!(
                "Enter a positive print box height in {}.",
                self.unit.name()
            )),
        }
    }

    /// Formats one canonical millimetre value for this unit without updating the draft.
    ///
    /// # Errors
    /// Returns the unit-conversion diagnostic when an inch value cannot be represented.
    pub(crate) fn format_mm(self, value_mm: f64) -> Result<String, String> {
        self.unit
            .display_from_mm(value_mm)
            .map(format_print_box_display_value)
    }

    /// Changes display units while keeping the canonical dimensions unchanged.
    ///
    /// The returned strings are a projection only; callers set the entries under GTK signal
    /// suppression. Repeated unit changes never parse rounded display strings back into authority.
    ///
    /// # Errors
    /// Returns an error if current visible text is invalid or the new unit cannot represent a value.
    pub(crate) fn change_unit(&mut self, unit: EntryUnit) -> Result<(String, String), String> {
        if !self.width_valid || !self.height_valid {
            return Err("Correct the print box dimensions before changing units.".into());
        }
        let width = format_box_dimension(self.width_mm, unit)?;
        let height = format_box_dimension(self.height_mm, unit)?;
        self.unit = unit;
        Ok((width, height))
    }
}

/// Parses one optional displayed dimension and converts it once to canonical millimetres.
///
/// # Errors
/// Returns a field-specific diagnostic for malformed, nonpositive, nonfinite, or unrepresentable
/// physical dimensions.
fn parse_box_dimension(value: &str, unit: EntryUnit, field: &str) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let parsed = parse_finite_positive(value.trim(), field)?;
    unit.threshold_to_mm(parsed)
        .map(Some)
        .map_err(|_| format!("{field} is outside the supported physical range."))
}

/// Formats one optional canonical dimension using shortest-roundtrip decimal text.
///
/// # Errors
/// Returns the unit-conversion diagnostic when the selected display unit cannot represent it.
fn format_box_dimension(value_mm: Option<f64>, unit: EntryUnit) -> Result<String, String> {
    value_mm.map_or_else(
        || Ok(String::new()),
        |value| {
            unit.display_from_mm(value)
                .map(format_print_box_display_value)
        },
    )
}

/// Removes only floating-point conversion noise near a decimal with twelve-place precision.
///
/// The canonical millimetre draft remains authoritative, so display normalization never changes
/// physical size or the pixel-fit calculation.
fn format_print_box_display_value(value: f64) -> String {
    const DISPLAY_SCALE: f64 = 1_000_000_000_000.0;
    let rounded = (value * DISPLAY_SCALE).round() / DISPLAY_SCALE;
    let tolerance = 8.0 * f64::EPSILON * value.abs();
    if rounded.is_finite() && (value - rounded).abs() <= tolerance {
        rounded.to_string()
    } else {
        value.to_string()
    }
}

/// Runtime-only strings for one complete physical-intent draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct IntentDraft {
    /// Selects the display unit; it is never persisted.
    pub(crate) unit: EntryUnit,
    /// Full-canvas physical width, blank only when both placement dimensions are unknown.
    pub(crate) width: String,
    /// Full-canvas physical height, blank only when both placement dimensions are unknown.
    pub(crate) height: String,
    /// Independent positive-feature threshold; zero disables this check.
    pub(crate) positive_width: String,
    /// Independent negative-gap threshold; zero disables this check.
    pub(crate) negative_gap: String,
}

impl IntentDraft {
    /// Formats applied millimetre authority for editing without storing presentation state.
    pub(crate) fn from_settings(
        settings: &PrintPreparationSettings,
        unit: EntryUnit,
    ) -> Result<Self, String> {
        let (width, height) = match settings.size_mm() {
            Some(size) => {
                let (width_mm, height_mm) = size.millimetres();
                (
                    format_decimal(unit.display_from_mm(width_mm)?),
                    format_decimal(unit.display_from_mm(height_mm)?),
                )
            }
            None => (String::new(), String::new()),
        };
        Ok(Self {
            unit,
            width,
            height,
            positive_width: format_decimal(
                unit.display_from_mm(settings.minimum_positive_feature_width_mm())?,
            ),
            negative_gap: format_decimal(
                unit.display_from_mm(settings.minimum_negative_gap_width_mm())?,
            ),
        })
    }

    /// Validates all four fields and returns one complete canonical project setting.
    ///
    /// Both physical fields may be blank to represent Unknown; one blank is always invalid.
    /// Thresholds are required, finite, and nonnegative, with zero disabling only that check.
    ///
    /// # Errors
    /// Returns a field-specific diagnostic for partial placement, malformed numbers, or domain
    /// validation failures. No document state changes here.
    #[cfg(test)]
    pub(crate) fn parse(&self) -> Result<PrintPreparationSettings, String> {
        let width_text = self.width.trim();
        let height_text = self.height.trim();
        let size_mm = match (width_text.is_empty(), height_text.is_empty()) {
            (true, true) => None,
            (true, false) | (false, true) => {
                return Err("Enter both physical dimensions, or leave both blank.".into());
            }
            (false, false) => {
                let width = parse_finite_positive(width_text, "Physical width")?;
                let height = parse_finite_positive(height_text, "Physical height")?;
                Some(
                    match self.unit {
                        EntryUnit::Millimetres => PhysicalPrintSizeMm::new(width, height),
                        EntryUnit::Inches => PhysicalPrintSizeMm::from_inches(width, height),
                    }
                    .map_err(|error| error.to_string())?,
                )
            }
        };
        let positive = self.unit.threshold_to_mm(parse_finite_nonnegative(
            self.positive_width.trim(),
            "Positive width threshold",
        )?)?;
        let negative = self.unit.threshold_to_mm(parse_finite_nonnegative(
            self.negative_gap.trim(),
            "Gap threshold",
        )?)?;
        PrintPreparationSettings::new(size_mm, positive, negative)
            .map_err(|error| error.to_string())
    }

    /// Applies edited fields over canonical untouched values to avoid unit-conversion drift.
    ///
    /// The changed flags follow the field order width, height, positive threshold, gap threshold.
    /// Untouched values come directly from `base`, even after repeated mm/in display conversions.
    ///
    /// # Errors
    /// Returns field validation and complete-canvas pairing errors without changing the base.
    pub(crate) fn parse_over(
        &self,
        base: &PrintPreparationSettings,
        changed: [bool; 4],
    ) -> Result<PrintPreparationSettings, String> {
        let base_size = base.size_mm().map(PhysicalPrintSizeMm::millimetres);
        let width_mm = if changed[0] {
            parse_optional_dimension(&self.width, self.unit, "Physical width")?
        } else {
            base_size.map(|size| size.0)
        };
        let height_mm = if changed[1] {
            parse_optional_dimension(&self.height, self.unit, "Physical height")?
        } else {
            base_size.map(|size| size.1)
        };
        let size_mm = match (width_mm, height_mm) {
            (None, None) => None,
            (Some(width), Some(height)) => {
                Some(PhysicalPrintSizeMm::new(width, height).map_err(|error| error.to_string())?)
            }
            _ => return Err("Enter both physical dimensions, or leave both blank.".into()),
        };
        let positive = if changed[2] {
            self.unit.threshold_to_mm(parse_finite_nonnegative(
                self.positive_width.trim(),
                "Positive width threshold",
            )?)?
        } else {
            base.minimum_positive_feature_width_mm()
        };
        let negative = if changed[3] {
            self.unit.threshold_to_mm(parse_finite_nonnegative(
                self.negative_gap.trim(),
                "Gap threshold",
            )?)?
        } else {
            base.minimum_negative_gap_width_mm()
        };
        PrintPreparationSettings::new(size_mm, positive, negative)
            .map_err(|error| error.to_string())
    }

    /// Reprojects live values into untouched fields after an external document-history change.
    ///
    /// User-edited fields remain byte-for-byte as entered; callers replace the canonical base
    /// with `base` and use the same `changed` flags when applying the combined form.
    ///
    /// # Errors
    /// Returns the unit-conversion error from formatting live applied values.
    pub(crate) fn rebase_untouched(
        &mut self,
        base: &PrintPreparationSettings,
        changed: [bool; 4],
    ) -> Result<(), String> {
        let live = Self::from_settings(base, self.unit)?;
        if !changed[0] {
            self.width = live.width;
        }
        if !changed[1] {
            self.height = live.height;
        }
        if !changed[2] {
            self.positive_width = live.positive_width;
        }
        if !changed[3] {
            self.negative_gap = live.negative_gap;
        }
        Ok(())
    }
}

/// Parses one optional physical axis and converts only edited nonblank values to mm.
fn parse_optional_dimension(
    value: &str,
    unit: EntryUnit,
    field: &str,
) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let parsed = parse_finite_positive(value.trim(), field)?;
    unit.threshold_to_mm(parsed).map(Some)
}

/// Describes the five viewer-only backdrops available in the review surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PreviewBackdrop {
    /// Follows the normative model-sensitive viewer default.
    #[default]
    Automatic,
    /// Uses a neutral alpha checkerboard.
    Checkerboard,
    /// Uses a light solid canvas.
    Light,
    /// Uses a dark solid canvas.
    Dark,
    /// Uses the separate artist-entered garment display color.
    Garment,
}

/// Names the advisory record family used by the bounded finding list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FindingCategory {
    /// All exact alpha-support components, including low-coverage support.
    #[default]
    Support,
    /// Components with engine-measured boundary-associated low coverage.
    BoundaryAssociated,
    /// Exact unresolved low-coverage runs retained by the engine.
    Unresolved,
    /// Positive-width stencil candidates.
    PositiveWidth,
    /// Negative-gap stencil candidates.
    NegativeGap,
}

impl PreviewBackdrop {
    /// Resolves one visible dropdown position, defaulting safely to Automatic.
    pub(crate) const fn from_position(position: u32) -> Self {
        match position {
            1 => Self::Checkerboard,
            2 => Self::Light,
            3 => Self::Dark,
            4 => Self::Garment,
            _ => Self::Automatic,
        }
    }
}

impl FindingCategory {
    /// Resolves one visible category position without inventing a hidden report family.
    pub(crate) const fn from_position(position: u32) -> Self {
        match position {
            1 => Self::BoundaryAssociated,
            2 => Self::Unresolved,
            3 => Self::PositiveWidth,
            4 => Self::NegativeGap,
            _ => Self::Support,
        }
    }

    /// Returns the visible category name grounded in engine report vocabulary.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Support => "Alpha support inventory",
            Self::BoundaryAssociated => "Boundary-associated faint coverage",
            Self::Unresolved => "Unresolved faint coverage",
            Self::PositiveWidth => "Positive-width candidates",
            Self::NegativeGap => "Gap-width candidates",
        }
    }

    /// Counts only the report records that are represented by this category.
    #[allow(dead_code)] // Retained for the accepted G2a report tests.
    pub(crate) fn record_count(self, report: &PreflightReport) -> usize {
        match self {
            Self::Support => report.support_components.len(),
            Self::BoundaryAssociated => report
                .support_components
                .iter()
                .filter(|component| component.boundary_associated_low_coverage_pixels > 0)
                .count(),
            Self::Unresolved => report
                .support_components
                .iter()
                .filter(|component| component.unresolved_low_coverage_pixels > 0)
                .count(),
            Self::PositiveWidth => report
                .positive_candidates
                .iter()
                .filter(|candidate| !candidate.negative_gap)
                .count(),
            Self::NegativeGap => report
                .negative_candidates
                .iter()
                .filter(|candidate| candidate.negative_gap)
                .count(),
        }
    }

    /// Resolves one visible category offset without retaining a second report-sized index.
    pub(crate) fn record_at<'a>(
        self,
        report: &'a PreflightReport,
        visible_index: usize,
    ) -> Option<FindingRecord<'a>> {
        match self {
            Self::Support => report
                .support_components
                .get(visible_index)
                .map(FindingRecord::Support),
            Self::BoundaryAssociated | Self::Unresolved => report
                .support_components
                .iter()
                .filter(|component| match self {
                    Self::BoundaryAssociated => {
                        component.boundary_associated_low_coverage_pixels > 0
                    }
                    Self::Unresolved => component.unresolved_low_coverage_pixels > 0,
                    _ => false,
                })
                .nth(visible_index)
                .map(FindingRecord::Support),
            Self::PositiveWidth | Self::NegativeGap => report
                .positive_candidates
                .iter()
                .chain(&report.negative_candidates)
                .filter(|candidate| candidate.negative_gap == (self == Self::NegativeGap))
                .nth(visible_index)
                .map(FindingRecord::Candidate),
        }
    }
}

/// Borrows one source-owned engine record for one visible page.
pub(crate) enum FindingRecord<'a> {
    /// An exact alpha-support component.
    Support(&'a SupportComponent),
    /// An exact positive- or negative-width candidate.
    Candidate(&'a ProbeCandidate),
}

/// Parses an explicit six-digit RGB display color.
///
/// # Errors
/// Returns a concise entry hint unless the value is exactly `#RRGGBB`.
pub(crate) fn parse_hex_color(value: &str) -> Result<[u8; 3], String> {
    let value = value.trim();
    let Some(hex) = value.strip_prefix('#') else {
        return Err("Use a six-digit color such as #7040A0.".into());
    };
    if hex.len() != 6 {
        return Err("Use a six-digit color such as #7040A0.".into());
    }
    let bytes = hex.as_bytes();
    let mut channels = [0; 3];
    for (index, channel) in channels.iter_mut().enumerate() {
        let high = hex_nibble(bytes[index * 2]);
        let low = hex_nibble(bytes[index * 2 + 1]);
        *channel = high
            .zip(low)
            .map(|(high, low)| high * 16 + low)
            .ok_or_else(|| "Use a six-digit color such as #7040A0.".to_owned())?;
    }
    Ok(channels)
}

/// Decodes one ASCII hexadecimal digit without slicing UTF-8 at untrusted byte offsets.
fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Resolves Automatic preview color for the selected document model, independent of export.
pub(crate) const fn automatic_preview_rgb(
    model: Option<toniator_domain::HalftoneChannelModel>,
) -> [u8; 3] {
    match model {
        Some(toniator_domain::HalftoneChannelModel::Rgb) => [0, 0, 0],
        Some(toniator_domain::HalftoneChannelModel::Cmyk) => [255, 255, 255],
        Some(toniator_domain::HalftoneChannelModel::SourceColorAlpha) | None => [128, 128, 128],
    }
}

/// Returns the output-target, placement, and exact two-axis PPI projection.
pub(crate) fn target_summary(
    target: OutputRasterTarget,
    settings: &PrintPreparationSettings,
) -> String {
    let target_text = format!("{} × {} px", target.width(), target.height());
    let Some(size) = settings.size_mm() else {
        return format!("{target_text} · physical size unknown · PPI unavailable");
    };
    let (width_mm, height_mm) = size.millimetres();
    match size.ppi_for_pixels(target.width(), target.height()) {
        Ok(ppi) => {
            let (horizontal, vertical) = ppi.axes();
            format!(
                "{target_text} · {} × {} mm · {} × {} PPI",
                format_summary_value(width_mm),
                format_summary_value(height_mm),
                format_summary_value(horizontal),
                format_summary_value(vertical)
            )
        }
        Err(_) => format!(
            "{target_text} · {} × {} mm · PPI unavailable",
            format_summary_value(width_mm),
            format_summary_value(height_mm)
        ),
    }
}

/// Returns an inspector summary that names applied intent and truthful check status.
pub(crate) fn inspector_summary(settings: &PrintPreparationSettings, status: &str) -> String {
    let size = settings.size_mm().map_or_else(
        || "Unknown size".to_owned(),
        |size| {
            let (width, height) = size.millimetres();
            format!(
                "{} × {} mm",
                format_summary_value(width),
                format_summary_value(height)
            )
        },
    );
    let positive = if settings.minimum_positive_feature_width_mm() == 0.0 {
        "positive width off".to_owned()
    } else {
        format!(
            "positive ≥ {} mm",
            format_summary_value(settings.minimum_positive_feature_width_mm())
        )
    };
    let gap = if settings.minimum_negative_gap_width_mm() == 0.0 {
        "gap width off".to_owned()
    } else {
        format!(
            "gap ≥ {} mm",
            format_summary_value(settings.minimum_negative_gap_width_mm())
        )
    };
    format!("{size} · {positive} · {gap} · {status}")
}

/// Resolves the effective export backing and whether PNG will flatten alpha.
pub(crate) fn export_backing_summary(
    choice: crate::print_preparation::BackgroundChoice,
    model: Option<toniator_domain::HalftoneChannelModel>,
) -> String {
    let effective = choice.resolve(model);
    let (provenance, color) = match (choice, effective) {
        (crate::print_preparation::BackgroundChoice::Automatic, RasterBackground::Transparent) => {
            ("Automatic", "transparent")
        }
        (crate::print_preparation::BackgroundChoice::Automatic, RasterBackground::OpaqueBlack) => {
            ("Automatic", "black")
        }
        (crate::print_preparation::BackgroundChoice::Automatic, RasterBackground::OpaqueWhite) => {
            ("Automatic", "white")
        }
        (_, RasterBackground::Transparent) => ("Explicit", "transparent"),
        (_, RasterBackground::OpaqueBlack) => ("Explicit", "black"),
        (_, RasterBackground::OpaqueWhite) => ("Explicit", "white"),
    };
    if effective == RasterBackground::Transparent {
        format!("PNG backing: {provenance} · {color}; transparency is retained.")
    } else {
        format!("PNG backing: {provenance} · {color}; export flattens transparency.")
    }
}

/// Counts exact RGBA display pixels with overflow protection.
pub(crate) fn display_bytes(target: OutputRasterTarget) -> Option<u64> {
    u64::from(target.width())
        .checked_mul(u64::from(target.height()))?
        .checked_mul(4)
}

/// Scales both preview layers by one factor while respecting GTK's bounded canvas dimensions.
pub(crate) fn preview_dimensions(target: OutputRasterTarget, requested_scale: f64) -> (i32, i32) {
    let width = f64::from(target.width());
    let height = f64::from(target.height());
    let scale = requested_scale
        .max(f64::MIN_POSITIVE)
        .min(MAX_PREVIEW_AXIS / width)
        .min(MAX_PREVIEW_AXIS / height);
    let scaled = |axis: f64| (axis * scale).round().clamp(1.0, MAX_PREVIEW_AXIS) as i32;
    (scaled(width), scaled(height))
}

/// Fits the complete raster into the current viewport without enlarging its native dimensions.
pub(crate) fn fitted_preview_dimensions(
    target: OutputRasterTarget,
    viewport_width: i32,
    viewport_height: i32,
) -> (i32, i32) {
    let scale = (f64::from(viewport_width.max(1)) / f64::from(target.width()))
        .min(f64::from(viewport_height.max(1)) / f64::from(target.height()))
        .min(1.0);
    preview_dimensions(target, scale)
}

/// Describes one exact support record without treating it as a unique physical defect.
#[allow(dead_code)] // Retained for the accepted G2a report tests.
pub(crate) fn support_row(component: &SupportComponent, category: FindingCategory) -> String {
    match category {
        FindingCategory::BoundaryAssociated => format!(
            "Component {} · {} boundary-associated low-coverage pixels · {} support pixels",
            component.id, component.boundary_associated_low_coverage_pixels, component.pixels
        ),
        FindingCategory::Unresolved => format!(
            "Component {} · {} unresolved low-coverage pixels",
            component.id, component.unresolved_low_coverage_pixels
        ),
        _ => format!(
            "Component {} · {} support pixels · max alpha {} · {} core pixels",
            component.id, component.pixels, component.max_alpha, component.core_pixels
        ),
    }
}

/// Describes one engine width candidate using pixel counts rather than defect language.
#[allow(dead_code)] // Retained for the accepted G2a report tests.
pub(crate) fn candidate_row(candidate: &ProbeCandidate) -> String {
    let side = if candidate.negative_gap {
        "Gap"
    } else {
        "Positive width"
    };
    format!(
        "{side} candidate · component {} · {} pixels · {:?}",
        candidate.original_component_id, candidate.pixels, candidate.kind
    )
}

/// Returns one source-owned record's bounds for exact location navigation.
pub(crate) fn record_bounds(
    report: &PreflightReport,
    raster: &RasterSurface,
    category: FindingCategory,
    index: usize,
) -> Result<Option<PixelBounds>, String> {
    let bounds = match category.record_at(report, index) {
        Some(FindingRecord::Support(component)) => match category {
            FindingCategory::BoundaryAssociated => boundary_bounds(raster, component)?,
            FindingCategory::Unresolved => bounds_for_runs(&component.unresolved_low_coverage_runs),
            FindingCategory::Support => Some(component.bounds),
            FindingCategory::PositiveWidth | FindingCategory::NegativeGap => None,
        },
        Some(FindingRecord::Candidate(candidate)) => Some(candidate.bounds),
        None => None,
    };
    Ok(bounds)
}

/// Bounds one exact published run sequence without retaining an extra run or pixel vector.
fn bounds_for_runs(runs: &[PixelRun]) -> Option<PixelBounds> {
    let first = runs.first()?;
    let mut bounds = PixelBounds {
        x0: first.x0,
        y0: first.y,
        x1_exclusive: first.x1_exclusive,
        y1_exclusive: first.y + 1,
    };
    for run in &runs[1..] {
        bounds.x0 = bounds.x0.min(run.x0);
        bounds.y0 = bounds.y0.min(run.y);
        bounds.x1_exclusive = bounds.x1_exclusive.max(run.x1_exclusive);
        bounds.y1_exclusive = bounds.y1_exclusive.max(run.y.saturating_add(1));
    }
    Some(bounds)
}

/// Bounds exactly the boundary-associated alpha subset used by its selected-run overlay.
fn boundary_bounds(
    raster: &RasterSurface,
    component: &SupportComponent,
) -> Result<Option<PixelBounds>, String> {
    let width = raster.width();
    let bytes = raster.pixels();
    let unresolved = &component.unresolved_low_coverage_runs;
    let mut unresolved_index = 0;
    let mut bounds: Option<PixelBounds> = None;
    let mut count = 0_u64;
    for support in &component.runs {
        for x in support.x0..support.x1_exclusive {
            while unresolved_index < unresolved.len()
                && (unresolved[unresolved_index].y < support.y
                    || (unresolved[unresolved_index].y == support.y
                        && unresolved[unresolved_index].x1_exclusive <= x))
            {
                unresolved_index += 1;
            }
            let is_unresolved = unresolved
                .get(unresolved_index)
                .is_some_and(|run| run.y == support.y && run.x0 <= x && x < run.x1_exclusive);
            let pixel_index = (u64::from(support.y) * u64::from(width) + u64::from(x))
                .checked_mul(4)
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(|| "Review pixel index exceeds the retained raster.".to_owned())?;
            let alpha = *bytes
                .get(pixel_index + 3)
                .ok_or_else(|| "Review support run exceeds the retained raster.".to_owned())?;
            if alpha > 0 && alpha < CORE_ALPHA && !is_unresolved {
                let pixel_bounds = PixelBounds {
                    x0: x,
                    y0: support.y,
                    x1_exclusive: x.saturating_add(1),
                    y1_exclusive: support.y.saturating_add(1),
                };
                bounds = Some(bounds.map_or(pixel_bounds, |mut current| {
                    current.x0 = current.x0.min(pixel_bounds.x0);
                    current.y0 = current.y0.min(pixel_bounds.y0);
                    current.x1_exclusive = current.x1_exclusive.max(pixel_bounds.x1_exclusive);
                    current.y1_exclusive = current.y1_exclusive.max(pixel_bounds.y1_exclusive);
                    current
                }));
                count += 1;
            }
        }
    }
    if count != component.boundary_associated_low_coverage_pixels {
        return Err("Boundary-associated runs do not match the engine inventory.".into());
    }
    Ok(bounds)
}

/// Builds one bounded transparent highlight raster from exact engine runs.
///
/// Boundary-associated faint coverage is projected from the engine's support runs and alpha
/// bytes, excluding its exact unresolved runs. This classifies published pixels only; it does
/// not repeat neighborhood morphology or modify the retained transparent source raster.
///
/// # Errors
/// Rejects an invalid target, an oversized display allocation, allocation failure, or a
/// boundary-associated pixel count that disagrees with the engine's published inventory.
pub(crate) fn highlight_rgba(
    pair: &PreflightRasterReport,
    category: FindingCategory,
    index: usize,
) -> Result<Vec<u8>, String> {
    let report = pair.report();
    let raster = pair.raster();
    let target = report.identity.target();
    let byte_count = display_bytes(target)
        .filter(|bytes| *bytes <= MAX_REVIEW_DISPLAY_BYTES)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| "Selected target exceeds the review display budget.".to_owned())?;
    if raster.width() != target.width() || raster.height() != target.height() {
        return Err("Review raster dimensions do not match the selected target.".into());
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(byte_count)
        .map_err(|_| "Review highlight allocation failed.".to_owned())?;
    pixels.resize(byte_count, 0);
    match category.record_at(report, index) {
        Some(FindingRecord::Support(component)) => match category {
            FindingCategory::BoundaryAssociated => {
                paint_boundary_runs(&mut pixels, raster, component)?;
            }
            FindingCategory::Unresolved => {
                paint_runs(
                    &mut pixels,
                    target.width(),
                    &component.unresolved_low_coverage_runs,
                );
            }
            _ => paint_runs(&mut pixels, target.width(), &component.runs),
        },
        Some(FindingRecord::Candidate(candidate)) => {
            paint_runs(&mut pixels, target.width(), &candidate.runs);
        }
        None => return Err("Selected finding is no longer available.".into()),
    }
    Ok(pixels)
}

/// Transfers one validated RGBA allocation to a GTK memory texture.
///
/// The vector moves into `glib::Bytes`; no second CPU-side overlay copy is kept by this helper.
///
/// # Errors
/// Rejects invalid dimensions, a mismatched byte length, or stride overflow.
pub(crate) fn texture_from_rgba(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<gtk::gdk::Texture, String> {
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| "Review texture dimensions overflow.".to_owned())?;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err("Review texture dimensions do not match its RGBA bytes.".into());
    }
    let stride = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| "Review texture row stride overflows.".to_owned())?;
    let bytes = glib::Bytes::from_owned(rgba);
    Ok(gtk::gdk::MemoryTexture::new(
        i32::try_from(width).map_err(|_| "Review texture width is too large.".to_owned())?,
        i32::try_from(height).map_err(|_| "Review texture height is too large.".to_owned())?,
        gtk::gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        stride,
    )
    .upcast())
}

/// Paints exact engine runs with alternating high-contrast viewer-only pixels.
fn paint_runs(pixels: &mut [u8], width: u32, runs: &[PixelRun]) {
    for run in runs {
        for x in run.x0..run.x1_exclusive {
            paint_highlight_pixel(pixels, width, x, run.y);
        }
    }
}

/// Projects boundary-associated low coverage by excluding exact unresolved support runs.
fn paint_boundary_runs(
    pixels: &mut [u8],
    raster: &RasterSurface,
    component: &SupportComponent,
) -> Result<(), String> {
    let width = raster.width();
    let bytes = raster.pixels();
    let unresolved = &component.unresolved_low_coverage_runs;
    let mut unresolved_index = 0;
    let mut count = 0_u64;
    for support in &component.runs {
        for x in support.x0..support.x1_exclusive {
            while unresolved_index < unresolved.len()
                && (unresolved[unresolved_index].y < support.y
                    || (unresolved[unresolved_index].y == support.y
                        && unresolved[unresolved_index].x1_exclusive <= x))
            {
                unresolved_index += 1;
            }
            let is_unresolved = unresolved
                .get(unresolved_index)
                .is_some_and(|run| run.y == support.y && run.x0 <= x && x < run.x1_exclusive);
            let pixel_index = (u64::from(support.y) * u64::from(width) + u64::from(x))
                .checked_mul(4)
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(|| "Review pixel index exceeds the display buffer.".to_owned())?;
            let alpha = *bytes
                .get(pixel_index + 3)
                .ok_or_else(|| "Review support run exceeds the retained raster.".to_owned())?;
            if alpha > 0 && alpha < CORE_ALPHA && !is_unresolved {
                paint_highlight_pixel(pixels, width, x, support.y);
                count += 1;
            }
        }
    }
    if count != component.boundary_associated_low_coverage_pixels {
        return Err("Boundary-associated runs do not match the engine inventory.".into());
    }
    Ok(())
}

/// Sets one alternating checker pixel in the overlay without touching source RGBA.
fn paint_highlight_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32) {
    let Some(offset) = (u64::from(y) * u64::from(width) + u64::from(x))
        .checked_mul(4)
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return;
    };
    if let Some(pixel) = pixels.get_mut(offset..offset + 4) {
        let color = if (x ^ y) & 1 == 0 {
            [255, 255, 0, 220]
        } else {
            [0, 0, 0, 220]
        };
        pixel.copy_from_slice(&color);
    }
}

/// Formats independent width checks from the engine's published status values.
pub(crate) fn width_status_summary(report: &PreflightReport) -> String {
    use toniator_engine::print_preflight::WidthCheckStatus;

    /// Maps one engine status to the concise status text shown in the inspector.
    fn label(status: WidthCheckStatus) -> &'static str {
        match status {
            WidthCheckStatus::Disabled => "disabled",
            WidthCheckStatus::UnavailablePlacement => "physical size unavailable",
            WidthCheckStatus::SamplingLimited => "sampling limited",
            WidthCheckStatus::AdvisoryCandidates => "advisory candidates",
            WidthCheckStatus::NoWidthCandidatesWithUnresolved => {
                "no width candidates; unresolved support remains"
            }
        }
    }
    format!(
        "Positive width: {} · Gap width: {}",
        label(report.positive_status),
        label(report.negative_status)
    )
}

/// Parses a finite positive scalar while retaining the visible field name in diagnostics.
///
/// # Errors
/// Returns an error when the text is not finite numeric input greater than zero.
fn parse_finite_positive(value: &str, field: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{field} must be a number greater than zero."))?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(format!("{field} must be a number greater than zero."));
    }
    Ok(parsed)
}

/// Parses a finite nonnegative scalar while retaining the visible field name in diagnostics.
///
/// # Errors
/// Returns an error when the text is not finite numeric input at least zero.
fn parse_finite_nonnegative(value: &str, field: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{field} must be a finite number at least zero."))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("{field} must be a finite number at least zero."));
    }
    Ok(if parsed == 0.0 { 0.0 } else { parsed })
}

/// Formats an entry value without introducing locale-dependent parsing requirements.
fn format_decimal(value: f64) -> String {
    value.to_string()
}

/// Keeps accepted nonzero applied values visible at the precision stored by document history.
fn format_summary_value(value: f64) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps highly rectangular zoomed rasters at one aspect-preserving scale on both layers.
    #[test]
    fn preview_zoom_clamps_rectangular_raster_with_a_common_scale() {
        let target = OutputRasterTarget::new(8_000, 1_000).unwrap();
        assert_eq!(preview_dimensions(target, 6.0), (32_768, 4_096));
        assert_eq!(fitted_preview_dimensions(target, 1_000, 800), (1_000, 125));
    }

    /// Admits a normal 12×16-inch 300-DPI exact preview without using review-pair limits.
    ///
    /// # Panics
    /// Panics if the checked payload or truthful zoom limit regresses.
    #[test]
    fn common_print_preview_has_separate_checked_budget() {
        let target = OutputRasterTarget::new(3_600, 4_800).unwrap();
        assert_eq!(display_bytes(target), Some(69_120_000));
        assert!(display_bytes(target).unwrap() <= MAX_PREPARED_PREVIEW_BYTES);
        assert!(can_show_exact_zoom(target, 2.0));
        let too_wide = OutputRasterTarget::new(40_000, 100).unwrap();
        assert!(!can_show_exact_zoom(too_wide, 1.0));
        assert!(can_show_exact_zoom(too_wide, 0.5));
    }

    /// Converts the standard 12×16-inch print box to and from millimetres without changing its target.
    ///
    /// # Panics
    /// Panics if the checked 300-DPI target or exact physical dimensions change during unit toggles.
    #[test]
    fn print_box_units_round_trip_twelve_by_sixteen_inches_at_300_dpi() {
        let canvas = toniator_domain::CanvasSpec {
            width: 3_600.0,
            height: 4_800.0,
        };
        let mut draft = PrintBoxDraft::from_mm(Some((304.8, 406.4)));
        let before = crate::png_export_size::fit_print_box(&canvas, 304.8, 406.4, 300.0).unwrap();
        assert_eq!(before.target().width(), 3_600);
        assert_eq!(before.target().height(), 4_800);
        assert_eq!(
            (
                format_box_dimension(draft.width_mm, draft.unit()).unwrap(),
                format_box_dimension(draft.height_mm, draft.unit()).unwrap(),
            ),
            ("304.8".into(), "406.4".into())
        );

        assert_eq!(
            draft.change_unit(EntryUnit::Inches).unwrap(),
            ("12".into(), "16".into())
        );
        assert_eq!(draft.dimensions_for_fit_mm().unwrap(), (304.8, 406.4));
        for _ in 0..10 {
            let (width, height) = draft.change_unit(EntryUnit::Millimetres).unwrap();
            assert_eq!((width.as_str(), height.as_str()), ("304.8", "406.4"));
            let (width, height) = draft.change_unit(EntryUnit::Inches).unwrap();
            assert_eq!((width.as_str(), height.as_str()), ("12", "16"));
        }
        let (width_mm, height_mm) = draft.dimensions_for_fit_mm().unwrap();
        let after =
            crate::png_export_size::fit_print_box(&canvas, width_mm, height_mm, 300.0).unwrap();
        assert_eq!(after.target(), before.target());
    }

    /// Keeps arbitrary fractional inch edits and their fitted pixel target stable over repeated toggles.
    ///
    /// # Panics
    /// Panics if shortest-roundtrip formatting changes canonical millimetres or the 300-DPI target.
    #[test]
    fn print_box_fractional_inches_keep_canonical_mm_and_target_without_drift() {
        let canvas = toniator_domain::CanvasSpec {
            width: 1_000.0,
            height: 1_000.0,
        };
        let mut draft = PrintBoxDraft::from_mm(None);
        draft.change_unit(EntryUnit::Inches).unwrap();
        draft.update_width("4.861290906785656").unwrap();
        draft.update_height("10.11785871915962").unwrap();
        let canonical = draft.dimensions_for_fit_mm().unwrap();
        let before =
            crate::png_export_size::fit_print_box(&canvas, canonical.0, canonical.1, 300.0)
                .unwrap();

        for _ in 0..50 {
            draft.change_unit(EntryUnit::Millimetres).unwrap();
            draft.change_unit(EntryUnit::Inches).unwrap();
        }
        assert_eq!(draft.dimensions_for_fit_mm().unwrap(), canonical);
        let after = crate::png_export_size::fit_print_box(&canvas, canonical.0, canonical.1, 300.0)
            .unwrap();
        assert_eq!(after.target(), before.target());
        assert_eq!(
            format_box_dimension(draft.width_mm, draft.unit()).unwrap(),
            "4.861290906785656"
        );
    }

    /// Proves blank placement is Unknown only when both axes are blank and thresholds stay active.
    #[test]
    fn intent_draft_requires_complete_physical_size_and_preserves_independent_thresholds() {
        let unknown = IntentDraft {
            unit: EntryUnit::Millimetres,
            width: String::new(),
            height: String::new(),
            positive_width: "0".into(),
            negative_gap: "1.25".into(),
        }
        .parse()
        .unwrap();
        assert_eq!(unknown.size_mm(), None);
        assert_eq!(unknown.minimum_positive_feature_width_mm(), 0.0);
        assert_eq!(unknown.minimum_negative_gap_width_mm(), 1.25);

        let partial = IntentDraft {
            width: "120".into(),
            ..IntentDraft::default()
        };
        assert!(
            partial
                .parse()
                .unwrap_err()
                .contains("both physical dimensions")
        );
    }

    /// Proves inch entry converts the complete placement and both thresholds to canonical mm.
    #[test]
    fn intent_draft_converts_inches_without_persisting_units() {
        let settings = IntentDraft {
            unit: EntryUnit::Inches,
            width: "12".into(),
            height: "8".into(),
            positive_width: "0.1".into(),
            negative_gap: "0".into(),
        }
        .parse()
        .unwrap();
        let size = settings.size_mm().unwrap();
        let (width_mm, height_mm) = size.millimetres();
        assert!((width_mm - 304.8).abs() < 1e-10);
        assert!((height_mm - 203.2).abs() < 1e-10);
        assert!((settings.minimum_positive_feature_width_mm() - 2.54).abs() < 1e-12);
        assert_eq!(settings.minimum_negative_gap_width_mm(), 0.0);
        assert_eq!(
            IntentDraft::from_settings(&settings, EntryUnit::Inches)
                .unwrap()
                .unit,
            EntryUnit::Inches
        );
    }

    /// Preserves tiny and non-decimal applied values through an untouched millimetre draft.
    #[test]
    fn intent_draft_round_trips_tiny_positive_and_non_four_decimal_thresholds() {
        let settings =
            PrintPreparationSettings::new(None, 0.000_012_345_678_9, 0.123_456_789).unwrap();
        let round_trip = IntentDraft::from_settings(&settings, EntryUnit::Millimetres)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(round_trip, settings);
    }

    /// Preserves canonical untouched fields across unit projection and a different-field edit.
    #[test]
    fn edited_draft_keeps_untouched_mm_values_exact_after_inches_projection() {
        let base = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(0.025, 0.05).unwrap()),
            0.025,
            0.05,
        )
        .unwrap();
        let mut inches = IntentDraft::from_settings(&base, EntryUnit::Inches).unwrap();
        inches.negative_gap = "0.2".into();
        let applied = inches
            .parse_over(&base, [false, false, false, true])
            .unwrap();
        assert_eq!(
            applied.size_mm().unwrap().millimetres().0,
            0.025,
            "untouched width is copied from canonical applied settings"
        );
        assert_eq!(
            applied.minimum_positive_feature_width_mm(),
            0.025,
            "untouched positive threshold is copied from canonical settings"
        );
        assert!((applied.minimum_negative_gap_width_mm() - 5.08).abs() < 1e-12);
    }

    /// Rebased dirty drafts apply one edited threshold without restoring an undone width.
    #[test]
    fn dirty_draft_rebases_untouched_width_to_live_history_before_apply() {
        let before_undo = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(100.0, 50.0).unwrap()),
            0.1,
            0.2,
        )
        .unwrap();
        let after_undo = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(80.0, 50.0).unwrap()),
            0.1,
            0.2,
        )
        .unwrap();
        let mut draft = IntentDraft::from_settings(&before_undo, EntryUnit::Millimetres).unwrap();
        draft.positive_width = "0.3".into();
        let changed = [false, false, true, false];
        draft.rebase_untouched(&after_undo, changed).unwrap();
        let applied = draft.parse_over(&after_undo, changed).unwrap();
        assert_eq!(draft.width, "80");
        assert_eq!(applied.size_mm().unwrap().millimetres().0, 80.0);
        assert_eq!(applied.minimum_positive_feature_width_mm(), 0.3);
    }

    /// Keeps tiny accepted physical sizes and thresholds visibly nonzero in applied summaries.
    #[test]
    fn applied_summaries_do_not_round_small_positive_values_to_zero() {
        let settings = PrintPreparationSettings::new(
            Some(PhysicalPrintSizeMm::new(0.000_012_345_678_9, 0.123_456_789).unwrap()),
            0.000_012_345_678_9,
            0.000_001_234_567_89,
        )
        .unwrap();
        let inspector = inspector_summary(&settings, "Not checked");
        let target = target_summary(OutputRasterTarget::new(1024, 512).unwrap(), &settings);
        assert!(inspector.contains("0.0000123456789"));
        assert!(inspector.contains("0.00000123456789"));
        assert!(target.contains("1024 × 512 px"));
        assert!(target.contains("0.0000123456789 × 0.123456789 mm"));
    }

    /// Keeps explicit garment color parsing bounded to a standard RGB hex triplet.
    #[test]
    fn garment_hex_color_requires_six_rgb_digits() {
        assert_eq!(parse_hex_color("#7040a0").unwrap(), [112, 64, 160]);
        assert!(parse_hex_color("7040A0").is_err());
        assert!(parse_hex_color("#fff").is_err());
        assert!(parse_hex_color("#gg0000").is_err());
        assert!(parse_hex_color("#é00000").is_err());
    }

    /// Preserves the actual target while marking physical dimensions unavailable when Unknown.
    #[test]
    fn target_summary_never_hides_known_pixel_dimensions() {
        let target = OutputRasterTarget::new(2048, 1024).unwrap();
        let text = target_summary(target, &PrintPreparationSettings::default());
        assert!(text.starts_with("2048 × 1024 px"));
        assert!(text.contains("PPI unavailable"));
    }

    /// Names automatic model-specific backing separately from transparent-raster analysis.
    #[test]
    fn export_backing_summary_reports_automatic_effective_color_and_flattening() {
        let automatic = crate::print_preparation::BackgroundChoice::Automatic;
        assert!(
            export_backing_summary(automatic, Some(toniator_domain::HalftoneChannelModel::Rgb))
                .contains("Automatic · black")
        );
        assert!(
            export_backing_summary(automatic, Some(toniator_domain::HalftoneChannelModel::Cmyk))
                .contains("flattens transparency")
        );
        assert!(
            export_backing_summary(
                automatic,
                Some(toniator_domain::HalftoneChannelModel::SourceColorAlpha)
            )
            .contains("transparency is retained")
        );
    }
}
