use gtk4::{gio::ListModel as GListModel, glib};

use crate::kcshot::KCShot;

glib::wrapper! {
    pub struct ListModel(ObjectSubclass<underlying::ListModel>)
        @implements GListModel;
}

impl ListModel {
    #[allow(clippy::new_without_default)]
    pub fn new(app: &KCShot) -> Self {
        glib::Object::new(&[("application", app)]).unwrap()
    }
}

mod underlying {
    use std::cell::RefCell;

    use gtk4::{
        gio,
        glib::{self, Object, ParamSpec, StaticType, ToValue, Value},
        prelude::*,
        subclass::prelude::*,
    };
    use once_cell::sync::Lazy;

    use crate::{appwindow::rowdata::RowData, db, kcshot::KCShot};

    #[derive(Default)]
    pub struct ListModel {
        app: RefCell<KCShot>,
        screenshots: RefCell<Vec<RowData>>,
    }

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
            self.screenshots.borrow().len() as u32
        }

        fn item(&self, list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            let n_items = self.n_items(list_model);
            if position >= n_items {
                const COUNT: i64 = 15;
                let new_screenshots =
                    db::fetch_screenshots(self.app.borrow().conn(), n_items as i64, COUNT);
                let new_screenshots = match new_screenshots {
                    Ok(n) => n,
                    Err(why) => {
                        tracing::error!("Encountered error: {:?}\n\twhile trying to fetch {} items from the database,\n\tstarting at index: {}", why, COUNT, n_items);
                        return None;
                    }
                };
                self.screenshots
                    .borrow_mut()
                    .extend(new_screenshots.iter().map(|s| RowData::new(s.clone())));
            }
            self.screenshots
                .borrow()
                .get(position as usize)
                .cloned()
                .map(|o| o.upcast())
        }
    }

    impl ObjectImpl for ListModel {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpec::new_object(
                    "application",
                    "Application",
                    "Application",
                    KCShot::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
            });

            PROPERTIES.as_ref()
        }

        #[tracing::instrument(skip(self))]
        fn property(&self, _: &Self::Type, _id: usize, pspec: &ParamSpec) -> glib::Value {
            match pspec.name() {
                "application" => self.app.borrow().to_value(),
                name => {
                    tracing::error!("Unknown property: {}", name);
                    panic!()
                }
            }
        }

        #[tracing::instrument(skip(self))]
        fn set_property(&self, _: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "application" => {
                    let application = value.get::<KCShot>().ok();
                    self.app.replace(application.unwrap());
                }
                name => tracing::warn!("Unknown property: {}", name),
            }
        }
    }
}
