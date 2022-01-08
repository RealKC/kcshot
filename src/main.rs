#[macro_use]
extern crate diesel;

use gtk4::prelude::*;

mod appwindow;
mod db;
mod editor;
mod historymodel;
mod kcshot;
mod postcapture;

use kcshot::KCShot;

fn main() {
    tracing_subscriber::fmt::init();

    let application = KCShot::new();

    application.connect_activate(kcshot::build_ui);

    application.run();
}
