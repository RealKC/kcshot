use gtk4::{
    gio::ListModel as GListModel,
    glib::{self, Sender},
    subclass::prelude::ObjectSubclassIsExt,
};

pub use self::rowdata::RowData;
use crate::kcshot::KCShot;

mod rowdata;

glib::wrapper! {
    pub struct HistoryModel(ObjectSubclass<underlying::ListModel>)
        @implements GListModel;
}

impl HistoryModel {
    pub fn new(application: &KCShot) -> Self {
        glib::Object::new(&[("application", application)]).unwrap()
    }

    /// Inserts a screenshot to the internally maintained list of screenshots.
    ///
    /// # Note
    /// This does not call `GListModel::items_changed`, you'll have to call it
    /// yourself properly.
    pub fn insert_screenshot(&self, screenshot: RowData) {
        self.imp().screenshots.borrow_mut().insert(0, screenshot);
    }
}

/// This type is used to notify the HistoryModel that a new screenshot was taken, and it additionally
/// carries a [`self::RowData`] with the newly taken screenshot.
///
/// It caused [`HistoryModel::insert_screenshot`] to be called.
pub type ModelNotifier = Sender<RowData>;

mod underlying {
    use std::{cell::RefCell, rc::Rc};

    use gtk4::{
        gio,
        glib::{self, Object, ParamSpec, ParamSpecObject, StaticType, ToValue, Value},
        prelude::*,
        subclass::prelude::*,
    };
    use once_cell::sync::Lazy;

    use super::rowdata::RowData;
    use crate::{db, kcshot::KCShot};

    #[derive(Default)]
    pub struct ListModel {
        pub(super) app: RefCell<Option<KCShot>>,
        pub(super) screenshots: Rc<RefCell<Vec<RowData>>>,
    }

    impl ListModel {
        fn app(&self) -> KCShot {
            self.app
                .borrow()
                .clone()
                .expect("ListModel::app must be set for it to work properly")
        }
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
            let n_items = db::number_of_history_itms(self.app().conn());
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
                    tracing::error!("Failed to get number of screenshots in history: {why}");
                    panic!() // yolo
                }
            }
        }

        #[tracing::instrument(skip(self))]
        fn item(&self, _: &Self::Type, position: u32) -> Option<Object> {
            let last_fetched_screenshot_index = {
                let len = self.screenshots.borrow().len();
                if len > 0 {
                    len - 1
                } else {
                    0
                }
            };

            if position as usize > last_fetched_screenshot_index
                || last_fetched_screenshot_index == 0
            {
                const COUNT: i64 = 15;
                let new_screenshots = db::fetch_screenshots(
                    self.app().conn(),
                    last_fetched_screenshot_index as i64,
                    COUNT,
                );
                let new_screenshots = match new_screenshots {
                    Ok(n) => n,
                    Err(why) => {
                        tracing::error!("Encountered error: {why:?}\n\twhile trying to fetch {COUNT} items from the database,\n\tstarting at index: {last_fetched_screenshot_index}");
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
                vec![ParamSpecObject::new(
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
        fn property(&self, _: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "application" => self.app.borrow().to_value(),
                name => {
                    tracing::error!("Unknown property: {name}");
                    panic!()
                }
            }
        }

        #[tracing::instrument(skip(self))]
        fn set_property(&self, _: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "application" => {
                    let application = value.get::<KCShot>().ok();
                    self.app.replace(application);
                }
                name => tracing::warn!("Unknown property: {name}"),
            }
        }
    }
}
