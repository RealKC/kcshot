use std::path::PathBuf;

use diesel::SqliteConnection;
use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use kcshot_data::settings::Settings;

use crate::{
    appwindow,
    history::{HistoryModel, ModelNotifier},
};

glib::wrapper! {
    pub struct KCShot(ObjectSubclass<underlying::KCShot>)
    @extends gio::Application, gtk4::Application,
    @implements gio::ActionGroup, gio::ActionMap;
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
            .build()
    }

    #[track_caller]
    pub fn the() -> Self {
        gio::Application::default()
            .and_downcast()
            .expect("The global application should be of type `KCShot`")
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

    pub fn tokio_rt(&self) -> Option<&tokio::runtime::Handle> {
        self.imp()
            .tokio_rt
            .as_ref()
            .map(tokio::runtime::Runtime::handle)
    }
}

mod underlying {
    use std::{
        cell::{Cell, OnceCell, RefCell},
        ops::ControlFlow,
    };

    use diesel::SqliteConnection;
    use gtk4::{
        gdk,
        gio::{self, prelude::*},
        glib,
        prelude::*,
        subclass::prelude::*,
    };

    use super::Settings;
    use crate::{
        appwindow, db,
        editor::EditorWindow,
        history::{HistoryModel, ModelNotifier},
        systray,
    };

    const NO_WINDOW_OPTION: &str = "no-window";
    const SCREENSHOT_OPTION: &str = "screenshot";

    pub struct KCShot {
        pub(super) show_main_window: Cell<bool>,
        pub(super) take_screenshot: Cell<bool>,
        pub(super) database_connection: RefCell<Option<SqliteConnection>>,
        history_model: RefCell<Option<HistoryModel>>,
        model_notifier: OnceCell<ModelNotifier>,
        pub(super) systray_initialised: Cell<bool>,
        pub(super) window: OnceCell<appwindow::AppWindow>,
        pub(super) tokio_rt: Option<tokio::runtime::Runtime>,
    }

    impl KCShot {
        pub(super) fn history_model(&self) -> HistoryModel {
            self.history_model.borrow().clone().unwrap()
        }

        pub(super) fn model_notifier(&self) -> ModelNotifier {
            self.model_notifier.get().cloned().unwrap()
        }

        fn add_cli_args(&self) {
            let app = self.obj();

            app.add_main_option(
                NO_WINDOW_OPTION,
                glib::Char::from(b'n'),
                glib::OptionFlags::NONE,
                glib::OptionArg::None,
                "Don't show any windows",
                None,
            );

            app.add_main_option(
                SCREENSHOT_OPTION,
                glib::Char::from(b's'),
                glib::OptionFlags::NONE,
                glib::OptionArg::None,
                &format!("Take a screenshot (mutually exclusive with --{NO_WINDOW_OPTION})"),
                None,
            );
        }

        fn setup_actions(&self) {
            let show_main_window = gio::ActionEntry::builder("show-main-window")
                .activate(|app: &super::KCShot, _, _| app.main_window().present())
                .build();

            let new_editor_window = gio::ActionEntry::builder("new-editor-window")
                .activate(|app: &super::KCShot, _, _| {
                    let editing_starts_with_cropping =
                        Settings::open().editing_starts_with_cropping();

                    EditorWindow::show(app.upcast_ref(), editing_starts_with_cropping);
                })
                .build();

            self.obj()
                .add_action_entries([show_main_window, new_editor_window]);
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
                tokio_rt: kcshot_screenshot::will_make_use_of_desktop_portals().then(|| {
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .worker_threads(1)
                        .max_blocking_threads(1)
                        .build()
                        .unwrap()
                }),
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
                .field("tokio_rt", &self.tokio_rt)
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

            self.add_cli_args();

            match db::open() {
                Ok(conn) => self.database_connection.replace(Some(conn)),
                Err(why) => {
                    tracing::error!("Failed to open history: {why}");
                    panic!(); // FIXME: It'd be nicer if we just popped a dialog or something
                }
            };

            self.history_model.replace(Some(HistoryModel::default()));
            let (tx, mut rx) = tokio::sync::mpsc::channel(16);

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
            glib::MainContext::default().spawn_local(async move {
                while let Some(screenshot) = rx.recv().await {
                    model.insert_screenshot(screenshot);
                    // I've tried moving this items_changed call inside HistoryModel::insert_screenshot,
                    // but the items showed up twice in the view if you had two windows opened for some reason.
                    model.items_changed(0, 0, 1);
                }
            });
        }
    }

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

                self.obj().main_window().present();
            }
        }

        fn handle_local_options(&self, options: &glib::VariantDict) -> ControlFlow<glib::ExitCode> {
            if let Err(err) = self.obj().register(None::<&gio::Cancellable>) {
                tracing::error!("Failed to register app: {err}");
                return ControlFlow::Break(glib::ExitCode::FAILURE);
            }

            match (
                options.contains(NO_WINDOW_OPTION),
                options.contains(SCREENSHOT_OPTION),
            ) {
                (true, true) => {
                    let prog_name = glib::prgname().unwrap_or_else(|| "kcshot".into());

                    eprintln!(
                        "{prog_name}: --{NO_WINDOW_OPTION} and --{SCREENSHOT_OPTION} mutually exclusive"
                    );

                    ControlFlow::Break(glib::ExitCode::FAILURE)
                }
                (true, false) => self.parent_handle_local_options(options),
                (false, true) => {
                    self.obj().activate_action("new-editor-window", None);

                    ControlFlow::Break(glib::ExitCode::SUCCESS)
                }
                (false, false) => {
                    self.obj().activate_action("show-main-window", None);
                    ControlFlow::Break(glib::ExitCode::SUCCESS)
                }
            }
        }

        fn startup(&self) {
            self.parent_startup();

            self.setup_actions();

            // This hold has no matching release intentionally so that the application keeps running
            // in the background even when no top-level windows are spawned. (This is the case when
            // we get started with `--no-window`)
            std::mem::forget(self.obj().hold());

            #[cfg(not(kcshot_linting))]
            {
                if let Err(why) = gio::resources_register_include!("compiled.gresource") {
                    tracing::error!("Failed loading resources: {why}");
                }
            }

            let settings = Settings::open();

            if settings.saved_screenshots_path().is_empty() {
                let default_folder = if cfg!(feature = "xdg-paths") {
                    xdg::BaseDirectories::with_prefix("kcshot")
                        .get_data_home()
                        .unwrap()
                        .join("Screenshots")
                } else {
                    std::env::current_dir().unwrap().join("Screenshots")
                };

                tracing::info!("'saved-screenshots-path' was empty, set it to {default_folder:?}");
                settings.set_saved_screenshots_path(default_folder.to_str().unwrap());
            }

            gtk4::Window::set_default_icon_name("kcshot");

            let provider = gtk4::CssProvider::new();
            provider.load_from_data(include_str!("style.css"));
            gtk4::style_context_add_provider_for_display(
                &gdk::Display::default().unwrap(),
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    impl GtkApplicationImpl for KCShot {}
}
