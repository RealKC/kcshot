#![allow(dead_code)]

use gtk4::prelude::*;

mod appwindow;
mod editor;

fn build_ui(app: &gtk4::Application) {
    let window = appwindow::AppWindow::new(app);

    window.show();
}

fn main() {
    tracing_subscriber::fmt::init();

    let application = gtk4::Application::new(Some("kc.kcshot"), Default::default());

    application.connect_activate(build_ui);

    application.run();
}
