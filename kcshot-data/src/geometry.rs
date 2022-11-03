mod point;
mod rectangle;

pub use point::*;
pub use rectangle::*;

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
