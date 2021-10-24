use std::f64::consts::PI;

use self::point::Point;
use cairo::{Context, Error as CairoError, ImageSurface};
use gtk::{
    gdk,
    gdk_pixbuf::{Colorspace, Pixbuf},
    prelude::GdkContextExt,
};
use image::{
    flat::{self, SampleLayout},
    imageops, FlatSamples, Rgb,
};
use tracing::{error, info, warn};

mod data;
mod stack;

pub use data::*;
pub use stack::*;

const HIGHLIGHT_COLOUR: Colour = Colour {
    red: 255,
    green: 255,
    blue: 0,
    alpha: 63,
};

/// The length of the arrowhead will be 1/10th of the length of the body
const ARROWHEAD_LENGTH_RATIO: f64 = 0.1;
/// How open/closed the arrowhead will be
const ARROWHEAD_APERTURE: f64 = PI / 6.0;

#[derive(Clone, Debug)]
pub enum Operation {
    Finish,
    Crop(Rectangle),
    WindowSelect(Rectangle),
    Blur {
        rect: Rectangle,
        radius: f32,
    },
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
            Operation::Blur { rect, radius } => {
                cairo.save()?;
                let pixbuf = gdk::pixbuf_get_from_surface(
                    surface,
                    rect.x as i32,
                    rect.y as i32,
                    rect.w as i32,
                    rect.h as i32,
                )
                .ok_or(Error::Pixbuf(*rect))?;

                blur(
                    cairo,
                    pixbuf,
                    *radius,
                    Point {
                        x: rect.x,
                        y: rect.y,
                    },
                )?;

                cairo.restore()?;
            }
            Operation::Pixelate(_) => todo!(),
            Operation::DrawLine { start, end, colour } => {
                info!("Line");
                draw_line(cairo, start, end, colour)?;
            }
            Operation::DrawRectangle { rect, colour } => {
                info!("Rectangle");
                draw_rectangle(cairo, rect, colour)?;
            }
            Operation::Text { text, border, fill } => todo!(),
            Operation::DrawArrow { start, end, colour } => {
                info!("Arrow");
                draw_arrow(cairo, start, end, colour)?;
            }
            Operation::Highlight { rect } => {
                info!("Highlight");
                draw_rectangle(cairo, rect, &HIGHLIGHT_COLOUR)?;
            }
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
    #[error("Encountered an error while converting a `image::flat::FlatSamples` to a `image::flat::View: {0}")]
    Image(#[from] flat::Error),
    #[error("Couldn't make Pixbuf from ImageSurface with rect: {0:?}")]
    Pixbuf(Rectangle),
    #[error("`pixel_bytes` on a Pixbuf returned None")]
    PixelBytes,
}

fn draw_rectangle(cairo: &Context, rect: &Rectangle, colour: &Colour) -> Result<(), Error> {
    let Rectangle { x, y, w, h } = *rect;
    cairo.rectangle(x, y, w, h);

    let (r, g, b, a) = colour.to_float_tuple();
    cairo.set_source_rgba(r, g, b, a);
    cairo.fill()?;

    Ok(())
}

fn draw_line(
    cairo: &Context,
    &Point { x: x1, y: y1 }: &Point,
    &Point { x: x2, y: y2 }: &Point,
    colour: &Colour,
) -> Result<(), Error> {
    cairo.move_to(x1, y1);
    cairo.line_to(x2, y2);

    let (r, g, b, a) = colour.to_float_tuple();
    cairo.set_source_rgba(r, g, b, a);
    cairo.stroke()?;

    Ok(())
}

fn draw_arrow(cairo: &Context, start: &Point, end: &Point, colour: &Colour) -> Result<(), Error> {
    let angle = get_line_angle(start, end);
    let length = (end.to_owned() - start.to_owned()).dist();
    let arrow_length = length * ARROWHEAD_LENGTH_RATIO;

    cairo.move_to(start.x, start.y);
    cairo.line_to(end.x, end.y);

    // Since cos(theta) = adjacent / hypothenuse, x1 = arrow_length * cos(theta)
    let x1 = -arrow_length * (angle - ARROWHEAD_APERTURE).cos();
    let x2 = -arrow_length * (angle + ARROWHEAD_APERTURE).cos();

    // Since sin(theta) = opposite / hypothenuse, y1 = arrow_length * sin(theta)
    let y1 = -arrow_length * (angle - ARROWHEAD_APERTURE).sin();
    let y2 = -arrow_length * (angle + ARROWHEAD_APERTURE).sin();

    cairo.rel_move_to(x1, y1);
    cairo.line_to(end.x, end.y);
    cairo.rel_line_to(x2, y2);

    let (r, g, b, a) = colour.to_float_tuple();
    cairo.set_source_rgba(r, g, b, a);
    cairo.stroke()?;

    Ok(())
}

fn get_line_angle(start: &Point, end: &Point) -> f64 {
    let Point { x, y } = end.to_owned() - start.to_owned();
    (y / x).atan()
}

pub fn blur(
    cairo: &Context,
    pixbuf: Pixbuf,
    sigma: f32,
    Point { x, y }: Point,
) -> Result<(), Error> {
    let flat_samples = FlatSamples {
        samples: pixbuf.pixel_bytes().ok_or(Error::PixelBytes)?.to_vec(),
        layout: SampleLayout {
            channels: pixbuf.n_channels() as u8,
            channel_stride: 1,
            width: pixbuf.width() as u32,
            width_stride: 3,
            height: pixbuf.height() as u32,
            height_stride: pixbuf.rowstride() as usize,
        },
        color_hint: None,
    };
    let image = flat_samples.as_view::<Rgb<u8>>()?;
    let mut blurred_image = imageops::blur(&image, sigma);
    let width = blurred_image.width() as i32;
    let height = blurred_image.height() as i32;
    let blurred_flat_samples = blurred_image.as_flat_samples_mut();

    let blurred_pixbuf = Pixbuf::from_mut_slice(
        blurred_flat_samples.samples,
        Colorspace::Rgb,
        false,
        8,
        width,
        height,
        blurred_flat_samples.layout.height_stride as i32,
    );

    cairo.save()?;
    cairo.set_operator(cairo::Operator::Over);
    cairo.set_source_pixbuf(&blurred_pixbuf, x, y);
    cairo.paint()?;
    cairo.restore()?;

    Ok(())
}
