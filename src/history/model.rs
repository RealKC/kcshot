use diesel::SqliteConnection;
use gtk4::{
    gio::ListModel as GListModel,
    glib::{self},
    subclass::prelude::ObjectSubclassIsExt,
};

use super::RowData;

glib::wrapper! {
    pub struct HistoryModel(ObjectSubclass<underlying::HistoryModel>)
        @implements GListModel;
}

impl HistoryModel {
    pub fn with_database_connection<F, R>(&self, f: F) -> R
    where
        F: Fn(&mut SqliteConnection) -> R,
    {
        self.imp().with_database_connection(f)
    }
}

impl Default for HistoryModel {
    fn default() -> Self {
        glib::Object::new()
    }
}

mod underlying {
    use std::{cell::RefCell, collections::HashMap};

    use adw::gio::prelude::ListModelExt;
    use diesel::{
        SqliteConnection,
        sqlite::{SqliteChangeOps, SqliteUpdateRouter},
    };
    use gtk4::{
        gio,
        glib::{self, Object, prelude::*},
        subclass::prelude::*,
    };

    use super::RowData;
    use crate::db::{self, schema};

    #[derive(Default)]
    pub struct HistoryModel {
        pub(super) database_connection: RefCell<Option<SqliteConnection>>,

        cache: RefCell<HashMap<u32, RowData>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryModel {
        const NAME: &'static str = "KcshotListModel";
        type Type = super::HistoryModel;
        type ParentType = Object;
        type Interfaces = (gio::ListModel,);
    }

    impl HistoryModel {
        pub(super) fn with_database_connection<F, R>(&self, f: F) -> R
        where
            F: Fn(&mut SqliteConnection) -> R,
        {
            let mut conn = self.database_connection.borrow_mut();
            f(conn.as_mut().unwrap())
        }

        fn attach_update_hooks(
            &self,
            conn: &mut SqliteConnection,
            tx: tokio::sync::mpsc::Sender<Event>,
        ) {
            conn.on_update(SqliteUpdateRouter::new().on(
                schema::screenshots::table,
                SqliteChangeOps::INSERT,
                {
                    let tx = tx.clone();
                    move |event| {
                        tracing::info!("Received insert for screenshots: {event:?}");

                        let _ = tx.try_send(Event::Added);
                    }
                },
            ));
        }
    }

    impl ListModelImpl for HistoryModel {
        fn item_type(&self) -> glib::Type {
            RowData::static_type()
        }

        fn n_items(&self) -> u32 {
            self.with_database_connection(db::number_of_history_itms)
                .unwrap() as u32
        }

        #[tracing::instrument(skip(self))]
        fn item(&self, position: u32) -> Option<Object> {
            if let Some(cached) = self.cache.borrow().get(&position) {
                tracing::trace!("Returned screenshot from cache: {cached:?}");

                return Some(cached.clone().upcast());
            }

            let fetched = self.with_database_connection(|conn| {
                db::get_screenshot_at_index(conn, position)
                    .map(RowData::new)
                    .ok()
            });

            if let Some(fetched) = fetched.clone() {
                self.cache.borrow_mut().insert(position, fetched);
            }

            tracing::trace!("Fetched screenshot from database: {fetched:?}");

            fetched.map(Cast::upcast)
        }
    }

    impl ObjectImpl for HistoryModel {
        fn constructed(&self) {
            let (tx, mut rx) = tokio::sync::mpsc::channel(16);

            match db::open() {
                Ok(mut conn) => {
                    self.attach_update_hooks(&mut conn, tx);
                    self.database_connection.replace(Some(conn))
                }
                Err(why) => {
                    tracing::error!("Failed to open history: {why}");
                    panic!(); // FIXME: It'd be nicer if we just popped a dialog or something
                }
            };

            let weak_self = self.obj().downgrade();
            glib::MainContext::default().spawn_local(async move {
                while let Some(event) = rx.recv().await {
                    #[expect(
                        irrefutable_let_patterns,
                        reason = "will stop being irrefutable once deletion is implemented"
                    )]
                    if let Event::Added = event {
                        let Some(strong) = weak_self.upgrade() else {
                            break;
                        };

                        *strong.imp().cache.borrow_mut() = HashMap::new();
                        strong.items_changed(0, 0, 1);
                    }
                }
            });
        }
    }

    enum Event {
        Added,
    }
}
