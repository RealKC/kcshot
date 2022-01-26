use cairo::ImageSurface;
use gtk4::glib;

use crate::kcshot::KCShot;

use super::{Result, Window, WmFeatures};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a desktop portal error: {0}")]
    Ashpd(#[from] ashpd::Error),
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let wm_features = WmFeatures {
        is_wayland: true,
        ..Default::default()
    };

    Ok(wm_features)
}

pub(crate) fn take_screenshot(app: &KCShot) -> Result<ImageSurface> {
    let window_identifier = app.window_identifier();
    let ctx = glib::MainContext::default();
    let uri = ctx
        .block_on(async { ashpd::desktop::screenshot::take(window_identifier, false, false).await })
        .map_err(Error::Ashpd)?;

    let mut file = std::fs::File::open(&uri)?;
    Ok(ImageSurface::create_from_png(&mut file)?)
}

pub(crate) fn get_windows() -> Result<Vec<Window>> {
    // FIXME: Look into ways we could do this
    Ok(vec![])
}
