use gtk4::glib;

mod data;
mod display_server;
mod operations;
mod textdialog;
mod underlying;
mod utils;

glib::wrapper! {
    pub struct EditorWindow(ObjectSubclass<underlying::EditorWindow>)
        @extends gtk4::Widget,  gtk4::Window, gtk4::ApplicationWindow;
}

impl EditorWindow {
    pub fn new(app: &gtk4::Application) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to make an EditorWindow")
    }
}
