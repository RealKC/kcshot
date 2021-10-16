use gtk::prelude::*;

fn build_ui(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.skips_taskbar_hint();
    window.set_keep_above(true);
    window.fullscreen();

    window.show();
}

fn main() {
    let application = gtk::Application::new(Some("kc.kcshot"), Default::default());

    application.connect_activate(build_ui);

    application.run();
}
