use std::path::PathBuf;

use diesel::SqliteConnection;
use gtk4::{gio, glib, subclass::prelude::*};
use kcshot_data::settings::Settings;

use crate::{
    appwindow,
    historymodel::{HistoryModel, ModelNotifier},
};

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
        glib::Object::builder()
            .property("application-id", "kc.kcshot")
            .property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build()
    }

    pub fn with_conn<F, R>(&self, f: F) -> R
    where
        F: Fn(&mut SqliteConnection) -> R,
    {
        let impl_ = self.imp();
        let mut conn = impl_.database_connection.borrow_mut();
        f(conn.as_mut().unwrap())
    }

    pub fn screenshot_folder() -> PathBuf {
        Settings::open().saved_screenshots_path().into()
    }

    /// This is to be used for the purpose of notifying the [`crate::historymodel::HistoryMode`]
    /// that a new screenshot was added, by sending a [`crate::historymodel::RowData`] with the
    /// newly taken screenshot to it.
    pub fn model_notifier(&self) -> ModelNotifier {
        self.imp().model_notifier()
    }

    pub fn history_model(&self) -> HistoryModel {
        self.imp().history_model()
    }

    /// Returns the "main" window of kcshot, i.e. the one that contains grid of screenshots and a button pain
    pub fn main_window(&self) -> appwindow::AppWindow {
        self.imp()
            .window
            .get_or_init(|| appwindow::AppWindow::new(self, &self.history_model()))
            .clone()
    }

    pub fn main_window_identifier(&self) -> kcshot_screenshot::WindowIdentifier {
        if !self.imp().native_main_window_initialised.get() {
            // HACK: Ensure the main window gets created on the xorg/wayland so it's fine to
            //       create a WindowIdentifier from the main window.
            //       (This code path should only be reached when -n or -s is passed to the
            //       first instance of kcshot.)
            use gtk4::prelude::{GtkWindowExt, WidgetExt};
            self.main_window().present();
            self.main_window().hide();

            // We don't need to do this dance multiple times, so guard against that.
            self.imp().native_main_window_initialised.set(true);
        }

        glib::MainContext::default().block_on(kcshot_screenshot::WindowIdentifier::from_native(
            &self.main_window(),
        ))
    }
}

mod underlying {
    use std::{
        cell::{Cell, RefCell},
        ffi::OsString,
    };

    use diesel::SqliteConnection;
    use gtk4::{
        gio::{self, prelude::*},
        glib,
        prelude::*,
        subclass::prelude::*,
    };
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::Settings;
    use crate::{
        appwindow, db,
        editor::EditorWindow,
        historymodel::{HistoryModel, ModelNotifier, RowData},
        systray,
    };

    pub struct KCShot {
        pub(super) show_main_window: Cell<bool>,
        pub(super) take_screenshot: Cell<bool>,
        pub(super) database_connection: RefCell<Option<SqliteConnection>>,
        history_model: RefCell<Option<HistoryModel>>,
        model_notifier: OnceCell<ModelNotifier>,
        pub(super) systray_initialised: Cell<bool>,
        pub(super) window: OnceCell<appwindow::AppWindow>,
        /// See comments inside of [`super::KCShot::main_window_identifier`]
        pub(super) native_main_window_initialised: Cell<bool>,
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
                show_main_window: Cell::new(true),
                take_screenshot: Cell::new(false),
                database_connection: Default::default(),
                history_model: Default::default(),
                model_notifier: Default::default(),
                systray_initialised: Cell::new(false),
                window: Default::default(),
                native_main_window_initialised: Cell::new(false),
            }
        }
    }

    impl std::fmt::Debug for KCShot {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("KCShot")
                .field("show_main_window", &self.show_main_window)
                .field("take_screenshot", &self.take_screenshot)
                .field("database_connection", &"<sqlite connection>")
                .field("history_model", &self.history_model)
                .field("model_notifier", &self.model_notifier)
                .field("systray_initialised", &self.systray_initialised)
                .field("window", &self.window)
                .field(
                    "native_main_window_intiialised",
                    &self.native_main_window_initialised,
                )
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
        fn constructed(&self) {
            self.parent_constructed();

            match db::open() {
                Ok(conn) => self.database_connection.replace(Some(conn)),
                Err(why) => {
                    tracing::error!("Failed to open history: {why}");
                    panic!(); // FIXME: It'd be nicer if we just popped a dialog or something
                }
            };

            self.history_model
                .replace(Some(HistoryModel::new(&self.obj())));
            let (tx, rx) = glib::MainContext::channel::<RowData>(glib::PRIORITY_DEFAULT);

            self.model_notifier
                .set(tx)
                .expect("KCShot::constructed called multiple times on the same instance!");

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
        fn activate(&self) {
            self.parent_activate();

            let take_screenshot = self.take_screenshot.get();
            let show_main_window = self.show_main_window.get();

            // We initialise the systray here because I believe that with other backends it might not be valid
            // to do it in startup (through we only support one systray backend for now...)
            if !self.systray_initialised.get() {
                systray::init(&self.obj());
                self.systray_initialised.set(true);
            }

            if take_screenshot {
                self.take_screenshot.set(false);

                let editing_starts_with_cropping = Settings::open().editing_starts_with_cropping();

                EditorWindow::show(self.obj().upcast_ref(), editing_starts_with_cropping);
            } else if show_main_window {
                self.show_main_window.set(false);

                // See comments in super::KCShot::main_window_identifier
                self.native_main_window_initialised.set(true);
                self.obj().main_window().present();
            }
        }

        // This is called in the primary instance
        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> i32 {
            let mut show_main_window = true;
            for argument in command_line.arguments() {
                if NO_WINDOW_FLAGS_OS.contains(&argument) {
                    show_main_window = false;
                    self.take_screenshot.set(false);
                } else if SCREENSHOT_FLAGS_OS.contains(&argument) {
                    self.take_screenshot.set(true);
                    show_main_window = false;
                }
            }
            self.show_main_window.set(show_main_window);

            self.obj().activate();

            -1
        }

        // This is called in remote instances
        fn local_command_line(&self, arguments: &mut gio::subclass::ArgumentList) -> Option<i32> {
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

        fn startup(&self) {
            self.parent_startup();

            // This hold has no matching release intentionally so that the application keeps running
            // in the background even when no top-level windows are spawned. (This is the case when
            // we get started with `--no-window`)
            std::mem::forget(self.obj().hold());

            if let Err(why) = gio::resources_register_include!("compiled.gresource") {
                tracing::error!("Failed loading resources: {why}");
            }

            let settings = Settings::open();

            if settings.saved_screenshots_path().is_empty() {
                let default_folder = if cfg!(feature = "xdg-paths") {
                    xdg::BaseDirectories::with_prefix("kcshot")
                        .unwrap()
                        .get_data_home()
                        .join("Screenshots")
                } else {
                    std::env::current_dir().unwrap().join("Screenshots")
                };

                tracing::info!("'saved-screenshots-path' was empty, set it to {default_folder:?}");
                settings.set_saved_screenshots_path(default_folder.to_str().unwrap());
            }

            gtk4::Window::set_default_icon_name("kcshot");
        }
    }

    impl GtkApplicationImpl for KCShot {}
}
