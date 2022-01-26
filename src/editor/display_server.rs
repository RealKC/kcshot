use std::io;

use cairo::{self, Error as CairoError, ImageSurface};
use once_cell::sync::OnceCell;
use tracing::error;

use super::data::Rectangle;

mod xorg;

type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered an error from cairo: {0}")]
    Cairo(#[from] CairoError),
    #[error("Encountered an I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to estabilish a connection to the X server: {0:?}")]
    XcbConnection(#[from] xcb::ConnError),
    #[error("Encountered an X protocol error: {0:?}")]
    XcbProtocol(xcb::ProtocolError),
    #[error("Got an error trying to make a temporary file: {0}")]
    TempFile(#[from] gtk4::glib::Error),
    #[error("Failed to take screenshot. (No root screens? No cursor?)")]
    FailedToTakeScreenshot,
    #[error("Failed to get screen resolution. (No root screens? No root windows on the screens that exist?")]
    FailedToGetScreenResolution,
    #[error("Failed to get windows")]
    FailedToGetWindows,
    #[error("Encountered an error interacting with the X server: {0}")]
    Xorg(#[from] xorg::Error),
}

impl From<cairo::IoError> for Error {
    fn from(cerr: cairo::IoError) -> Self {
        match cerr {
            cairo::IoError::Cairo(cairo) => Self::Cairo(cairo),
            cairo::IoError::Io(io) => Self::Io(io),
        }
    }
}

impl From<xcb::Error> for Error {
    fn from(xerror: xcb::Error) -> Self {
        match xerror {
            xcb::Error::Connection(err) => Self::XcbConnection(err),
            xcb::Error::Protocol(err) => Self::XcbProtocol(err),
        }
    }
}

#[derive(Debug)]
pub struct Window {
    /// This fields contains the rect of the window that also encompasses window decorations
    pub outer_rect: Rectangle,
    /// This fields contains the rect of the window that **only** encompasses the content
    pub content_rect: Rectangle,
}

#[derive(Default, Clone, Copy)]
struct WmFeatures {
    /// Whether the WM supports retrieving the client list
    supports_client_list: bool,
    /// Whether the WM supports retrieving the extents of frames around windows
    supports_frame_extents: bool,
}

impl WmFeatures {
    /// Talks with the WM to get the featurs we're interested in
    /// These get cached and as such calling it multiple times in succession should be cheap
    fn get() -> Result<&'static Self> {
        static FEATURES: OnceCell<WmFeatures> = OnceCell::new();

        FEATURES.get_or_try_init(xorg::get_wm_features)
    }
}

pub fn can_retrieve_windows() -> bool {
    match WmFeatures::get() {
        Ok(wm_features) => wm_features.supports_client_list,
        Err(why) => {
            tracing::info!("Encountered {why} in can_retrieve_windows\n\treturning false");
            false
        }
    }
}

pub fn take_screenshot() -> Result<ImageSurface> {
    xorg::take_screenshot()
}

/// Obtains a list of all windows from the display server, the list is in stacking order.
pub fn get_windows() -> Result<Vec<Window>> {
    xorg::get_windows()
}

/// Gets the screen resolution
///
/// # Returns
/// The first item of the tuple is the width, the second is the height
pub fn get_screen_resolution() -> Result<(i32, i32)> {
    xorg::get_screen_resolution()
}
