use cairo::{Context, Error as CairoError};
use gtk4::pango::FontDescription;
use kcshot_data::{Text, colour::Colour, geometry::*};
use pangocairo::functions::{create_layout, show_layout, update_layout};
use rand::Rng;

pub use self::{stack::*, tool::*};
use super::utils::CairoExt;

mod pixelops;
mod shapes;
mod stack;
mod tool;

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
        line_width: f64,
    },
    DrawRectangle {
        rect: Rectangle,
        border: Colour,
        fill: Colour,
        line_width: f64,
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
        line_width: f64,
    },
    Highlight {
        rect: Rectangle,
    },
    DrawEllipse {
        ellipse: Ellipse,
        border: Colour,
        fill: Colour,
        line_width: f64,
    },
    Bubble {
        centre: Point,
        bubble_colour: Colour,
        text_colour: Colour,
        number: i32,
        font_description: FontDescription,
    },
    Pencil {
        start: Point,
        points: Vec<Point>,
        colour: Colour,
        line_width: f64,
    },
}

impl Operation {
    fn create_default_for_tool(
        tool: Tool,
        start: Point,
        bubble_index: &mut i32,
        primary_colour: Colour,
        secondary_colour: Colour,
        line_width: f64,
    ) -> Self {
        let rect = Rectangle {
            x: start.x,
            y: start.y,
            w: 1.0,
            h: 1.0,
        };

        let font_description = FontDescription::from_string("Fira Code, 40pt");

        match tool {
            Tool::Save => panic!("`Tool::Save` should never be converted to an `Operation`"),
            Tool::CropAndSave | Tool::Crop => Self::Crop(Rectangle {
                x: start.x,
                y: start.y,
                // Width and height are zero to signal `OperationStack::crop_rectangle` to return `None`
                w: 0.0,
                h: 0.0,
            }),
            Tool::Line => Self::DrawLine {
                start,
                end: start,
                colour: secondary_colour,
                line_width,
            },
            Tool::Arrow => Self::DrawArrow {
                start,
                end: start,
                colour: secondary_colour,
                line_width,
            },
            Tool::Rectangle => Self::DrawRectangle {
                rect,
                border: secondary_colour,
                fill: primary_colour,
                line_width,
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
                line_width,
            },
            Tool::Highlight => Self::Highlight { rect },
            Tool::Pixelate => Self::Pixelate {
                rect,
                seed: rand::rng().random(),
            },
            Tool::Blur => Self::Blur { rect, radius: 5.0 },
            Tool::AutoincrementBubble => {
                let bubble = Self::Bubble {
                    centre: start,
                    bubble_colour: primary_colour,
                    text_colour: secondary_colour,
                    number: *bubble_index,
                    font_description,
                };
                *bubble_index += 1;
                bubble
            }
            Tool::Text => Self::Text {
                top_left: start,
                text: String::new(),
                // We use secondary colour here as the primary one is more likely to be transparent,
                // given that's the default, and people are likely to use boxes and ellipses to try
                // and bring things into attention, and in those situations the primary colour is
                // used to fill those shapes.
                colour: secondary_colour,
                font_description,
            },
            Tool::Pencil => Self::Pencil {
                start,
                points: vec![],
                colour: secondary_colour,
                line_width,
            },
        }
    }

    pub fn execute(
        &self,
        cairo: &Context,
        is_in_draw_event: bool,
        should_crop_indicators_be_dashed: bool,
    ) -> Result<(), Error> {
        match self {
            Operation::Crop(rect) => {
                if is_in_draw_event {
                    cairo.save()?;

                    let Rectangle { x, y, w, h } = rect.normalised();
                    cairo.rectangle(x, y, w, h);
                    // When we are in draw events (aka this is being shown to the user), we want to make it clear
                    // they are selecting the region which will be cropped
                    cairo.set_source_colour(Colour {
                        red: 0,
                        green: 127,
                        blue: 190,
                        alpha: 255,
                    });
                    if should_crop_indicators_be_dashed {
                        cairo.set_dash(&[4.0, 21.0, 4.0], 0.0);
                    }
                    cairo.set_line_width(2.0);
                    cairo.stroke()?;
                    cairo.restore()?;
                } else {
                    // When we are not in draw events (aka the image is being saved), we just want to crop.
                    // However, that is not done here, but rather inside EditorWindow::do_save_surface
                }
            }
            Operation::Blur { rect, radius } => {
                cairo.save()?;
                pixelops::blur(cairo, *radius as usize, rect.normalised())?;
                cairo.restore()?;
            }
            Operation::Pixelate { rect, seed } => {
                let rect = rect.normalised();

                pixelops::pixelate(cairo, &rect, *seed)?;
            }
            Operation::DrawLine {
                start,
                end,
                colour,
                line_width,
            } => {
                shapes::draw_line(cairo, *start, *end, *colour, *line_width)?;
            }
            Operation::DrawRectangle {
                rect,
                border,
                fill,
                line_width,
            } => {
                shapes::draw_rectangle(cairo, rect, *border, *fill, *line_width)?;
            }
            Operation::Text {
                top_left,
                text,
                colour,
                font_description,
            } => {
                cairo.save()?;
                draw_text_at(cairo, *top_left, text, *colour, font_description);
                cairo.restore()?;
            }
            Operation::DrawArrow {
                start,
                end,
                colour,
                line_width,
            } => {
                shapes::draw_arrow(cairo, *start, *end, *colour, *line_width)?;
            }
            Operation::Highlight { rect } => {
                shapes::draw_rectangle(cairo, rect, INVISIBLE, HIGHLIGHT_COLOUR, 1.0)?;
            }
            Operation::DrawEllipse {
                ellipse,
                border,
                fill,
                line_width,
            } => {
                cairo.save()?;
                shapes::draw_ellipse(cairo, ellipse, *border, *fill, *line_width)?;
                cairo.restore()?;
            }
            Operation::Bubble {
                centre,
                bubble_colour,
                text_colour,
                number,
                font_description,
            } => {
                let Point { x, y } = centre;
                let num_str = number.to_string();

                let ellipse = Ellipse {
                    x: x - BUBBLE_RADIUS,
                    y: y - BUBBLE_RADIUS,
                    w: 2.0 * BUBBLE_RADIUS,
                    h: 2.0 * BUBBLE_RADIUS,
                };

                shapes::draw_ellipse(cairo, &ellipse, INVISIBLE, *bubble_colour, 1.0)?;
                draw_text_centred_at(
                    cairo,
                    *centre,
                    num_str.as_str(),
                    *text_colour,
                    font_description,
                );
            }
            Operation::Pencil {
                start,
                points,
                colour,
                line_width,
            } => {
                cairo.save()?;
                cairo.set_line_width(*line_width);
                cairo.set_source_colour(*colour);
                cairo.move_to(start.x, start.y);
                for point in points {
                    cairo.line_to(point.x, point.y);
                }
                cairo.stroke()?;
                cairo.restore()?;
            }
        };

        Ok(())
    }

    pub fn set_text(&mut self, input_text: Text) {
        if let Operation::Text {
            text,
            colour,
            font_description,
            ..
        } = self
        {
            *text = input_text.string;
            *font_description = input_text.font_description;
            *colour = input_text.colour;
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Encountered a cairo error: {0}")]
    Cairo(#[from] CairoError),
    #[error("Encountered a cairo error while trying to borrow something: {0}")]
    Borrow(#[from] cairo::BorrowError),
    #[error("Couldn't make Pixbuf from ImageSurface with rect: {0:?}")]
    Pixbuf(Rectangle),
    #[error("`pixel_bytes` on a Pixbuf returned None")]
    PixelBytes,
}

fn draw_text_at(
    cairo: &Context,
    Point { x, y }: Point,
    text: &str,
    colour: Colour,
    font_description: &FontDescription,
) {
    let layout = create_layout(cairo);

    layout.set_markup(text);
    layout.set_font_description(Some(font_description));
    cairo.move_to(x, y);
    cairo.set_source_colour(colour);
    update_layout(cairo, &layout);
    show_layout(cairo, &layout);
}

fn draw_text_centred_at(
    cairo: &Context,
    Point { x, y }: Point,
    text: &str,
    colour: Colour,
    font_description: &FontDescription,
) {
    let layout = create_layout(cairo);

    layout.set_markup(text);
    layout.set_font_description(Some(font_description));

    let pixel_extents = layout.pixel_extents().1;
    let w = pixel_extents.width() as f64;
    let h = pixel_extents.height() as f64;

    cairo.move_to(x - w / 2.0, y - h / 2.0);
    cairo.set_source_colour(colour);
    update_layout(cairo, &layout);
    show_layout(cairo, &layout);
}
