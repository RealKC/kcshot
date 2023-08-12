use gtk4::{glib, prelude::*, subclass::prelude::*};

use crate::editor::operations::Tool;

glib::wrapper! {
    pub struct ToolButton(ObjectSubclass<underlying::ToolButton>)
        @extends gtk4::Widget;
}

impl ToolButton {
    pub fn set_active(&self, active: bool) {
        self.imp().toggle.get().set_active(active);
    }
}

/// This functions returns whether the button calling this needs to immediately start the saving
/// process on click
///
/// This is applicable only for the "crop-first" mode, as there the "Save" action is logically
/// the final thing you do, and needing to click somewhere on screen would be weird
pub fn should_start_saving_immediately(tool: Tool) -> bool {
    matches!(tool, Tool::Save)
}

mod underlying {
    use std::{
        cell::{Cell, RefCell},
        marker::PhantomData,
    };

    use gtk4::{
        glib::{self, Properties},
        prelude::*,
        subclass::prelude::*,
        CompositeTemplate,
    };

    use super::should_start_saving_immediately;
    use crate::{
        editor::{operations::Tool, EditorWindow},
        ext::DisposeExt,
    };

    #[derive(Debug, Properties, CompositeTemplate)]
    #[template(file = "src/editor/toolbar/toolbutton.blp")]
    #[properties(wrapper_type = super::ToolButton)]
    pub struct ToolButton {
        #[property(get, set)]
        spinner: RefCell<Option<gtk4::SpinButton>>,
        #[property(get, set)]
        primary: RefCell<Option<gtk4::Button>>,
        #[property(get, set)]
        secondary: RefCell<Option<gtk4::Button>>,
        #[property(get, set)]
        editor: RefCell<Option<EditorWindow>>,
        #[property(get, set = Self::set_tool, builder(Tool::CropAndSave))]
        tool: Cell<Tool>,
        #[property(set = Self::set_group)]
        group: PhantomData<Option<super::ToolButton>>,

        #[template_child]
        pub(super) toggle: TemplateChild<gtk4::ToggleButton>,
        #[template_child]
        image: TemplateChild<gtk4::Image>,
    }

    impl Default for ToolButton {
        fn default() -> Self {
            Self {
                spinner: Default::default(),
                primary: Default::default(),
                secondary: Default::default(),
                editor: Default::default(),
                tool: Cell::new(Tool::CropAndSave),
                group: PhantomData,
                toggle: Default::default(),
                image: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ToolButton {
        const NAME: &'static str = "KCShotToolButton";
        type Type = super::ToolButton;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BinLayout>();
            klass.set_css_name("toggle");
            klass.set_accessible_role(gtk4::AccessibleRole::Button);

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ToolButton {
        fn constructed(&self) {
            self.parent_constructed();

            let tool = self.tool.get();
            self.image.set_resource(Some(tool.path()));
            self.toggle.set_tooltip_markup(Some(tool.tooltip()));
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    impl WidgetImpl for ToolButton {}

    #[gtk4::template_callbacks]
    impl ToolButton {
        #[template_callback]
        fn on_clicked(&self, _: &gtk4::ToggleButton) {
            let editor = self.editor.borrow().clone().unwrap();
            let tool = self.tool.get();

            editor.set_current_tool(tool);
            if should_start_saving_immediately(tool) {
                editor.save_image();
            }
        }

        #[template_callback]
        fn on_toggled(&self, toggle: &gtk4::ToggleButton) {
            // This can probably be achieved using property bindings... somehow

            if let Some(spinner) = &*self.spinner.borrow() {
                spinner.set_visible(toggle.is_active());
            }

            if let Some(primary) = &*self.primary.borrow() {
                primary.set_visible(toggle.is_active());
            }

            if let Some(secondary) = &*self.secondary.borrow() {
                secondary.set_visible(toggle.is_active());
            }
        }

        fn set_group(&self, tool_button: Option<super::ToolButton>) {
            if let Some(tool_button) = tool_button {
                self.toggle.set_group(Some(&tool_button.imp().toggle.get()));
            }
        }

        fn set_tool(&self, tool: Tool) {
            self.tool.set(tool);
            self.image.set_resource(Some(tool.path()));
            self.toggle.set_tooltip_markup(Some(tool.tooltip()));
        }
    }
}
