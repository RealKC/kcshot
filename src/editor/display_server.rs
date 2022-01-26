use std::{env, io};

use cairo::{self, Error as CairoError, ImageSurface};
use gtk4::{
    prelude::{DisplayExt, MonitorExt, SurfaceExt},
    traits::NativeExt,
};
use once_cell::sync::OnceCell;
use tracing::error;

use crate::kcshot::KCShot;

use super::data::Rectangle;

mod wayland;
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
    #[error("Failed to get windows")]
    FailedToGetWindows,
    #[error("Encountered an error interacting with the X server: {0}")]
    Xorg(#[from] xorg::Error),
    #[error("Encountered an error interacting with the Wayland stack: {0}")]
    Wayland(#[from] wayland::Error),
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

#[derive(Default, Clone, Copy, Debug)]
struct WmFeatures {
    /// Whether the WM supports retrieving the client list
    supports_client_list: bool,
    /// Whether the WM supports retrieving the extents of frames around windows
    supports_frame_extents: bool,
    // Whether we're running under Wayland as a wayland client or not
    is_wayland: bool,
}

impl WmFeatures {
    /// Talks with the WM to get the featurs we're interested in
    /// These get cached and as such calling it multiple times in succession should be cheap
    fn get() -> Result<&'static Self> {
        static FEATURES: OnceCell<WmFeatures> = OnceCell::new();

        FEATURES.get_or_try_init(Self::get_impl)
    }

    fn get_impl() -> Result<WmFeatures> {
        let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "".into());
        let xdg_session_type = env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "".into());
        // For checking that Wayland features work even when kcshot is used under X (but on desktops providing
        // the necessary portals)
        let force_wayland_emulation =
            env::var("KCSHOT_FORCE_USE_PORTALS").unwrap_or_else(|_| "".into());

        let is_wayland = wayland_display.to_lowercase().contains("wayland")
            || xdg_session_type == "wayland"
            || force_wayland_emulation == "1";

        if is_wayland {
            wayland::get_wm_features()
        } else {
            xorg::get_wm_features()
        }
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

pub fn take_screenshot(app: &KCShot) -> Result<ImageSurface> {
    if WmFeatures::get()?.is_wayland {
        wayland::take_screenshot(app)
    } else {
        xorg::take_screenshot()
    }
}

/// Obtains a list of all windows from the display server, the list is in stacking order.
pub fn get_windows() -> Result<Vec<Window>> {
    if WmFeatures::get()?.is_wayland {
        wayland::get_windows()
    } else {
        xorg::get_windows()
    }
}

/// Gets the screen resolution
pub fn get_screen_resolution(window: &gtk4::Window) -> Rectangle {
    // Code based on https://discourse.gnome.org/t/get-screen-resolution-scale-factor-and-width-and-height-in-mm-for-wayland/7448

    let surface = window.surface();
    let display = surface.display();
    let monitor = display.monitor_at_surface(&surface);
    let geometry = monitor.geometry();

    Rectangle {
        x: geometry.x() as f64,
        y: geometry.y() as f64,
        w: geometry.width() as f64,
        h: geometry.height() as f64,
    }
}
