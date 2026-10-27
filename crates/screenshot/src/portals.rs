use cairo::ImageSurface;
use gtk4::{
    gio, glib,
    prelude::{FileExt, InputStreamExtManual},
};

use crate::Result;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a desktop portal error: {0}")]
    Ashpd(#[from] ashpd::Error),
    #[error("Failed opening file(uri={uri}) for reading: {error}")]
    GioFile { error: glib::Error, uri: String },
}

pub(super) fn take_screenshot(tokio: Option<&tokio::runtime::Handle>) -> Result<ImageSurface> {
    let uri = tokio
        .expect("kcshot is attempting to use portals but there is no tokio runtime running")
        .block_on(async {
            ashpd::desktop::screenshot::Screenshot::request()
                .interactive(false)
                .modal(false)
                .send()
                .await
                .and_then(|r| r.response())
                .map(|s| s.uri().to_string())
        })
        .map_err(Error::Ashpd)?;

    let file = gio::File::for_uri(&uri);
    let read = file
        .read(gio::Cancellable::NONE)
        .map_err(|error| Error::GioFile {
            error,
            uri: uri.clone(),
        })?;

    // This is intentionally not using `?` to ensure screenshot file is deleted even if the surface can't be
    // created.
    let screenshot = ImageSurface::create_from_png(&mut read.into_read());

    // The org.freedesktop.Screenshot portal places the screenshots inside the user's home instead of
    // making temp files, so this is to ensure that they get deleted and the user's home isn't polluted.
    glib::MainContext::default().spawn_local(async move {
        if let Err(why) = file.delete_future(glib::Priority::LOW).await {
            tracing::error!("Failed to delete file {uri} due to {why}");
        }
    });

    Ok(screenshot?)
}
