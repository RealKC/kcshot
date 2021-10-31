use std::{
    ffi::CString,
    fs::File,
    io,
    os::{
        raw::{c_char, c_int, c_uint},
        unix::io::FromRawFd,
    },
    ptr::NonNull,
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

struct XDisplay(NonNull<x11::xlib::_XDisplay>);

impl XDisplay {
    fn open_default() -> Result<Self, Error> {
        // SAFETY: This call is valid
        //         See: https://www.x.org/releases/X11R7.7/doc/libX11/libX11/libX11.html#Opening_the_Display
        NonNull::new(unsafe { XOpenDisplay(std::ptr::null()) })
            .ok_or(Error::FailedToOpenDisplay)
            .map(Self)
    }

    fn as_ptr(&self) -> *mut x11::xlib::_XDisplay {
        self.0.as_ptr()
    }
}

impl Drop for XDisplay {
    fn drop(&mut self) {
        // SAFETY: This call is okay as `self.0` was obtained from a call to XOpenDisplay
        //         Also see https://www.x.org/releases/X11R7.7/doc/libX11/libX11/libX11.html#Closing_the_Display
        //         As this is called in a drop impl, and due to the way Xlib resources are managed, this is fine.
        unsafe { XCloseDisplay(self.as_ptr()) };
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered an error from cairo: {0}")]
    Cairo(#[from] CairoError),
    #[error("Encountered an I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to obtain an XDisplay. (Is DISPLAY set? Is an X server running?)")]
    FailedToOpenDisplay,
    #[error("Failed to fetch the root window where the cursor was from the X server")]
    FailedToGetRootWindow,
    #[error("Failed to get window attributes")]
    FailedToGetWindowAttributes,
}

impl From<cairo::IoError> for Error {
    fn from(cerr: cairo::IoError) -> Self {
        match cerr {
            cairo::IoError::Cairo(cairo) => Self::Cairo(cairo),
            cairo::IoError::Io(io) => Self::Io(io),
        }
    }
}

#[tracing::instrument]
pub fn take_screenshot() -> Result<ImageSurface, Error> {
    info!("entered");
    let display = XDisplay::open_default()?;
    let (window_where_cursor_is, screen_where_the_cursor_is) =
        find_window_where_cursor_is(display.as_ptr())?;

    // SAFETY: This is a C struct so this should be a valid byte pattern for it. I hope.
    let mut attributes: XWindowAttributes = unsafe { std::mem::zeroed() };
    // SAFETY: All the arguments are valid
    let rc = unsafe {
        XGetWindowAttributes(
            display.as_ptr(),
            window_where_cursor_is,
            &mut attributes as _,
        )
    };
    if rc == 0 {
        return Err(Error::FailedToGetWindowAttributes);
    }

    // SAFETY: All the arguments are valid
    let visual = unsafe { XDefaultVisual(display.as_ptr(), screen_where_the_cursor_is) };

    // SAFETY: All the arguments are valid, but I think cairo will return an error surface even if one is not?
    let screenshot = unsafe {
        ffi::cairo_xlib_surface_create(
            display.as_ptr(),
            window_where_cursor_is,
            visual,
            attributes.width,
            attributes.height,
        )
    };
    // Just to be sure
    debug_assert!(!screenshot.is_null());

    // SAFETY: `screenshot` is nonnull
    match unsafe { ffi::cairo_surface_status(screenshot) } {
        0 => {}
        err => return Err(CairoError::from(err).into()),
    }

    let template = CString::new("screenshot.XXXXXX.png")
        .expect("CString::new shouldn't fail with string literals AFAIK")
        .into_raw();

    // SAFETY: `path` is a nonnull, and a valid pointer. Also even if the template is invalid,
    //         `mkstemps` should just return an error in that case.
    let fd = unsafe { libc::mkstemps(template, 4) };
    // SAFETY: We got this pointer earlier from calling CString::into_raw so this should be ok.
    let path = unsafe { CString::from_raw(template) };
    if fd == -1 {
        return Err(io::Error::last_os_error().into());
    }

    // SAFETY: This is the only call to `from_raw_fd` for this specific fd, furthermore it's verified
    //         to be a valid fd just above.
    let mut file = unsafe { File::from_raw_fd(fd) };

    // SAFETY: All the arguments are valid
    match unsafe { cairo_surface_write_to_png(screenshot, path.as_ptr()) } {
        0 => {}
        err => return Err(CairoError::from(err).into()),
    }

    Ok(ImageSurface::create_from_png(&mut file)?)
}

/// Gets the screen resolution
///
/// # Returns
/// The first item of the tuple is the width, the second is the height
pub fn get_screen_resolution() -> Result<(i32, i32), Error> {
    let display = XDisplay::open_default()?;
    let (window_where_cursor_is, _) = find_window_where_cursor_is(display.as_ptr())?;
    // SAFETY: This is a C struct so this should be a valid byte pattern for it. I hope.
    let mut attributes: XWindowAttributes = unsafe { std::mem::zeroed() };
    // SAFETY: All the arguments are valid
    let rc = unsafe {
        XGetWindowAttributes(
            display.as_ptr(),
            window_where_cursor_is,
            &mut attributes as _,
        )
    };

    if rc == 0 {
        return Err(Error::FailedToGetWindowAttributes);
    }

    Ok((attributes.width, attributes.height))
}

fn find_window_where_cursor_is(display: *mut Display) -> Result<(Window, c_int), Error> {
    // SAFETY: `display` isn't null, and it's obtained from a good call to XOpenDisplay, should be
    //         good?
    let screen_count = unsafe { XScreenCount(display) };

    for screen in 0..screen_count {
        let mut root_x = c_int::default();
        let mut root_y = c_int::default();
        let mut win_x = c_int::default();
        let mut win_y = c_int::default();
        let mut mask = c_uint::default();
        let mut child = Window::default();
        let mut window_with_cursor = Window::default();

        // See:
        //  * XQueryPointer: https://www.x.org/releases/X11R7.7/doc/man/man3/XQueryPointer.3.xhtml
        // SAFETY: All arguments should be guaranteed valid, except for the value returned by XRootWindow
        //         However, if XRootWindow returns an invalid Window, XQueryPointer will return an error
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
            return Ok((window_with_cursor, screen));
        }
    }

    Err(Error::FailedToGetRootWindow)
}
