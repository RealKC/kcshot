use std::{
    ffi::CString,
    fs::File,
    os::{
        raw::{c_char, c_int, c_uint},
        unix::io::FromRawFd,
    },
};

use cairo::{
    self,
    ffi::{self, cairo_status_t, cairo_surface_t},
    Error as CairoError, ImageSurface,
};
use tracing::{error, info, warn};
use x11::xlib::{
    Display, Window, XCloseDisplay, XDefaultVisual, XGetWindowAttributes, XOpenDisplay,
    XQueryPointer, XRootWindow, XScreenCount, XWindowAttributes,
};

extern "C" {
    /// cairo-rs doesn't expose it in its ffi module, so I have to write its declaration myself
    /// Here's the cairo docs for it: https://cairographics.org/manual/cairo-PNG-Support.html#cairo-surface-write-to-png
    fn cairo_surface_write_to_png(
        surface: *mut cairo_surface_t,
        filename: *const c_char,
    ) -> cairo_status_t;
}

struct XDisplay(*mut x11::xlib::_XDisplay);

impl XDisplay {
    fn open_default() -> Self {
        Self(unsafe { XOpenDisplay(std::ptr::null_mut()) })
    }
}

impl Drop for XDisplay {
    fn drop(&mut self) {
        unsafe { XCloseDisplay(self.0) };
    }
}

#[tracing::instrument]
pub fn take_screenshot() -> Option<ImageSurface> {
    info!("entered");
    let display = XDisplay::open_default();
    let (window_where_cursor_is, screen_where_the_cursor_is) =
        find_window_where_cursor_is(display.0)?;

    let mut attributes: XWindowAttributes = unsafe { std::mem::zeroed() };
    // FIXME: Figure out how to handle possible errors
    unsafe { XGetWindowAttributes(display.0, window_where_cursor_is, &mut attributes as _) };

    let visual = unsafe { XDefaultVisual(display.0, screen_where_the_cursor_is) };

    let screenshot = unsafe {
        ffi::cairo_xlib_surface_create(
            display.0,
            window_where_cursor_is,
            visual,
            attributes.width,
            attributes.height,
        )
    };

    let path = CString::new("screenshot.XXXXXX.png").unwrap().into_raw();
    let fd = unsafe { libc::mkstemps(path, 4) };
    assert!(fd != -1);

    let rc = unsafe { cairo_surface_write_to_png(screenshot, path as _) };
    match rc {
        0 => {}
        err => {
            error!("{:?}", CairoError::from(err));
            panic!()
        }
    }

    // To deallocate that memory
    let _path = unsafe { CString::from_raw(path) };

    let mut file = unsafe { File::from_raw_fd(fd) };
    ImageSurface::create_from_png(&mut file).ok()
}

fn find_window_where_cursor_is(display: *mut Display) -> Option<(Window, c_int)> {
    let screen_count = unsafe { XScreenCount(display) };

    for screen in 0..screen_count {
        let mut root_x = c_int::default();
        let mut root_y = c_int::default();
        let mut win_x = c_int::default();
        let mut win_y = c_int::default();
        let mut mask = c_uint::default();
        let mut child = Window::default();
        let mut window_with_cursor = Window::default();

        let rc = unsafe {
            XQueryPointer(
                display,
                XRootWindow(display, screen),
                &mut window_with_cursor as _,
                &mut child as _,
                &mut root_x as _,
                &mut root_y as _,
                &mut win_x as _,
                &mut win_y as _,
                &mut mask as _,
            )
        };

        if rc == x11::xlib::True {
            return Some((window_with_cursor, screen));
        }
    }

    None
}
