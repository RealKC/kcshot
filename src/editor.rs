use gtk4::{gio, glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use kcshot_data::colour::Colour;

use self::operations::Tool;
use crate::kcshot::KCShot;

pub mod colourbutton;
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
        @implements gtk4::ConstraintTarget, gtk4::Buildable, gtk4::Accessible,
                    gtk4::ShortcutManager, gtk4::Root, gtk4::Native, gio::ActionMap, gio::ActionGroup;
}

impl EditorWindow {
    pub fn new(app: &gtk4::Application, editing_starts_with_cropping: bool) -> Self {
        glib::Object::builder::<Self>()
            .property("application", app)
            .property("editing-starts-with-cropping", editing_starts_with_cropping)
            .build()
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
