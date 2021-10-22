use std::f64::consts::PI;

use cairo::{Context, Error as CairoError, ImageSurface};
use tracing::{error, info};

pub struct OperationStack(Vec<Operation>);

impl OperationStack {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn push_new_operation(&mut self, op: Operation) {
        self.0.push(op);
    }

    pub fn change_top_operation_border_colour(&mut self, _new_colour: Colour) {
        todo!()
    }

    pub fn change_top_operation_fill_colour(&mut self, _new_colour: Colour) {
        todo!()
    }

    pub fn change_top_operation_end(&mut self, _new_end: Point) {
        todo!();
    }

    pub fn execute(&self, surface: &mut ImageSurface, cairo: &Context) {
        for operation in &self.0 {
            if let Err(why) = operation.execute(surface, cairo) {
                error!("{}", why);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    fn to_float_tuple(self) -> (f64, f64, f64, f64) {
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
pub struct Point {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    upper_left_corner: Point,
    lower_right_corner: Point,
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
    pub fn execute(&self, surface: &mut ImageSurface, cairo: &Context) -> Result<(), CairoError> {
        match self {
            Operation::Finish => todo!(),
            Operation::Crop(_) => todo!(),
            Operation::WindowSelect(_) => todo!(),
            Operation::Blur(_) => todo!(),
            Operation::Pixelate(_) => todo!(),
            Operation::DrawLine { start, end, colour } => todo!(),
            Operation::DrawRectangle { rect, colour } => todo!(),
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
