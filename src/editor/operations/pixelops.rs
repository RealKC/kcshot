use cairo::Context;
use gtk4::{
    gdk::prelude::GdkCairoContextExt,
    gdk_pixbuf::{Colorspace, Pixbuf},
};
use image::{flat::SampleLayout, imageops, FlatSamples, Rgb};
use rand::{prelude::StdRng, Rng, SeedableRng};

use super::Error;
use crate::editor::{
    data::{Point, Rectangle},
    utils,
};

/// How big will pixelate boxes be, in this case, we will group the rectangle into 4x4 boxes, which we will set all of its pixels to the same value
const PIXELATE_SIZE: u64 = 4;

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

pub fn pixelate(
    cairo: &Context,
    surface: &cairo::Surface,
    rect: &Rectangle,
    seed: u64,
) -> Result<(), Error> {
    let mut rng = StdRng::seed_from_u64(seed);

    let pixbuf = utils::pixbuf_for(surface, *rect).ok_or(Error::Pixbuf(*rect))?;

    // SAFETY: The pixbuf is newly created so there should be only one reference to the pixel data
    let pixels = unsafe { pixbuf.pixels() };

    let rowstride = pixbuf.rowstride() as u64;
    let bytes_per_pixel = 3 * (pixbuf.bits_per_sample() / 8) as u64;
    let &Rectangle { x, y, w, h } = rect;

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
