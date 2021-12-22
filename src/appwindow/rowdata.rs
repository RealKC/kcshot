use gtk4::{
    glib::{self, ToValue},
    subclass::prelude::*,
};

use crate::db::models::Screenshot;

glib::wrapper! {
    pub struct RowData(ObjectSubclass<underlying::RowData>);
}

impl RowData {
    #[allow(clippy::new_without_default)]
    pub fn new(screenshot: Screenshot) -> Self {
        let Screenshot {
            path, time, url, ..
        } = screenshot;
        glib::Object::new(&[
            ("path", &path.to_value()),
            ("time", &time.to_value()),
            ("url", &url.to_value()),
        ])
        .unwrap()
    }

    pub fn path(&self) -> Option<String> {
        let this = underlying::RowData::from_instance(self);

        this.path.borrow().clone()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::glib::{self, subclass::prelude::*, ToValue};
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
                    glib::ParamSpec::new_string(
                        "path",
                        "Path",
                        "Path",
                        None,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_string(
                        "time",
                        "Time",
                        "Time",
                        Some(""),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_string(
                        "url",
                        "URL",
                        "URL",
                        None,
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "path" => self.path.borrow().to_value(),
                "time" => self.time.borrow().to_value(),
                "url" => self.url.borrow().to_value(),
                name => panic!("Tried to get property {} which does not exist", name),
            }
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
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
                    tracing::warn!(
                        "Tried setting property {} which does not exist on this object: {:?}",
                        name,
                        self
                    )
                }
            }
        }
    }
}
