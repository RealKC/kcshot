use diesel::SqliteConnection;
use gtk4::{gio, glib, prelude::*, subclass::prelude::*};

use crate::{appwindow, editor, historymodel::ModelNotifier};

glib::wrapper! {
    pub struct KCShot(ObjectSubclass<underlying::KCShot>) @extends gio::Application, gtk4::Application, @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for KCShot {
    fn default() -> Self {
        Self::new()
    }
}

impl KCShot {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &"kc.kcshot"),
            ("flags", &gio::ApplicationFlags::HANDLES_COMMAND_LINE),
        ])
        .expect("Failed to create KCShot")
    }

    pub fn conn(&self) -> &SqliteConnection {
        let impl_ = self.imp();
        impl_.database_connection.get().unwrap()
    }

    pub fn screenshot_folder() -> String {
        let settings = gio::Settings::new("kc.kcshot");
        settings.string("saved-screenshots-path").into()
    }

    /// This is to be used for the purpose of notifying the [`crate::historymodel::HistoryMode`]
    /// that a new screenshot was added, by sending a [`crate::historymodel::RowData`] with the
    /// newly taken screenshot to it.
    pub fn model_notifier(&self) -> ModelNotifier {
        self.imp().model_notifier()
    }
}

pub fn build_ui(app: &KCShot) {
    let instance = app.imp();

    let take_screenshot = *instance.take_screenshot.borrow();
    let show_main_window = *instance.show_main_window.borrow();

    let history_model = instance.history_model();

    let window = appwindow::AppWindow::new(app, &history_model);

    if take_screenshot {
        instance.take_screenshot.replace(false);
        let window = editor::EditorWindow::new(app.upcast_ref());
        window.set_decorated(false);
        window.fullscreen();

        window.show()
    } else if show_main_window {
        instance.show_main_window.replace(false);

        window.show()
    }
}

mod underlying {
    use std::{cell::RefCell, ffi::OsString};

    use diesel::SqliteConnection;
    use gtk4::{
        gio::{self, prelude::*},
        glib::{self},
        subclass::prelude::*,
    };
    use once_cell::sync::{Lazy, OnceCell};

    use crate::{
        db,
        historymodel::{HistoryModel, ModelNotifier, RowData},
    };

    pub struct KCShot {
        pub(super) show_main_window: RefCell<bool>,
        pub(super) take_screenshot: RefCell<bool>,
        pub(super) database_connection: OnceCell<SqliteConnection>,
        history_model: RefCell<Option<HistoryModel>>,
        model_notifier: OnceCell<ModelNotifier>,
    }

    impl KCShot {
        pub(super) fn history_model(&self) -> HistoryModel {
            self.history_model.borrow().clone().unwrap()
        }

        pub(super) fn model_notifier(&self) -> ModelNotifier {
            self.model_notifier.get().cloned().unwrap()
        }
    }

    impl Default for KCShot {
        fn default() -> Self {
            Self {
                show_main_window: RefCell::new(true),
                take_screenshot: RefCell::new(false),
                database_connection: Default::default(),
                history_model: Default::default(),
                model_notifier: Default::default(),
            }
        }
    }

    impl std::fmt::Debug for KCShot {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("KCShot")
                .field("show_main_window", &self.show_main_window)
                .field("take_screenshot", &self.take_screenshot)
                .field("database_connection", &"<sqlite connection>")
                .finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KCShot {
        const NAME: &'static str = "KCShot";
        type Type = super::KCShot;
        type ParentType = gtk4::Application;
    }

    impl ObjectImpl for KCShot {
        fn constructed(&self, obj: &Self::Type) {
            let res = self.database_connection.set(db::open().unwrap());

            if res.is_err() {
                tracing::error!("Failed setting self.database_connection");
            }

            self.history_model.replace(Some(HistoryModel::new(obj)));
            let (tx, rx) = glib::MainContext::channel::<RowData>(glib::PRIORITY_DEFAULT);

            if self.model_notifier.set(tx).is_err() {
                tracing::error!("KCShot::constructed called multiple times on the same instance!");
                panic!()
            }

            let model = self.history_model();
            // The purpose of this code is to ensure that the model behaves correctly to its consumers
            // > Stated another way: in general, it is assumed that code making a series of accesses
            // > to the model via the API, without returning to the mainloop, and without calling
            // > other code, will continue to view the same contents of the model.
            // (src: https://docs.gtk.org/gio/method.ListModel.items_changed.html)
            //
            // It appears that this is the proper way to achieve that.
            rx.attach(
                None,
                glib::clone!(@weak model => @default-return Continue(false), move |msg| {
                    model.insert_screenshot(msg);
                    // I've tried moving this items_changed call inside HistoryModel::insert_screenshot,
                    // but the items showed up twice in the view if you had two windows opened for some reason.
                    model.items_changed(0, 0, 1);
                    Continue(true)
                }),
            );
        }
    }

    const LONG: usize = 1;

    static SCREENSHOT_FLAGS_OS: Lazy<Vec<OsString>> =
        Lazy::new(|| vec!["-s".into(), "--screenshot".into()]);
    const SCREENSHOT_FLAGS: &[&str] = &["-s", "--screenshot"];
    static NO_WINDOW_FLAGS_OS: Lazy<Vec<OsString>> =
        Lazy::new(|| vec!["-n".into(), "--no-window".into()]);
    const NO_WINDOW_FLAGS: &[&str] = &["-n", "--no-window"];

    impl ApplicationImpl for KCShot {
        // This is called in the primary instance
        fn command_line(
            &self,
            app: &Self::Type,
            command_line: &gio::ApplicationCommandLine,
        ) -> i32 {
            let mut show_main_window = true;
            for argument in command_line.arguments() {
                if NO_WINDOW_FLAGS_OS.contains(&argument) {
                    show_main_window = false;
                    self.take_screenshot.replace(false);
                } else if SCREENSHOT_FLAGS_OS.contains(&argument) {
                    self.take_screenshot.replace(true);
                    show_main_window = false;
                }
            }
            self.show_main_window.replace(show_main_window);

            app.activate();

            -1
        }

        // This is called in remote instances
        fn local_command_line(
            &self,
            _: &Self::Type,
            arguments: &mut gio::subclass::ArgumentList,
        ) -> Option<i32> {
            let prog_name = glib::prgname().unwrap_or_else(|| "kcshot".to_string());
            let usage = format!(
                r#"Usage:
  {} [OPTION...]

Help Options:
  -h, --help           Show help options

Application Options:
  -n, --no-window      Don't show any windows
  -s, --screenshot     Take a screenshot (mutually exclusive with -n)
"#,
                prog_name
            );

            if arguments.contains(&"-h".into()) || arguments.contains(&"--help".into()) {
                eprintln!("{}", usage);
                return Some(0);
            }

            let take_screenshot = arguments.iter().any(|os| SCREENSHOT_FLAGS_OS.contains(os));
            let no_window = arguments.iter().any(|os| NO_WINDOW_FLAGS_OS.contains(os));

            if take_screenshot && no_window {
                eprintln!(
                    "{}: {} and {} are mutually exclusive\n{}",
                    prog_name, SCREENSHOT_FLAGS[LONG], NO_WINDOW_FLAGS[LONG], usage
                );
                return Some(1);
            }

            None
        }

        fn startup(&self, application: &Self::Type) {
            self.parent_startup(application);

            if let Err(why) = gio::resources_register_include!("compiled.gresource") {
                tracing::error!("Failed loading resources: {why}");
            }

            let settings = gio::Settings::new("kc.kcshot");

            if settings.string("saved-screenshots-path").is_empty() {
                #[cfg(not(feature = "xdg"))]
                let default_folder = std::env::current_dir().unwrap();
                #[cfg(feature = "xdg")]
                let default_folder = xdg::BaseDirectories::with_prefix("kcshot")
                    .unwrap()
                    .get_data_home();

                tracing::info!("'saved-screenshots-path' was empty, set it to {default_folder:?}");
                settings
                    .set_string("saved-screenshots-path", default_folder.to_str().unwrap())
                    .unwrap();
            }
        }
    }

    impl GtkApplicationImpl for KCShot {}
}
