use gtk4::{glib, subclass::prelude::ObjectSubclassIsExt};

use self::{
    data::Colour,
    operations::{SelectionMode, Tool},
};

mod data;
mod display_server;
mod operations;
mod textdialog;
mod toolbar;
mod underlying;
mod utils;

glib::wrapper! {
    pub struct EditorWindow(ObjectSubclass<underlying::EditorWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow;
}

impl EditorWindow {
    pub fn new(app: &gtk4::Application) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to make an EditorWindow")
    }

    fn set_current_tool(&self, tool: Tool) {
        self.imp()
            .image
            .borrow_mut()
            .as_mut()
            .unwrap()
            .operation_stack
            .set_current_tool(tool)
    }

    /// Returns the primary colour of the editor
    ///
    /// The primary colour is the one used for colouring text and filling in shapes
    fn primary_colour(&self) -> Colour {
        self.imp()
            .image
            .borrow()
            .as_ref()
            .unwrap()
            .operation_stack
            .primary_colour
    }

    fn set_primary_colour(&self, colour: Colour) {
        let mut image = self.imp().image.borrow_mut();
        let image = image.as_mut().unwrap();

        image.operation_stack.primary_colour = colour;
    }

    /// Returns the secondary colour of the editor
    ///
    /// The secondary colour is used for borders, and the bubble colour in case of bubbles
    fn secondary_colour(&self) -> Colour {
        self.imp()
            .image
            .borrow()
            .as_ref()
            .unwrap()
            .operation_stack
            .secondary_colour
    }

    fn set_secondary_colour(&self, colour: Colour) {
        let mut image = self.imp().image.borrow_mut();
        let image = image.as_mut().unwrap();

        image.operation_stack.secondary_colour = colour;
    }

    fn set_selection_mode(&self, selection_mode: SelectionMode) {
        let image = &self.imp().image;

        match image.try_borrow_mut() {
            Ok(mut image) => {
                let image = image.as_mut().unwrap();
                image.operation_stack.selection_mode = selection_mode;
            }
            Err(why) => tracing::info!("Image already borrowed: {why}"),
        }
    }
}
