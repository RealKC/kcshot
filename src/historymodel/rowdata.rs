use gtk4::{
    glib::{self, ToValue},
    subclass::prelude::*,
};

use crate::db::models::Screenshot;

glib::wrapper! {
    pub struct RowData(ObjectSubclass<underlying::RowData>);
}

impl RowData {
    pub fn new(screenshot: Screenshot) -> Self {
        let Screenshot {
            path, time, url, ..
        } = screenshot;
        Self::new_from_components(path, time, url)
    }

    pub fn new_from_components(path: Option<String>, time: String, url: Option<String>) -> Self {
        glib::Object::new(&[
            ("path", &path.to_value()),
            ("time", &time.to_value()),
            ("url", &url.to_value()),
        ])
    }

    pub fn path(&self) -> Option<String> {
        let this = self.imp();

        this.path.borrow().clone()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::glib::{self, subclass::prelude::*, ParamSpecBuilderExt, ToValue};
    use once_cell::sync::Lazy;

    #[derive(Default, Debug)]
    pub struct RowData {
        pub(super) path: RefCell<Option<String>>,
        pub(super) time: RefCell<String>,
        pub(super) url: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RowData {
        const NAME: &'static str = "KcshotRowData";
        type ParentType = glib::Object;
        type Type = super::RowData;
    }

    impl ObjectImpl for RowData {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("path")
                        .default_value(None)
                        .readwrite()
                        .build(),
                    glib::ParamSpecString::builder("time")
                        .default_value(Some(""))
                        .readwrite()
                        .build(),
                    glib::ParamSpecString::builder("url")
                        .default_value(None)
                        .readwrite()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "path" => self.path.borrow().to_value(),
                "time" => self.time.borrow().to_value(),
                "url" => self.url.borrow().to_value(),
                name => panic!("Tried to get property {} which does not exist", name),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "path" => {
                    let path = value.get::<Option<String>>().unwrap();
                    self.path.replace(path);
                }
                "time" => {
                    let time = value.get().unwrap();
                    self.time.replace(time);
                }
                "url" => {
                    let url = value.get::<Option<String>>().unwrap();
                    self.url.replace(url);
                }
                name => {
                    tracing::warn!("Tried setting property {name} which does not exist on this object: {self:?}");
                }
            }
        }
    }
}
