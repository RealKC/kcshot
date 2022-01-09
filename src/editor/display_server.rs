use std::{
    ffi::CString,
    io,
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
use tracing::error;
use xcb::{x, Xid};

extern "C" {
    /// cairo-rs doesn't expose it in its ffi module, so I have to write its declaration myself
    /// Here's the cairo docs for it: https://cairographics.org/manual/cairo-PNG-Support.html#cairo-surface-write-to-png
    fn cairo_surface_write_to_png(
        surface: *mut cairo_surface_t,
        filename: *const c_char,
    ) -> cairo_status_t;
}

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

// Some things in this file are inspired by the code written here https://giters.com/psychon/x11rb/issues/328
// archive.org link: https://web.archive.org/web/20220109220701/https://giters.com/psychon/x11rb/issues/328 [1]

pub fn take_screenshot() -> Result<ImageSurface, Error> {
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

            let (file, stream) = gtk4::gio::File::new_tmp("screenshot.XXXXXX.png")?;
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

    Err(Error::FailedToTakeScreenshot)
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

/// Gets the screen resolution
///
/// # Returns
/// The first item of the tuple is the width, the second is the height
pub fn get_screen_resolution() -> Result<(i32, i32), Error> {
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
            let geometry = connection.wait_for_reply(geometry_cookie)?;
            return Ok((geometry.width() as i32, geometry.height() as i32));
        }
    }

    Err(Error::FailedToGetScreenResolution)
}
