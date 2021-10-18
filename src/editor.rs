use gtk::{glib, prelude::*, subclass::prelude::*};

mod screenshot;
mod underlying;

glib::wrapper! {
    pub struct EditorWindow(ObjectSubclass<underlying::EditorWindow>)
        @extends gtk::Widget, gtk::Container, gtk::Bin, gtk::Window, gtk::ApplicationWindow;
}

impl EditorWindow {
    pub fn new(app: &gtk::Application) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to make an EditorWindow")
    }
}
