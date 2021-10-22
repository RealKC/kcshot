use self::point::Point;

pub mod point;

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    pub fn to_float_tuple(self) -> (f64, f64, f64, f64) {
        let Colour {
            red,
            green,
            blue,
            alpha,
        } = self;
        (
            red as f64 / 255.0,
            green as f64 / 255.0,
            blue as f64 / 255.0,
            alpha as f64 / 255.0,
        )
    }

    fn is_invisible(self) -> bool {
        self.alpha == 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub upper_left_corner: Point,
    pub lower_right_corner: Point,
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
