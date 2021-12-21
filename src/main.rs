#![allow(dead_code)]

use gtk4::prelude::*;

mod appwindow;
mod editor;
mod kcshot;

use kcshot::KCShot;

fn build_ui(app: &KCShot) {
    let window = appwindow::AppWindow::new(app.upcast_ref());

    window.show();
}

fn main() {
    tracing_subscriber::fmt::init();

    let application = KCShot::new();

    application.connect_activate(build_ui);

    application.run();
}
