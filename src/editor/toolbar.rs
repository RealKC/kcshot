use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};

use self::toolbutton::ToolButton;
use super::operations::Tool;

mod toolbutton;

glib::wrapper! {
    pub struct ToolbarWidget(ObjectSubclass<underlying::ToolbarWidget>)
        @extends gtk4::Widget;
}

impl ToolbarWidget {
    pub fn new(parent_editor: &super::EditorWindow, editing_started_with_cropping: bool) -> Self {
        let obj = glib::Object::builder::<Self>()
            .property("editor", parent_editor)
            .property(
                "editing-started-with-cropping",
                editing_started_with_cropping,
            )
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
    use std::cell::Cell;

    use gtk4::{
        glib::{self, ParamSpec, Properties, WeakRef},
        prelude::*,
        subclass::prelude::*,
        CompositeTemplate,
    };
    use kcshot_data::colour::Colour;

    use super::toolbutton::{should_start_saving_immediately, ToolButton};
    use crate::{
        editor::{colourchooser, operations::Tool, utils::CairoExt, EditorWindow},
        ext::DisposeExt,
        log_if_err,
    };

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::ToolbarWidget)]
    #[template(file = "src/editor/toolbar.blp")]
    pub struct ToolbarWidget {
        #[property(get, set, construct_only)]
        pub(super) editor: WeakRef<EditorWindow>,
        #[property(get, set, construct_only)]
        editing_started_with_cropping: Cell<bool>,
        #[template_child]
        pub(super) group_source: TemplateChild<ToolButton>,
        #[template_child]
        primary: TemplateChild<gtk4::Button>,
        #[template_child]
        primary_button_drawing_area: TemplateChild<gtk4::DrawingArea>,
        #[template_child]
        secondary: TemplateChild<gtk4::Button>,
        #[template_child]
        secondary_button_drawing_area: TemplateChild<gtk4::DrawingArea>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ToolbarWidget {
        const NAME: &'static str = "KCShotToolbarWidget";
        type Type = super::ToolbarWidget;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk4::BoxLayout>();

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ToolbarWidget {
        fn constructed(&self) {
            let group_source_tool = if self.editing_started_with_cropping.get() {
                Tool::Save
            } else {
                Tool::CropAndSave
            };
            self.group_source.set_tool(group_source_tool);
            let is_group_source_active = !should_start_saving_immediately(group_source_tool)
                || self.editing_started_with_cropping.get();
            self.group_source.set_active(is_group_source_active);

            self.primary_button_drawing_area.set_draw_func({
                let editor = self.editor();
                move |_, cairo, _, _| Self::primary_color_draw_func(editor.clone(), cairo)
            });

            self.secondary_button_drawing_area.set_draw_func({
                let editor = self.editor();
                move |_, cairo, _, _| Self::secondary_color_draw_func(editor.clone(), cairo)
            });
        }

        fn dispose(&self) {
            self.obj().dispose_children();
        }

        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }
    }

    impl WidgetImpl for ToolbarWidget {}

    #[gtk4::template_callbacks]
    impl ToolbarWidget {
        #[track_caller]
        fn editor(&self) -> EditorWindow {
            self.editor.upgrade().unwrap()
        }

        fn primary_color_draw_func(editor: EditorWindow, cairo: &cairo::Context) {
            cairo.set_operator(cairo::Operator::Over);

            let primary_colour = editor.primary_colour();
            if primary_colour.alpha != 0 {
                cairo.rectangle(0.0, 0.0, 20.0, 20.0);
                cairo.set_source_colour(primary_colour);
                log_if_err!(cairo.fill());
            } else {
                // Instead of drawing nothing (what a fully transparent colour is) we draw a
                // checkerboard pattern instead
                cairo.set_source_colour(Colour {
                    red: 0xff,
                    green: 0x00,
                    blue: 0xdc,
                    alpha: 0xff,
                });
                cairo.rectangle(0.0, 0.0, 10.0, 10.0);
                log_if_err!(cairo.fill());
                cairo.rectangle(10.0, 10.0, 10.0, 10.0);
                log_if_err!(cairo.fill());

                cairo.set_source_colour(Colour::BLACK);
                cairo.rectangle(0.0, 10.0, 10.0, 10.0);
                log_if_err!(cairo.fill());
                cairo.rectangle(10.0, 0.0, 10.0, 10.0);
                log_if_err!(cairo.fill());
            }

            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(0.0, 0.0, 20.0, 20.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());
        }

        fn secondary_color_draw_func(editor: EditorWindow, cairo: &cairo::Context) {
            cairo.set_operator(cairo::Operator::Over);

            // The interior contour of the square
            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(5.0, 5.0, 10.0, 10.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());

            // The empty square representing the border
            cairo.set_source_colour(editor.secondary_colour());
            cairo.rectangle(3.0, 3.0, 14.0, 14.0);
            cairo.set_line_width(4.0);
            log_if_err!(cairo.stroke());

            // The exterior contour of the square
            cairo.set_source_colour(Colour::BLACK);
            cairo.rectangle(1.0, 1.0, 18.0, 18.0);
            cairo.set_line_width(1.0);
            log_if_err!(cairo.stroke());
        }

        #[template_callback]
        fn on_primary_colour_clicked(&self, _: &gtk4::Button) {
            let dialog = colourchooser::dialog(&self.editor());
            let drawing_area = self.primary_button_drawing_area.get();
            dialog.connect_response(move |editor, colour| {
                editor.set_primary_colour(colour);
                drawing_area.queue_draw();
            });
            dialog.show();
        }

        #[template_callback]
        fn on_secondary_colour_clicked(&self, _: &gtk4::Button) {
            let dialog = colourchooser::dialog(&self.editor());
            let drawing_area = self.secondary_button_drawing_area.get();
            dialog.connect_response(move |editor, colour| {
                editor.set_secondary_colour(colour);
                drawing_area.queue_draw();
            });
            dialog.show();
        }

        #[template_callback]
        fn on_line_width_changed(&self, spinner: &gtk4::SpinButton) {
            self.editor().set_line_width(spinner.value());
        }
    }
}
