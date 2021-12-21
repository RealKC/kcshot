use gtk4::{gio, glib};

glib::wrapper! {
    pub struct KCShot(ObjectSubclass<imp::KCShot>) @extends gio::Application, gtk4::Application, @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for KCShot {
    fn default() -> Self {
        Self::new()
    }
}

impl KCShot {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &"kc.kcshot"),
            ("flags", &gio::ApplicationFlags::empty()),
        ])
        .expect("Failed to create KCShot")
    }
}

mod imp {

    use gtk4::{glib, subclass::prelude::*};

    #[derive(Debug, Default)]
    pub struct KCShot {}

    #[glib::object_subclass]
    impl ObjectSubclass for KCShot {
        const NAME: &'static str = "KCShot";
        type Type = super::KCShot;
        type ParentType = gtk4::Application;
    }

    impl ObjectImpl for KCShot {}
    impl ApplicationImpl for KCShot {
        fn activate(&self, _application: &Self::Type) {}
    }
    impl GtkApplicationImpl for KCShot {}
}
