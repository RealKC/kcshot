use std::{env, io, sync::OnceLock};

use cairo::{self, Error as CairoError, ImageSurface};
use kcshot_data::geometry::Rectangle;

mod portals;
mod wayland;
mod xorg;

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
    #[error("Encountered an error interacting with the FDo portals stack: {0}")]
    Portals(#[from] portals::Error),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisplayServerKind {
    X11 { can_retrieve_windows: bool },
    GenericWayland,
    Hyprland,
    Niri,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenshotMethod {
    Xcb,
    Portals,
    WlrScreencopy,
}

#[derive(Clone, Copy, Debug)]
struct WmFeatures {
    display_server_kind: DisplayServerKind,
    screenshot_method: ScreenshotMethod,
}

impl WmFeatures {
    /// Talks with the WM to get the features we're interested in
    /// These get cached and as such calling it multiple times in succession should be cheap
    fn get() -> Result<&'static Self> {
        static FEATURES: OnceLock<WmFeatures> = OnceLock::new();

        Ok(if let Some(val) = FEATURES.get() {
            val
        } else {
            FEATURES
                .set(Self::get_impl()?)
                .expect("FEATURES cannot be initialised at this point");
            FEATURES.get().unwrap()
        })
    }

    fn get_impl() -> Result<WmFeatures> {
        let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let xdg_session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let force_use_portals = env::var("KCSHOT_FORCE_USE_PORTALS").unwrap_or_default();

        let is_wayland = wayland_display.to_lowercase().contains("wayland")
            || xdg_session_type.eq_ignore_ascii_case("wayland");

        let mut wm_features = if is_wayland {
            wayland::get_wm_features()
        } else {
            xorg::get_wm_features()
        }?;

        if force_use_portals == "1" || ashpd::is_sandboxed() {
            wm_features.screenshot_method = ScreenshotMethod::Portals;
        }

        Ok(wm_features)
    }

    fn can_retrieve_windows(self) -> bool {
        use DisplayServerKind::*;
        matches!(
            self.display_server_kind,
            X11 {
                can_retrieve_windows: true,
            } | Hyprland
        )
    }
}

pub fn take_screenshot(tokio: Option<&tokio::runtime::Handle>) -> Result<ImageSurface> {
    match WmFeatures::get()?.screenshot_method {
        ScreenshotMethod::Xcb => xorg::take_screenshot(),
        ScreenshotMethod::Portals => portals::take_screenshot(tokio),
        ScreenshotMethod::WlrScreencopy => wayland::take_screenshot(),
    }
}

/// Obtains a list of all windows from the display server, the list is in stacking order.
pub fn get_windows() -> Result<Vec<Window>> {
    match WmFeatures::get()?.screenshot_method {
        ScreenshotMethod::Xcb => xorg::get_windows(),
        ScreenshotMethod::Portals => Ok(vec![]),
        ScreenshotMethod::WlrScreencopy => wayland::get_windows(),
    }
}

pub fn is_wayland() -> bool {
    WmFeatures::get()
        .map(|f| !matches!(f.display_server_kind, DisplayServerKind::X11 { .. }))
        .unwrap_or(false)
}

pub fn will_make_use_of_desktop_portals() -> bool {
    let Ok(wm_features) = WmFeatures::get() else {
        return false;
    };

    if wm_features.screenshot_method == ScreenshotMethod::Portals {
        return true;
    }

    false
}
