use gtk4::{gio, glib, prelude::*, subclass::prelude::*};

use crate::{appwindow, editor};

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
}

pub fn build_ui(app: &KCShot) {
    let instance = underlying::KCShot::from_instance(app);

    let take_screenshot = *instance.take_screenshot.borrow();
    let show_main_window = *instance.show_main_window.borrow();

    if take_screenshot {
        instance.take_screenshot.replace(false);
        let window = editor::EditorWindow::new(app.upcast_ref());
        window.set_decorated(false);
        window.fullscreen();

        window.show()
    } else if show_main_window {
        instance.show_main_window.replace(false);
        let window = appwindow::AppWindow::new(app);

        window.show()
    }
}

mod underlying {
    use std::{cell::RefCell, ffi::OsString};

    use gtk4::{
        gio::{self, prelude::*},
        glib,
        subclass::prelude::*,
    };
    use once_cell::sync::Lazy;

    #[derive(Debug)]
    pub struct KCShot {
        pub(super) show_main_window: RefCell<bool>,
        pub(super) take_screenshot: RefCell<bool>,
    }

    impl Default for KCShot {
        fn default() -> Self {
            Self {
                show_main_window: RefCell::new(true),
                take_screenshot: RefCell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KCShot {
        const NAME: &'static str = "KCShot";
        type Type = super::KCShot;
        type ParentType = gtk4::Application;
    }

    impl ObjectImpl for KCShot {}

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
            tracing::info!("hm");
            for argument in command_line.arguments() {
                // FIXME: Right now, if we specify `-n` for the first instance, it doesn't keep running
                //        in background, we should fix that.
                if NO_WINDOW_FLAGS_OS.contains(&argument) {
                    self.show_main_window.replace(false);
                    self.take_screenshot.replace(false);
                } else if SCREENSHOT_FLAGS_OS.contains(&argument) {
                    self.take_screenshot.replace(true);
                    self.show_main_window.replace(false);
                }
            }

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

            tracing::info!("Got arguments: {:?}", arguments);

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
    }

    impl GtkApplicationImpl for KCShot {}
}
