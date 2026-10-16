use gtk4::{gdk, glib};
use kcshot_data::geometry::Rectangle;

pub trait SurfaceExt {
    fn to_texture(&self, rectangle: Rectangle) -> Result<gdk::Texture, cairo::IoError>;
}

impl SurfaceExt for cairo::ImageSurface {
    fn to_texture(&self, rectangle: Rectangle) -> Result<gdk::Texture, cairo::IoError> {
        let clipped_surface = self
            .create_for_rectangle(rectangle.into_cairo())?
            .map_to_image(None)?;

        let mut buf = vec![];
        clipped_surface.with_data(|a| buf = a.to_vec()).unwrap();

        Ok(gdk::MemoryTextureBuilder::new()
            .set_format(gdk::MemoryFormat::B8g8r8x8)
            .set_width(clipped_surface.width())
            .set_height(clipped_surface.height())
            .set_stride(clipped_surface.stride() as usize)
            .set_bytes(Some(&glib::Bytes::from_owned(buf)))
            .set_color_state(&gdk::ColorState::srgb())
            .build())
    }
}
