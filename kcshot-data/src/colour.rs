use std::borrow::Cow;

use gtk4::{
    gdk::RGBA,
    glib::{self, prelude::*},
};

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Colour {
    #[must_use]
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
    #[must_use]
    pub const fn serialise_to_u32(self) -> u32 {
        let Colour {
            red,
            green,
            blue,
            alpha,
        } = self;

        ((red as u32) << 24) | ((green as u32) << 16) | ((blue as u32) << 8) | (alpha as u32)
    }

    /// Creates a [`Self`] from a `u32` whose byte layout is assumed to be `RGBA`.
    ///
    /// [`Self::serialise_to_u32`] will create a `u32` in this layout, and you should use this
    /// function paired with that one.
    #[must_use]
    pub const fn deserialise_from_u32(raw: u32) -> Self {
        // NOTE: formatting is disabled on this function because IMO this looks nicer
        let red = (raw >> 24) as u8;
        let green = ((raw >> 16) & 0xFF) as u8;
        let blue = ((raw >> 8) & 0xFF) as u8;
        let alpha = (raw & 0xFF) as u8;

        Self {
            red,
            green,
            blue,
            alpha,
        }
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

impl From<Colour> for glib::Variant {
    fn from(value: Colour) -> Self {
        value.serialise_to_u32().to_variant()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl From<RGBA> for Hsv {
    fn from(color: RGBA) -> Self {
        let (h, s, v) = gtk4::rgb_to_hsv(color.red(), color.green(), color.blue());

        Hsv { h, s, v }
    }
}

impl From<Hsv> for RGBA {
    fn from(color: Hsv) -> Self {
        let (red, green, blue) = gtk4::hsv_to_rgb(color.h, color.s, color.v);

        Self::new(red, green, blue, 1.0)
    }
}

impl Hsv {
    #[must_use]
    pub fn to_colour(self) -> Colour {
        Colour::from_gdk_rgba(self.into())
    }

    #[must_use]
    pub fn as_int(&self) -> (i32, i32, i32) {
        (
            (self.h * 359.0 + 1.0) as i32,
            (self.s * 99.0 + 1.0) as i32,
            (self.v * 99.0 + 1.0) as i32,
        )
    }

    #[must_use]
    pub fn from_int(h: i32, s: i32, v: i32) -> Self {
        Self {
            h: (h as f32 - 1.0) / 359.0,
            s: (s as f32 - 1.0) / 99.0,
            v: (v as f32 - 1.0) / 99.0,
        }
    }
}
