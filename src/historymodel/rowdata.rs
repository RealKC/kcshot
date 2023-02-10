use gtk4::glib;

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
        glib::Object::builder()
            .property("path", path)
            .property("time", time)
            .property("url", url)
            .build()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::glib::{self, prelude::*, subclass::prelude::*, ParamSpecBuilderExt, Properties};

    #[derive(Default, Debug, Properties)]
    #[properties(wrapper_type = super::RowData)]
    pub struct RowData {
        #[property(get, set, default_value = None)]
        pub(super) path: RefCell<Option<String>>,
        #[property(get, set, default_value = "")]
        pub(super) time: RefCell<String>,
        #[property(get, set, default_value = None)]
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
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, id, pspec)
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            Self::derived_set_property(self, id, value, pspec);
        }
    }
}
