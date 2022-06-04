use std::borrow::Cow;

use gtk4::{
    gdk::RGBA,
    glib::{self, FromVariant, StaticVariantType, ToVariant},
    pango::FontDescription,
};

pub use self::point::Point;

mod point;

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    pub fn from_gdk_rgba(rgba: RGBA) -> Self {
        Self {
            red: (rgba.red() * 255.0).floor() as u8,
            green: (rgba.green() * 255.0).floor() as u8,
            blue: (rgba.blue() * 255.0).floor() as u8,
            alpha: (rgba.alpha() * 255.0).floor() as u8,
        }
    }

    pub const BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 255,
    };

    /// Serialises `self` as an u32 where each byte represents a component of `Colour`.
    pub const fn serialise_to_u32(self) -> u32 {
        let Colour {
            red,
            green,
            blue,
            alpha,
        } = self;

        (red as u32) << 24 | (green as u32) << 16 | (blue as u32) << 8 | (alpha as u32)
    }

    /// Creates a [`Self`] from a `u32` whose byte layout is assumed to be `RGBA`.
    ///
    /// [`Self::serialise_to_u32`] will create a `u32` in this layout, and you should use this
    /// function paired with that one.
    #[rustfmt::skip]
    pub const fn deserialise_from_u32(raw: u32) -> Self {
        // NOTE: formatting is disabled on this function because IMO this looks nicer
        let red   = (raw >> 24       ) as u8;
        let green = (raw >> 16 & 0xFF) as u8;
        let blue  = (raw >>  8 & 0xFF) as u8;
        let alpha = (raw       & 0xFF) as u8;

        Self { red, green, blue, alpha }
    }
}

impl StaticVariantType for Colour {
    fn static_variant_type() -> Cow<'static, glib::VariantTy> {
        Cow::Borrowed(glib::VariantTy::UINT32)
    }
}

impl FromVariant for Colour {
    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        let raw = u32::from_variant(variant)?;
        Some(Self::deserialise_from_u32(raw))
    }
}

impl ToVariant for Colour {
    fn to_variant(&self) -> glib::Variant {
        self.serialise_to_u32().to_variant()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rectangle {
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

    pub fn contains(&self, Point { x: x1, y: y1 }: Point) -> bool {
        let &Rectangle { x, y, w, h } = self;
        (x..x + w).contains(&x1) && (y..y + h).contains(&y1)
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }
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

#[derive(Debug)]
pub struct Text {
    pub string: String,
    pub font_description: FontDescription,
    pub colour: Colour,
}
