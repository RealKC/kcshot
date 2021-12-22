use gtk4::glib;

use crate::kcshot::KCShot;

mod model;
mod rowdata;
mod underlying;

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<underlying::AppWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow;
}

impl AppWindow {
    pub fn new(app: &KCShot) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to make an AppWindow")
    }
}