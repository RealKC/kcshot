use gtk4::glib;

use crate::kcshot::KCShot;

mod underlying;

use crate::historymodel::HistoryModel;

glib::wrapper! {
    pub struct AppWindow(ObjectSubclass<underlying::AppWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow;
}

impl AppWindow {
    pub fn new(app: &KCShot, history_model: &HistoryModel) -> Self {
        glib::Object::new(&[("application", app), ("history-model", history_model)])
            .expect("Failed to make an AppWindow")
    }
}
