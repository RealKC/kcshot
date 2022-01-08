use diesel::SqliteConnection;
use gtk4::{
    gio::ListModel as GListModel, glib, prelude::ListModelExt, subclass::prelude::ObjectSubclassExt,
};

use crate::{db, kcshot::KCShot};

glib::wrapper! {
    pub struct HistoryModel(ObjectSubclass<underlying::ListModel>)
        @implements GListModel;
}

impl HistoryModel {
    #[allow(clippy::new_without_default)]
    pub fn new(app: &KCShot) -> Self {
        glib::Object::new(&[("application", app)]).unwrap()
    }

    pub fn add_item_to_history(
        &self,
        conn: &SqliteConnection,
        path_: Option<String>,
        time_: String,
        url_: Option<String>,
    ) {
        if let Err(why) = db::add_screenshot_to_history(conn, path_, time_, url_) {
            tracing::error!("Failed to add screenshot to history: {:?}", why);
            return;
        }
        let impl_ = underlying::ListModel::from_instance(self);
        impl_.screenshots.borrow_mut().clear();
        self.items_changed(0, 0, 1)
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
        pub(super) screenshots: RefCell<Vec<RowData>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ListModel {
        const NAME: &'static str = "KcshotListModel";
        type Type = super::HistoryModel;
        type ParentType = Object;
        type Interfaces = (gio::ListModel,);
    }

    impl ListModelImpl for ListModel {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            RowData::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            let n_items = db::number_of_history_itms(self.app.borrow().conn());
            match n_items {
                Ok(n_items) => {
                    assert!(
                        0 <= n_items && n_items <= u32::MAX as i64,
                        "n_items was {} which is not within [0, i32::MAX]",
                        n_items
                    );
                    n_items as u32
                }
                Err(why) => {
                    tracing::error!("Failed to get number of screenshots in history: {:?}", why);
                    panic!() // yolo
                }
            }
        }

        #[tracing::instrument(skip(self))]
        fn item(&self, _: &Self::Type, position: u32) -> Option<glib::Object> {
            let last_fetched_screenshot_index = {
                let len = self.screenshots.borrow().len();
                if len > 0 {
                    len - 1
                } else {
                    0
                }
            };

            tracing::info!("{}", last_fetched_screenshot_index);

            if position as usize > last_fetched_screenshot_index
                || last_fetched_screenshot_index == 0
            {
                tracing::info!("entered");
                const COUNT: i64 = 15;
                let new_screenshots = db::fetch_screenshots(
                    self.app.borrow().conn(),
                    last_fetched_screenshot_index as i64,
                    COUNT,
                );
                let new_screenshots = match new_screenshots {
                    Ok(n) => n,
                    Err(why) => {
                        tracing::error!("Encountered error: {:?}\n\twhile trying to fetch {} items from the database,\n\tstarting at index: {}", why, COUNT, last_fetched_screenshot_index);
                        return None;
                    }
                };

                self.screenshots
                    .borrow_mut()
                    .extend(new_screenshots.into_iter().map(RowData::new));
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
