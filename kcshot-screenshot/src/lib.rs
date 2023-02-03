use std::{env, io};

use cairo::{self, Error as CairoError, ImageSurface};
use kcshot_data::geometry::Rectangle;
use once_cell::sync::OnceCell;
use tracing::error;

mod wayland;
mod xorg;

pub use ashpd::WindowIdentifier;

type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered an error from cairo: {0}")]
    Cairo(#[from] CairoError),
    #[error("Encountered an I/O error: {0}")]
    Io(#[from] io::Error),
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

#[derive(Debug)]
pub struct Window {
    /// This fields contains the rect of the window that also encompasses window decorations
    pub outer_rect: Rectangle,
    /// This fields contains the rect of the window that **only** encompasses the content
    pub content_rect: Rectangle,
}

#[derive(Default, Clone, Copy, Debug)]
struct WmFeatures {
    /// Whether the WM supports retrieving the window list
    supports_retrieving_windows: bool,
    // Whether we're running under Wayland as a wayland client or not
    is_wayland: bool,
}

impl WmFeatures {
    /// Talks with the WM to get the features we're interested in
    /// These get cached and as such calling it multiple times in succession should be cheap
    fn get() -> Result<&'static Self> {
        static FEATURES: OnceCell<WmFeatures> = OnceCell::new();

        FEATURES.get_or_try_init(Self::get_impl)
    }

    fn get_impl() -> Result<WmFeatures> {
        let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let xdg_session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();
        // For checking that Wayland features work even when kcshot is used under X (but on desktops providing
        // the necessary portals)
        let force_wayland_emulation = env::var("KCSHOT_FORCE_USE_PORTALS").unwrap_or_default();

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

pub fn take_screenshot(
    window_identifier: WindowIdentifier,
    tokio: Option<&tokio::runtime::Handle>,
) -> Result<ImageSurface> {
    if WmFeatures::get()?.is_wayland {
        wayland::take_screenshot(window_identifier, tokio)
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

pub fn will_make_use_of_desktop_portals() -> bool {
    WmFeatures::get().map(|wm| wm.is_wayland).unwrap_or(false)
}
