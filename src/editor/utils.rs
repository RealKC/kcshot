use super::operations::{Colour, Rectangle};

use cairo::Context;
use gtk::gdk::{self, gdk_pixbuf::Pixbuf};

pub fn pixbuf_for(surface: &cairo::Surface, rectangle: Rectangle) -> Option<Pixbuf> {
    let Rectangle { x, y, w, h } = rectangle;
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
