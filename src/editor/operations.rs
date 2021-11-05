use std::f64::consts::PI;

use cairo::{Context, Error as CairoError, ImageSurface};
use gtk::{
    gdk_pixbuf::{Colorspace, Pixbuf},
    pango::FontDescription,
    prelude::GdkContextExt,
};
use image::{
    flat::{self, SampleLayout},
    imageops, FlatSamples, Rgb,
};
use rand::{prelude::StdRng, Rng, SeedableRng};
use tracing::{error, info};

mod stack;

pub use stack::*;

use super::{
    data::*,
    utils::{self, CairoExt},
};

const HIGHLIGHT_COLOUR: Colour = Colour {
    red: 255,
    green: 255,
    blue: 0,
    alpha: 63,
};

const INVISIBLE: Colour = Colour {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};

/// The length of the arrowhead will be 1/10th of the length of the body
const ARROWHEAD_LENGTH_RATIO: f64 = 0.1;
/// How open/closed the arrowhead will be
const ARROWHEAD_APERTURE: f64 = PI / 6.0;
/// How big will pixelate boxes be, in this case, we will group the rectangle into 4x4 boxes, which we will set all of its pixels to the same value
const PIXELATE_SIZE: u64 = 4;
/// How big the bubbles will be
const BUBBLE_RADIUS: f64 = 10.0;

#[derive(Clone, Debug)]
pub enum Operation {
    Crop(Rectangle),
    Blur {
        rect: Rectangle,
        radius: f32,
    },
    Pixelate {
        rect: Rectangle,
        seed: u64,
    },
    DrawLine {
        start: Point,
        end: Point,
        colour: Colour,
    },
    DrawRectangle {
        rect: Rectangle,
        border: Colour,
        fill: Colour,
    },
    Text {
        top_left: Point,
        text: String,
        colour: Colour,
        font_description: FontDescription,
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
    Bubble {
        centre: Point,
        bubble_colour: Colour,
        text_colour: Colour,
        number: i32,
        font_description: FontDescription,
    },
}

/// This enum is like [Operations] but without any associated data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    CropAndSave = 0,
    Line = 1,
    Arrow = 2,
    Rectangle = 3,
    Ellipse = 4,
    Highlight = 5,
    Pixelate = 6,
    Blur = 7,
    AutoincrementBubble = 8,
    Text = 9,
}

impl Tool {
    pub const fn path(self) -> &'static str {
        match self {
            Tool::CropAndSave => "resources/tool-rectanglecrop.png",
            Tool::Line => "resources/tool-line.png",
            Tool::Arrow => "resources/tool-arrow.png",
            Tool::Rectangle => "resources/tool-rectangle.png",
            Tool::Ellipse => "resources/tool-ellipse.png",
            Tool::Highlight => "resources/tool-highlight.png",
            Tool::Pixelate => "resources/tool-pixelate.png",
            Tool::Blur => "resources/tool-blur.png",
            Tool::AutoincrementBubble => "resources/tool-autoincrementbubble.png",
            Tool::Text => "resources/tool-text.png",
        }
    }
}

impl Operation {
    fn create_default_for_tool(
        tool: Tool,
        start: Point,
        bubble_index: &mut i32,
        primary_colour: Colour,
        secondary_colour: Colour,
    ) -> Self {
        let rect = Rectangle {
            x: start.x,
            y: start.y,
            w: 1.0,
            h: 1.0,
        };

        let font_description = FontDescription::from_string("Fira Code, 40pt");

        match tool {
            Tool::CropAndSave => Self::Crop(Rectangle {
                x: start.x,
                y: start.y,
                // Width and height are zero to signal `OperationStack::crop_rectangle` to return `None`
                w: 0.0,
                h: 0.0,
            }),
            Tool::Line => Self::DrawLine {
                start,
                end: start,
                colour: primary_colour,
            },
            Tool::Arrow => Self::DrawArrow {
                start,
                end: start,
                colour: primary_colour,
            },
            Tool::Rectangle => Self::DrawRectangle {
                rect,
                border: secondary_colour,
                fill: primary_colour,
            },
            Tool::Ellipse => Self::DrawEllipse {
                ellipse: Ellipse {
                    x: start.x,
                    y: start.y,
                    w: 1.0,
                    h: 1.0,
                },
                border: secondary_colour,
                fill: primary_colour,
            },
            Tool::Highlight => Self::Highlight { rect },
            Tool::Pixelate => Self::Pixelate {
                rect,
                seed: rand::thread_rng().gen(),
            },
            Tool::Blur => Self::Blur { rect, radius: 5.0 },
            Tool::AutoincrementBubble => {
                let bubble = Self::Bubble {
                    centre: start,
                    bubble_colour: secondary_colour,
                    text_colour: primary_colour,
                    number: *bubble_index,
                    font_description,
                };
                *bubble_index += 1;
                bubble
            }
            Tool::Text => Self::Text {
                top_left: start,
                text: "".into(),
                colour: primary_colour,
                font_description,
            },
        }
    }

    #[allow(unused_variables)]
    pub fn execute(
        &self,
        surface: &ImageSurface,
        cairo: &Context,
        is_in_draw_event: bool,
    ) -> Result<(), Error> {
        match self {
            Operation::Crop(Rectangle { x, y, w, h }) => {
                if is_in_draw_event {
                    cairo.save()?;

                    cairo.rectangle(*x, *y, *w, *h);
                    // When we are in draw events (aka this is being shown to the user), we want to make it clear
                    // they are selecting the region which will be cropped
                    cairo.set_source_colour(Colour {
                        red: 0,
                        green: 127,
                        blue: 190,
                        alpha: 255,
                    });
                    cairo.set_dash(&[4.0, 21.0, 4.0], 0.0);
                    cairo.stroke()?;
                    cairo.restore()?;
                } else {
                    // When we are not in draw events (aka the image is being saved), we just want to crop.
                    // However, that is not done here, but rather inside EditorWindow::do_save_surface
                }
            }
            Operation::Blur { rect, radius } => {
                cairo.save()?;
                let pixbuf = utils::pixbuf_for(surface, *rect).ok_or(Error::Pixbuf(*rect))?;

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
            Operation::Pixelate { rect, seed } => {
                info!("Pixelate");

                let pixbuf = utils::pixbuf_for(surface, *rect).ok_or(Error::Pixbuf(*rect))?;

                pixelate(cairo, pixbuf, rect, *seed)?;
            }
            Operation::DrawLine { start, end, colour } => {
                info!("Line");
                draw_line(cairo, *start, *end, colour)?;
            }
            Operation::DrawRectangle { rect, border, fill } => {
                info!("Rectangle");
                draw_rectangle(cairo, rect, *border, *fill)?;
            }
            Operation::Text {
                top_left,
                text,
                colour,
                font_description,
            } => {
                info!("Text");
                cairo.save()?;
                draw_text_at(cairo, *top_left, text, *colour, font_description)?;
                cairo.restore()?;
            }
            Operation::DrawArrow { start, end, colour } => {
                info!("Arrow");
                draw_arrow(cairo, *start, *end, *colour)?;
            }
            Operation::Highlight { rect } => {
                info!("Highlight");
                draw_rectangle(cairo, rect, INVISIBLE, HIGHLIGHT_COLOUR)?;
            }
            Operation::DrawEllipse {
                ellipse,
                border,
                fill,
            } => {
                info!("Ellipse");
                cairo.save()?;
                draw_ellipse(cairo, ellipse, *border, *fill)?;
                cairo.restore()?;
            }
            Operation::Bubble {
                centre,
                bubble_colour,
                text_colour,
                number,
                font_description,
            } => {
                info!("Bubble");
                let Point { x, y } = centre;
                let num_str = number.to_string();

                let ellipse = Ellipse {
                    x: x - BUBBLE_RADIUS,
                    y: y - BUBBLE_RADIUS,
                    w: 2.0 * BUBBLE_RADIUS,
                    h: 2.0 * BUBBLE_RADIUS,
                };

                draw_ellipse(cairo, &ellipse, INVISIBLE, *bubble_colour)?;
                draw_text_centred_at(
                    cairo,
                    *centre,
                    num_str.as_str(),
                    *text_colour,
                    font_description,
                )?;
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

fn draw_rectangle(
    cairo: &Context,
    rect: &Rectangle,
    border: Colour,
    fill: Colour,
) -> Result<(), Error> {
    cairo.save()?;
    let Rectangle { x, y, w, h } = *rect;
    cairo.rectangle(x, y, w, h);

    cairo.set_source_colour(fill);
    cairo.fill_preserve()?;

    cairo.set_source_colour(border);
    cairo.stroke()?;
    cairo.restore()?;

    Ok(())
}

fn draw_ellipse(
    cairo: &Context,
    ellipse: &Ellipse,
    border: Colour,
    fill: Colour,
) -> Result<(), Error> {
    cairo.save()?;
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
    // 4. Draw a border arround it
    cairo.stroke()?;

    Ok(())
}

fn draw_line(
    cairo: &Context,
    Point { x: x1, y: y1 }: Point,
    Point { x: x2, y: y2 }: Point,
    colour: &Colour,
) -> Result<(), Error> {
    cairo.move_to(x1, y1);
    cairo.line_to(x2, y2);
    cairo.set_source_colour(*colour);
    cairo.stroke()?;

    Ok(())
}

fn draw_arrow(cairo: &Context, start: Point, end: Point, colour: Colour) -> Result<(), Error> {
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
    cairo.stroke()?;

    Ok(())
}

fn get_line_angle(start: Point, end: Point) -> f64 {
    let Point { x, y } = end.to_owned() - start.to_owned();
    y.atan2(x)
}

fn draw_text_at(
    cairo: &Context,
    Point { x, y }: Point,
    text: &str,
    colour: Colour,
    font_description: &FontDescription,
) -> Result<(), Error> {
    let layout = pangocairo::create_layout(cairo).unwrap();

    layout.set_markup(text);
    layout.set_font_description(Some(font_description));
    cairo.move_to(x, y);
    cairo.set_source_colour(colour);
    pangocairo::update_layout(cairo, &layout);
    pangocairo::show_layout(cairo, &layout);
    Ok(())
}

fn draw_text_centred_at(
    cairo: &Context,
    Point { x, y }: Point,
    text: &str,
    colour: Colour,
    font_description: &FontDescription,
) -> Result<(), Error> {
    let layout = pangocairo::create_layout(cairo).unwrap();

    layout.set_markup(text);
    layout.set_font_description(Some(font_description));

    let pixel_extents = layout.pixel_extents().1;
    let w = pixel_extents.width as f64;
    let h = pixel_extents.height as f64;

    cairo.move_to(x - w / 2.0, y - h / 2.0);
    cairo.set_source_colour(colour);
    pangocairo::update_layout(cairo, &layout);
    pangocairo::show_layout(cairo, &layout);
    Ok(())
}

fn blur(cairo: &Context, pixbuf: Pixbuf, sigma: f32, Point { x, y }: Point) -> Result<(), Error> {
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

fn pixelate(
    cairo: &Context,
    pixbuf: Pixbuf,
    &Rectangle { x, y, w, h }: &Rectangle,
    seed: u64,
) -> Result<(), Error> {
    let mut rng = StdRng::seed_from_u64(seed);

    let rowstride = pixbuf.rowstride() as u64;
    let bytes_per_pixel = 3 * (pixbuf.bits_per_sample() / 8) as u64;
    let pixels = unsafe { pixbuf.pixels() };

    for i in (0..(w as u64)).step_by(PIXELATE_SIZE as usize) {
        for j in (0..(h as u64)).step_by(PIXELATE_SIZE as usize) {
            let pixelate_size_x: u64 = PIXELATE_SIZE.min(w as u64 - i);
            let pixelate_size_y: u64 = PIXELATE_SIZE.min(h as u64 - j);

            let sample_x: u64 = i + rng.gen_range(0..pixelate_size_x);
            let sample_y: u64 = j + rng.gen_range(0..pixelate_size_y);

            // Note that we don't multiply sample_y by bytes_per_pixel since its size is contained inside rowstride
            let row_index = sample_y * rowstride + bytes_per_pixel * sample_x;
            let mut sample = Vec::with_capacity(bytes_per_pixel as usize);
            for k in 0..bytes_per_pixel as usize {
                sample.push(pixels[row_index as usize + k]);
            }

            for pixel_x in i..(i + pixelate_size_x) {
                for pixel_y in j..(j + pixelate_size_y) {
                    let row_index = pixel_y * rowstride + bytes_per_pixel * pixel_x;
                    for k in 0..bytes_per_pixel {
                        pixels[(row_index + k) as usize] = sample[k as usize];
                    }
                }
            }
        }
    }

    cairo.save()?;
    cairo.set_operator(cairo::Operator::Over);
    cairo.set_source_pixbuf(&pixbuf, x, y);
    cairo.paint()?;
    cairo.restore()?;

    Ok(())
}
