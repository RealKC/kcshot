use cairo::Context;
use gtk4::{
    gdk::prelude::GdkCairoContextExt,
    gdk_pixbuf::{Colorspace, Pixbuf},
};
use rand::{prelude::StdRng, Rng, SeedableRng};
use stackblur_iter::{blur_argb as stackblur, imgref::ImgRefMut};

use super::Error;
use crate::editor::{data::Rectangle, utils};

/// How big will pixelate boxes be, in this case, we will group the rectangle into 4x4 boxes, which we will set all of its pixels to the same value
const PIXELATE_SIZE: u64 = 4;

pub fn blur(
    cairo: &Context,
    surface: &cairo::Surface,
    radius: usize,
    rect @ Rectangle { x, y, .. }: Rectangle,
) -> Result<(), Error> {
    if rect.area() < 1.0 {
        return Ok(());
    }

    let pixbuf = utils::pixbuf_for(surface, rect).ok_or(Error::Pixbuf(rect))?;
    assert_eq!(
        pixbuf.colorspace(),
        Colorspace::Rgb,
        "Blur only supports the Rgb colourspace"
    );
    assert_eq!(
        pixbuf.bits_per_sample(),
        8,
        "Our blur can't handle Pixbufs that don't have 8 bits per sample"
    );

    let height = pixbuf.height() as usize;
    let width = pixbuf.width() as usize;
    let has_alpha = pixbuf.has_alpha();
    let n_channels = pixbuf.n_channels() as usize;

    assert!(
        n_channels == 3 && !has_alpha,
        "Our code can't handle Pixbufs that also have the alpha channel"
    );

    let mut pixels = pixbuf.pixel_bytes().ok_or(Error::PixelBytes)?.to_vec();
    let extend_by = 3 - pixels.len() % n_channels;
    pixels.resize(pixels.len() + extend_by, 0);

    let (mut pixels, stride) = blur_rgb(pixels, width, height, pixbuf.rowstride() as usize, radius);

    cairo.save()?;
    cairo.set_operator(cairo::Operator::Over);
    let pixbuf = Pixbuf::from_mut_slice(
        &mut pixels,
        Colorspace::Rgb,
        has_alpha,
        8,
        width as i32,
        height as i32,
        stride,
    );
    cairo.set_source_pixbuf(&pixbuf, x, y);
    cairo.paint()?;
    cairo.restore()?;

    Ok(())
}

/// Blurs an rgb image, the size of the passed in pixel buffer must be a multiple of 3.
///
/// # Returns
/// The blurred buffer and its row stride
fn blur_rgb(
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    stride: usize,
    radius: usize,
) -> (Vec<u8>, i32) {
    assert!(
        pixels.len() % 3 == 0,
        "The pixel buffer's length should be a multiple of 3, but it was '{}'.",
        pixels.len()
    );

    let mut pixels_iter = pixels.into_iter();
    let mut pixels = Vec::with_capacity(width * height);

    for _ in 0..height {
        for _ in 0..width {
            pixels.push(u32::from_be_bytes([
                0xff,
                pixels_iter.next().unwrap(),
                pixels_iter.next().unwrap(),
                pixels_iter.next().unwrap(),
            ]));
        }

        // GdkPixbuf's stride may not be a multiple of 3, which would mean that a naive approach of
        // just calling `.chunks` on the Vec would result in channels getting shifted by either 1
        // or 2, this would result in the image becoming grayscale due to the vertical pass of
        // stackblur averaging out different channels, and also X axis drift.
        //
        // The fix is to simply skip the extra bytes in the row.
        //
        // See https://github.com/LoganDark/stackblur-iter/issues/8#issuecomment-1176850999
        for _ in 0..stride.saturating_sub(width * 3) {
            pixels_iter.next();
        }
    }

    let mut img = ImgRefMut::new(&mut pixels, width, height);

    stackblur(&mut img, radius);

    (
        pixels
            .into_iter()
            .flat_map(|pixel| {
                let [_, r, g, b] = pixel.to_be_bytes();

                [r, g, b]
            })
            .collect(),
        width as i32 * 3,
    )
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
