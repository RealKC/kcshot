use gtk4::{gio::ListModel as GListModel, glib};

glib::wrapper! {
    pub struct ListModel(ObjectSubclass<underlying::ListModel>)
        @implements GListModel;
}

impl ListModel {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }
}

mod underlying {
    use gtk4::{
        gio,
        glib::{self, Object, StaticType},
        prelude::*,
        subclass::prelude::*,
    };

    use crate::appwindow::rowdata::RowData;

    #[derive(Default)]
    pub struct ListModel;

    #[glib::object_subclass]
    impl ObjectSubclass for ListModel {
        const NAME: &'static str = "KcshotListModel";
        type Type = super::ListModel;
        type ParentType = Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ListModelImpl for ListModel {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            RowData::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            3
        }

        fn item(&self, _list_model: &Self::Type, _position: u32) -> Option<glib::Object> {
            Some(RowData::new().upcast())
        }
    }

    impl ObjectImpl for ListModel {}
}
