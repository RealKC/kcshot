#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::{env, fs, io, path};

use gtk4::{glib, prelude::*};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use self::kcshot::KCShot;

mod appwindow;
mod db;
mod editor;
mod ext;
mod history;
mod kcshot;
mod postcapture;
mod settings_window;
mod systray;

fn main() -> glib::ExitCode {
    let collector = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::Layer::new().with_writer(io::stderr));

    match make_file_writer() {
        Ok(file_writer) => {
            let collector = collector.with(fmt::Layer::new().with_writer(file_writer));

            tracing::subscriber::set_global_default(collector).expect("Failed to setup logging");
        }
        Err(why) => {
            tracing::subscriber::set_global_default(collector).expect("Failed to setup logging");

            tracing::info!("Failed to initialise file_writer: {why}");
        }
    }

    let prev = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s
        } else {
            "non-string payload"
        };

        let backtrace = std::backtrace::Backtrace::capture();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unknown thread>");

        if let Some(location) = info.location() {
            tracing::error!(
                "thread '{thread_name}' panicked at: {}:{}{}: '{payload}'\n{}",
                location.file(),
                location.line(),
                location.column(),
                backtrace
            );
        } else {
            tracing::error!(
                "thread '{thread_name}' panicked: '{payload}'\n{}",
                backtrace
            );
        }

        prev(info);
    }));

    let application = KCShot::new();

    let rc = application.run();

    if cfg!(feature = "heaptrack") {
        // SAFETY: At this point there should be no more active cairo objects. IF there are, that is to
        //         be considered a bug, as it likely means we are leaking cairo objects in some manner.
        //         I believe in that case _some_ kind of assertion will fire.
        unsafe {
            cairo::debug_reset_static_data();
        }
    }

    rc
}

#[derive(thiserror::Error, Debug)]
enum LogFileError {
    #[error("Failed to get state directory: {0}")]
    Xdg(#[from] xdg::BaseDirectoriesError),
    #[error("Failed to open file at path='{path}' with error='{error}'")]
    File {
        error: io::Error,
        path: path::PathBuf,
    },
    #[error("Failed to make file: {0}")]
    Io(#[from] io::Error),
    #[error("Writing to a log file was disabled through environment variables")]
    DisabledByEnv,
}

fn make_file_writer() -> Result<fs::File, LogFileError> {
    if env::var("KCSHOT_DISABLE_LOG_FILE").unwrap_or_else(|_| "0".into()) == "1" {
        return Err(LogFileError::DisabledByEnv);
    }

    let base_directories = xdg::BaseDirectories::with_prefix("kcshot")?;
    let pid = std::process::id();
    let path = base_directories.place_state_file(format!("logs/kcshot-{pid}.log"))?;

    fs::File::create(path.clone()).map_err(|error| LogFileError::File { error, path })
}
