use gtk::gdk::RGBA;

pub mod point;

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    pub fn from_gdk_rgba(
        RGBA {
            red,
            green,
            blue,
            alpha,
        }: RGBA,
    ) -> Self {
        Self {
            red: (red * 255.0).floor() as u8,
            green: (green * 255.0).floor() as u8,
            blue: (blue * 255.0).floor() as u8,
            alpha: (alpha * 255.0).floor() as u8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A struct representing an ellipse
///
/// Properties:
/// * has a radius of w/2 (= a) in the x axis
/// * has a radius of h/2 (= b) in the y axis
/// * center is at (x + w/2, y + h/2) (= (x0, y0))
#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
