use cairo::Context;
use gtk4::gdk;
use kcshot_cairo::{SurfaceExt as _, log_if_err};
use kcshot_data::{
    colour::Colour,
    geometry::{Point, Rectangle},
};
use kcshot_screenshot::Window;

use crate::operations::OperationStack;

#[derive(Debug)]
pub struct Image {
    pub surface: cairo::ImageSurface,
    pub operation_stack: OperationStack,
}

impl Image {
    pub fn new(
        image: cairo::ImageSurface,
        windows: Vec<Window>,
        screen_dimensions: Rectangle,
        editing_started_with_cropping: bool,
        primary_colour: Colour,
        secondary_colour: Colour,
    ) -> Self {
        Self {
            surface: image,
            operation_stack: OperationStack::new(
                windows,
                screen_dimensions,
                editing_started_with_cropping,
                primary_colour,
                secondary_colour,
            ),
        }
    }

    pub fn draw_in(&self, cairo: &Context, is_in_draw_event: bool) {
        cairo.set_operator(cairo::Operator::Source);
        log_if_err!(cairo.set_source_surface(&self.surface, 0f64, 0f64));
        log_if_err!(cairo.paint());
        cairo.set_operator(cairo::Operator::Over);

        self.operation_stack.execute(cairo, is_in_draw_event);
    }

    pub fn to_texture(&self, point: Option<Point>) -> Result<gdk::Texture, ImageToTextureError> {
        let rectangle = self
            .operation_stack
            .crop_region(point)
            .unwrap_or(self.operation_stack.screen_dimensions);

        self.surface
            .to_texture(rectangle.normalised())
            .map_err(|source| ImageToTextureError {
                source,
                surface: self.surface.clone(),
                rectangle,
            })
    }

    pub fn get_colour_at(&self, x: f64, y: f64) -> Colour {
        let (x, y) = (x as usize, y as usize);

        let stride = self.surface.stride() as usize;
        // NOTE: We multiply by 4 here because CAIRO_FORMAT_RGB24 pixels are 4 bytes in size
        let idx = x * 4 + (y * stride);

        let mut red = 255;
        let mut green = 255;
        let mut blue = 255;

        self.surface
            .with_data(|data| {
                // NOTE: The documentation for CAIRO_FORMAT_RGB24 doesn't mention it explicitly, but
                //       we can extrapolate from the docs for CAIRO_FORMAT_ARGB that **both** formats
                //       are stored native-endian, in our case this means little-endian, so "each pixel
                //       is a 32-bit quantity, with the upper 8 bits unused. Red, Green, and Blue are
                //       stored in the remaining 24 bits in that order. (Since 1.0)" ends up meaning
                //       that the order of the channels in memory is blue-green-red-unused.
                // See: https://cairographics.org/manual/cairo-Image-Surfaces.html#cairo-format-t
                blue = data[idx];
                green = data[idx + 1];
                red = data[idx + 2];
            })
            .unwrap();

        Colour {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Failed to write surface: {surface:?} to PNG with crop region {rectangle:#?}: {source:?}")]
pub struct ImageToTextureError {
    #[source]
    source: cairo::IoError,
    surface: cairo::ImageSurface,
    rectangle: Rectangle,
}
