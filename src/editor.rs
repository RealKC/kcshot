use gtk4::{gio, glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use kcshot_data::{colour::Colour, settings::Settings};

use self::operations::Tool;
use crate::kcshot::KCShot;

mod colourchooser;
mod colourchooserdialog;
mod colourwheel;
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
        let editor = glib::Object::builder::<Self>()
            .property("application", app)
            .property("editing-starts-with-cropping", editing_starts_with_cropping)
            .build();

        let settings = Settings::open();

        let restored_primary_colour = settings.last_used_primary_colour();
        let restored_secondary_colour = settings.last_used_secondary_colour();

        editor.set_primary_colour(restored_primary_colour);
        editor.set_secondary_colour(restored_secondary_colour);

        editor
    }

    pub fn show(app: &gtk4::Application, editing_starts_with_cropping: bool) {
        let window = Self::new(app, editing_starts_with_cropping);
        window.set_decorated(false);
        window.show();
        window.fullscreen();

        let surface = window
            .native()
            .and_then(|native| native.surface())
            .and_downcast::<gdk4_x11::X11Surface>();

        if let Some(surface) = surface {
            surface.set_skip_taskbar_hint(true);
            surface.set_skip_pager_hint(true);
        }
    }

    async fn pick_colour(&self) -> Colour {
        self.imp().pick_colour().await
    }

    fn set_current_tool(&self, tool: Tool) {
        self.imp().with_image_mut("set_current_tool", |image| {
            image.operation_stack.set_current_tool(tool);
        });
    }

    /// Returns the primary colour of the editor
    ///
    /// The primary colour is the one used for filling in shapes
    #[track_caller]
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

        let settings = Settings::open();
        if let Err(why) = settings.try_set_last_used_primary_colour(colour) {
            tracing::warn!("Failed to update `last-used-primary-colour` setting value: {why}");
        }
    }

    /// Returns the secondary colour of the editor
    ///
    /// The secondary colour is used for lines, the text colour in case of bubbles and as the
    /// default colour for text and the pencil
    #[track_caller]
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

        let settings = Settings::open();
        if let Err(why) = settings.try_set_last_used_secondary_colour(colour) {
            tracing::warn!("Failed to update `last-used-secondary-colour` setting value: {why}");
        }
    }

    fn set_line_width(&self, line_width: f64) {
        self.imp().with_image_mut("set_line_width", |image| {
            image.operation_stack.line_width = line_width;
        });
    }

    fn save_image(&self) {
        self.imp()
            .with_image_mut("EditorWindow::save_image", |image| {
                KCShot::the().with_conn(|conn| {
                    underlying::EditorWindow::do_save_surface(
                        &KCShot::the().model_notifier(),
                        conn,
                        self.upcast_ref(),
                        image,
                        None,
                    );
                });
            });
    }
}
