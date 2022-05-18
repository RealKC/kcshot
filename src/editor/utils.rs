use cairo::Context;
use gtk4::gdk::{self, gdk_pixbuf::Pixbuf};

use super::data::{Colour, Rectangle};

#[macro_export]
macro_rules! log_if_err {
    ($call:expr) => {
        match $call {
            Ok(_) => {}
            Err(err) => ::tracing::error!(
                "Got error: {err:?}\n\twith the following call: {}",
                ::std::stringify!($call)
            ),
        }
    };
}

pub fn pixbuf_for(surface: &cairo::Surface, rectangle: Rectangle) -> Option<Pixbuf> {
    // We normalise the rectangle in case we get a rectangle with negative width and height
    // which could happen when the user makes a rectangle by starting with the bottom-right corner
    // instead of the top-left corner.
    let Rectangle { x, y, w, h } = rectangle.normalised();
    let src_x = x.floor() as i32;
    let src_y = y.floor() as i32;
    let width = w.ceil() as i32;
    let height = h.ceil() as i32;

    gdk::pixbuf_get_from_surface(surface, src_x, src_y, width, height)
}

pub trait CairoExt {
    fn set_source_colour(&self, colour: Colour);
}

impl CairoExt for Context {
    fn set_source_colour(&self, colour: Colour) {
        let Colour {
            red,
            green,
            blue,
            alpha,
        } = colour;

        let red = red as f64 / 255.0;
        let green = green as f64 / 255.0;
        let blue = blue as f64 / 255.0;
        let alpha = alpha as f64 / 255.0;

        self.set_source_rgba(red, green, blue, alpha);
    }
}

pub struct ContextLogger<'s> {
    ctx: &'s str,
    method: &'static str,
}

impl<'s> ContextLogger<'s> {
    pub fn new(ctx: &'s str, method: &'static str) -> Self {
        tracing::trace!("\x1b[32mEntering\x1b[0m context inside {method}: '{ctx}'");

        Self { ctx, method }
    }
}

impl<'s> Drop for ContextLogger<'s> {
    fn drop(&mut self) {
        let Self { method, ctx } = self;
        tracing::trace!("\x1b[31mExiting\x1b[0m context inside {method}: '{ctx}'");
    }
}
