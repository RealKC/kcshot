#![expect(
    clippy::unnecessary_wraps,
    reason = "The point of the wraps is to keep a consistent interface between xorg and wayland implementations"
)]

use cairo::{Format as CairoImageFormat, ImageSurface};
use kcshot_data::{geometry::Rectangle, settings::Settings};
use libwayshot::WayshotConnection;

use super::{Result, Window, WmFeatures};
use crate::DisplayServerKind;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to deserialize output of '{command}': {error}")]
    Deserialize {
        error: serde_json::Error,
        command: String,
    },
    #[error("Failed to take screenshot: {0}")]
    Wayshot(#[from] libwayshot::Error),
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP");

    let display_server_kind = match xdg_current_desktop {
        Ok(xdg_current_desktop) => {
            if xdg_current_desktop.eq_ignore_ascii_case("hyprland") {
                DisplayServerKind::Hyprland
            } else if xdg_current_desktop.eq_ignore_ascii_case("niri") {
                DisplayServerKind::Niri
            } else {
                tracing::warn!(
                    "Unknown Wayland compositor ('{xdg_current_desktop}'), assuming a generic Wayland setup."
                );
                DisplayServerKind::GenericWayland
            }
        }
        Err(why) => {
            tracing::warn!(
                "Failed to retrieve $XDG_CURRENT_DESKTOP, assuming a generic Wayland setup: {why}"
            );
            DisplayServerKind::GenericWayland
        }
    };

    let wm_features = WmFeatures {
        display_server_kind,
        screenshot_method: match display_server_kind {
            DisplayServerKind::Niri | DisplayServerKind::Hyprland => {
                crate::ScreenshotMethod::WlrScreencopy
            }
            _ => crate::ScreenshotMethod::Portals,
        },
    };

    Ok(wm_features)
}

pub(super) fn take_screenshot() -> Result<ImageSurface> {
    let wayshot_connection = WayshotConnection::new().map_err(Error::Wayshot)?;
    let image_buffer = wayshot_connection
        .screenshot_all(Settings::open().capture_mouse_cursor())
        .map_err(Error::Wayshot)?;

    let width = image_buffer.width();
    let height = image_buffer.height();
    let stride = CairoImageFormat::Rgb24.stride_for_width(width)?;

    let screenshot = ImageSurface::create_for_data(
        image_buffer
            .into_vec()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| [c[2], c[1], c[0], c[3]])
            .collect::<Vec<_>>(),
        CairoImageFormat::Rgb24,
        width as i32,
        height as i32,
        stride,
    )?;

    Ok(screenshot)
}

pub(super) fn get_windows() -> Result<Vec<Window>> {
    let wm_features = WmFeatures::get()?;

    if wm_features.display_server_kind == DisplayServerKind::Hyprland {
        get_windows_hyprland()
    } else {
        Ok(vec![])
    }
}

fn get_windows_hyprland() -> Result<Vec<Window>> {
    use serde::{Deserialize, de::DeserializeOwned};

    #[derive(Deserialize)]
    struct HyprWindow {
        at: [f64; 2],
        size: [f64; 2],
        workspace: HyprWorkspace,
        monitor: i32,
    }

    #[derive(Deserialize)]
    struct HyprWorkspace {
        id: i32,
    }

    #[derive(Deserialize)]
    struct HyprBorderSize {
        int: i32,
    }

    fn spawn_and_parse_output<O: DeserializeOwned>(command_str: &str) -> Result<O> {
        let mut argv = command_str.split_ascii_whitespace();
        let mut command = std::process::Command::new(argv.next().unwrap());

        for arg in argv {
            command.arg(arg);
        }

        let output = command.output()?;

        Ok(
            serde_json::from_slice(&output.stdout).map_err(|error| Error::Deserialize {
                error,
                command: command_str.into(),
            })?,
        )
    }

    let border_size =
        spawn_and_parse_output::<HyprBorderSize>("hyprctl -j getoption general:border_size")?.int
            as f64;
    let active_window = spawn_and_parse_output::<HyprWindow>("hyprctl -j activewindow")?;
    let hypr_windows: Vec<_> = spawn_and_parse_output::<Vec<HyprWindow>>("hyprctl -j clients")?
        .into_iter()
        .filter(|win| {
            win.workspace.id == active_window.workspace.id && win.monitor == active_window.monitor
        })
        .collect();

    let mut windows = Vec::with_capacity(hypr_windows.len());

    for window in hypr_windows {
        let outer_rect = Rectangle {
            x: window.at[0] - border_size,
            y: window.at[1] - border_size,
            w: window.size[0] + 2.0 * border_size,
            h: window.size[1] + 2.0 * border_size,
        };

        let content_rect = Rectangle {
            x: window.at[0],
            y: window.at[1],
            w: window.size[0],
            h: window.size[1],
        };

        windows.push(Window {
            outer_rect,
            content_rect,
        });
    }

    Ok(windows)
}
