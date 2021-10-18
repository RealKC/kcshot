#![allow(dead_code)]

use std::{ffi::c_void, os::raw::c_char};

use gtk::prelude::*;

mod editor;

fn build_ui(app: &gtk::Application) {
    let window = editor::EditorWindow::new(app);
    window.set_decorated(false);
    window.skips_taskbar_hint();
    window.set_keep_above(true);
    window.fullscreen();

    window.show_all();
}

fn main() {
    tracing_subscriber::fmt::init();

    let application = gtk::Application::new(Some("kc.kcshot"), Default::default());

    application.connect_activate(build_ui);

    application.run();
}
