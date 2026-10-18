use cairo::{Context, ImageSurface};
use kcshot_data::geometry::Rectangle;
use rand::{RngExt, SeedableRng, prelude::StdRng};
use stackblur_iter::{blur_argb as stackblur, imgref::ImgRefMut};

use super::Error;

/// How big will pixelate boxes be, in this case, we will group the rectangle into 4x4 boxes, which we will set all of its pixels to the same value
const PIXELATE_SIZE: u64 = 4;

fn copy_rect_to_image_surface(
    surface: &cairo::Surface,
    rect: &Rectangle,
) -> Result<ImageSurface, Error> {
    // This is more or less a Rust copy of:
    // https://gitlab.gnome.org/GNOME/gtk/-/blob/e4db180344c8f6d60b3b043ad70980776b297d69/gdk/deprecated/gdkpixbuf.c#L50-72

    let copy = ImageSurface::create(cairo::Format::ARgb32, rect.w as i32, rect.h as i32)?;

    let context = Context::new(&copy)?;
    context.set_operator(cairo::Operator::Source);
    context.set_source_surface(surface, -rect.x, -rect.y)?;
    context.paint()?;

    Ok(copy)
}

fn get_bytes_per_pixel(fmt: cairo::Format) -> usize {
    match fmt {
        cairo::Format::ARgb32 | cairo::Format::Rgb24 => 4,
        _ => panic!("Tried to get bytes per pixel of unsupported format: {fmt:?}"),
    }
}

pub fn blur(cairo: &Context, radius: usize, rect: Rectangle) -> Result<(), Error> {
    let rect @ Rectangle { x, y, w, h } = rect.normalised();

    if rect.area() < 1.0 {
        return Ok(());
    }

    let clipped_surface = copy_rect_to_image_surface(&cairo.target(), &rect)?;

    let height = h as usize;
    let width = w as usize;
    let stride = clipped_surface.stride() as usize;

    let mut pixels = vec![];
    clipped_surface.with_data(|bytes| pixels = bytes.to_vec())?;

    let (pixels, stride) = blur_rgb(
        pixels,
        width,
        height,
        stride,
        radius,
        get_bytes_per_pixel(clipped_surface.format()),
    );

    cairo.save()?;
    cairo.set_operator(cairo::Operator::Over);
    cairo.set_source_surface(
        &ImageSurface::create_for_data(
            pixels,
            cairo::Format::Rgb24,
            width as i32,
            height as i32,
            stride,
        )?,
        x,
        y,
    )?;
    // cairo.set_source_surface(&*clipped_surface, x, y);
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
    bytes_per_pixel: usize,
) -> (Vec<u8>, i32) {
    assert!(
        [3, 4].contains(&bytes_per_pixel),
        "Unsupported bytes per pixel value: {bytes_per_pixel}",
    );

    let mut pixels_iter = pixels.into_iter();
    let mut pixels = Vec::with_capacity(width * height);

    if bytes_per_pixel == 4 {
        for _ in 0..height {
            for _ in 0..width {
                pixels.push(u32::from_be_bytes([
                    pixels_iter.next().unwrap(),
                    pixels_iter.next().unwrap(),
                    pixels_iter.next().unwrap(),
                    pixels_iter.next().unwrap(),
                ]));
            }
        }
    } else {
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
    }

    let mut img = ImgRefMut::new(&mut pixels, width, height);

    stackblur(&mut img, radius);

    let pixel_data = if bytes_per_pixel == 4 {
        pixels
            .into_iter()
            .flat_map(|pixel| {
                let [a, r, g, b] = pixel.to_be_bytes();

                [a, r, g, b]
            })
            .collect()
    } else {
        pixels
            .into_iter()
            .flat_map(|pixel| {
                let [_, r, g, b] = pixel.to_be_bytes();

                [r, g, b]
            })
            .collect()
    };

    (pixel_data, (width * bytes_per_pixel) as i32)
}

pub fn pixelate(cairo: &Context, rect: &Rectangle, seed: u64) -> Result<(), Error> {
    if rect.area() < 1.0 {
        return Ok(());
    }

    let mut rng = StdRng::seed_from_u64(seed);

    let mut clipped_surface = copy_rect_to_image_surface(&cairo.target(), rect)?;

    let rowstride = clipped_surface.stride() as u64;
    let bytes_per_pixel = get_bytes_per_pixel(clipped_surface.format()) as u64;

    let mut pixels = clipped_surface.data()?;

    let &Rectangle { x, y, w, h } = rect;

    for i in (0..(w as u64)).step_by(PIXELATE_SIZE as usize) {
        for j in (0..(h as u64)).step_by(PIXELATE_SIZE as usize) {
            let pixelate_size_x: u64 = PIXELATE_SIZE.min(w as u64 - i);
            let pixelate_size_y: u64 = PIXELATE_SIZE.min(h as u64 - j);

            let sample_x: u64 = i + rng.random_range(0..pixelate_size_x);
            let sample_y: u64 = j + rng.random_range(0..pixelate_size_y);

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

    drop(pixels);

    cairo.save()?;
    cairo.set_operator(cairo::Operator::Over);
    cairo.set_source_surface(&clipped_surface, x, y)?;
    cairo.paint()?;
    cairo.restore()?;

    Ok(())
}
