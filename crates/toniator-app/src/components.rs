//! Composite GTK resource components used by the Toniator application shell.
//!
//! These types own stable presentation structure only. They deliberately do
//! not own a document, history, evaluator, or persistence state.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod main_shell {
    use super::*;

    /// Installs the resource-defined adaptive main-window composition.
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/silentbutdigital/Toniator/window.ui")]
    pub struct ToniatorMainShell {
        #[template_child]
        pub main_banner: gtk::TemplateChild<adw::Banner>,
        #[template_child]
        pub workspace_split: gtk::TemplateChild<adw::OverlaySplitView>,
    }

    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ToniatorMainShell {
        const NAME: &'static str = "ToniatorMainShell";
        type Type = super::ToniatorMainShell;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            Self::bind_template(class);
        }

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

    /// Returns the template-owned status banner for artist-facing feedback.
    pub fn banner(&self) -> adw::Banner {
        self.imp().main_banner.get()
    }

    /// Returns the template-owned adaptive split container.
    pub fn split(&self) -> adw::OverlaySplitView {
        self.imp().workspace_split.get()
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
        pub draft_scroll: gtk::TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub draft_actions: gtk::TemplateChild<gtk::Box>,
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
    /// Installs the dynamic draft controls in the template-owned scroll area.
    pub fn set_editor(&self, editor: &gtk::Box) {
        self.imp().draft_scroll.set_child(Some(editor));
    }
    /// Appends a stable footer action to the template-owned action row.
    pub fn append_action(&self, action: &impl IsA<gtk::Widget>) {
        self.imp().draft_actions.append(action);
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
