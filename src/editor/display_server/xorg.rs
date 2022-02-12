use std::{
    ffi::CString,
    os::{raw::c_char, unix::prelude::OsStrExt},
};

use cairo::{
    self,
    ffi::{
        cairo_status_t, cairo_surface_status, cairo_surface_t, cairo_xcb_surface_create,
        STATUS_SUCCESS as CAIRO_STATUS_SUCCESS,
    },
    Error as CairoError, ImageSurface,
};
use gtk4::prelude::{FileExt, IOStreamExt, InputStreamExtManual};
use once_cell::sync::OnceCell;
use xcb::{
    shape,
    x::{
        self, Atom, MapState, Window as XWindow, ATOM_ATOM, ATOM_CARDINAL, ATOM_NONE, ATOM_WINDOW,
    },
    Xid,
};

use super::{Result, Window, WmFeatures};
use crate::editor::data::Rectangle;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Got an error trying to make a temporary file: {0}")]
    TempFile(#[from] gtk4::glib::Error),
    #[error("WM does not support EWMH")]
    WmDoesNotSupportEwmh,
    #[error("WM does not support _NET_CLIENT_LIST_STACKING")]
    WmDoesNotSupportWindowList,
    #[error("WM does not support _NET_FRAME_EXTENTS")]
    WmDoesNotSupportFrameExtents,
    #[error("Failed to get root window")]
    FailedToGetRootWindow,
}

// Some things in this file are inspired by the code written here https://giters.com/psychon/x11rb/issues/328
// archive.org link: https://web.archive.org/web/20220109220701/https://giters.com/psychon/x11rb/issues/328 [1]

pub(super) fn take_screenshot() -> Result<ImageSurface> {
    extern "C" {
        /// cairo-rs doesn't expose it in its ffi module, so I have to write its declaration myself
        /// Here's the cairo docs for it: https://cairographics.org/manual/cairo-PNG-Support.html#cairo-surface-write-to-png
        fn cairo_surface_write_to_png(
            surface: *mut cairo_surface_t,
            filename: *const c_char,
        ) -> cairo_status_t;
    }

    fn find_xcb_visualtype(conn: &xcb::Connection, visual_id: u32) -> Option<x::Visualtype> {
        for root in conn.get_setup().roots() {
            for depth in root.allowed_depths() {
                for visual in depth.visuals() {
                    if visual.visual_id() == visual_id {
                        return Some(*visual);
                    }
                }
            }
        }

        None
    }

    let (connection, _) = xcb::Connection::connect(None)?;
    let setup = connection.get_setup();

    for root_screen in setup.roots() {
        let window = root_screen.root();
        let pointer_cookie = connection.send_request(&x::QueryPointer { window });
        let geometry_cookie = connection.send_request(&x::GetGeometry {
            drawable: x::Drawable::Window(window),
        });

        let pointer_reply = connection.wait_for_reply(pointer_cookie)?;
        if pointer_reply.same_screen() {
            let geometry_reply = connection.wait_for_reply(geometry_cookie)?;
            let mut visualtype = match find_xcb_visualtype(&connection, root_screen.root_visual()) {
                Some(visualtype) => visualtype,
                None => continue,
            };
            let raw_connection = connection.get_raw_conn();
            let width = geometry_reply.width() as i32;
            let height = geometry_reply.height() as i32;

            // SAFETY: cairo doesn't touch the pointers we give it, so
            //      * the connection should be fine to pass through,
            //      * the visualtype is (hopefully) ABI compatible with C
            // Also see [1]
            let screenshot = unsafe {
                cairo_xcb_surface_create(
                    raw_connection as _,
                    window.resource_id(),
                    &mut visualtype as *mut _ as _,
                    width,
                    height,
                )
            };

            let surface_status = unsafe { cairo_surface_status(screenshot) };
            if surface_status != CAIRO_STATUS_SUCCESS {
                return Err(CairoError::from(surface_status).into());
            }

            let (file, stream) =
                gtk4::gio::File::new_tmp(Some("screenshot.XXXXXX.png")).map_err(Error::TempFile)?;
            let path = file.path().unwrap();
            let path = CString::new(path.as_os_str().as_bytes()).unwrap();

            // SAFETY: * screenshot is a valid surface (see above the `cairo_surface_status` call)
            //         *  path is a valid nul terminated c-string (we should've bailed out above otherwise)
            match unsafe { cairo_surface_write_to_png(screenshot, path.as_ptr()) } {
                0 => {}
                err => return Err(CairoError::from(err).into()),
            }

            // Why do we this instead of just returning an XcbSurface?
            // When I first started experimenting with writing a screenshot-utility, I did it in C++
            // using xlib and Cairo::XlibSurface. That had some behaviour I disliked: when switching
            // tags, the surface displayed the contents of the new tag, instead of the old one. This
            // happened when I tested cairo-rs's XcbSurface, I assume it'll be the same, so we end
            // up writing the screenshot to a .png and then reading it again

            return Ok(ImageSurface::create_from_png(
                &mut stream.input_stream().into_read(),
            )?);
        }
    }

    Err(super::Error::FailedToTakeScreenshot)
}

/// This structs contains the atoms we'll use multiple times over the course of the program and as
/// such are cached. None of the atoms here will ever be [`xcb::x::ATOM_NONE`]
struct AtomsOfInterest {
    /// This corresponds to _NET_CLIENT_LIST_STACKING, querrying this property on the root window
    /// gives us the list of windows in stacking order.
    ///
    /// https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381391305328
    wm_client_list: Atom,
    /// This corresponds to _NET_FRAME_EXTENTS, querrrying this property on a window gives us the
    /// widths of the left, right, top and bottom borders added by a window manager,
    ///
    /// Some window managers have this attom despite not actually supporting it.
    ///
    /// https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381391244864
    frame_extents: Atom,
    /// This corresponds to _NET_WM_STATE, querrying this property on a window returns the window
    /// state, i.e. whether the window is fullscreen or not.
    ///
    /// https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html#idm46476783496896
    window_state: Atom,
    /// This corresponds to _NET_WM_STATE_FULLSCREEN, it indicates that the window is fullscreen.
    ///
    /// https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html#idm46476783496896
    /// (Same as above spec link)
    window_is_fullscreen: Atom,
}

impl AtomsOfInterest {
    fn get(connection: &xcb::Connection) -> Result<&Self> {
        static ATOMS_OF_INTEREST: OnceCell<AtomsOfInterest> = OnceCell::new();

        ATOMS_OF_INTEREST.get_or_try_init(|| {
            let wm_client_list = connection.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_CLIENT_LIST_STACKING",
            });
            let frame_extents = connection.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_FRAME_EXTENTS",
            });
            let window_state = connection.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_WM_STATE",
            });
            let window_is_fullscreen = connection.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_WM_STATE_FULLSCREEN",
            });

            let wm_client_list = connection.wait_for_reply(wm_client_list)?.atom();
            let frame_extents = connection.wait_for_reply(frame_extents)?.atom();
            let window_state = connection.wait_for_reply(window_state)?.atom();
            let window_is_fullscreen = connection.wait_for_reply(window_is_fullscreen)?.atom();

            if wm_client_list == ATOM_NONE {
                return Err(Error::WmDoesNotSupportWindowList.into());
            }
            if frame_extents == ATOM_NONE {
                return Err(Error::WmDoesNotSupportFrameExtents.into());
            }
            if window_state == ATOM_NONE {
                return Err(Error::WmDoesNotSupportFrameExtents.into());
            }
            if window_is_fullscreen == ATOM_NONE {
                return Err(Error::WmDoesNotSupportFrameExtents.into());
            }

            Ok(Self {
                wm_client_list,
                frame_extents,
                window_state,
                window_is_fullscreen,
            })
        })
    }
}

/// Obtains a list of all windows from the display server, the list is in stacking order.
pub(super) fn get_windows() -> Result<Vec<Window>> {
    let (connection, _) = xcb::Connection::connect(None)?;
    let setup = connection.get_setup();

    // Requires an WM that supports EWMH. Will gracefully fallback if not available

    let wm_features = WmFeatures::get()?;

    if !wm_features.supports_client_list {
        return Err(Error::WmDoesNotSupportWindowList.into());
    }

    let &AtomsOfInterest { wm_client_list, .. } = AtomsOfInterest::get(&connection)?;

    for root_screen in setup.roots() {
        let root_window = root_screen.root();
        let pointer_cookie = connection.send_request(&x::QueryPointer {
            window: root_window,
        });

        let pointer_reply = connection.wait_for_reply(pointer_cookie)?;
        if pointer_reply.same_screen() {
            let list = connection.send_request(&x::GetProperty {
                delete: false,
                window: root_window,
                property: wm_client_list,
                r#type: ATOM_WINDOW,
                long_offset: 0,
                long_length: 128, // If the user has more than 128 windows on their desktop, that's their problem really
            });
            let list = connection.wait_for_reply(list)?;

            let mut windows = Vec::with_capacity(128);

            for window in list.value::<XWindow>().iter().copied() {
                let attributes = connection.send_request(&x::GetWindowAttributes { window });
                let attributes = connection.wait_for_reply(attributes)?;
                if attributes.map_state() != MapState::Viewable {
                    continue;
                }

                let window_extents = connection.send_request(&shape::QueryExtents {
                    destination_window: window,
                });
                let window_extents = connection.wait_for_reply(window_extents)?;

                let translated_window_coords = connection.send_request(&x::TranslateCoordinates {
                    src_window: window,
                    dst_window: root_window,
                    src_x: window_extents.bounding_shape_extents_x(),
                    src_y: window_extents.bounding_shape_extents_y(),
                });

                let translated_window_coords =
                    connection.wait_for_reply(translated_window_coords)?;

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
    // If the WM doesn't support getting frame extents, don't bother doing any work
    if !WmFeatures::get()?.supports_frame_extents {
        return Ok(content_rect);
    }

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

    let frame_extents = connection.wait_for_reply(frame_extents)?;
    let window_states = connection.wait_for_reply(window_state)?;

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
        w: w + (right + top),
        h: h + (top + bottom),
    })
}

pub(super) fn get_wm_features() -> Result<WmFeatures> {
    let (connection, _) = xcb::Connection::connect(None)?;

    //https://specifications.freedesktop.org/wm-spec/latest/ar01s03.html#idm46476783603760
    let supported_ewmh_atoms = connection.send_request(&x::InternAtom {
        only_if_exists: true,
        name: b"_NET_SUPPORTED",
    });

    let supported_ewmh_atoms = connection.wait_for_reply(supported_ewmh_atoms)?;
    let AtomsOfInterest {
        wm_client_list,
        frame_extents,
        ..
    } = AtomsOfInterest::get(&connection)?;

    if supported_ewmh_atoms.atom() == ATOM_NONE {
        return Err(Error::WmDoesNotSupportEwmh.into());
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
    let supported_ewmh_atoms = connection.wait_for_reply(supported_ewmh_atoms)?;

    // NOTE: This sets WmFeatures::is_wayland to false
    let mut wm_features = WmFeatures::default();

    for atom in supported_ewmh_atoms.value::<x::Atom>() {
        if atom == wm_client_list {
            wm_features.supports_client_list = true;
        } else if atom == frame_extents {
            wm_features.supports_frame_extents = true;
        }
    }

    Ok(wm_features)
}
