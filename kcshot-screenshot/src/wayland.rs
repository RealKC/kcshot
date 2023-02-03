#![allow(clippy::unnecessary_wraps /*, reason = "The point of the wraps is to keep a consistent interface between xorg and wayland implementations" */)]

use cairo::ImageSurface;
use gtk4::{
    gio, glib,
    prelude::{FileExt, InputStreamExtManual},
};

use super::{Result, Window, WmFeatures};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a desktop portal error: {0}")]
    Ashpd(#[from] ashpd::Error),
    #[error("Failed opening file(uri={uri}) for reading: {error}")]
    GioFile { error: glib::Error, uri: String },
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let wm_features = WmFeatures {
        is_wayland: true,
        ..Default::default()
    };

    Ok(wm_features)
}

pub(super) fn take_screenshot(tokio: Option<&tokio::runtime::Handle>) -> Result<ImageSurface> {
    let uri = tokio
        .expect("kcshot is attempting to use portals but there is no tokio runtime running")
        .block_on(async {
            ashpd::desktop::screenshot::ScreenshotRequest::default()
                .interactive(false)
                .modal(false)
                .build()
                .await
        })
        .map_err(Error::Ashpd)?
        .to_string();

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
        if let Err(why) = file.delete_future(glib::PRIORITY_LOW).await {
            tracing::error!("Failed to delete file {uri} due to {why}");
        }
    });

    Ok(screenshot?)
}

pub(super) fn get_windows() -> Result<Vec<Window>> {
    // FIXME: Look into ways we could do this
    Ok(vec![])
}
