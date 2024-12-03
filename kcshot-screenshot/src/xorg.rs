use std::sync::OnceLock;

use cairo::{self, Format as CairoImageFormat, ImageSurface};
use kcshot_data::{
    geometry::{Point, Rectangle},
    settings::Settings,
};
use xcb::{
    shape,
    x::{
        self, ImageFormat as XImageFormat, MapState, Window as XWindow, ATOM_ATOM, ATOM_CARDINAL,
        ATOM_NONE, ATOM_WINDOW,
    },
    xfixes,
};

use super::{Result, Window, WmFeatures};
use crate::DisplayServerKind;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("WM does not support _NET_CLIENT_LIST_STACKING")]
    WmDoesNotSupportWindowList,
    #[error("WM does not support _NET_FRAME_EXTENTS")]
    WmDoesNotSupportFrameExtents,
    #[error("Failed to get root window")]
    FailedToGetRootWindow,
    #[error("Failed to establish a connection to the X server: {0:?}")]
    XcbConnection(#[from] xcb::ConnError),
    #[error("Encountered an X protocol error: {0:?}")]
    XcbProtocol(xcb::ProtocolError),
}

impl From<xcb::Error> for Error {
    fn from(xerror: xcb::Error) -> Self {
        match xerror {
            xcb::Error::Connection(err) => Self::XcbConnection(err),
            xcb::Error::Protocol(err) => Self::XcbProtocol(err),
        }
    }
}

pub(super) fn take_screenshot() -> Result<ImageSurface> {
    let (connection, _) = xcb::Connection::connect_with_extensions(
        None,
        &[],
        &[xcb::Extension::Shape, xcb::Extension::XFixes],
    )
    .map_err(Error::from)?;
    let setup = connection.get_setup();

    // We need to make the X server aware that we wish to use the XFIXES extension
    let _query_version = connection.send_request(&xfixes::QueryVersion {
        client_major_version: xfixes::MAJOR_VERSION,
        client_minor_version: xfixes::MINOR_VERSION,
    });

    for root_screen in setup.roots() {
        let window = root_screen.root();
        let pointer_cookie = connection.send_request(&x::QueryPointer { window });

        let pointer_reply = connection
            .wait_for_reply(pointer_cookie)
            .map_err(Error::from)?;
        if pointer_reply.same_screen() {
            // Just because the cursor is on the same screen(monitor) as the root window, it doesn't
            // mean the root window spans a single monitor, in fact it can span multiple.
            // So we get the monitor that's under the cursor here.
            let screenshot_bounds = retrieve_bounds_of_monitor_under_cursor(
                &connection,
                Point {
                    x: pointer_reply.root_x() as _,
                    y: pointer_reply.root_y() as _,
                },
                window,
            )?;

            let screenshot_cookie = connection.send_request(&x::GetImage {
                format: XImageFormat::ZPixmap,
                drawable: x::Drawable::Window(window),
                x: screenshot_bounds.x as _,
                y: screenshot_bounds.y as _,
                width: screenshot_bounds.w as _,
                height: screenshot_bounds.h as _,
                plane_mask: u32::MAX,
            });

            let mut screenshot = connection
                .wait_for_reply(screenshot_cookie)
                .map_err(Error::from)?
                .data()
                .to_vec();

            let capture_mouse_cursor = Settings::open().capture_mouse_cursor();

            if capture_mouse_cursor {
                let cursor_cookie = connection.send_request(&xfixes::GetCursorImage {});
                let cursor = connection.wait_for_reply(cursor_cookie);
                match cursor {
                    Ok(cursor) => {
                        overlay_cursor(cursor, &mut screenshot, screenshot_bounds);
                    }
                    Err(why) => tracing::info!("Unable to fetch cursor data: {why:?}"),
                }
            }

            let stride = CairoImageFormat::Rgb24.stride_for_width(screenshot_bounds.w as u32)?;
            let screenshot = ImageSurface::create_for_data(
                screenshot,
                CairoImageFormat::Rgb24,
                screenshot_bounds.w as i32,
                screenshot_bounds.h as i32,
                stride,
            )?;

            return Ok(screenshot);
        }
    }

    Err(super::Error::FailedToTakeScreenshot)
}

fn retrieve_bounds_of_monitor_under_cursor(
    connection: &xcb::Connection,
    cursor_position: Point,
    window: XWindow,
) -> Result<Rectangle> {
    let get_monitors = connection.send_request(&xcb::randr::GetMonitors {
        window,
        get_active: true,
    });
    let monitors = connection
        .wait_for_reply(get_monitors)
        .map_err(Error::from)?;

    for monitor in monitors.monitors() {
        let monitor_rect = Rectangle {
            x: monitor.x() as _,
            y: monitor.y() as _,
            w: monitor.width() as _,
            h: monitor.height() as _,
        };

        if monitor_rect.contains(cursor_position) {
            return Ok(monitor_rect);
        }
    }

    tracing::error!("Failed to find any monitors. (How?!)");
    unreachable!()
}

fn overlay_cursor(cursor: xfixes::GetCursorImageReply, screenshot: &mut [u8], bounds: Rectangle) {
    // These computations give us the coords of the top left corner of the mouse cursor
    // We use saturating arithmetic because cursor.{x,y}() may be smaller than cursor.{x,y}hot() when
    // the cursor is close to the left and top edges of the screen
    let cx = (cursor.x() as usize).saturating_sub(cursor.xhot() as usize) - bounds.x as usize;
    let cy = (cursor.y() as usize).saturating_sub(cursor.yhot() as usize) - bounds.y as usize;

    let w = cursor.width() as usize;
    let h = cursor.height() as usize;

    let cursor = cursor.cursor_image();

    // We use these variables to ensure that we don't attempt do out of bounds or wrapping
    // writes, which would either crash the application or draw the cursor on the other side
    // of the screen
    let w_draw = usize::min(w, bounds.w as usize - cx);
    let h_draw = usize::min(h, bounds.h as usize - cy);

    for x in 0..w_draw {
        #[expect(
            clippy::identity_op,
            reason = "Identity ops add a symmetry that makes the code nicer and easier to read."
        )]
        for y in 0..h_draw {
            let r = cursor[y * w + x] >> 0 & 0xff;
            let g = cursor[y * w + x] >> 8 & 0xff;
            let b = cursor[y * w + x] >> 16 & 0xff;
            let a = cursor[y * w + x] >> 24 & 0xff;

            // We multiply by 4 because the screenshot is stored in RGB-Unused byte format
            let pixel_idx = 4 * bounds.w as usize * (cy + y) + 4 * (cx + x);

            // Cursor data is RGBA, but screenshot data is RGB-Unused byte, so we do manual
            // blending to paste the cursor _over_ the image
            if a == 255 {
                screenshot[pixel_idx + 0] = r as u8;
                screenshot[pixel_idx + 1] = g as u8;
                screenshot[pixel_idx + 2] = b as u8;
            } else if a == 0 {
                // Ignore transparent pixels, as we don't need to do any blending to them
                continue;
            } else {
                let blend =
                    |target, source, alpha| target + (source * (255 - alpha) + 255 / 2) / 255;
                screenshot[pixel_idx + 0] = blend(r, screenshot[pixel_idx + 0] as u32, a) as u8;
                screenshot[pixel_idx + 1] = blend(g, screenshot[pixel_idx + 1] as u32, a) as u8;
                screenshot[pixel_idx + 2] = blend(b, screenshot[pixel_idx + 1] as u32, a) as u8;
            };
        }
    }
}

xcb::atoms_struct! {
    /// This structs contains the atoms we'll use multiple times over the course of the program and as
    /// such are cached. None of the atoms here will ever be [`xcb::x::ATOM_NONE`]
    #[derive(Debug)]
    struct AtomsOfInterest {
        /// This corresponds to _NET_CLIENT_LIST_STACKING, querying this property on the root window
        /// gives us the list of windows in stacking order.
        ///
        /// https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381391305328
        wm_client_list => b"_NET_CLIENT_LIST_STACKING",
        /// This corresponds to _NET_FRAME_EXTENTS, quarrying this property on a window gives us the
        /// widths of the left, right, top and bottom borders added by a window manager,
        ///
        /// Some window managers have this atom despite not actually supporting it.
        ///
        /// https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381391244864
        frame_extents => b"_NET_FRAME_EXTENTS",
        /// This corresponds to _NET_WM_STATE, querying this property on a window returns the window
        /// state, i.e. whether the window is fullscreen or not.
        ///
        /// https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html#idm46476783496896
        window_state => b"_NET_WM_STATE",
        /// This corresponds to _NET_WM_STATE_FULLSCREEN, it indicates that the window is fullscreen.
        ///
        /// https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html#idm46476783496896
        /// (Same as above spec link)
        window_is_fullscreen => b"_NET_WM_STATE_FULLSCREEN",
    }
}

impl AtomsOfInterest {
    fn get(connection: &xcb::Connection) -> Result<&Self> {
        static ATOMS_OF_INTEREST: OnceLock<AtomsOfInterest> = OnceLock::new();

        let init = || {
            let Self {
                wm_client_list,
                frame_extents,
                window_state,
                window_is_fullscreen,
            } = Self::intern_all(connection).map_err(Error::from)?;

            if wm_client_list == ATOM_NONE {
                return Err(Error::WmDoesNotSupportWindowList);
            }

            if [frame_extents, window_state, window_is_fullscreen].contains(&ATOM_NONE) {
                return Err(Error::WmDoesNotSupportFrameExtents);
            }

            Ok(Self {
                wm_client_list,
                frame_extents,
                window_state,
                window_is_fullscreen,
            })
        };

        Ok(if let Some(val) = ATOMS_OF_INTEREST.get() {
            val
        } else {
            ATOMS_OF_INTEREST
                .set(init()?)
                .expect("ATOMS_OF_INTEREST cannot be initialised at this point");
            ATOMS_OF_INTEREST.get().unwrap()
        })
    }
}

/// Obtains a list of all windows from the display server, the list is in stacking order.
pub(super) fn get_windows() -> Result<Vec<Window>> {
    // Requires an WM that supports EWMH. Will gracefully fallback if not available

    let wm_features = WmFeatures::get()?;

    if !wm_features.can_retrieve_windows() {
        return Err(Error::WmDoesNotSupportWindowList.into());
    }

    let (connection, _) = xcb::Connection::connect(None).map_err(Error::from)?;
    let setup = connection.get_setup();

    let &AtomsOfInterest { wm_client_list, .. } = AtomsOfInterest::get(&connection)?;

    for root_screen in setup.roots() {
        let root_window = root_screen.root();
        let pointer_cookie = connection.send_request(&x::QueryPointer {
            window: root_window,
        });

        let pointer_reply = connection
            .wait_for_reply(pointer_cookie)
            .map_err(Error::from)?;
        if pointer_reply.same_screen() {
            let list = connection.send_request(&x::GetProperty {
                delete: false,
                window: root_window,
                property: wm_client_list,
                r#type: ATOM_WINDOW,
                long_offset: 0,
                long_length: 128, // If the user has more than 128 windows on their desktop, that's their problem really
            });
            let list = connection.wait_for_reply(list).map_err(Error::from)?;

            let mut windows = Vec::with_capacity(128);

            for window in list.value::<XWindow>().iter().copied() {
                let attributes = connection.send_request(&x::GetWindowAttributes { window });
                let attributes = connection.wait_for_reply(attributes).map_err(Error::from)?;
                if attributes.map_state() != MapState::Viewable {
                    continue;
                }

                let window_extents = connection.send_request(&shape::QueryExtents {
                    destination_window: window,
                });
                let window_extents = connection
                    .wait_for_reply(window_extents)
                    .map_err(Error::from)?;

                let translated_window_coords = connection.send_request(&x::TranslateCoordinates {
                    src_window: window,
                    dst_window: root_window,
                    src_x: window_extents.bounding_shape_extents_x(),
                    src_y: window_extents.bounding_shape_extents_y(),
                });

                let translated_window_coords = connection
                    .wait_for_reply(translated_window_coords)
                    .map_err(Error::from)?;

                let content_rect = Rectangle {
                    x: translated_window_coords.dst_x() as f64,
                    y: translated_window_coords.dst_y() as f64,
                    w: window_extents.bounding_shape_extents_width() as f64,
                    h: window_extents.bounding_shape_extents_height() as f64,
                };

                windows.push(Window {
                    outer_rect: get_window_outer_rect(&connection, content_rect, window)?,
                    content_rect,
                });
            }

            return Ok(windows);
        }
    }

    Err(super::Error::FailedToGetWindows)
}

/// Returns the outer rect of a window
///
/// The outer rect is the content rect expanded to include window borders (usually decorations)
/// added by the window manager, however it will be same as the content rect in the following cases:
/// * the window manager doesn't support retrieving frame_extents
/// * the window is fullscreen
fn get_window_outer_rect(
    connection: &xcb::Connection,
    content_rect: Rectangle,
    window: XWindow,
) -> Result<Rectangle> {
    let &AtomsOfInterest {
        frame_extents,
        window_state,
        window_is_fullscreen,
        ..
    } = AtomsOfInterest::get(connection)?;

    let frame_extents = connection.send_request(&x::GetProperty {
        delete: false,
        window,
        property: frame_extents,
        r#type: ATOM_CARDINAL,
        long_offset: 0,
        long_length: 4,
    });
    let window_state = connection.send_request(&x::GetProperty {
        delete: false,
        window,
        property: window_state,
        r#type: ATOM_ATOM,
        long_offset: 0,
        // This is how many states I counted, hopefully this is enough.
        long_length: 1024,
    });

    let frame_extents = connection
        .wait_for_reply(frame_extents)
        .map_err(Error::from)?;
    let window_states = connection
        .wait_for_reply(window_state)
        .map_err(Error::from)?;

    let mut fullscreen = false;
    if window_states.length() != 0 {
        for state in window_states.value::<x::Atom>() {
            if state == &window_is_fullscreen {
                fullscreen = true;
            }
        }
    }
    // If the window is fullscreen, ignore the frame extents and just return the content_rect
    if fullscreen {
        return Ok(content_rect);
    }

    let Rectangle { x, y, w, h } = content_rect;

    let left = frame_extents.value::<u32>()[0] as f64;
    let right = frame_extents.value::<u32>()[1] as f64;
    let top = frame_extents.value::<u32>()[2] as f64;
    let bottom = frame_extents.value::<u32>()[3] as f64;

    Ok(Rectangle {
        x: x - left,
        y: y - top,
        w: w + (right + left),
        h: h + (top + bottom),
    })
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let (connection, _) = xcb::Connection::connect(None).map_err(Error::from)?;

    //https://specifications.freedesktop.org/wm-spec/latest/ar01s03.html#idm46476783603760
    let supported_ewmh_atoms = connection.send_request(&x::InternAtom {
        only_if_exists: true,
        name: b"_NET_SUPPORTED",
    });

    let supported_ewmh_atoms = connection
        .wait_for_reply(supported_ewmh_atoms)
        .map_err(Error::from)?;
    let AtomsOfInterest {
        wm_client_list,
        frame_extents,
        ..
    } = AtomsOfInterest::get(&connection)?;

    // NOTE: This sets WmFeatures::is_wayland to false
    let mut wm_features = WmFeatures {
        display_server_kind: DisplayServerKind::X11 {
            can_retrieve_windows: false,
        },
        should_use_portals: false,
    };

    if supported_ewmh_atoms.atom() == ATOM_NONE {
        tracing::info!(
            "Your WM does not support EWMH, so kcshot won't be able to retrieve window rects"
        );
        return Ok(wm_features);
    }

    let root = connection
        .get_setup()
        .roots()
        .next()
        .ok_or(Error::FailedToGetRootWindow)?;

    let supported_ewmh_atoms = connection.send_request(&x::GetProperty {
        delete: false,
        window: root.root(),
        property: supported_ewmh_atoms.atom(),
        r#type: ATOM_ATOM,
        long_offset: 0,
        long_length: 50, // I think the spec defines less than this, but it's (hopefully) fine
    });
    let supported_ewmh_atoms = connection
        .wait_for_reply(supported_ewmh_atoms)
        .map_err(Error::from)?;

    let mut wm_supports_retrieving_client_list = false;
    let mut wm_supports_retrieving_window_rects = false;
    for atom in supported_ewmh_atoms.value::<x::Atom>() {
        if atom == wm_client_list {
            wm_supports_retrieving_client_list = true;
        } else if atom == frame_extents {
            wm_supports_retrieving_window_rects = true;
        }
    }

    wm_features.display_server_kind = DisplayServerKind::X11 {
        can_retrieve_windows: wm_supports_retrieving_client_list
            && wm_supports_retrieving_window_rects,
    };

    Ok(wm_features)
}
