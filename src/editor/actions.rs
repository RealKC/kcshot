use gtk4::{
    gio,
    glib::{self},
    prelude::*,
    subclass::prelude::*,
};
use kcshot_data::colour::Colour;

use crate::editor::{operations::Tool, underlying::EditorWindow};

impl EditorWindow {
    pub fn setup_actions(&self) {
        type This = super::EditorWindow;

        let undo_action = gio::ActionEntry::builder("undo")
            .activate(move |this: &This, _, _| this.imp().undo())
            .build();

        let redo_action = gio::ActionEntry::builder("redo")
            .activate(move |this: &This, _, _| this.imp().redo())
            .build();

        let tool_action = gio::ActionEntry::builder("set-tool")
            .parameter_type(Some(&Tool::static_variant_type()))
            .state(self.obj().get_current_tool().unwrap().to_variant())
            .activate(move |this: &This, action, tool| {
                if let Some(tool) = tool {
                    action.set_state(tool);
                };
                this.imp().set_tool(tool);
            })
            .build();

        let primary_colour_action = gio::ActionEntry::builder("set-primary-colour")
            .parameter_type(Some(&Colour::static_variant_type()))
            .activate(move |this: &This, _, colour| this.imp().set_primary_color_action(colour))
            .build();

        let secondary_colour_action = gio::ActionEntry::builder("set-secondary-colour")
            .parameter_type(Some(&Colour::static_variant_type()))
            .activate(move |this: &This, _, colour| this.imp().set_secondary_color_action(colour))
            .build();

        let line_width_action = gio::ActionEntry::builder("set-line-width")
            .parameter_type(Some(&f64::static_variant_type()))
            .activate(move |this: &This, _, line_width| {
                this.imp().set_line_width_action(line_width);
            })
            .build();

        self.obj().add_action_entries([
            undo_action,
            redo_action,
            tool_action,
            primary_colour_action,
            secondary_colour_action,
            line_width_action,
        ]);
    }

    fn undo(&self) {
        self.with_image_mut("win.undo activated", |image| {
            image.operation_stack.undo();
            self.drawing_area.queue_draw();
        });
    }

    fn redo(&self) {
        self.with_image_mut("win.redo activated", |image| {
            image.operation_stack.redo();
            self.drawing_area.queue_draw();
        });
    }

    fn set_tool(&self, tool: Option<&glib::Variant>) {
        tracing::trace!("win.set-tool activated");

        let Some(tool) = tool else {
            tracing::warn!("param given to win.set-tool was None");
            return;
        };

        let Some(tool) = tool.get::<Tool>() else {
            tracing::warn!("param is not of type Tool");
            return;
        };

        self.obj().set_current_tool(tool);

        if tool.is_immediate_save() {
            self.obj().save_image();
        }
    }

    fn set_primary_color_action(&self, colour: Option<&glib::Variant>) {
        tracing::trace!("win.set-primary-color activated");

        let Some(colour) = colour else {
            tracing::warn!("param given to win.set-primary-color was None");
            return;
        };

        let Some(colour) = colour.get::<Colour>() else {
            tracing::warn!("param is not of type Color, but: {:?}", colour.type_());
            return;
        };

        self.obj().set_primary_colour(colour);
    }

    fn set_secondary_color_action(&self, colour: Option<&glib::Variant>) {
        tracing::trace!("win.set-secondary-color activated");

        let Some(colour) = colour else {
            tracing::warn!("param given to win.set-secondary-color was None");
            return;
        };

        let Some(colour) = colour.get::<Colour>() else {
            tracing::warn!("param is not of type Color, but: {:?}", colour.type_());
            return;
        };

        self.obj().set_secondary_colour(colour);
    }

    fn set_line_width_action(&self, line_width: Option<&glib::Variant>) {
        tracing::trace!("win.set-line-width activated");

        let Some(line_width) = line_width else {
            tracing::warn!("param given to win.set-line-width was None");
            return;
        };

        let Some(line_width) = line_width.get::<f64>() else {
            tracing::warn!("param is not of type f64, but: {:?}", line_width.type_());
            return;
        };

        self.obj().set_line_width(line_width);
    }
}
