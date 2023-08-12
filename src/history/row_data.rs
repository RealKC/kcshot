use gtk4::{glib, subclass::prelude::*};

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

    pub fn set_context_menu(&self, menu: gtk4::PopoverMenu) {
        self.imp().context_menu.replace(Some(menu));
    }

    pub fn context_menu(&self) -> Option<gtk4::PopoverMenu> {
        self.imp().context_menu.borrow().clone()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::glib::{self, prelude::*, subclass::prelude::*, Properties};

    #[derive(Default, Debug, Properties)]
    #[properties(wrapper_type = super::RowData)]
    pub struct RowData {
        #[property(get, set, default_value = None)]
        pub(super) path: RefCell<Option<String>>,
        #[property(get, set, default_value = "")]
        pub(super) time: RefCell<String>,
        #[property(get, set, default_value = None)]
        pub(super) url: RefCell<Option<String>>,

        /// We need to keep the context menu here because creating context menus on every right click
        /// behaves strangely
        pub(super) context_menu: RefCell<Option<gtk4::PopoverMenu>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RowData {
        const NAME: &'static str = "KcshotRowData";
        type ParentType = glib::Object;
        type Type = super::RowData;
    }

    #[glib::derived_properties]
    impl ObjectImpl for RowData {}
}
