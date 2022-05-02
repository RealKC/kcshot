use std::f64::consts::PI;

use cairo::Context;

use super::Error;
use crate::editor::{data::*, utils::CairoExt};

/// The length of the arrowhead will be 1/10th of the length of the body
const ARROWHEAD_LENGTH_RATIO: f64 = 0.1;
/// How open/closed the arrowhead will be
const ARROWHEAD_APERTURE: f64 = PI / 6.0;

pub fn draw_rectangle(
    cairo: &Context,
    rect: &Rectangle,
    border: Colour,
    fill: Colour,
    line_width: f64,
) -> Result<(), Error> {
    cairo.save()?;
    let Rectangle { x, y, w, h } = rect.normalised();
    cairo.rectangle(x, y, w, h);

    cairo.set_source_colour(fill);
    cairo.fill_preserve()?;

    cairo.set_source_colour(border);
    cairo.set_line_width(line_width);
    cairo.stroke()?;
    cairo.restore()?;

    Ok(())
}

pub fn draw_ellipse(
    cairo: &Context,
    ellipse: &Ellipse,
    border: Colour,
    fill: Colour,
    line_width: f64,
) -> Result<(), Error> {
    cairo.save()?;
    // Avoid initial line from previous point if one exists
    cairo.new_sub_path();
    // 1. Position our ellipse at (x, y)
    cairo.translate(ellipse.x, ellipse.y);
    // 2. Scale its x coordinates by w, and its y coordinates by h
    cairo.scale(ellipse.w, ellipse.h);
    // 3. Create it by faking a circle on [0,1]x[0,1] centered on (0.5, 0.5)
    cairo.arc(0.5, 0.5, 1.0, 0.0, 2.0 * PI);
    cairo.set_source_colour(fill);
    cairo.fill_preserve()?;
    cairo.restore()?;

    cairo.set_source_colour(border);
    cairo.set_line_width(line_width);
    // 4. Draw a border arround it
    cairo.stroke()?;

    Ok(())
}

pub fn draw_line(
    cairo: &Context,
    Point { x: x1, y: y1 }: Point,
    Point { x: x2, y: y2 }: Point,
    colour: Colour,
    line_width: f64,
) -> Result<(), Error> {
    cairo.move_to(x1, y1);
    cairo.line_to(x2, y2);
    cairo.set_source_colour(colour);
    cairo.set_line_width(line_width);
    cairo.stroke()?;

    Ok(())
}

pub fn draw_arrow(
    cairo: &Context,
    start: Point,
    end: Point,
    colour: Colour,
    line_width: f64,
) -> Result<(), Error> {
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

    cairo.set_source_colour(colour);
    cairo.set_line_width(line_width);
    cairo.stroke()?;

    Ok(())
}

fn get_line_angle(start: Point, end: Point) -> f64 {
    let Point { x, y } = end - start;
    y.atan2(x)
}
