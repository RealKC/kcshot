use cairo::glib::Cast;
use gtk4::{
    gio,
    gio::prelude::SettingsExt,
    glib,
    subclass::prelude::ObjectSubclassIsExt,
    traits::{GtkWindowExt, NativeExt, WidgetExt},
};

use self::{
    data::Colour,
    operations::{SelectionMode, Tool},
};
use crate::kcshot;

mod data;
mod display_server;
mod operations;
mod textdialog;
mod toolbar;
mod underlying;
mod utils;

glib::wrapper! {
    pub struct EditorWindow(ObjectSubclass<underlying::EditorWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow,
        @implements gio::ActionMap;
}

impl EditorWindow {
    pub fn new(app: &gtk4::Application) -> Self {
        let editor = glib::Object::new::<Self>(&[("application", app)])
            .expect("Failed to make an EditorWindow");

        let settings = kcshot::open_settings();

        let restored_primary_colour = settings.uint("last-used-primary-colour");
        let restored_secondary_colour = settings.uint("last-used-secondary-colour");

        editor.set_primary_colour(Colour::deserialise_from_u32(restored_primary_colour));
        editor.set_secondary_colour(Colour::deserialise_from_u32(restored_secondary_colour));

        editor
    }

    pub fn show(app: &gtk4::Application) {
        let window = Self::new(app);
        window.set_decorated(false);
        window.show();
        window.fullscreen();

        let surface = window
            .native()
            .map(|native| native.surface())
            .expect("An EditorWindow should have a gdk::Surface")
            .downcast::<gdk4_x11::X11Surface>();

        if let Ok(surface) = surface {
            surface.set_skip_taskbar_hint(true);
            surface.set_skip_pager_hint(true);
        }
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
    /// The primary colour is the one used for filling in shapes
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

        let settings = kcshot::open_settings();
        if let Err(why) = settings.set_uint("last-used-primary-colour", colour.serialise_to_u32()) {
            tracing::warn!("Failed to update `last-used-primary-colour` setting value: {why}")
        }
    }

    /// Returns the secondary colour of the editor
    ///
    /// The secondary colour is used for lines, the text colour in case of bubbles and as the
    /// default colour for text and the pencil
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

        let settings = kcshot::open_settings();
        if let Err(why) = settings.set_uint("last-used-secondary-colour", colour.serialise_to_u32())
        {
            tracing::warn!("Failed to update `last-used-secondary-colour` setting value: {why}");
        }
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

    fn set_line_width(&self, line_width: f64) {
        let image = &self.imp().image;

        match image.try_borrow_mut() {
            Ok(mut image) => {
                let image = image.as_mut().unwrap();
                image.operation_stack.line_width = line_width;
            }
            Err(why) => tracing::info!("Image already borrowed: {why}"),
        }
    }
}
