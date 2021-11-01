use super::operations::Rectangle;

use gtk::gdk::{self, gdk_pixbuf::Pixbuf};

pub fn pixbuf_for(surface: &cairo::Surface, rectangle: Rectangle) -> Option<Pixbuf> {
    let Rectangle { x, y, w, h } = rectangle;
    let src_x = x.floor() as i32;
    let src_y = y.floor() as i32;
    let width = w.ceil() as i32;
    let height = h.ceil() as i32;

    gdk::pixbuf_get_from_surface(surface, src_x, src_y, width, height)
}
