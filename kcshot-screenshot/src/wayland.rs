#![expect(
    clippy::unnecessary_wraps,
    reason = "The point of the wraps is to keep a consistent interface between xorg and wayland implementations"
)]

use cairo::ImageSurface;
use gtk4::{
    gio, glib,
    prelude::{FileExt, InputStreamExtManual},
};
use kcshot_data::geometry::Rectangle;

use super::{Result, Window, WmFeatures};
use crate::DisplayServerKind;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a desktop portal error: {0}")]
    Ashpd(#[from] ashpd::Error),
    #[error("Failed opening file(uri={uri}) for reading: {error}")]
    GioFile { error: glib::Error, uri: String },
    #[error("Failed to deserialize output of '{command}': {error}")]
    Deserialize {
        error: serde_json::Error,
        command: String,
    },
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP");

    let display_server_kind = match xdg_current_desktop {
        Ok(xdg_current_desktop) => {
            if xdg_current_desktop.eq_ignore_ascii_case("hyprland") {
                DisplayServerKind::Hyprland
            } else {
                tracing::warn!("Unknown Wayland compositor ('{xdg_current_desktop}'), assuming a generic Wayland setup.");
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
        should_use_portals: false,
    };

    Ok(wm_features)
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

pub(super) fn get_windows() -> Result<Vec<Window>> {
    let wm_features = WmFeatures::get()?;

    if wm_features.display_server_kind == DisplayServerKind::Hyprland {
        get_windows_hyprland()
    } else {
        Ok(vec![])
    }
}

fn get_windows_hyprland() -> Result<Vec<Window>> {
    use serde::{de::DeserializeOwned, Deserialize};

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
