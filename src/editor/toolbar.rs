use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};
use kcshot_data::colour::Colour;

use self::toolbutton::ToolButton;
use super::operations::Tool;

mod toolbutton;

glib::wrapper! {
    pub struct ToolbarWidget(ObjectSubclass<underlying::ToolbarWidget>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl ToolbarWidget {
    pub fn new(
        parent_editor: &super::EditorWindow,
        editing_started_with_cropping: bool,
        primary_colour: Colour,
        secondary_colour: Colour,
    ) -> Self {
        let obj = glib::Object::builder::<Self>()
            .property("editor", parent_editor)
            .property(
                "editing-started-with-cropping",
                editing_started_with_cropping,
            )
            .property("primary-colour", primary_colour)
            .property("secondary-colour", secondary_colour)
            .build();

        // We want to start as hidden if editing started with cropping
        obj.set_visible(!editing_started_with_cropping);

        obj
    }

    pub fn key_activates_tool(&self, key: gdk::Key) -> bool {
        if let Some(tool) = key.to_unicode().and_then(Tool::from_unicode) {
            self.imp().editor.upgrade().unwrap().set_current_tool(tool);
            let mut current_button = self.imp().group_source.get();
            loop {
                if current_button.tool() == tool {
                    current_button.set_active(true);
                    return true;
                }

                match current_button.next_sibling() {
                    Some(next_sibling) => match next_sibling.downcast::<ToolButton>() {
                        Ok(next) => current_button = next,
                        Err(_) => continue,
                    },
                    None => break,
                }
            }
        }

        false
    }
}

mod underlying {
    use std::{cell::Cell, marker::PhantomData};

    use gtk4::{
        CompositeTemplate,
        glib::{self, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
    };
    use kcshot_data::colour::Colour;

    use super::toolbutton::ToolButton;
    use crate::{
        editor::{
            EditorWindow, colourbutton::ColourButton, colourchooserdialog::ColourChooserDialog,
            operations::Tool,
        },
        ext::DisposeExt,
    };

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::ToolbarWidget)]
    #[template(file = "src/editor/toolbar.blp")]
    pub struct ToolbarWidget {
        #[property(get, set, construct_only)]
        pub(super) editor: WeakRef<EditorWindow>,
        #[property(get, set, construct_only)]
        editing_started_with_cropping: Cell<bool>,
        #[property( set = Self::set_primary_colour)]
        primary_colour: PhantomData<Cell<Colour>>,
        #[property(set = Self::set_secondary_colour)]
        secondary_colour: PhantomData<Cell<Colour>>,

        #[template_child]
        pub(super) group_source: TemplateChild<ToolButton>,
        #[template_child]
        primary: TemplateChild<ColourButton>,
        #[template_child]
        secondary: TemplateChild<ColourButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ToolbarWidget {
        const NAME: &'static str = "KCShotToolbarWidget";
        type Type = super::ToolbarWidget;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BoxLayout>();
            klass.set_css_name("kcshot-toolbar");

            klass.bind_template();
            klass.bind_template_callbacks();

            ColourButton::static_type();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ToolbarWidget {
        fn constructed(&self) {
            let group_source_tool = if self.editing_started_with_cropping.get() {
                Tool::Save
            } else {
                Tool::CropAndSave
            };
            self.group_source.set_tool(group_source_tool);
            let is_group_source_active =
                group_source_tool.is_immediate_save() || self.editing_started_with_cropping.get();
            self.group_source.set_active(is_group_source_active);
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }
    }

    impl WidgetImpl for ToolbarWidget {}

    impl ToolbarWidget {
        fn set_primary_colour(&self, colour: Colour) {
            self.primary.set_colour(colour);
        }

        fn set_secondary_colour(&self, colour: Colour) {
            self.secondary.set_colour(colour);
        }
    }

    #[gtk4::template_callbacks]
    impl ToolbarWidget {
        #[track_caller]
        fn editor(&self) -> EditorWindow {
            self.editor.upgrade().unwrap()
        }

        #[template_callback]
        async fn on_primary_colour_clicked(&self, button: &gtk4::Button) {
            let dialog = ColourChooserDialog::new(&self.editor(), self.primary.colour());

            dialog.present();

            let colour = dialog.colour().await;

            if let Err(why) = button.activate_action("win.set-primary-colour", Some(&colour.into()))
            {
                tracing::warn!("Failed to activate win.set-primary-colour: {why:?}");
            }

            self.primary.set_colour(colour);
        }

        #[template_callback]
        async fn on_secondary_colour_clicked(&self, button: &gtk4::Button) {
            let dialog = ColourChooserDialog::new(&self.editor(), self.secondary.colour());

            dialog.present();

            let colour = dialog.colour().await;
            if let Err(why) =
                button.activate_action("win.set-secondary-colour", Some(&colour.into()))
            {
                tracing::warn!("Failed to activate win.set-secondary-colour: {why:?}");
            }

            self.secondary.set_colour(colour);
        }

        #[template_callback]
        fn on_line_width_changed(&self, spinner: &gtk4::SpinButton) {
            self.editor().set_line_width(spinner.value());
        }
    }
}
