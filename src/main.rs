#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::{env, fs, io, path, process::ExitCode};

use gtk4::prelude::*;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use self::kcshot::KCShot;

mod appwindow;
mod db;
mod editor;
mod historymodel;
mod kcshot;
mod postcapture;
mod properties;
mod systray;

fn main() -> ExitCode {
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

    let application = KCShot::new();

    let rc = ExitCode::from(application.run() as u8);

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
