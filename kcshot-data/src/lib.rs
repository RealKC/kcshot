use colour::Colour;
use gtk4::pango::FontDescription;

pub mod colour;
pub mod geometry;
pub mod settings;

#[derive(Debug)]
pub struct Text {
    pub string: String,
    pub font_description: FontDescription,
    pub colour: Colour,
}
