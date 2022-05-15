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
    pub fn new(app: &gtk4::Application, editing_starts_with_cropping: bool) -> Self {
        let editor = glib::Object::new::<Self>(&[
            ("application", app),
            (
                "editing-starts-with-cropping",
                &editing_starts_with_cropping,
            ),
        ])
        .expect("Failed to make an EditorWindow");

        let settings = kcshot::open_settings();

        let restored_primary_colour = settings.uint("last-used-primary-colour");
        let restored_secondary_colour = settings.uint("last-used-secondary-colour");

        editor.set_primary_colour(Colour::deserialise_from_u32(restored_primary_colour));
        editor.set_secondary_colour(Colour::deserialise_from_u32(restored_secondary_colour));

        editor
    }

    pub fn show(app: &gtk4::Application, editing_starts_with_cropping: bool) {
        let window = Self::new(app, editing_starts_with_cropping);
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
        self.imp().with_image_mut("set_current_tool", |image| {
            image.operation_stack.set_current_tool(tool);
        });
    }

    /// Returns the primary colour of the editor
    ///
    /// The primary colour is the one used for filling in shapes
    fn primary_colour(&self) -> Colour {
        self.imp()
            .with_image("get primary_colour", |image| {
                image.operation_stack.primary_colour
            })
            .unwrap()
    }

    fn set_primary_colour(&self, colour: Colour) {
        self.imp().with_image_mut("set_primary_colour", |image| {
            image.operation_stack.primary_colour = colour;
        });

        let settings = kcshot::open_settings();
        if let Err(why) = settings.set_uint("last-used-primary-colour", colour.serialise_to_u32()) {
            tracing::warn!("Failed to update `last-used-primary-colour` setting value: {why}");
        }
    }

    /// Returns the secondary colour of the editor
    ///
    /// The secondary colour is used for lines, the text colour in case of bubbles and as the
    /// default colour for text and the pencil
    fn secondary_colour(&self) -> Colour {
        self.imp()
            .with_image("get secondary_colour", |image| {
                image.operation_stack.secondary_colour
            })
            .unwrap()
    }

    fn set_secondary_colour(&self, colour: Colour) {
        self.imp().with_image_mut("set_secondary_colour", |image| {
            image.operation_stack.secondary_colour = colour;
        });

        let settings = kcshot::open_settings();
        if let Err(why) = settings.set_uint("last-used-secondary-colour", colour.serialise_to_u32())
        {
            tracing::warn!("Failed to update `last-used-secondary-colour` setting value: {why}");
        }
    }

    fn set_selection_mode(&self, selection_mode: SelectionMode) {
        self.imp().with_image_mut("set_selection_mode", |image| {
            image.operation_stack.selection_mode = selection_mode;
        });
    }

    fn set_line_width(&self, line_width: f64) {
        self.imp().with_image_mut("set_line_width", |image| {
            image.operation_stack.line_width = line_width;
        });
    }
}
