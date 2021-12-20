use gtk4::{
    glib::{self, ToValue},
    subclass::prelude::*,
};

glib::wrapper! {
    pub struct RowData(ObjectSubclass<underlying::RowData>);
}

impl RowData {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        glib::Object::new(&[("path", &"orange.jpg".to_value())]).unwrap()
    }

    pub fn path(&self) -> String {
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
        pub(super) path: RefCell<String>,
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
                vec![glib::ParamSpec::new_string(
                    "path",
                    "Path",
                    "Path",
                    Some(""), // Default value
                    glib::ParamFlags::READWRITE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "path" => self.path.borrow().to_value(),
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
                    let path = value.get().unwrap();
                    self.path.replace(path);
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
