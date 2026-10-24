use kcshot_data::colour::Colour;

use crate::editor::operations::OperationStack;

#[derive(Debug)]
pub struct Image {
    pub surface: cairo::ImageSurface,
    pub operation_stack: OperationStack,
}

impl Image {
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
