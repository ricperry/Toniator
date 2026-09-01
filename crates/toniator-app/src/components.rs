//! Composite GTK resource components used by the Toniator application shell.
//!
//! These types own stable presentation structure only. They deliberately do
//! not own a document, history, evaluator, or persistence state.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod main_shell {
    use super::*;

    /// Installs the resource-defined GTK main-window composition.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/window.ui")]
    pub struct ToniatorMainShell {
        #[template_child]
        pub header: gtk::TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub main_banner_revealer: gtk::TemplateChild<gtk::Revealer>,
        #[template_child]
        pub main_banner: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub main_banner_dismiss: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub workspace_split: gtk::TemplateChild<gtk::Paned>,
        #[template_child]
        pub file_button: gtk::TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub channel_settings_drawer: gtk::TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub window_title: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub page_stack: gtk::TemplateChild<gtk::Stack>,
        #[template_child]
        pub preview_picture: gtk::TemplateChild<gtk::Picture>,
        #[template_child]
        pub viewer: gtk::TemplateChild<gtk::Overlay>,
        #[template_child]
        pub preview_progress: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub preview_overall_progress_label: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub preview_progress_label: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub preview_progress_bar: gtk::TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub preview_stage_progress_bar: gtk::TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub error_label: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub inspector_scroll: gtk::TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub inspector_status: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub model_selector: gtk::TemplateChild<gtk::DropDown>,
        #[template_child]
        pub channel_selector: gtk::TemplateChild<gtk::DropDown>,
        #[template_child]
        pub inspector_catalog: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub inspector_descriptors: gtk::TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorMainShell {
        const NAME: &'static str = "ToniatorMainShell";
        type Type = super::ToniatorMainShell;
        type ParentType = gtk::Box;

        /// Binds the compiled main-window template once for this widget class.
        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        /// Initializes one main-window shell from the bound resource template.
        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorMainShell {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorMainShell {}
    impl gtk::subclass::box_::BoxImpl for ToniatorMainShell {}
}

glib::wrapper! {
    /// Provides the stable resource-owned shell around the canvas and sidebar.
    pub struct ToniatorMainShell(ObjectSubclass<main_shell::ToniatorMainShell>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorMainShell {
    /// Creates the immutable resource-defined shell before dynamic views attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Applies one visible banner message without affecting application authority.
    pub fn set_banner(&self, message: Option<&str>) {
        self.imp()
            .main_banner
            .set_label(message.unwrap_or_default());
        self.imp()
            .main_banner_revealer
            .set_reveal_child(message.is_some());
    }

    /// Returns the explicit control that hides the currently presented banner message.
    pub fn banner_dismiss(&self) -> gtk::Button {
        self.imp().main_banner_dismiss.get()
    }

    /// Returns the template-owned conventional GTK split container.
    pub fn split(&self) -> gtk::Paned {
        self.imp().workspace_split.get()
    }

    /// Returns the static File menu button whose menu model is runtime-owned.
    pub fn file_button(&self) -> gtk::MenuButton {
        self.imp().file_button.get()
    }

    /// Returns the static channel-settings visibility control.
    pub fn drawer(&self) -> gtk::ToggleButton {
        self.imp().channel_settings_drawer.get()
    }

    /// Transfers the Blueprint-owned header out of the content box so the
    /// application window can register it as its sole client-side titlebar.
    pub fn detach_titlebar(&self) -> gtk::HeaderBar {
        let header = self.imp().header.get();
        self.remove(&header);
        header
    }

    /// Returns the template-owned window title label.
    pub fn title(&self) -> gtk::Label {
        self.imp().window_title.get()
    }

    /// Returns the template-owned page stack.
    pub fn stack(&self) -> gtk::Stack {
        self.imp().page_stack.get()
    }

    /// Returns the template-owned canonical preview picture.
    pub fn picture(&self) -> gtk::Picture {
        self.imp().preview_picture.get()
    }

    /// Returns the template-owned preview overlay.
    pub fn viewer(&self) -> gtk::Overlay {
        self.imp().viewer.get()
    }

    /// Returns the template-owned main-preview progress overlay.
    pub fn progress(&self) -> gtk::Box {
        self.imp().preview_progress.get()
    }

    /// Returns the visible overall-preview progress label.
    pub fn overall_progress_label(&self) -> gtk::Label {
        self.imp().preview_overall_progress_label.get()
    }

    /// Returns the current main-preview phase label.
    pub fn progress_label(&self) -> gtk::Label {
        self.imp().preview_progress_label.get()
    }

    /// Returns the determinate main-preview progress bar.
    pub fn progress_bar(&self) -> gtk::ProgressBar {
        self.imp().preview_progress_bar.get()
    }

    /// Returns determinate completion within the currently named preview stage.
    pub fn stage_progress_bar(&self) -> gtk::ProgressBar {
        self.imp().preview_stage_progress_bar.get()
    }

    /// Returns the template-owned error presentation label.
    pub fn error(&self) -> gtk::Label {
        self.imp().error_label.get()
    }

    /// Returns the template-owned sidebar scroll surface.
    pub fn inspector_scroll(&self) -> gtk::ScrolledWindow {
        self.imp().inspector_scroll.get()
    }

    /// Returns the template-owned sidebar status label.
    pub fn inspector_status(&self) -> gtk::Label {
        self.imp().inspector_status.get()
    }

    /// Returns the template-owned model selector.
    pub fn model_selector(&self) -> gtk::DropDown {
        self.imp().model_selector.get()
    }

    /// Returns the template-owned selected-channel selector.
    pub fn channel_selector(&self) -> gtk::DropDown {
        self.imp().channel_selector.get()
    }

    /// Returns the dynamic catalog slot beneath the static sidebar controls.
    pub fn inspector_catalog(&self) -> gtk::Box {
        self.imp().inspector_catalog.get()
    }

    /// Returns the dynamic descriptor slot beneath the static sidebar controls.
    pub fn inspector_descriptors(&self) -> gtk::Box {
        self.imp().inspector_descriptors.get()
    }
}

impl Default for ToniatorMainShell {
    fn default() -> Self {
        Self::new()
    }
}

mod channel_editor {
    use super::*;

    /// Installs the resource-owned persistent channel-editor structure.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/channel-editor.ui")]
    pub struct ToniatorChannelEditor {
        #[template_child]
        pub editor_status: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub editor_content: gtk::TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorChannelEditor {
        const NAME: &'static str = "ToniatorChannelEditor";
        type Type = super::ToniatorChannelEditor;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorChannelEditor {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorChannelEditor {}
    impl gtk::subclass::box_::BoxImpl for ToniatorChannelEditor {}
}

glib::wrapper! {
    /// Provides template-owned status and dynamic-content slots for one channel.
    pub struct ToniatorChannelEditor(ObjectSubclass<channel_editor::ToniatorChannelEditor>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorChannelEditor {
    /// Creates the stable sidebar editor before immutable view rows attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Returns the template-owned artist feedback label.
    pub fn status(&self) -> gtk::Label {
        self.imp().editor_status.get()
    }

    /// Returns the template-owned dynamic channel-control container.
    pub fn content(&self) -> gtk::Box {
        self.imp().editor_content.get()
    }
}

impl Default for ToniatorChannelEditor {
    fn default() -> Self {
        Self::new()
    }
}

mod pattern_editor_shell {
    use super::*;

    /// Installs the resource-owned private Pattern Editor layout.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/pattern-editor.ui")]
    pub struct ToniatorPatternEditorShell {
        #[template_child]
        pub draft_status: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub draft_preview: gtk::TemplateChild<gtk::Picture>,
        #[template_child]
        pub draft_preview_spinner: gtk::TemplateChild<gtk::Spinner>,
        #[template_child]
        pub draft_scroll: gtk::TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub draft_actions: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub draft_introduction: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub draft_history: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub draft_current_pattern: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub construction_layout: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub construction_sidebar: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub resource_list: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub new_structure: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub construction_canvas_heading: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub construction_canvas: gtk::TemplateChild<gtk::DrawingArea>,
        #[template_child]
        pub coordinate_x: gtk::TemplateChild<gtk::Entry>,
        #[template_child]
        pub coordinate_y: gtk::TemplateChild<gtk::Entry>,
        #[template_child]
        pub selected_point_label: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub primary_rows: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub advanced_rows: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub make_curve: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub make_line: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub insert_node: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub delete_node: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub motif_direction_row: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub smooth_direction: gtk::TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub corner_direction: gtk::TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub motif_terminal_handle_row: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub edit_left_terminal_handle: gtk::TemplateChild<gtk::Button>,
        #[template_child]
        pub edit_right_terminal_handle: gtk::TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorPatternEditorShell {
        const NAME: &'static str = "ToniatorPatternEditorShell";
        type Type = super::ToniatorPatternEditorShell;
        type ParentType = gtk::Box;
        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }
        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }
    impl glib::subclass::object::ObjectImpl for ToniatorPatternEditorShell {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorPatternEditorShell {}
    impl gtk::subclass::box_::BoxImpl for ToniatorPatternEditorShell {}
}

glib::wrapper! {
    /// Provides the template-owned private editor structure.
    pub struct ToniatorPatternEditorShell(ObjectSubclass<pattern_editor_shell::ToniatorPatternEditorShell>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorPatternEditorShell {
    /// Creates a private editor shell before dynamic draft rows attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
    /// Returns the template-owned draft status label.
    pub fn status(&self) -> gtk::Label {
        self.imp().draft_status.get()
    }
    /// Returns the template-owned private-preview paintable surface.
    pub fn picture(&self) -> gtk::Picture {
        self.imp().draft_preview.get()
    }
    /// Returns the template-owned private-preview pending indicator.
    pub fn spinner(&self) -> gtk::Spinner {
        self.imp().draft_preview_spinner.get()
    }
    /// Appends a stable footer action to the template-owned action row.
    pub fn append_action(&self, action: &impl IsA<gtk::Widget>) {
        self.imp().draft_actions.append(action);
    }
    /// Returns the static draft disclosure label.
    pub fn introduction(&self) -> gtk::Label {
        self.imp().draft_introduction.get()
    }
    /// Returns the template-owned draft-history label.
    pub fn history(&self) -> gtk::Label {
        self.imp().draft_history.get()
    }
    /// Returns the template-owned current-pattern label.
    pub fn current_pattern(&self) -> gtk::Label {
        self.imp().draft_current_pattern.get()
    }
    /// Returns the GTK-owned construction container whose orientation may follow window width.
    pub fn construction_layout(&self) -> gtk::Box {
        self.imp().construction_layout.get()
    }
    /// Returns the GTK-owned construction sidebar whose width follows the reflow policy.
    pub fn construction_sidebar(&self) -> gtk::Box {
        self.imp().construction_sidebar.get()
    }
    /// Returns the dynamic authored-resource list slot.
    pub fn resource_list(&self) -> gtk::Box {
        self.imp().resource_list.get()
    }
    /// Returns the purpose-labelled construction trigger.
    pub fn new_structure(&self) -> gtk::Button {
        self.imp().new_structure.get()
    }
    /// Returns the visible heading that labels the interactive construction canvas.
    pub fn construction_canvas_heading(&self) -> gtk::Label {
        self.imp().construction_canvas_heading.get()
    }
    /// Returns the dynamic private-draft drawing surface.
    pub fn construction_canvas(&self) -> gtk::DrawingArea {
        self.imp().construction_canvas.get()
    }
    /// Returns the selected-anchor X entry.
    pub fn coordinate_x(&self) -> gtk::Entry {
        self.imp().coordinate_x.get()
    }
    /// Returns the selected-anchor Y entry.
    pub fn coordinate_y(&self) -> gtk::Entry {
        self.imp().coordinate_y.get()
    }
    /// Returns the visible selection label paired with construction-coordinate editing.
    pub fn selected_point_label(&self) -> gtk::Label {
        self.imp().selected_point_label.get()
    }
    /// Returns the dynamic ordinary-descriptor slot.
    pub fn primary_rows(&self) -> gtk::Box {
        self.imp().primary_rows.get()
    }
    /// Returns the dynamic advanced-descriptor slot.
    pub fn advanced_rows(&self) -> gtk::Box {
        self.imp().advanced_rows.get()
    }
    /// Returns the static line-to-curve action control.
    pub fn make_curve(&self) -> gtk::Button {
        self.imp().make_curve.get()
    }
    /// Returns the static curve-to-line action control.
    pub fn make_line(&self) -> gtk::Button {
        self.imp().make_line.get()
    }
    /// Returns the static segment-splitting action control.
    pub fn insert_node(&self) -> gtk::Button {
        self.imp().insert_node.get()
    }
    /// Returns the static selected-node deletion action control.
    pub fn delete_node(&self) -> gtk::Button {
        self.imp().delete_node.get()
    }
    /// Returns the Motif-only terminal-direction presentation group.
    pub fn motif_direction_row(&self) -> gtk::Box {
        self.imp().motif_direction_row.get()
    }
    /// Returns the local Smooth-direction choice for Curve Motif terminal handles.
    pub fn smooth_direction(&self) -> gtk::ToggleButton {
        self.imp().smooth_direction.get()
    }
    /// Returns the local Corner-direction choice for Curve Motif terminal handles.
    pub fn corner_direction(&self) -> gtk::ToggleButton {
        self.imp().corner_direction.get()
    }
    /// Returns the Motif-only terminal-handle selection actions container.
    pub fn motif_terminal_handle_row(&self) -> gtk::Box {
        self.imp().motif_terminal_handle_row.get()
    }
    /// Returns the explicit left terminal-handle selection or conversion action.
    pub fn edit_left_terminal_handle(&self) -> gtk::Button {
        self.imp().edit_left_terminal_handle.get()
    }
    /// Returns the explicit right terminal-handle selection or conversion action.
    pub fn edit_right_terminal_handle(&self) -> gtk::Button {
        self.imp().edit_right_terminal_handle.get()
    }
}

impl Default for ToniatorPatternEditorShell {
    fn default() -> Self {
        Self::new()
    }
}

mod preset_row {
    use super::*;

    /// Installs one stable resource-owned pattern catalog row.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/preset-row.ui")]
    pub struct ToniatorPresetRow {
        #[template_child]
        pub preset_name: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub preset_description: gtk::TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorPresetRow {
        const NAME: &'static str = "ToniatorPresetRow";
        type Type = super::ToniatorPresetRow;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorPresetRow {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorPresetRow {}
    impl gtk::subclass::box_::BoxImpl for ToniatorPresetRow {}
}

glib::wrapper! {
    /// Provides stable template-owned presentation for one named preset.
    pub struct ToniatorPresetRow(ObjectSubclass<preset_row::ToniatorPresetRow>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorPresetRow {
    /// Creates an empty catalog row before immutable preset text is projected.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Sets the artist-facing name while retaining resource-owned structure.
    pub fn set_preset_name(&self, name: &str) {
        self.imp().preset_name.set_label(name);
    }

    /// Sets the concise artist-facing description for this stable catalog item.
    pub fn set_preset_description(&self, description: &str) {
        self.imp().preset_description.set_label(description);
    }
}

impl Default for ToniatorPresetRow {
    fn default() -> Self {
        Self::new()
    }
}

mod advanced_settings_shell {
    use super::*;

    /// Installs the Blueprint-owned private Advanced Settings composition.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/advanced-settings.ui")]
    pub struct ToniatorAdvancedSettingsShell {
        #[template_child]
        pub advanced_status: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub advanced_preview: gtk::TemplateChild<gtk::Picture>,
        #[template_child]
        pub advanced_controls: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub advanced_actions: gtk::TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorAdvancedSettingsShell {
        const NAME: &'static str = "ToniatorAdvancedSettingsShell";
        type Type = super::ToniatorAdvancedSettingsShell;
        type ParentType = gtk::Box;

        /// Binds the compiled Advanced Settings template once for this widget class.
        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        /// Initializes one private-settings shell from the bound resource template.
        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorAdvancedSettingsShell {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorAdvancedSettingsShell {}
    impl gtk::subclass::box_::BoxImpl for ToniatorAdvancedSettingsShell {}
}

glib::wrapper! {
    /// Provides the stable private Advanced Settings layout and dynamic slots.
    pub struct ToniatorAdvancedSettingsShell(ObjectSubclass<advanced_settings_shell::ToniatorAdvancedSettingsShell>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorAdvancedSettingsShell {
    /// Creates the resource-owned modal shell before private controls attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Returns the private draft status presentation.
    pub fn status(&self) -> gtk::Label {
        self.imp().advanced_status.get()
    }

    /// Returns the canonical private-preview picture.
    pub fn preview(&self) -> gtk::Picture {
        self.imp().advanced_preview.get()
    }

    /// Returns the dynamic Source and Output controls container.
    pub fn controls(&self) -> gtk::Box {
        self.imp().advanced_controls.get()
    }

    /// Appends one explicit modal action without transferring history authority.
    pub fn append_action(&self, action: &impl IsA<gtk::Widget>) {
        self.imp().advanced_actions.append(action);
    }
}

impl Default for ToniatorAdvancedSettingsShell {
    /// Creates the same empty private-settings shell as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

mod pattern_wizard_shell {
    use super::*;

    /// Installs the Blueprint-owned Pattern Wizard shell and its stable dynamic slots.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/pattern-wizard.ui")]
    pub struct ToniatorPatternWizardShell {
        #[template_child]
        pub wizard_breadcrumb: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_status: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_layout: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub wizard_gallery_panel: gtk::TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub wizard_cards: gtk::TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub wizard_page_panel: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub wizard_controls_scroll: gtk::TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub wizard_controls: gtk::TemplateChild<gtk::Box>,
        #[template_child]
        pub wizard_preview: gtk::TemplateChild<gtk::Picture>,
        #[template_child]
        pub wizard_spinner: gtk::TemplateChild<gtk::Spinner>,
        #[template_child]
        pub wizard_actions: gtk::TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorPatternWizardShell {
        const NAME: &'static str = "ToniatorPatternWizardShell";
        type Type = super::ToniatorPatternWizardShell;
        type ParentType = gtk::Box;

        /// Binds the compiled Pattern Wizard template exactly once for this widget class.
        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        /// Initializes one private wizard shell from the bound resource template.
        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorPatternWizardShell {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorPatternWizardShell {}
    impl gtk::subclass::box_::BoxImpl for ToniatorPatternWizardShell {}
}

glib::wrapper! {
    /// Provides the stable private Pattern Wizard layout and dynamic presentation slots.
    pub struct ToniatorPatternWizardShell(ObjectSubclass<pattern_wizard_shell::ToniatorPatternWizardShell>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorPatternWizardShell {
    /// Creates one resource-owned wizard shell before catalog cards and actions attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Returns the target and family breadcrumb presentation label.
    pub fn breadcrumb(&self) -> gtk::Label {
        self.imp().wizard_breadcrumb.get()
    }

    /// Returns the visible and accessible wizard status label.
    pub fn status(&self) -> gtk::Label {
        self.imp().wizard_status.get()
    }

    /// Returns the responsive group container for gallery/page and preview.
    pub fn layout(&self) -> gtk::Box {
        self.imp().wizard_layout.get()
    }

    /// Returns the full-width Presets container shown only on the first fixed wizard card.
    pub fn gallery_panel(&self) -> gtk::ScrolledWindow {
        self.imp().wizard_gallery_panel.get()
    }

    /// Returns the responsive catalog-card flow container.
    pub fn gallery(&self) -> gtk::FlowBox {
        self.imp().wizard_cards.get()
    }

    /// Returns the full-width fixed-card editing container shown after Presets.
    pub fn page_panel(&self) -> gtk::Box {
        self.imp().wizard_page_panel.get()
    }

    /// Returns the bounded scrolling viewport that keeps long fixed-card controls reachable.
    pub fn page_scroll(&self) -> gtk::ScrolledWindow {
        self.imp().wizard_controls_scroll.get()
    }

    /// Returns the dynamic review or capability-driven edit-page container.
    ///
    /// Static preview and spinner widgets remain outside this slot so page replacement cannot
    /// remove the private preview presentation.
    pub fn page(&self) -> gtk::Box {
        self.imp().wizard_controls.get()
    }

    /// Returns the private canonical preview paintable surface.
    pub fn preview(&self) -> gtk::Picture {
        self.imp().wizard_preview.get()
    }

    /// Returns the latest-only private preview pending indicator.
    pub fn spinner(&self) -> gtk::Spinner {
        self.imp().wizard_spinner.get()
    }

    /// Appends one explicit wizard action without transferring document authority.
    pub fn append_action(&self, action: &impl IsA<gtk::Widget>) {
        self.imp().wizard_actions.append(action);
    }
}

impl Default for ToniatorPatternWizardShell {
    /// Creates the same empty wizard shell as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

mod pattern_wizard_card {
    use super::*;

    /// Installs the Blueprint-owned repeated Pattern Wizard card hierarchy.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/pattern-wizard-card.ui")]
    pub struct ToniatorPatternWizardCard {
        #[template_child]
        pub wizard_card_thumbnail: gtk::TemplateChild<gtk::Image>,
        #[template_child]
        pub wizard_card_name: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_card_category: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_card_current_candidate: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_card_description: gtk::TemplateChild<gtk::Label>,
        #[template_child]
        pub wizard_card_unavailable: gtk::TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorPatternWizardCard {
        const NAME: &'static str = "ToniatorPatternWizardCard";
        type Type = super::ToniatorPatternWizardCard;
        type ParentType = gtk::Box;

        /// Binds the compiled repeated-card template once for this widget class.
        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Group);
            Self::bind_template(class);
        }

        /// Initializes one data-populated card from its static Blueprint hierarchy.
        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorPatternWizardCard {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorPatternWizardCard {}
    impl gtk::subclass::box_::BoxImpl for ToniatorPatternWizardCard {}
}

glib::wrapper! {
    /// Provides one reusable Blueprint-owned Pattern Wizard gallery card.
    pub struct ToniatorPatternWizardCard(ObjectSubclass<pattern_wizard_card::ToniatorPatternWizardCard>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorPatternWizardCard {
    /// Creates an empty reusable card before catalog data and actions are projected.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Sets the card thumbnail from one bundled image resource without changing catalog authority.
    pub fn set_thumbnail_resource(&self, resource: &str) {
        self.imp()
            .wizard_card_thumbnail
            .set_resource(Some(resource));
    }

    /// Sets the card thumbnail from a canonical rendered paintable without changing renderer authority.
    pub fn set_thumbnail_paintable(&self, paintable: Option<&gtk::gdk::Texture>) {
        self.imp().wizard_card_thumbnail.set_paintable(paintable);
    }

    /// Sets the card name from immutable catalog metadata.
    pub fn set_name(&self, name: &str) {
        self.imp().wizard_card_name.set_label(name);
    }

    /// Relates this explicit accessible group to its visible card title without storing catalog state.
    pub fn relate_accessible_name_to_card_title(&self) {
        let title = self.imp().wizard_card_name.get();
        self.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
    }

    /// Sets the card's presentation-only family label from the caller's recipe projection.
    pub fn set_category(&self, category: &str) {
        self.imp().wizard_card_category.set_label(category);
    }

    /// Marks this card as the stable-ID current candidate without changing its catalog record.
    pub fn set_current_candidate(&self, current: bool) {
        self.imp()
            .wizard_card_current_candidate
            .set_visible(current);
        if current {
            self.add_css_class("toniator-wizard-card-current");
        } else {
            self.remove_css_class("toniator-wizard-card-current");
        }
    }

    /// Sets the card description from immutable catalog metadata.
    pub fn set_description(&self, description: &str) {
        self.imp().wizard_card_description.set_label(description);
    }

    /// Shows or hides the visible unavailable explanation for this gate’s edit policy.
    pub fn set_unavailable(&self, explanation: Option<&str>) {
        let label = self.imp().wizard_card_unavailable.get();
        label.set_label(explanation.unwrap_or_default());
        label.set_visible(explanation.is_some());
    }
}

impl Default for ToniatorPatternWizardCard {
    /// Creates the same empty card as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

mod confirmation_content {
    use super::*;

    /// Installs the resource-owned explanatory content for modal confirmations.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/confirmation-dialog.ui")]
    pub struct ToniatorConfirmationContent {
        #[template_child]
        pub confirmation_detail: gtk::TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorConfirmationContent {
        const NAME: &'static str = "ToniatorConfirmationContent";
        type Type = super::ToniatorConfirmationContent;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorConfirmationContent {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorConfirmationContent {}
    impl gtk::subclass::box_::BoxImpl for ToniatorConfirmationContent {}
}

glib::wrapper! {
    /// Provides template-owned artist-facing copy for a confirmation surface.
    pub struct ToniatorConfirmationContent(ObjectSubclass<confirmation_content::ToniatorConfirmationContent>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorConfirmationContent {
    /// Creates empty confirmation content before its current consequence is shown.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Sets the visible consequence without storing or changing application state.
    pub fn set_detail(&self, detail: &str) {
        self.imp().confirmation_detail.set_label(detail);
    }
}

impl Default for ToniatorConfirmationContent {
    fn default() -> Self {
        Self::new()
    }
}

mod png_export_options {
    use super::*;

    /// Installs the resource-owned fixed PNG export option layout.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/png-export-options.ui")]
    pub struct ToniatorPngExportOptions {
        #[template_child]
        pub png_background: gtk::TemplateChild<gtk::DropDown>,
        #[template_child]
        pub png_antialiasing: gtk::TemplateChild<gtk::DropDown>,
        #[template_child]
        pub png_dimensions: gtk::TemplateChild<gtk::Entry>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorPngExportOptions {
        const NAME: &'static str = "ToniatorPngExportOptions";
        type Type = super::ToniatorPngExportOptions;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl glib::subclass::object::ObjectImpl for ToniatorPngExportOptions {}
    impl gtk::subclass::widget::WidgetImpl for ToniatorPngExportOptions {}
    impl gtk::subclass::box_::BoxImpl for ToniatorPngExportOptions {}
}

glib::wrapper! {
    /// Provides the template-owned fixed PNG export option controls.
    pub struct ToniatorPngExportOptions(ObjectSubclass<png_export_options::ToniatorPngExportOptions>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ToniatorPngExportOptions {
    /// Creates the fixed presentation surface before runtime defaults and callbacks attach.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Returns the runtime-modelled PNG backing selector.
    pub fn background(&self) -> gtk::DropDown {
        self.imp().png_background.get()
    }

    /// Returns the PNG raster-antialiasing selector.
    pub fn antialiasing(&self) -> gtk::DropDown {
        self.imp().png_antialiasing.get()
    }

    /// Returns the optional PNG output-dimension entry.
    pub fn dimensions(&self) -> gtk::Entry {
        self.imp().png_dimensions.get()
    }
}

impl Default for ToniatorPngExportOptions {
    fn default() -> Self {
        Self::new()
    }
}
