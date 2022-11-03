use super::Point;

#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rectangle {
    #[must_use = "This function doesn't modify `self`, but returns a new `Rectangle`"]
    pub fn normalised(&self) -> Self {
        let Self {
            mut x,
            mut y,
            mut w,
            mut h,
        } = *self;

        if w < 0.0 {
            x += w;
            w = w.abs();
        }

        if h < 0.0 {
            y += h;
            h = h.abs();
        }

        Self { x, y, w, h }
    }

    #[must_use]
    pub fn contains(&self, Point { x: x1, y: y1 }: Point) -> bool {
        let &Rectangle { x, y, w, h } = self;
        (x..x + w).contains(&x1) && (y..y + h).contains(&y1)
    }

    #[must_use]
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}
