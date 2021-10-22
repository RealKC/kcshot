use std::f64::consts::PI;

use self::point::Point;
use cairo::{Context, Error as CairoError, ImageSurface};
use tracing::{error, info, warn};

mod data;
mod stack;

pub use data::*;
pub use stack::*;

#[derive(Clone, Debug)]
pub enum Operation {
    Finish,
    Crop(Rectangle),
    WindowSelect(Rectangle),
    Blur(Rectangle),
    Pixelate(Rectangle),
    DrawLine {
        start: Point,
        end: Point,
        colour: Colour,
    },
    DrawRectangle {
        rect: Rectangle,
        colour: Colour,
    },
    Text {
        text: String,
        border: Colour,
        fill: Colour,
    },
    DrawArrow {
        start: Point,
        end: Point,
        colour: Colour,
    },
    Highlight {
        rect: Rectangle,
    },
    DrawEllipse {
        ellipse: Ellipse,
        border: Colour,
        fill: Colour,
    },
}

impl Operation {
    #[allow(unused_variables)]
    pub fn execute(&self, surface: &mut ImageSurface, cairo: &Context) -> Result<(), Error> {
        match self {
            Operation::Finish => todo!(),
            Operation::Crop(_) => todo!(),
            Operation::WindowSelect(_) => todo!(),
            Operation::Blur(_) => todo!(),
            Operation::Pixelate(_) => todo!(),
            Operation::DrawLine { start, end, colour } => todo!(),
            Operation::DrawRectangle { rect, colour } => {
                info!("Rectangle");
                draw_rectangle(cairo, rect, colour)?;
            }
            Operation::Text { text, border, fill } => todo!(),
            Operation::DrawArrow { start, end, colour } => todo!(),
            Operation::Highlight { rect } => todo!(),
            Operation::DrawEllipse {
                ellipse,
                border,
                fill,
            } => {
                info!("Ellipse");
                cairo.save()?;

                cairo.save()?;
                // 1. Position our ellipse at (x, y)
                cairo.translate(ellipse.x, ellipse.y);
                // 2. Scale its x coordinates by w, and its y coordinates by h
                cairo.scale(ellipse.w, ellipse.h);
                // 3. Create it by faking a circle on [0,1]x[0,1] centered on (0.5, 0.5)
                cairo.arc(0.5, 0.5, 1.0, 0.0, 2.0 * PI);
                let (r, g, b, a) = fill.to_float_tuple();
                cairo.set_source_rgba(r, g, b, a);
                cairo.fill_preserve()?;
                cairo.restore()?;

                let (r, g, b, a) = border.to_float_tuple();
                cairo.set_source_rgba(r, g, b, a);
                // 4. Draw a border arround it
                cairo.stroke()?;

                cairo.restore()?;
            }
        };

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a cairo error: {0}")]
    Cairo(#[from] CairoError),
    #[error("Encountered a cairo error while trying to borrow something: {0}")]
    Borrow(#[from] cairo::BorrowError),
}

fn draw_rectangle(cairo: &Context, rect: &Rectangle, colour: &Colour) -> Result<(), Error> {
    let Point { x, y } = rect.upper_left_corner;
    let Point {
        x: width,
        y: height,
    } = rect.lower_right_corner - rect.upper_left_corner;
    cairo.rectangle(x, y, width, height);

    let (r, g, b, a) = colour.to_float_tuple();
    cairo.set_source_rgba(r, g, b, a);
    cairo.fill()?;

    Ok(())
}
