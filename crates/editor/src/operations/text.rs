use cairo::Context;
use kcshot_cairo::CairoExt as _;
use kcshot_data::{colour::Colour, geometry::*};
use pangocairo::{
    functions::{create_layout, show_layout, update_layout},
    pango::FontDescription,
};

pub fn draw_text_at(
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

pub fn draw_text_centred_at(
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
